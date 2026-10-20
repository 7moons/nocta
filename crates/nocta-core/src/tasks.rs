use crate::{models::*, CoreError, CoreResult};
use chrono::Utc;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

struct Entry {
    task: AccountTask,
    cancel: CancellationToken,
    started: Instant,
    /// Whether the task got past its queue and did anything.
    ran: bool,
}
#[derive(Default)]
struct Registry {
    entries: BTreeMap<u32, Entry>,
    accounts: HashMap<u32, u32>,
    finished: BTreeSet<u32>,
}
#[derive(Default)]
pub struct Tasks {
    registry: Mutex<Registry>,
    serial: AtomicU32,
    closing: AtomicBool,
    pub changed: Notify,
    quarantine: Arc<crate::browser::Quarantine>,
}

impl Tasks {
    pub fn new(quarantine: Arc<crate::browser::Quarantine>) -> Self {
        Self {
            quarantine,
            ..Default::default()
        }
    }
    pub fn cleanup_pending(&self) -> Vec<u32> {
        self.quarantine.list()
    }
    pub fn reserve(
        &self,
        accounts: &[u32],
        kind: TaskKind,
    ) -> CoreResult<Vec<(u32, u32, CancellationToken)>> {
        if self.closing.load(Ordering::SeqCst) {
            return Err(CoreError::Cancelled);
        }
        let mut registry = self.registry.lock().unwrap();
        if self.closing.load(Ordering::SeqCst) {
            return Err(CoreError::Cancelled);
        }
        let mut seen = HashSet::new();
        let accounts = accounts
            .iter()
            .copied()
            .filter(|id| seen.insert(*id))
            .collect::<Vec<_>>();
        if accounts.iter().any(|id| registry.accounts.contains_key(id)) {
            return Err(CoreError::Busy);
        }
        if self.quarantine.contains_any(&accounts) {
            return Err(CoreError::Browser(
                "Browser cleanup is still pending for a selected account. Wait for the retry or restart Nocta."
                    .into(),
            ));
        }
        let mut reserved = Vec::new();
        for account_id in accounts {
            let id = self.serial.fetch_add(1, Ordering::SeqCst) + 1;
            let cancel = CancellationToken::new();
            registry.entries.insert(
                id,
                Entry {
                    task: AccountTask {
                        id,
                        account_id,
                        kind,
                        status: TaskStatus::Queued,
                        detail: "Waiting to start.".into(),
                        created_at: Utc::now().to_rfc3339(),
                        finished_at: None,
                        duration_ms: None,
                    },
                    cancel: cancel.clone(),
                    started: Instant::now(),
                    ran: false,
                },
            );
            registry.accounts.insert(account_id, id);
            reserved.push((id, account_id, cancel));
        }
        self.changed.notify_one();
        Ok(reserved)
    }
    pub fn ensure_idle(&self, ids: &[u32]) -> CoreResult<()> {
        let registry = self.registry.lock().unwrap();
        if ids.iter().any(|id| registry.accounts.contains_key(id))
            || self.quarantine.contains_any(ids)
        {
            return Err(CoreError::Busy);
        }
        Ok(())
    }
    pub fn kind(&self, id: u32, kind: TaskKind) {
        if let Some(entry) = self.registry.lock().unwrap().entries.get_mut(&id) {
            if !entry.task.status.terminal() {
                entry.task.kind = kind;
            }
        }
        self.changed.notify_one();
    }
    pub fn set(&self, id: u32, status: TaskStatus, detail: impl Into<String>) {
        self.update(id, status, detail.into(), true);
    }
    pub(crate) fn finish(
        &self,
        id: u32,
        status: TaskStatus,
        detail: String,
    ) -> Option<AccountTask> {
        if !status.terminal() {
            return None;
        }
        self.update(id, status, detail, false)
    }
    pub(crate) fn release_finished(&self, task: &AccountTask) {
        self.release_many(std::slice::from_ref(task));
    }
    pub(crate) fn release_many(&self, tasks: &[AccountTask]) {
        let mut registry = self.registry.lock().unwrap();
        for task in tasks {
            if registry.accounts.get(&task.account_id) == Some(&task.id) {
                registry.accounts.remove(&task.account_id);
            }
        }
        self.changed.notify_one();
    }
    fn prune(registry: &mut Registry) {
        while registry.finished.len() > 200 {
            if let Some(id) = registry.finished.pop_first() {
                registry.entries.remove(&id);
            }
        }
    }
    fn update(
        &self,
        id: u32,
        status: TaskStatus,
        detail: String,
        release: bool,
    ) -> Option<AccountTask> {
        let mut registry = self.registry.lock().unwrap();
        let mut updated = None;
        if let Some(entry) = registry.entries.get_mut(&id) {
            if entry.task.status.terminal() {
                return None;
            }
            entry.task.status = if entry.cancel.is_cancelled() && !status.terminal() {
                TaskStatus::Cancelling
            } else {
                status
            };
            entry.task.detail = detail;
            entry.ran |= matches!(status, TaskStatus::Running | TaskStatus::WaitingUser);
            if status.terminal() {
                entry.task.finished_at = Some(Utc::now().to_rfc3339());
                entry.task.duration_ms =
                    Some(entry.started.elapsed().as_millis().min(u32::MAX as u128) as u32);
                updated = Some(entry.task.clone());
                let account_id = entry.task.account_id;
                if release {
                    registry.accounts.remove(&account_id);
                }
                registry.finished.insert(id);
            }
        }
        Self::prune(&mut registry);
        self.changed.notify_one();
        updated
    }
    /// Cancels the running and queued tasks and returns how many were not cancelled yet.
    pub fn stop(&self, include_manual: bool) -> usize {
        let mut registry = self.registry.lock().unwrap();
        let mut stopped = 0;
        for entry in registry.entries.values_mut() {
            if !entry.task.status.terminal()
                && (include_manual
                    || entry.task.kind != TaskKind::OpenBrowser
                    || entry.task.status != TaskStatus::WaitingUser)
            {
                stopped += usize::from(!entry.cancel.is_cancelled());
                entry.cancel.cancel();
                entry.task.status = TaskStatus::Cancelling;
                entry.task.detail = "Stopping and cleaning up…".into();
            }
        }
        self.changed.notify_one();
        stopped
    }
    /// Whether a task was cancelled while still queued. Such tasks did nothing, so they leave no
    /// log line or last operation behind: stopping a check of thousands of accounts would
    /// otherwise push the whole log out.
    pub fn cancelled_in_queue(&self, id: u32) -> bool {
        self.registry
            .lock()
            .unwrap()
            .entries
            .get(&id)
            .is_some_and(|entry| !entry.ran && entry.cancel.is_cancelled())
    }
    pub fn cancel(&self, id: u32) -> CoreResult<()> {
        let mut registry = self.registry.lock().unwrap();
        let entry = registry.entries.get_mut(&id).ok_or(CoreError::NotFound)?;
        if !entry.task.status.terminal() {
            entry.cancel.cancel();
            entry.task.status = TaskStatus::Cancelling;
            entry.task.detail = "Stopping and cleaning up…".into();
        }
        self.changed.notify_one();
        Ok(())
    }
    pub fn snapshot(&self) -> Vec<AccountTask> {
        self.registry
            .lock()
            .unwrap()
            .entries
            .values()
            .map(|entry| entry.task.clone())
            .collect()
    }
    pub fn get(&self, id: u32) -> Option<AccountTask> {
        self.registry
            .lock()
            .unwrap()
            .entries
            .get(&id)
            .map(|entry| entry.task.clone())
    }
    pub fn shutdown(&self) {
        self.closing.store(true, Ordering::SeqCst);
        let _ = self.stop(true);
    }
}

pub struct ConcurrencyGate {
    active: AtomicU32,
    limit: AtomicU32,
    changed: Notify,
}
pub struct Permit(Arc<ConcurrencyGate>);
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
        self.0.changed.notify_waiters();
    }
}
impl ConcurrencyGate {
    pub fn new(limit: u32) -> Arc<Self> {
        Arc::new(Self {
            active: AtomicU32::new(0),
            limit: AtomicU32::new(limit.max(1)),
            changed: Notify::new(),
        })
    }
    pub fn set_limit(&self, limit: u32) {
        self.limit.store(limit.max(1), Ordering::SeqCst);
        self.changed.notify_waiters();
    }
    pub async fn acquire(self: &Arc<Self>, cancel: &CancellationToken) -> CoreResult<Permit> {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if cancel.is_cancelled() {
                return Err(CoreError::Cancelled);
            }
            let active = self.active.load(Ordering::SeqCst);
            if active < self.limit.load(Ordering::SeqCst)
                && self
                    .active
                    .compare_exchange(active, active + 1, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            {
                return Ok(Permit(self.clone()));
            }
            tokio::select! { biased; _ = cancel.cancelled() => return Err(CoreError::Cancelled), _ = notified => {} }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finished_result_survives_history_pruning_and_lease_waits_for_persistence() {
        let tasks = Tasks::default();
        let oldest = tasks.reserve(&[1], TaskKind::LaunchGame).unwrap()[0].0;
        for id in 2..=202 {
            let task = tasks.reserve(&[id], TaskKind::CheckHealth).unwrap()[0].0;
            tasks.set(task, TaskStatus::Succeeded, "Checked");
        }
        let result = tasks
            .finish(oldest, TaskStatus::Failed, "No launch handler.".into())
            .unwrap();
        assert!(tasks.get(oldest).is_none());
        assert_eq!(result.detail, "No launch handler.");
        assert!(result.duration_ms.is_some() && result.finished_at.is_some());
        assert!(tasks.reserve(&[1], TaskKind::LaunchGame).is_err());
        tasks.release_finished(&result);
        let next = tasks.reserve(&[1], TaskKind::LaunchGame).unwrap()[0].0;
        // An old completion must not release a newer operation's lease.
        tasks.release_finished(&result);
        assert!(tasks.ensure_idle(&[1]).is_err());
        tasks.set(next, TaskStatus::Cancelled, "Stopped");
        assert!(tasks.ensure_idle(&[1]).is_ok());
    }
    #[test]
    fn only_tasks_stopped_in_the_queue_count_as_cancelled_in_queue() {
        let tasks = Tasks::default();
        let jobs = tasks.reserve(&[1, 2], TaskKind::CheckHealth).unwrap();
        let (running, queued) = (jobs[0].0, jobs[1].0);
        tasks.set(running, TaskStatus::Running, "Checking");
        assert!(!tasks.cancelled_in_queue(queued));
        assert_eq!(tasks.stop(false), 2);
        assert!(!tasks.cancelled_in_queue(running));
        assert!(tasks.cancelled_in_queue(queued));
        assert_eq!(tasks.stop(false), 0, "a second stop counts nothing again");
    }
    #[test]
    fn conflicts_are_atomic_and_stop_preserves_manual_browsers() {
        let tasks = Tasks::default();
        let manual = tasks.reserve(&[1], TaskKind::OpenBrowser).unwrap();
        tasks.set(manual[0].0, TaskStatus::WaitingUser, "Browser open");
        let queued_browser = tasks.reserve(&[3], TaskKind::OpenBrowser).unwrap();
        assert!(tasks.reserve(&[1, 2], TaskKind::CheckHealth).is_err());
        let check = tasks.reserve(&[2], TaskKind::CheckHealth).unwrap();
        tasks.stop(false);
        assert!(!manual[0].2.is_cancelled());
        assert!(queued_browser[0].2.is_cancelled());
        assert!(check[0].2.is_cancelled());
        assert!(tasks.ensure_idle(&[2]).is_err());
        tasks.set(check[0].0, TaskStatus::Cancelled, "Cleaned up");
        assert!(tasks.ensure_idle(&[2]).is_ok());
        tasks.shutdown();
        assert!(manual[0].2.is_cancelled());
        assert!(tasks.reserve(&[3], TaskKind::CheckHealth).is_err());
    }
    #[test]
    fn quarantined_accounts_cannot_be_leased_until_cleanup_succeeds() {
        let quarantine = Arc::new(crate::browser::Quarantine::default());
        let tasks = Tasks::new(quarantine.clone());
        quarantine.insert(4);
        assert!(tasks.reserve(&[4], TaskKind::CheckHealth).is_err());
        assert!(tasks.ensure_idle(&[4]).is_err());
        assert_eq!(tasks.cleanup_pending(), vec![4]);
        quarantine.remove(4);
        assert!(tasks.reserve(&[4], TaskKind::CheckHealth).is_ok());
    }
    #[tokio::test]
    async fn concurrency_wait_is_cancellable_and_limit_can_change() {
        let gate = ConcurrencyGate::new(1);
        let cancel = CancellationToken::new();
        let first = gate.acquire(&cancel).await.unwrap();
        let pending = gate.acquire(&cancel);
        tokio::pin!(pending);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut pending)
                .await
                .is_err()
        );
        gate.set_limit(2);
        let second = pending.await.unwrap();
        let stopped = CancellationToken::new();
        stopped.cancel();
        assert!(gate.acquire(&stopped).await.is_err());
        drop(first);
        drop(second);
        assert!(gate.acquire(&cancel).await.is_ok());
    }
}

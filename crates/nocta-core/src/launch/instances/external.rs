//! Roblox clients that Nocta did not start. They are only listed, and closed only when the
//! user asks.
use super::*;

impl InstanceManager {
    pub fn external_snapshot(&self) -> Vec<ExternalInstance> {
        self.external
            .lock()
            .unwrap()
            .iter()
            .map(|process| ExternalInstance {
                id: external_id(process),
                pid: process.pid,
                process_name: process.name.clone(),
                started_at: chrono::DateTime::from_timestamp_millis(process.created_ms as i64)
                    .map(|time| time.to_rfc3339())
                    .unwrap_or_default(),
            })
            .collect()
    }

    /// Closes a game client Nocta did not launch, at the user's explicit request. The process is reopened
    /// through the identity-checked handle, so a PID reused since the last monitor pass is rejected.
    pub async fn close_external(self: &Arc<Self>, id: &str) -> CoreResult<()> {
        let process = self
            .external
            .lock()
            .unwrap()
            .iter()
            .find(|process| external_id(process) == id)
            .cloned()
            .ok_or(CoreError::NotFound)?;
        let platform = self.platform.clone();
        let target = process.clone();
        blocking(move || platform.open(&target)?.terminate()).await?;
        self.external
            .lock()
            .unwrap()
            .retain(|item| item.key() != process.key());
        self.changed.notify_one();
        Ok(())
    }

    /// Closes every Roblox client and launcher: Nocta's own instances and any started elsewhere.
    /// Used when a running client holds the singleton lock that multi-instance needs.
    pub async fn close_all_roblox(self: &Arc<Self>) -> CoreResult<u32> {
        let mut closed = 0;
        let mut failure = None;
        match self.close_all().await {
            Ok(count) => closed += count,
            Err(error) => failure = Some(error),
        }
        // Holding the dispatch lock keeps a concurrent launch from starting a client mid-sweep.
        let _serial = self.dispatch.lock().await;
        let platform = self.platform.clone();
        let processes = blocking(move || platform.processes()).await?;
        let claimed = self.claimed();
        let targets = processes
            .into_iter()
            .filter(|process| !claimed.contains(&process.key()))
            .collect::<Vec<_>>();
        let platform = self.platform.clone();
        let (count, error) = blocking(move || {
            let mut count = 0;
            let mut error = None;
            for process in targets {
                match platform
                    .open(&process)
                    .and_then(|handle| handle.terminate())
                {
                    Ok(()) => count += 1,
                    Err(failed) => error = Some(failed),
                }
            }
            Ok((count, error))
        })
        .await?;
        closed += count;
        failure = failure.or(error);
        self.external.lock().unwrap().clear();
        self.changed.notify_one();
        match failure {
            Some(error) if closed == 0 => Err(error),
            _ => Ok(closed),
        }
    }

    /// Paths of every running Roblox client, used to keep in-use versions from being deleted.
    pub async fn running_paths(&self) -> Vec<String> {
        let platform = self.platform.clone();
        blocking(move || platform.processes())
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|process| process.path)
            .collect()
    }
}

pub(super) fn external_id(process: &ProcessInfo) -> String {
    format!("{}:{}", process.pid, process.created_ms)
}

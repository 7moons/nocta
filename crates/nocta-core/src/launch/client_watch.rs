//! Watching running clients through their Roblox logs: when they are in game, why they were
//! disconnected, and waiting for one before starting the next.
use super::{
    client_logs::{self, LogEvent, LogWatch},
    Launcher,
};
use crate::{
    models::{AppConfig, ClientStage, InstanceStatus},
    CoreError, CoreResult,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};
use tokio_util::sync::CancellationToken;

/// Without a log after this long, a client is not waited for any more (its log may live
/// elsewhere, for example with a custom bootstrapper).
const NO_LOG_GIVE_UP: Duration = Duration::from_secs(45);
const WAIT_POLL: Duration = Duration::from_secs(1);

struct Tracked {
    since: SystemTime,
    registered: Instant,
    watch: Option<LogWatch>,
}

pub struct ClientLogs {
    dir: Mutex<Option<PathBuf>>,
    tracked: Mutex<HashMap<String, Tracked>>,
    claimed: Mutex<HashSet<PathBuf>>,
}

impl ClientLogs {
    pub fn new(dir: Option<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            dir: Mutex::new(dir),
            tracked: Mutex::new(HashMap::new()),
            claimed: Mutex::new(HashSet::new()),
        })
    }

    pub fn enabled(&self) -> bool {
        self.dir.lock().unwrap().is_some()
    }

    #[cfg(test)]
    pub fn set_dir(&self, dir: Option<PathBuf>) {
        *self.dir.lock().unwrap() = dir;
    }

    /// Starts following the log of a client that was just started.
    pub fn track(&self, instance: &str, since: SystemTime) {
        self.tracked.lock().unwrap().insert(
            instance.to_owned(),
            Tracked {
                since,
                registered: Instant::now(),
                watch: None,
            },
        );
    }

    /// Whether the client's log was found; `None` when it is not tracked.
    pub fn has_log(&self, instance: &str) -> Option<bool> {
        self.tracked
            .lock()
            .unwrap()
            .get(instance)
            .map(|tracked| tracked.watch.is_some())
    }

    fn registered(&self, instance: &str) -> Option<Instant> {
        self.tracked
            .lock()
            .unwrap()
            .get(instance)
            .map(|tracked| tracked.registered)
    }

    /// New events of every tracked client; clients in `alive` only, the others are forgotten.
    fn poll(&self, alive: &HashSet<String>) -> Vec<(String, LogEvent)> {
        let Some(dir) = self.dir.lock().unwrap().clone() else {
            return Vec::new();
        };
        let mut tracked = self.tracked.lock().unwrap();
        let mut claimed = self.claimed.lock().unwrap();
        tracked.retain(|id, entry| {
            let keep = alive.contains(id);
            if !keep {
                if let Some(watch) = &entry.watch {
                    claimed.remove(&watch.path);
                }
            }
            keep
        });
        // Earlier launches claim their log first.
        let mut order: Vec<(Instant, String)> = tracked
            .iter()
            .map(|(id, entry)| (entry.registered, id.clone()))
            .collect();
        order.sort();
        let mut events = Vec::new();
        for (_, id) in order {
            let entry = tracked.get_mut(&id).unwrap();
            if entry.watch.is_none() {
                if let Some(path) = client_logs::find_new_log(&dir, entry.since, &claimed) {
                    claimed.insert(path.clone());
                    entry.watch = Some(LogWatch::new(path));
                }
            }
            if let Some(watch) = entry.watch.as_mut() {
                match watch.poll() {
                    Ok(found) => events.extend(found.into_iter().map(|event| (id.clone(), event))),
                    Err(error) => tracing::debug!(%error, "Client log not readable"),
                }
            }
        }
        events
    }
}

pub fn disconnect_text(code: u32, name: &str) -> String {
    let what = client_logs::describe_disconnect(code);
    if name.is_empty() {
        format!("{code}: {what}")
    } else {
        format!("{code}: {what} ({name})")
    }
}

/// How waiting for a client to join ended.
#[derive(Debug, PartialEq, Eq)]
pub enum JoinWait {
    InGame,
    Disconnected(String),
    Exited,
    NoLog,
    TimedOut,
}

impl Launcher {
    /// Reads new lines of the clients' logs and records their stage. Returns the events.
    pub async fn refresh_client_logs(&self) -> Vec<(String, LogEvent)> {
        if !self.client_logs.enabled() {
            return Vec::new();
        }
        let alive: HashSet<String> = self
            .instances
            .snapshot()
            .into_iter()
            .filter(|instance| instance.status != InstanceStatus::Exited)
            .map(|instance| instance.id)
            .collect();
        let logs = self.client_logs.clone();
        let events = tokio::task::spawn_blocking(move || logs.poll(&alive))
            .await
            .unwrap_or_default();
        for (id, event) in &events {
            match event {
                LogEvent::Joining { .. } => {
                    self.instances.set_stage(id, ClientStage::Joining, None)
                }
                LogEvent::InGame => self.instances.set_stage(id, ClientStage::InGame, None),
                LogEvent::Disconnected { code, name } => {
                    if client_logs::is_unexpected(*code) {
                        self.instances.set_stage(
                            id,
                            ClientStage::Disconnected,
                            Some(disconnect_text(*code, name)),
                        );
                    } else {
                        // Leaving on purpose (a teleport): the next join follows.
                        self.instances.set_stage(id, ClientStage::Joining, None);
                    }
                }
            }
        }
        events
    }

    /// Waits until the client is in its game (or why it never got there), up to `timeout`.
    pub async fn wait_until_in_game(
        &self,
        instance: &str,
        timeout: Duration,
        cancel: &CancellationToken,
    ) -> CoreResult<JoinWait> {
        let started = Instant::now();
        loop {
            self.refresh_client_logs().await;
            let Some(client) = self
                .instances
                .snapshot()
                .into_iter()
                .find(|client| client.id == instance)
            else {
                return Ok(JoinWait::Exited);
            };
            if client.status == InstanceStatus::Exited {
                return Ok(JoinWait::Exited);
            }
            match client.stage {
                ClientStage::InGame => return Ok(JoinWait::InGame),
                ClientStage::Disconnected => {
                    return Ok(JoinWait::Disconnected(
                        client.disconnect.unwrap_or_default(),
                    ))
                }
                _ => {}
            }
            let no_log = self.client_logs.has_log(instance) != Some(true)
                && self
                    .client_logs
                    .registered(instance)
                    .is_none_or(|at| at.elapsed() > NO_LOG_GIVE_UP);
            if no_log {
                return Ok(JoinWait::NoLog);
            }
            if started.elapsed() >= timeout {
                return Ok(JoinWait::TimedOut);
            }
            tokio::select! {
                _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                _ = tokio::time::sleep(WAIT_POLL) => {}
            }
        }
    }

    /// With `InGame` pacing, waits for the previous account's client to join before the next
    /// account starts. Returns `false` when the fixed pause should be used instead (no logs).
    pub(super) async fn pace_after(
        &self,
        task_id: u32,
        previous: &str,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> bool {
        if config.launch_pacing != crate::models::LaunchPacing::InGame
            || !self.client_logs.enabled()
        {
            return false;
        }
        self.sessions.tasks.set(
            task_id,
            crate::models::TaskStatus::Queued,
            "Waiting for the previous account to join its game.".to_owned(),
        );
        let timeout = Duration::from_secs(config.in_game_timeout_secs as u64);
        let outcome = match self.wait_until_in_game(previous, timeout, cancel).await {
            Ok(outcome) => outcome,
            Err(_) => return true,
        };
        let note = match &outcome {
            JoinWait::InGame => None,
            JoinWait::Disconnected(why) => Some(format!(
                "The previous client was disconnected ({why}); starting the next account."
            )),
            JoinWait::Exited => Some(
                "The previous client closed before joining; starting the next account.".to_owned(),
            ),
            JoinWait::TimedOut => Some(format!(
                "The previous client did not join within {} s; starting the next account.",
                config.in_game_timeout_secs
            )),
            JoinWait::NoLog => return false,
        };
        if let Some(note) = note {
            let _ = self.accounts.log("WARNING", "LAUNCH", &note, None).await;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnects_are_described_for_people() {
        assert_eq!(
            disconnect_text(277, "DisconnectConnectionLost"),
            "277: the connection was lost (DisconnectConnectionLost)"
        );
        assert_eq!(disconnect_text(999, ""), "999: an unknown reason");
    }
}

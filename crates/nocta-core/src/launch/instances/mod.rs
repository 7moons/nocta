//! Ownership tracking for game clients started by Nocta.
//!
//! Only processes that appear after Nocta's own launch request are claimed. Each claimed process is held through a
//! process handle that proves identity (the HANDLE and its creation time),
//! so PID reuse can never redirect a close request to an unrelated process. Game clients keep running when Nocta
//! exits; they are closed only by explicit user commands.
//!
//! Clients Nocta did not launch are listed as external instances. They are never claimed for an account and are
//! closed only when the user asks (one at a time, or "close all Roblox" to free the multi-instance lock).
use crate::{
    models::{
        ExternalInstance, GameInstance, InstanceStatus, LaunchMode, LauncherState,
        MultiInstanceState,
    },
    CoreError, CoreResult,
};
use chrono::Utc;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub path: Option<String>,
    /// Process creation time as Unix milliseconds.
    pub created_ms: u64,
    pub player: bool,
}
impl ProcessInfo {
    fn key(&self) -> (u32, u64) {
        (self.pid, self.created_ms)
    }
}

/// What anti-AFK does in one client: press a key or click a point of its window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfkInput {
    Key(crate::models::AntiAfkAction),
    Click(crate::models::ClickPoint),
}

impl AfkInput {
    /// The account's own click point, else the general action (a click goes to the general point).
    pub fn choose(
        config: &crate::models::AppConfig,
        account_point: Option<crate::models::ClickPoint>,
    ) -> Self {
        match (account_point, config.anti_afk_action) {
            (Some(point), _) => Self::Click(point),
            (None, crate::models::AntiAfkAction::Click) => Self::Click(config.anti_afk_click),
            (None, action) => Self::Key(action),
        }
    }
}

/// A monitor's work area (without the taskbar), in screen pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Monitor {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

pub trait OwnedProcess: Send + Sync {
    fn pid(&self) -> u32;
    /// `Ok(false)` once the exact process identity has exited.
    fn alive(&self) -> CoreResult<bool>;
    fn terminate(&self) -> CoreResult<()>;
}

pub trait NativePlatform: Send + Sync + 'static {
    /// Registered `roblox-player` handler, discovered from the OS association (never guessed).
    fn handler(&self) -> CoreResult<Option<String>>;
    /// Passes the literal URI to the OS handler. Success means dispatch only, not a joined game.
    fn dispatch(&self, uri: &str) -> CoreResult<()>;
    /// Roblox client/launcher processes currently running.
    fn processes(&self) -> CoreResult<Vec<ProcessInfo>>;
    /// Opens an identity-checked handle to a process observed by `processes`.
    fn open(&self, process: &ProcessInfo) -> CoreResult<Box<dyn OwnedProcess>>;
    fn multi_instance_supported(&self) -> bool;
    /// Minimizes the visible top-level windows of `pid`; `false` while it has none yet.
    fn minimize(&self, _pid: u32) -> bool {
        false
    }
    /// Presses the anti-AFK key or clicks in the client's window; `false` when it has no window.
    fn nudge(&self, _pid: u32, _input: AfkInput) -> bool {
        false
    }
    /// Pages out the client's unused memory.
    fn trim_memory(&self, _pid: u32) -> bool {
        false
    }
    /// The client's working set (RAM it holds) in megabytes.
    fn memory_mb(&self, _pid: u32) -> Option<u64> {
        None
    }
    /// Memory the client allocated (its commit) in megabytes.
    fn committed_mb(&self, _pid: u32) -> Option<u64> {
        None
    }
    /// Seconds since the user last used the keyboard or mouse.
    fn user_idle_secs(&self) -> Option<u64> {
        None
    }
    /// RAM and commit of the whole computer.
    fn system_memory(&self) -> Option<crate::models::SystemMemory> {
        None
    }
    fn set_title(&self, _pid: u32, _title: &str) -> bool {
        false
    }
    fn monitors(&self) -> Vec<Monitor> {
        Vec::new()
    }
    fn place_window(&self, _pid: u32, _x: i32, _y: i32, _width: u32, _height: u32) -> bool {
        false
    }
    fn resize_window(&self, _pid: u32, _width: u32, _height: u32) -> bool {
        false
    }
    /// Whether the client's window is "Not responding"; `None` without a window.
    fn hung(&self, _pid: u32) -> Option<bool> {
        None
    }
    fn apply_profile(&self, _pid: u32, _profile: &crate::models::ClientProfile) -> CoreResult<()> {
        Ok(())
    }
    fn keep_awake(&self, _on: bool) {}
    /// Lets the user drag a Roblox window's border below Roblox's minimum size.
    fn free_resize(&self, _on: bool) {}
}

pub const REGISTRATION_WINDOW: Duration = Duration::from_secs(20);
pub const REGISTRATION_POLL: Duration = Duration::from_millis(500);
pub const MONITOR_INTERVAL: Duration = Duration::from_secs(3);
/// Clock slack between the dispatch timestamp and OS process creation times.
const CREATION_SLACK_MS: u64 = 2000;
const KEEP_EXITED: usize = 200;

struct Tracked {
    instance: GameInstance,
    handle: Option<Arc<dyn OwnedProcess>>,
    process: Option<ProcessInfo>,
    /// Launcher awaiting handoff to a player process.
    launcher: Option<ProcessInfo>,
    dispatched_ms: u64,
    baseline: HashSet<(u32, u64)>,
    claim_open: bool,
}

pub struct LaunchRecord<'a> {
    pub account_id: u32,
    pub username: &'a str,
    pub place_id: &'a str,
    pub mode: LaunchMode,
}

/// A client Nocta started that closed by itself (crash, kick or the user closing its window),
/// not through Nocta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExitEvent {
    pub instance_id: String,
    pub account_id: u32,
    pub username: String,
    pub place_id: String,
}

pub struct InstanceManager {
    platform: Arc<dyn NativePlatform>,
    /// Instances Nocta is closing right now; their exit is not reported as a crash.
    closing: Mutex<HashSet<String>>,
    exits: Mutex<Option<tokio::sync::mpsc::UnboundedSender<ExitEvent>>>,
    tracked: Mutex<Vec<Tracked>>,
    /// Running game clients Nocta did not launch, refreshed by every monitor pass.
    external: Mutex<Vec<ProcessInfo>>,
    dispatch: tokio::sync::Mutex<()>,
    pub changed: Arc<Notify>,
    multi: Mutex<super::multi::MultiInstance>,
    registration_window: Duration,
    poll: Duration,
}

fn now_ms() -> u64 {
    Utc::now().timestamp_millis().max(0) as u64
}

impl InstanceManager {
    pub fn new(platform: Arc<dyn NativePlatform>, changed: Arc<Notify>) -> Arc<Self> {
        Arc::new(Self {
            platform,
            closing: Mutex::new(HashSet::new()),
            exits: Mutex::new(None),
            tracked: Mutex::new(Vec::new()),
            external: Mutex::new(Vec::new()),
            dispatch: tokio::sync::Mutex::new(()),
            changed,
            multi: Mutex::new(super::multi::MultiInstance::default()),
            registration_window: REGISTRATION_WINDOW,
            poll: REGISTRATION_POLL,
        })
    }
    #[cfg(test)]
    pub(crate) fn fast(platform: Arc<dyn NativePlatform>) -> Arc<Self> {
        Arc::new(Self {
            platform,
            closing: Mutex::new(HashSet::new()),
            exits: Mutex::new(None),
            tracked: Mutex::new(Vec::new()),
            external: Mutex::new(Vec::new()),
            dispatch: tokio::sync::Mutex::new(()),
            changed: Arc::new(Notify::new()),
            multi: Mutex::new(super::multi::MultiInstance::default()),
            registration_window: Duration::from_millis(300),
            poll: Duration::from_millis(10),
        })
    }

    /// Receives every client that closes by itself. A new subscription replaces the old one.
    pub fn subscribe_exits(&self) -> tokio::sync::mpsc::UnboundedReceiver<ExitEvent> {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        *self.exits.lock().unwrap() = Some(sender);
        receiver
    }

    pub(super) fn report_exit(&self, event: ExitEvent) {
        if let Some(sender) = self.exits.lock().unwrap().as_ref() {
            let _ = sender.send(event);
        }
    }

    pub fn native() -> Arc<dyn NativePlatform> {
        super::native::platform()
    }

    pub fn state(&self) -> LauncherState {
        let handler = self.platform.handler();
        let multi = self.multi.lock().unwrap();
        LauncherState {
            handler_registered: matches!(handler, Ok(Some(_))),
            handler_path: handler.ok().flatten(),
            multi_instance: if self.platform.multi_instance_supported() {
                multi.state()
            } else {
                MultiInstanceState::Unsupported
            },
            multi_instance_supported: self.platform.multi_instance_supported(),
            detail: multi.detail(),
        }
    }

    /// Applies the multi-instance setting. Must succeed before an enabled setting is saved or a launch starts.
    pub fn set_multi_instance(&self, enabled: bool) -> CoreResult<MultiInstanceState> {
        if enabled && !self.platform.multi_instance_supported() {
            return Err(CoreError::Validation(
                "Multi-instance is not supported on this platform.".into(),
            ));
        }
        let mut multi = self.multi.lock().unwrap();
        let state = if enabled {
            multi.acquire()?
        } else {
            multi.release();
            MultiInstanceState::Disabled
        };
        self.changed.notify_one();
        if enabled
            && !matches!(
                state,
                MultiInstanceState::Acquired | MultiInstanceState::Abandoned
            )
        {
            return Err(CoreError::Validation(multi.detail().unwrap_or_else(|| {
                "Multi-instance could not be enabled. Close running Roblox clients and try again.".into()
            })));
        }
        Ok(state)
    }
    pub fn ensure_multi_instance(&self) -> CoreResult<()> {
        let state = self.multi.lock().unwrap().state();
        if matches!(
            state,
            MultiInstanceState::Acquired | MultiInstanceState::Abandoned
        ) {
            return Ok(());
        }
        match self.set_multi_instance(true)? {
            MultiInstanceState::Acquired | MultiInstanceState::Abandoned => Ok(()),
            _ => Err(CoreError::Validation(
                "Multi-instance could not be enabled. Close running Roblox clients and try again."
                    .into(),
            )),
        }
    }

    pub fn snapshot(&self) -> Vec<GameInstance> {
        self.tracked
            .lock()
            .unwrap()
            .iter()
            .map(|tracked| tracked.instance.clone())
            .collect()
    }

    pub fn start_monitor(self: &Arc<Self>, stop: CancellationToken) -> tokio::task::JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    _ = tokio::time::sleep(MONITOR_INTERVAL) => manager.poll().await,
                }
            }
        })
    }

    /// Closes one owned instance. Processes not started by Nocta are never targeted.
    pub async fn close(self: &Arc<Self>, id: &str) -> CoreResult<()> {
        let handle = {
            let mut tracked = self.tracked.lock().unwrap();
            let entry = tracked
                .iter_mut()
                .find(|tracked| tracked.instance.id == id)
                .ok_or(CoreError::NotFound)?;
            match entry.handle.clone() {
                Some(handle) => handle,
                None => {
                    if entry.instance.status != InstanceStatus::Exited {
                        entry.instance.status = InstanceStatus::Exited;
                        entry.instance.exited_at = Some(Utc::now().to_rfc3339());
                        entry.instance.detail = "Removed: no owned process was tracked.".into();
                        self.changed.notify_one();
                    }
                    return Ok(());
                }
            }
        };
        self.closing.lock().unwrap().insert(id.to_owned());
        let result = blocking(move || handle.terminate()).await;
        self.closing.lock().unwrap().remove(id);
        let mut tracked = self.tracked.lock().unwrap();
        if let Some(entry) = tracked.iter_mut().find(|tracked| tracked.instance.id == id) {
            match &result {
                Ok(()) => {
                    entry.handle = None;
                    entry.instance.status = InstanceStatus::Exited;
                    entry.instance.exited_at = Some(Utc::now().to_rfc3339());
                    entry.instance.detail = "Closed by Nocta.".into();
                    entry.launcher = None;
                }
                // Ownership is kept so the user can retry.
                Err(_) => entry.instance.detail = "Closing failed. Try again.".into(),
            }
        }
        self.changed.notify_one();
        result
    }

    pub async fn close_all(self: &Arc<Self>) -> CoreResult<u32> {
        let ids = self
            .tracked
            .lock()
            .unwrap()
            .iter()
            .filter(|tracked| tracked.instance.status != InstanceStatus::Exited)
            .map(|tracked| tracked.instance.id.clone())
            .collect::<Vec<_>>();
        let mut closed = 0;
        let mut failure = None;
        for id in ids {
            match self.close(&id).await {
                Ok(()) => closed += 1,
                Err(error) => failure = Some(error),
            }
        }
        match failure {
            Some(error) if closed == 0 => Err(error),
            _ => Ok(closed),
        }
    }

    /// Records what the client's log says about it.
    /// Records how much memory a client holds. Small changes are not published, so the window
    /// is not updated every few seconds for nothing.
    pub fn set_memory(&self, id: &str, megabytes: u32) {
        let mut tracked = self.tracked.lock().unwrap();
        if let Some(entry) = tracked.iter_mut().find(|entry| entry.instance.id == id) {
            let changed = entry
                .instance
                .memory_mb
                .is_none_or(|old| old.abs_diff(megabytes) >= 16);
            if changed {
                entry.instance.memory_mb = Some(megabytes);
                self.changed.notify_one();
            }
        }
    }

    pub fn set_stage(
        &self,
        id: &str,
        stage: crate::models::ClientStage,
        disconnect: Option<String>,
    ) {
        let mut tracked = self.tracked.lock().unwrap();
        if let Some(entry) = tracked.iter_mut().find(|entry| entry.instance.id == id) {
            if entry.instance.stage != stage
                || (disconnect.is_some() && entry.instance.disconnect != disconnect)
            {
                entry.instance.stage = stage;
                if disconnect.is_some() {
                    entry.instance.disconnect = disconnect;
                }
                self.changed.notify_one();
            }
        }
    }

    /// Moves a client's start time back, so tests can check timed restarts.
    #[cfg(test)]
    pub fn backdate(&self, id: &str, secs: i64) {
        let mut tracked = self.tracked.lock().unwrap();
        if let Some(entry) = tracked.iter_mut().find(|entry| entry.instance.id == id) {
            let started = chrono::DateTime::parse_from_rfc3339(&entry.instance.started_at).unwrap();
            entry.instance.started_at = (started - chrono::Duration::seconds(secs)).to_rfc3339();
        }
    }

    /// The platform, for window and process control of running clients.
    pub fn platform(&self) -> Arc<dyn NativePlatform> {
        self.platform.clone()
    }

    /// Presses the anti-AFK key in a client's window.
    pub async fn nudge(&self, pid: u32, input: AfkInput) -> bool {
        let platform = self.platform.clone();
        blocking(move || Ok(platform.nudge(pid, input)))
            .await
            .unwrap_or(false)
    }

    pub async fn trim_memory(&self, pid: u32) -> bool {
        let platform = self.platform.clone();
        blocking(move || Ok(platform.trim_memory(pid)))
            .await
            .unwrap_or(false)
    }

    pub fn memory_mb(&self, pid: u32) -> Option<u64> {
        self.platform.memory_mb(pid)
    }

    pub fn committed_mb(&self, pid: u32) -> Option<u64> {
        self.platform.committed_mb(pid)
    }

    pub fn system_memory(&self) -> Option<crate::models::SystemMemory> {
        self.platform.system_memory()
    }

    pub fn user_idle_secs(&self) -> Option<u64> {
        self.platform.user_idle_secs()
    }

    /// Minimizes a client's window once it appears (the AFK profile). Gives up after a minute.
    pub fn minimize_when_shown(self: &Arc<Self>, pid: u32) {
        let manager = self.clone();
        tokio::spawn(async move {
            for _ in 0..30 {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let platform = manager.platform.clone();
                if blocking(move || Ok(platform.minimize(pid)))
                    .await
                    .unwrap_or(false)
                {
                    break;
                }
            }
        });
    }

    /// Releases the multi-instance helper. Game clients are intentionally left running.
    pub fn shutdown(&self) {
        self.multi.lock().unwrap().release();
    }
}

/// Starts the game client detached, with the launch URI as its only argument (as the system handler does).
/// The process is not awaited; the monitor claims it like any other launch.
fn spawn_client(executable: &std::path::Path, uri: &str) -> CoreResult<()> {
    let mut command = std::process::Command::new(executable);
    command
        .arg(uri)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(directory) = executable.parent() {
        command.current_dir(directory);
    }
    command.spawn().map(drop).map_err(|error| {
        CoreError::Validation(format!(
            "Nocta's Roblox client could not be started: {error}"
        ))
    })
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> CoreResult<T> + Send + 'static,
) -> CoreResult<T> {
    tokio::task::spawn_blocking(work).await.map_err(|_| {
        CoreError::Validation("A native launcher operation stopped unexpectedly.".into())
    })?
}

#[cfg(test)]
pub(crate) mod fake;

mod external;
#[cfg(test)]
mod tests;
mod tracking;

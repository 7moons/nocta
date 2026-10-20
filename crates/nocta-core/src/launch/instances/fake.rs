//! A scriptable platform for tests: processes appear and exit when a test says so.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// A window move: pid, x, y, width, height.
pub type Placement = (u32, i32, i32, u32, u32);

/// Scriptable platform: `on_dispatch` decides which processes appear after a URI is dispatched.
#[derive(Default)]
pub struct FakePlatform {
    pub running: Mutex<Vec<ProcessInfo>>,
    pub dispatched: Mutex<Vec<String>>,
    pub killed: Mutex<Vec<(u32, u64)>>,
    pub spawn_on_dispatch: Mutex<Vec<ProcessInfo>>,
    pub no_handler: AtomicBool,
    pub fail_enumeration: AtomicBool,
    pub next_pid: AtomicU32,
    pub nudged: Mutex<Vec<u32>>,
    pub clicks: Mutex<Vec<(u32, crate::models::ClickPoint)>>,
    pub trimmed: Mutex<Vec<u32>>,
    pub memory: Mutex<std::collections::HashMap<u32, u64>>,
    /// Seconds since the user's last input; `None` when unknown.
    pub user_idle: Mutex<Option<u64>>,
    pub hung: Mutex<HashSet<u32>>,
    pub titles: Mutex<std::collections::HashMap<u32, String>>,
    pub placed: Mutex<Vec<Placement>>,
    pub resized: Mutex<Vec<(u32, u32, u32)>>,
    pub profiles: Mutex<Vec<(u32, crate::models::ClientProfile)>>,
    pub monitors: Mutex<Vec<Monitor>>,
    pub awake: AtomicBool,
}
impl FakePlatform {
    pub fn process(&self, name: &str, player: bool, parent: u32) -> ProcessInfo {
        let pid = 1000 + self.next_pid.fetch_add(1, Ordering::SeqCst);
        ProcessInfo {
            pid,
            parent_pid: parent,
            name: name.into(),
            path: Some(format!("C:/Roblox/{name}")),
            created_ms: now_ms(),
            player,
        }
    }
}
struct FakeHandle {
    platform: Arc<FakePlatform>,
    key: (u32, u64),
}
impl OwnedProcess for FakeHandle {
    fn pid(&self) -> u32 {
        self.key.0
    }
    fn alive(&self) -> CoreResult<bool> {
        Ok(self
            .platform
            .running
            .lock()
            .unwrap()
            .iter()
            .any(|process| process.key() == self.key))
    }
    fn terminate(&self) -> CoreResult<()> {
        let mut running = self.platform.running.lock().unwrap();
        // Identity check: only the exact (pid, creation time) is terminated.
        running.retain(|process| process.key() != self.key);
        self.platform.killed.lock().unwrap().push(self.key);
        Ok(())
    }
}
pub struct Shared(pub Arc<FakePlatform>);
impl NativePlatform for Shared {
    fn handler(&self) -> CoreResult<Option<String>> {
        Ok((!self.0.no_handler.load(Ordering::SeqCst))
            .then(|| "C:/Roblox/RobloxPlayerBeta.exe".into()))
    }
    fn dispatch(&self, uri: &str) -> CoreResult<()> {
        self.0.dispatched.lock().unwrap().push(uri.into());
        let spawned = std::mem::take(&mut *self.0.spawn_on_dispatch.lock().unwrap());
        self.0.running.lock().unwrap().extend(spawned);
        Ok(())
    }
    fn processes(&self) -> CoreResult<Vec<ProcessInfo>> {
        if self.0.fail_enumeration.load(Ordering::SeqCst) {
            return Err(CoreError::Validation("enumeration failed".into()));
        }
        Ok(self.0.running.lock().unwrap().clone())
    }
    fn open(&self, process: &ProcessInfo) -> CoreResult<Box<dyn OwnedProcess>> {
        Ok(Box::new(FakeHandle {
            platform: self.0.clone(),
            key: process.key(),
        }))
    }
    fn multi_instance_supported(&self) -> bool {
        false
    }
    fn nudge(&self, pid: u32, input: AfkInput) -> bool {
        self.0.nudged.lock().unwrap().push(pid);
        if let AfkInput::Click(point) = input {
            self.0.clicks.lock().unwrap().push((pid, point));
        }
        true
    }
    fn trim_memory(&self, pid: u32) -> bool {
        self.0.trimmed.lock().unwrap().push(pid);
        true
    }
    fn memory_mb(&self, pid: u32) -> Option<u64> {
        self.0.memory.lock().unwrap().get(&pid).copied()
    }
    fn committed_mb(&self, pid: u32) -> Option<u64> {
        self.0.memory.lock().unwrap().get(&pid).copied()
    }
    fn user_idle_secs(&self) -> Option<u64> {
        *self.0.user_idle.lock().unwrap()
    }
    fn set_title(&self, pid: u32, title: &str) -> bool {
        self.0.titles.lock().unwrap().insert(pid, title.to_owned());
        true
    }
    fn monitors(&self) -> Vec<Monitor> {
        self.0.monitors.lock().unwrap().clone()
    }
    fn place_window(&self, pid: u32, x: i32, y: i32, width: u32, height: u32) -> bool {
        self.0
            .placed
            .lock()
            .unwrap()
            .push((pid, x, y, width, height));
        true
    }
    fn resize_window(&self, pid: u32, width: u32, height: u32) -> bool {
        self.0.resized.lock().unwrap().push((pid, width, height));
        true
    }
    fn hung(&self, pid: u32) -> Option<bool> {
        Some(self.0.hung.lock().unwrap().contains(&pid))
    }
    fn apply_profile(&self, pid: u32, profile: &crate::models::ClientProfile) -> CoreResult<()> {
        self.0.profiles.lock().unwrap().push((pid, *profile));
        Ok(())
    }
    fn keep_awake(&self, on: bool) {
        self.0.awake.store(on, Ordering::SeqCst);
    }
}

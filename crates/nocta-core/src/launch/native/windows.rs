use super::super::instances::{NativePlatform, OwnedProcess, ProcessInfo};
use crate::{CoreError, CoreResult};
use windows_sys::Win32::{
    Foundation::{CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE, WAIT_TIMEOUT},
    System::{
        Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        Threading::{
            GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
            WaitForSingleObject, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
        },
    },
    UI::{
        Shell::{
            AssocQueryStringW, ShellExecuteExW, ASSOCF_IS_PROTOCOL, ASSOCSTR_EXECUTABLE,
            SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW,
        },
        WindowsAndMessaging::SW_SHOWNORMAL,
    },
};

pub const PLAYER: &str = "robloxplayerbeta.exe";
pub const LAUNCHER: &str = "robloxplayerlauncher.exe";

pub struct WindowsPlatform;

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
fn from_wide(value: &[u16]) -> String {
    let end = value.iter().position(|c| *c == 0).unwrap_or(value.len());
    String::from_utf16_lossy(&value[..end])
}
fn filetime_ms(time: FILETIME) -> u64 {
    let ticks = ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64;
    // FILETIME is 100 ns since 1601-01-01.
    (ticks / 10_000).saturating_sub(11_644_473_600_000)
}

struct Handle(HANDLE);
// SAFETY: process handles may be used from any thread.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

fn open(pid: u32, access: u32) -> Option<Handle> {
    let handle = unsafe { OpenProcess(access, 0, pid) };
    (!handle.is_null()).then_some(Handle(handle))
}
fn creation_ms(handle: &Handle) -> Option<u64> {
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let ok =
        unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) };
    (ok != 0).then(|| filetime_ms(created))
}
fn image_path(handle: &Handle) -> Option<String> {
    let mut buffer = vec![0u16; 32768];
    let mut size = buffer.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(handle.0, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut size)
    };
    (ok != 0).then(|| String::from_utf16_lossy(&buffer[..size as usize]))
}

impl WindowsPlatform {
    fn launcher_names(&self) -> Vec<String> {
        let mut names = vec![LAUNCHER.to_owned()];
        if let Ok(Some(handler)) = self.handler() {
            if let Some(name) = std::path::Path::new(&handler).file_name() {
                let name = name.to_string_lossy().to_lowercase();
                if name != PLAYER && !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        names
    }
}

impl NativePlatform for WindowsPlatform {
    fn handler(&self) -> CoreResult<Option<String>> {
        let protocol = wide("roblox-player");
        let verb = wide("open");
        let mut buffer = vec![0u16; 1024];
        let mut length = buffer.len() as u32;
        let result = unsafe {
            AssocQueryStringW(
                ASSOCF_IS_PROTOCOL,
                ASSOCSTR_EXECUTABLE,
                protocol.as_ptr(),
                verb.as_ptr(),
                buffer.as_mut_ptr(),
                &mut length,
            )
        };
        if result < 0 {
            return Ok(None);
        }
        let path = from_wide(&buffer);
        Ok((!path.is_empty() && std::path::Path::new(&path).is_file()).then_some(path))
    }

    fn dispatch(&self, uri: &str) -> CoreResult<()> {
        if !uri.starts_with("roblox-player:") || uri.contains(['\0', '\r', '\n', '"', ' ']) {
            return Err(CoreError::Validation(
                "Refusing to dispatch an invalid launch URI.".into(),
            ));
        }
        let verb = wide("open");
        let file = zeroize::Zeroizing::new(wide(uri));
        // COM is required by some protocol handlers; initialise on this blocking thread only.
        let com = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.nShow = SW_SHOWNORMAL;
        // The URI is passed literally to the registered handler; no shell or command line is built.
        let ok = unsafe { ShellExecuteExW(&mut info) };
        if com >= 0 {
            unsafe { CoUninitialize() };
        }
        if ok == 0 {
            return Err(CoreError::Validation(
                "Windows could not open the Roblox launch link. Check the Roblox installation."
                    .into(),
            ));
        }
        Ok(())
    }

    fn processes(&self) -> CoreResult<Vec<ProcessInfo>> {
        let launchers = self.launcher_names();
        // Without a system handler (Nocta's own client only), game clients are still listed;
        // launchers need the handler's folder and are then skipped.
        let handler_parent = self.handler()?.and_then(|handler| {
            std::path::Path::new(&handler)
                .parent()
                .map(|path| path.to_string_lossy().to_lowercase())
        });
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
            return Err(CoreError::Validation("Cannot enumerate processes.".into()));
        }
        let snapshot = Handle(snapshot);
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut processes = Vec::new();
        let mut more = unsafe { Process32FirstW(snapshot.0, &mut entry) } != 0;
        while more {
            let name = from_wide(&entry.szExeFile);
            let lower = name.to_lowercase();
            let player = lower == PLAYER;
            if player || launchers.contains(&lower) {
                if let Some(handle) = open(entry.th32ProcessID, PROCESS_QUERY_LIMITED_INFORMATION) {
                    if let Some(created_ms) = creation_ms(&handle) {
                        let path = image_path(&handle);
                        let same_installation = handler_parent.is_some()
                            && path.as_ref().and_then(|path| {
                                std::path::Path::new(path)
                                    .parent()
                                    .map(|parent| parent.to_string_lossy().to_lowercase())
                            }) == handler_parent;
                        // Game clients are recognised from any install (bootstrappers such as
                        // Bloxstrap keep their own version folders); launcher names stay tied to the
                        // registered handler's folder because they are less distinctive.
                        if !same_installation && !player {
                            more = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
                            continue;
                        }
                        processes.push(ProcessInfo {
                            pid: entry.th32ProcessID,
                            parent_pid: entry.th32ParentProcessID,
                            name,
                            path,
                            created_ms,
                            player,
                        });
                    }
                }
            }
            more = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
        }
        Ok(processes)
    }

    fn open(&self, process: &ProcessInfo) -> CoreResult<Box<dyn OwnedProcess>> {
        let handle = open(
            process.pid,
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE,
        )
        .ok_or_else(|| CoreError::Validation("The Roblox process could not be opened.".into()))?;
        // Identity check after opening: a reused PID has a different creation time or image.
        if creation_ms(&handle) != Some(process.created_ms)
            || (process.path.is_some() && image_path(&handle) != process.path)
        {
            return Err(CoreError::Validation(
                "The observed process identity changed.".into(),
            ));
        }
        Ok(Box::new(WindowsProcess {
            pid: process.pid,
            handle,
        }))
    }

    fn multi_instance_supported(&self) -> bool {
        true
    }

    fn minimize(&self, pid: u32) -> bool {
        super::windows_window::minimize(pid)
    }
    fn nudge(&self, pid: u32, input: crate::launch::instances::AfkInput) -> bool {
        super::windows_window::nudge(pid, input)
    }
    fn trim_memory(&self, pid: u32) -> bool {
        super::windows_window::trim_memory(pid)
    }
    fn memory_mb(&self, pid: u32) -> Option<u64> {
        super::windows_window::memory_mb(pid)
    }
    fn committed_mb(&self, pid: u32) -> Option<u64> {
        super::windows_window::committed_mb(pid)
    }
    fn system_memory(&self) -> Option<crate::models::SystemMemory> {
        super::windows_window::system_memory()
    }
    fn user_idle_secs(&self) -> Option<u64> {
        super::windows_window::user_idle_secs()
    }
    fn set_title(&self, pid: u32, title: &str) -> bool {
        super::windows_control::set_title(pid, title)
    }
    fn monitors(&self) -> Vec<crate::launch::instances::Monitor> {
        super::windows_control::monitors()
    }
    fn place_window(&self, pid: u32, x: i32, y: i32, width: u32, height: u32) -> bool {
        super::windows_control::place_window(pid, x, y, width, height)
    }
    fn resize_window(&self, pid: u32, width: u32, height: u32) -> bool {
        super::windows_control::resize_window(pid, width, height)
    }
    fn hung(&self, pid: u32) -> Option<bool> {
        super::windows_control::hung(pid)
    }
    fn apply_profile(&self, pid: u32, profile: &crate::models::ClientProfile) -> CoreResult<()> {
        super::windows_control::apply_profile(pid, profile)
    }
    fn keep_awake(&self, on: bool) {
        super::windows_control::keep_awake(on)
    }
    fn free_resize(&self, on: bool) {
        super::windows_resize::set_enabled(on)
    }
}

/// Holding the HANDLE keeps the process object referenced, so terminate always targets this exact process.
struct WindowsProcess {
    pid: u32,
    handle: Handle,
}
impl OwnedProcess for WindowsProcess {
    fn pid(&self) -> u32 {
        self.pid
    }
    fn alive(&self) -> CoreResult<bool> {
        Ok(unsafe { WaitForSingleObject(self.handle.0, 0) } == WAIT_TIMEOUT)
    }
    fn terminate(&self) -> CoreResult<()> {
        if !self.alive()? {
            return Ok(());
        }
        if unsafe { TerminateProcess(self.handle.0, 0) } == 0 && self.alive()? {
            return Err(CoreError::Validation(
                "The game client could not be closed.".into(),
            ));
        }
        let _ = unsafe { WaitForSingleObject(self.handle.0, 5000) };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_identity_uses_creation_time_and_handle() {
        // Own child process stands in for a game client: identity, liveness and terminate on a real handle.
        let mut child = std::process::Command::new("cmd.exe")
            .args(["/C", "ping -n 30 127.0.0.1 >NUL"])
            .spawn()
            .unwrap();
        let handle = open(child.id(), PROCESS_QUERY_LIMITED_INFORMATION).unwrap();
        let info = ProcessInfo {
            pid: child.id(),
            parent_pid: std::process::id(),
            name: "cmd.exe".into(),
            path: image_path(&handle),
            created_ms: creation_ms(&handle).unwrap(),
            player: true,
        };
        let mut wrong = info.clone();
        wrong.created_ms += 1;
        assert!(
            WindowsPlatform.open(&wrong).is_err(),
            "PID reuse is rejected"
        );
        let owned = WindowsPlatform.open(&info).unwrap();
        assert!(owned.alive().unwrap());
        owned.terminate().unwrap();
        assert!(!owned.alive().unwrap());
        let _ = child.wait();
        assert!(
            filetime_ms(FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0
            }) == 0
        );
    }

    #[test]
    fn dispatch_rejects_non_roblox_uris_and_enumeration_works() {
        assert!(WindowsPlatform.dispatch("https://example.com").is_err());
        assert!(WindowsPlatform
            .dispatch("roblox-player:1+x \"&calc")
            .is_err());
        assert!(WindowsPlatform.processes().is_ok());
        let _ = WindowsPlatform.handler().unwrap();
    }
}

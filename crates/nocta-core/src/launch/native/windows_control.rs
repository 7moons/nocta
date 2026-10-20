//! Placing client windows, naming them, noticing frozen ones, limiting what a client may use,
//! and keeping the computer awake.
use super::windows_window::process_windows;
use crate::{
    launch::instances::Monitor,
    models::{ClientProfile, ProcessPriority},
    CoreError, CoreResult,
};
use std::sync::atomic::{AtomicBool, Ordering};
use windows_sys::core::BOOL;
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, LPARAM, RECT},
    Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectCpuRateControlInformation,
            SetInformationJobObject, JOBOBJECT_CPU_RATE_CONTROL_INFORMATION,
            JOB_OBJECT_CPU_RATE_CONTROL_ENABLE, JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
        },
        Memory::{
            SetProcessWorkingSetSizeEx, QUOTA_LIMITS_HARDWS_MAX_DISABLE,
            QUOTA_LIMITS_HARDWS_MAX_ENABLE, QUOTA_LIMITS_HARDWS_MIN_DISABLE,
        },
        Power::{SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED},
        Threading::{
            OpenProcess, ProcessMemoryPriority, ProcessPowerThrottling, SetPriorityClass,
            SetProcessInformation, BELOW_NORMAL_PRIORITY_CLASS, IDLE_PRIORITY_CLASS,
            MEMORY_PRIORITY_INFORMATION, MEMORY_PRIORITY_LOW, MEMORY_PRIORITY_NORMAL,
            NORMAL_PRIORITY_CLASS, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
            PROCESS_SET_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE,
        },
    },
    UI::WindowsAndMessaging::{
        IsHungAppWindow, IsIconic, SendMessageTimeoutW, SetWindowPos, ShowWindow, SMTO_ABORTIFHUNG,
        SMTO_NORMAL, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSENDCHANGING,
        SWP_NOZORDER, SW_RESTORE, WM_SETTEXT,
    },
};

const MONITORINFOF_PRIMARY: u32 = 1;

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Sets the title of the client's windows; `false` while it has none. A frozen window is
/// skipped instead of freezing Nocta with it.
pub fn set_title(pid: u32, title: &str) -> bool {
    let windows = process_windows(pid);
    let text = wide(title);
    for window in &windows {
        let mut result = 0usize;
        // SAFETY: `text` is NUL-terminated and outlives the call, which waits at most a second.
        unsafe {
            SendMessageTimeoutW(
                *window,
                WM_SETTEXT,
                0,
                text.as_ptr() as LPARAM,
                SMTO_ABORTIFHUNG | SMTO_NORMAL,
                1000,
                &mut result,
            )
        };
    }
    !windows.is_empty()
}

/// Work areas (without the taskbar) of the connected monitors.
pub fn monitors() -> Vec<Monitor> {
    unsafe extern "system" fn visit(
        monitor: HMONITOR,
        _dc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        // SAFETY: `data` is the Vec passed to EnumDisplayMonitors below.
        let found = unsafe { &mut *(data as *mut Vec<Monitor>) };
        let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        // SAFETY: `info` is valid with its size set.
        if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
            let work = info.rcWork;
            found.push(Monitor {
                x: work.left,
                y: work.top,
                width: (work.right - work.left).max(0) as u32,
                height: (work.bottom - work.top).max(0) as u32,
                primary: info.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        1
    }
    let mut found: Vec<Monitor> = Vec::new();
    // SAFETY: the callback only touches `found`, which outlives the call.
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(visit),
            &mut found as *mut Vec<Monitor> as LPARAM,
        )
    };
    found
}

/// Moves and sizes the client's main window; restores it first when minimized.
pub fn place_window(pid: u32, x: i32, y: i32, width: u32, height: u32) -> bool {
    let Some(window) = process_windows(pid).into_iter().next() else {
        return false;
    };
    // SAFETY: stale handles make these calls fail harmlessly.
    unsafe {
        if IsIconic(window) != 0 {
            ShowWindow(window, SW_RESTORE);
        }
        SetWindowPos(
            window,
            std::ptr::null_mut(),
            x,
            y,
            width as i32,
            height as i32,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS | SWP_NOSENDCHANGING,
        ) != 0
    }
}

/// Changes only the size of the client's main window. Like [`place_window`], any size works,
/// down to a single pixel: Roblox keeps its window at least 800×600 by answering the
/// size-limits question Windows asks before each change, and `SWP_NOSENDCHANGING` skips that
/// question. Nothing is injected into Roblox; dragging the border by hand still stops at its
/// minimum.
pub fn resize_window(pid: u32, width: u32, height: u32) -> bool {
    let Some(window) = process_windows(pid).into_iter().next() else {
        return false;
    };
    // SAFETY: stale handles make the call fail harmlessly.
    unsafe {
        SetWindowPos(
            window,
            std::ptr::null_mut(),
            0,
            0,
            width as i32,
            height as i32,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE | SWP_ASYNCWINDOWPOS | SWP_NOSENDCHANGING,
        ) != 0
    }
}

/// Whether the client's window stopped handling messages (Windows' "Not responding").
pub fn hung(pid: u32) -> Option<bool> {
    let window = process_windows(pid).into_iter().next()?;
    // SAFETY: plain query on a window handle.
    Some(unsafe { IsHungAppWindow(window) } != 0)
}

fn open(pid: u32, access: u32) -> CoreResult<HANDLE> {
    // SAFETY: OpenProcess returns null on failure; callers close the handle.
    let handle = unsafe { OpenProcess(access, 0, pid) };
    if handle.is_null() {
        return Err(CoreError::Validation(
            "The client's process could not be opened.".into(),
        ));
    }
    Ok(handle)
}

/// Applies priority, efficiency mode and the CPU limit to a running client.
pub fn apply_profile(pid: u32, profile: &ClientProfile) -> CoreResult<()> {
    let handle = open(
        pid,
        PROCESS_SET_INFORMATION | PROCESS_SET_QUOTA | PROCESS_TERMINATE,
    )?;
    let result = (|| {
        let class = match profile.priority {
            ProcessPriority::Normal => NORMAL_PRIORITY_CLASS,
            ProcessPriority::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
            ProcessPriority::Idle => IDLE_PRIORITY_CLASS,
        };
        // SAFETY: `handle` is open with PROCESS_SET_INFORMATION.
        if unsafe { SetPriorityClass(handle, class) } == 0 {
            return Err(CoreError::Validation(
                "The client's priority could not be set.".into(),
            ));
        }
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
            StateMask: if profile.eco_mode {
                PROCESS_POWER_THROTTLING_EXECUTION_SPEED
            } else {
                0
            },
        };
        // SAFETY: `state` is a valid PROCESS_POWER_THROTTLING_STATE for this information class.
        if unsafe {
            SetProcessInformation(
                handle,
                ProcessPowerThrottling,
                &state as *const _ as *const _,
                std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
            )
        } == 0
        {
            return Err(CoreError::Validation(
                "Efficiency mode could not be set.".into(),
            ));
        }
        // Pages of a low-priority process are the first Windows takes back when memory runs
        // short, so main accounts keep theirs.
        let memory = MEMORY_PRIORITY_INFORMATION {
            MemoryPriority: if profile.low_memory_priority {
                MEMORY_PRIORITY_LOW
            } else {
                MEMORY_PRIORITY_NORMAL
            },
        };
        // SAFETY: `memory` is a valid MEMORY_PRIORITY_INFORMATION for this information class.
        if unsafe {
            SetProcessInformation(
                handle,
                ProcessMemoryPriority,
                &memory as *const _ as *const _,
                std::mem::size_of::<MEMORY_PRIORITY_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(CoreError::Validation(
                "The client's memory priority could not be set.".into(),
            ));
        }
        if profile.cpu_limit_percent > 0 {
            limit_cpu(handle, profile.cpu_limit_percent)?;
        }
        // After the job: joining one can reset the working-set limits.
        limit_ram(handle, profile)?;
        Ok(())
    })();
    // SAFETY: opened above.
    unsafe { CloseHandle(handle) };
    result
}

/// Roblox keeps at least 425 MB of RAM for itself (its working-set minimum), so Windows never
/// takes it below that even when memory runs short. With a cap, the client holds at most that
/// much; with low memory priority, the minimum is dropped so Windows can balance clients itself.
/// Without either, Roblox's own limits stay.
fn limit_ram(process: HANDLE, profile: &ClientProfile) -> CoreResult<()> {
    const FLOOR: usize = 16 * 1024 * 1024;
    let (maximum, flags) = if profile.ram_cap_mb > 0 {
        (
            profile.ram_cap_mb as usize * 1024 * 1024,
            QUOTA_LIMITS_HARDWS_MIN_DISABLE | QUOTA_LIMITS_HARDWS_MAX_ENABLE,
        )
    } else if profile.low_memory_priority {
        // A soft maximum only guides Windows; the client may grow past it.
        (
            512 * 1024 * 1024,
            QUOTA_LIMITS_HARDWS_MIN_DISABLE | QUOTA_LIMITS_HARDWS_MAX_DISABLE,
        )
    } else {
        return Ok(());
    };
    // SAFETY: `process` is open with PROCESS_SET_QUOTA.
    if unsafe { SetProcessWorkingSetSizeEx(process, FLOOR, maximum, flags) } == 0 {
        return Err(CoreError::Validation(
            "The client's RAM limit could not be set.".into(),
        ));
    }
    Ok(())
}

/// Puts the process in its own job with a hard CPU cap. The job lives as long as the process.
fn limit_cpu(process: HANDLE, percent: u32) -> CoreResult<()> {
    // SAFETY: an unnamed job; the handle is closed below, which leaves the job (and its limit)
    // in place for the process, since it is not kill-on-close.
    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err(CoreError::Validation(
                "The CPU limit could not be set.".into(),
            ));
        }
        let mut info: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION = std::mem::zeroed();
        info.ControlFlags =
            JOB_OBJECT_CPU_RATE_CONTROL_ENABLE | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP;
        // In 1/100 of a percent of all cores.
        info.Anonymous.CpuRate = percent.clamp(1, 100) * 100;
        let ok = SetInformationJobObject(
            job,
            JobObjectCpuRateControlInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_CPU_RATE_CONTROL_INFORMATION>() as u32,
        ) != 0
            && AssignProcessToJobObject(job, process) != 0;
        CloseHandle(job);
        if ok {
            Ok(())
        } else {
            Err(CoreError::Validation(
                "The CPU limit could not be set.".into(),
            ))
        }
    }
}

static AWAKE: AtomicBool = AtomicBool::new(false);
static AWAKE_THREAD: std::sync::Once = std::sync::Once::new();

/// Keeps the computer from sleeping while `on` (the screen may still turn off). Windows ties
/// this to a thread, so one long-lived thread holds it.
pub fn keep_awake(on: bool) {
    AWAKE.store(on, Ordering::SeqCst);
    AWAKE_THREAD.call_once(|| {
        std::thread::spawn(|| {
            let mut current = None;
            loop {
                let wanted = AWAKE.load(Ordering::SeqCst);
                if current != Some(wanted) {
                    let flags = if wanted {
                        ES_CONTINUOUS | ES_SYSTEM_REQUIRED
                    } else {
                        ES_CONTINUOUS
                    };
                    // SAFETY: plain call; affects this thread's execution state only.
                    unsafe { SetThreadExecutionState(flags) };
                    current = Some(wanted);
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Threading::{
        GetPriorityClass, GetProcessInformation, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    #[test]
    fn priority_efficiency_mode_and_cpu_limit_apply_to_a_real_process() {
        let mut child = std::process::Command::new("cmd")
            .args(["/C", "ping -n 6 127.0.0.1 >NUL"])
            .spawn()
            .unwrap();
        let pid = child.id();
        apply_profile(
            pid,
            &ClientProfile {
                priority: ProcessPriority::Idle,
                eco_mode: true,
                cpu_limit_percent: 10,
                window_width: 0,
                window_height: 0,
                low_memory_priority: true,
                trim_every_secs: 0,
                ram_cap_mb: 300,
            },
        )
        .unwrap();
        let handle = open(
            pid,
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SET_INFORMATION,
        )
        .unwrap();
        // SAFETY: valid handle and output buffer.
        unsafe {
            assert_eq!(GetPriorityClass(handle), IDLE_PRIORITY_CLASS);
            let mut state: PROCESS_POWER_THROTTLING_STATE = std::mem::zeroed();
            state.Version = PROCESS_POWER_THROTTLING_CURRENT_VERSION;
            assert_ne!(
                GetProcessInformation(
                    handle,
                    ProcessPowerThrottling,
                    &mut state as *mut _ as *mut _,
                    std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
                ),
                0
            );
            assert_eq!(
                state.StateMask & PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
                PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
                "efficiency mode is on"
            );
            let mut memory = MEMORY_PRIORITY_INFORMATION { MemoryPriority: 0 };
            assert_ne!(
                GetProcessInformation(
                    handle,
                    ProcessMemoryPriority,
                    &mut memory as *mut _ as *mut _,
                    std::mem::size_of::<MEMORY_PRIORITY_INFORMATION>() as u32,
                ),
                0
            );
            assert_eq!(
                memory.MemoryPriority, MEMORY_PRIORITY_LOW,
                "low memory priority"
            );
            let (mut minimum, mut maximum, mut flags) = (0usize, 0usize, 0u32);
            assert_ne!(
                windows_sys::Win32::System::Memory::GetProcessWorkingSetSizeEx(
                    handle,
                    &mut minimum,
                    &mut maximum,
                    &mut flags,
                ),
                0
            );
            assert_eq!(maximum, 300 * 1024 * 1024, "RAM capped at 300 MB");
            assert_ne!(
                flags & QUOTA_LIMITS_HARDWS_MAX_ENABLE,
                0,
                "a hard cap, not a hint"
            );
            CloseHandle(handle);
        }
        let _ = child.kill();
        let _ = child.wait();
        assert!(!monitors().is_empty(), "at least one monitor");
        assert_eq!(
            monitors().iter().filter(|monitor| monitor.primary).count(),
            1
        );
    }
}

#[cfg(test)]
mod real_window {
    use super::*;
    use windows_sys::Win32::{
        Foundation::{HWND, LRESULT, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW,
            Power::{CallNtPowerInformation, SystemExecutionState},
        },
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetWindowRect,
            GetWindowTextW, PeekMessageW, RegisterClassW, TranslateMessage, MINMAXINFO, MSG,
            PM_REMOVE, SW_SHOWNOACTIVATE, WM_GETMINMAXINFO, WNDCLASSW, WS_EX_NOACTIVATE,
            WS_OVERLAPPEDWINDOW,
        },
    };

    unsafe extern "system" fn procedure(
        window: HWND,
        message: u32,
        w: WPARAM,
        l: LPARAM,
    ) -> LRESULT {
        // Like Roblox: the window answers that it may not be smaller than 800×600.
        if message == WM_GETMINMAXINFO {
            let info = l as *mut MINMAXINFO;
            unsafe {
                (*info).ptMinTrackSize.x = 800;
                (*info).ptMinTrackSize.y = 600;
            }
            return 0;
        }
        unsafe { DefWindowProcW(window, message, w, l) }
    }

    fn pump() {
        // SAFETY: standard message loop for windows owned by this thread.
        unsafe {
            let mut message: MSG = std::mem::zeroed();
            while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    /// Names, places and checks a real window of this process, without taking focus; then
    /// stops answering messages to check "Not responding" detection.
    #[test]
    #[ignore = "opens a window on the screen for a few seconds"]
    fn a_real_window_is_named_placed_and_seen_hanging() {
        let class = wide("NoctaWindowControlTest");
        let window = unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let mut definition: WNDCLASSW = std::mem::zeroed();
            definition.lpfnWndProc = Some(procedure);
            definition.hInstance = instance;
            definition.lpszClassName = class.as_ptr();
            RegisterClassW(&definition);
            let window = CreateWindowExW(
                WS_EX_NOACTIVATE,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                50,
                50,
                400,
                300,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            ShowWindow(window, SW_SHOWNOACTIVATE);
            window
        };
        pump();
        let pid = std::process::id();
        assert!(set_title(pid, "Nocta · Alice_1"));
        let mut text = [0u16; 64];
        let length = unsafe { GetWindowTextW(window, text.as_mut_ptr(), 64) } as usize;
        assert_eq!(String::from_utf16_lossy(&text[..length]), "Nocta · Alice_1");

        assert!(place_window(pid, 120, 80, 640, 360));
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        unsafe { GetWindowRect(window, &mut rect) };
        assert_eq!(
            (
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top
            ),
            (120, 80, 640, 360)
        );
        assert!(resize_window(pid, 800, 450));
        unsafe { GetWindowRect(window, &mut rect) };
        assert_eq!(
            (rect.left, rect.right - rect.left, rect.bottom - rect.top),
            (120, 800, 450)
        );
        // A plain resize stops at the window's own minimum, as with Roblox…
        unsafe {
            SetWindowPos(
                window,
                std::ptr::null_mut(),
                0,
                0,
                120,
                90,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE,
            );
            GetWindowRect(window, &mut rect);
        }
        assert_eq!((rect.right - rect.left, rect.bottom - rect.top), (800, 600));
        // …Nocta's goes below it, down to a single pixel.
        for (width, height) in [(120, 90), (1, 1)] {
            assert!(resize_window(pid, width, height));
            unsafe { GetWindowRect(window, &mut rect) };
            assert_eq!(
                (rect.right - rect.left, rect.bottom - rect.top),
                (width as i32, height as i32)
            );
        }
        assert!(resize_window(pid, 800, 450));

        pump();
        assert_eq!(hung(pid), Some(false), "a window that answers is not hung");
        // Stop answering messages: Windows calls a window hung after about five seconds.
        std::thread::sleep(std::time::Duration::from_secs(6));
        let seen_hung = std::thread::spawn(move || hung(pid)).join().unwrap();
        assert_eq!(seen_hung, Some(true), "an unresponsive window is hung");
        // Naming a frozen window gives up quickly instead of freezing Nocta.
        let started = std::time::Instant::now();
        std::thread::spawn(move || set_title(pid, "Nocta · frozen"))
            .join()
            .unwrap();
        assert!(
            started.elapsed() < std::time::Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
        pump();
        unsafe { DestroyWindow(window) };

        keep_awake(true);
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let mut state: u32 = 0;
        unsafe {
            CallNtPowerInformation(
                SystemExecutionState,
                std::ptr::null(),
                0,
                &mut state as *mut u32 as *mut _,
                4,
            )
        };
        println!("system execution state while kept awake: {state:#x}");
        assert_ne!(state & ES_SYSTEM_REQUIRED, 0, "the system is kept awake");
        keep_awake(false);
        std::thread::sleep(std::time::Duration::from_millis(1200));
    }
}

//! Windows of running game clients, their memory, and keeping them from idling out.
use crate::{
    launch::instances::AfkInput,
    models::{AntiAfkAction, ClickPoint},
};
use windows_sys::core::BOOL;
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT},
    Graphics::Gdi::ClientToScreen,
    System::{
        ProcessStatus::{K32EmptyWorkingSet, K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
        SystemInformation::GetTickCount,
        Threading::{
            AttachThreadInput, GetCurrentThreadId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            PROCESS_SET_QUOTA,
        },
    },
    UI::{
        Input::KeyboardAndMouse::{
            GetLastInputInfo, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
            KEYEVENTF_KEYUP, LASTINPUTINFO, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT,
            VIRTUAL_KEY, VK_MENU, VK_RIGHT, VK_SPACE,
        },
        WindowsAndMessaging::{
            BringWindowToTop, EnumWindows, GetClientRect, GetCursorPos, GetForegroundWindow,
            GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetCursorPos, SetForegroundWindow,
            ShowWindow, SW_MINIMIZE, SW_RESTORE,
        },
    },
};

/// Visible top-level windows of `pid`.
pub fn process_windows(pid: u32) -> Vec<HWND> {
    struct Search {
        pid: u32,
        windows: Vec<HWND>,
    }
    unsafe extern "system" fn visit(window: HWND, data: LPARAM) -> BOOL {
        // SAFETY: `data` is the `Search` passed to EnumWindows below, alive for the call.
        let search = unsafe { &mut *(data as *mut Search) };
        let mut owner = 0u32;
        // SAFETY: `window` comes from EnumWindows; `owner` is a valid out pointer.
        unsafe { GetWindowThreadProcessId(window, &mut owner) };
        // SAFETY: `window` comes from EnumWindows.
        if owner == search.pid && unsafe { IsWindowVisible(window) } != 0 {
            search.windows.push(window);
        }
        1
    }
    let mut search = Search {
        pid,
        windows: Vec::new(),
    };
    // SAFETY: the callback only touches `search`, which outlives the call.
    unsafe { EnumWindows(Some(visit), &mut search as *mut Search as LPARAM) };
    search.windows
}

pub fn minimize(pid: u32) -> bool {
    let windows = process_windows(pid);
    for window in &windows {
        // SAFETY: ShowWindow tolerates windows that closed meanwhile.
        unsafe { ShowWindow(*window, SW_MINIMIZE) };
    }
    !windows.is_empty()
}

fn key(code: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: code,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn mouse(flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Screen position of `point` (percent of the client area) in `window`, kept inside it.
fn screen_point(window: HWND, point: ClickPoint) -> Option<POINT> {
    let mut area = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: `area` is a valid out pointer; a stale window makes the call fail.
    if unsafe { GetClientRect(window, &mut area) } == 0 || area.right <= 0 || area.bottom <= 0 {
        return None;
    }
    let along = |size: i32, percent: u32| (size as i64 * percent.min(100) as i64 / 100) as i32;
    let mut at = POINT {
        x: along(area.right, point.x_percent).clamp(0, area.right - 1),
        y: along(area.bottom, point.y_percent).clamp(0, area.bottom - 1),
    };
    // SAFETY: `at` is a valid in/out pointer.
    (unsafe { ClientToScreen(window, &mut at) } != 0).then_some(at)
}

fn send(inputs: &[INPUT]) {
    // SAFETY: the slice is valid for the call and INPUT has the documented size.
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
}

/// Gives `window` keyboard focus. Windows only lets the app the user is in hand out focus, so
/// this joins the foreground window's input queue for the switch.
fn focus(window: HWND) -> bool {
    // SAFETY: plain Win32 calls on window handles that may close at any time; each call
    // tolerates a stale handle. The input queues are detached again before returning.
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground == window {
            return true;
        }
        let ours = GetCurrentThreadId();
        let theirs = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
        let attached = theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, 1) != 0;
        // A key event of our own also unlocks SetForegroundWindow.
        send(&[key(VK_MENU, false), key(VK_MENU, true)]);
        BringWindowToTop(window);
        SetForegroundWindow(window);
        if attached {
            AttachThreadInput(ours, theirs, 0);
        }
        for _ in 0..20 {
            if GetForegroundWindow() == window {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }
}

/// Brings the client's window forward for a moment, presses the key or clicks, and gives the
/// window the user was in back (minimizing the client again if it was, and putting the mouse
/// back after a click). Roblox only counts input sent to its focused window, which is why this
/// cannot run fully in the background. Input is only sent once the client really has focus, so
/// it never lands in another window.
pub fn nudge(pid: u32, input: AfkInput) -> bool {
    let Some(window) = process_windows(pid).into_iter().next() else {
        return false;
    };
    let input = match input {
        AfkInput::Key(AntiAfkAction::Click) => AfkInput::Click(ClickPoint::default()),
        other => other,
    };
    // SAFETY: see `focus`.
    unsafe {
        let previous = GetForegroundWindow();
        let minimized = IsIconic(window) != 0;
        if minimized {
            ShowWindow(window, SW_RESTORE);
        }
        let focused = focus(window);
        if focused {
            std::thread::sleep(std::time::Duration::from_millis(50));
            match input {
                AfkInput::Key(action) => {
                    let code = if action == AntiAfkAction::Jump {
                        VK_SPACE
                    } else {
                        VK_RIGHT
                    };
                    send(&[key(code, false)]);
                    std::thread::sleep(std::time::Duration::from_millis(60));
                    send(&[key(code, true)]);
                }
                AfkInput::Click(point) => {
                    if let Some(at) = screen_point(window, point) {
                        let mut cursor = POINT { x: 0, y: 0 };
                        let had_cursor = GetCursorPos(&mut cursor) != 0;
                        SetCursorPos(at.x, at.y);
                        std::thread::sleep(std::time::Duration::from_millis(30));
                        send(&[mouse(MOUSEEVENTF_LEFTDOWN)]);
                        std::thread::sleep(std::time::Duration::from_millis(60));
                        send(&[mouse(MOUSEEVENTF_LEFTUP)]);
                        std::thread::sleep(std::time::Duration::from_millis(30));
                        if had_cursor {
                            SetCursorPos(cursor.x, cursor.y);
                        }
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
        if minimized {
            ShowWindow(window, SW_MINIMIZE);
        }
        if !previous.is_null() && previous != window {
            focus(previous);
        }
        focused
    }
}

/// Seconds since the user last touched the keyboard or mouse.
pub fn user_idle_secs() -> Option<u64> {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` is a valid LASTINPUTINFO with its size set.
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return None;
    }
    // SAFETY: no arguments.
    let now = unsafe { GetTickCount() };
    Some(now.wrapping_sub(info.dwTime) as u64 / 1000)
}

fn with_process<T>(
    pid: u32,
    access: u32,
    work: impl FnOnce(windows_sys::Win32::Foundation::HANDLE) -> T,
) -> Option<T> {
    // SAFETY: OpenProcess returns null on failure; the handle is closed below.
    let handle = unsafe { OpenProcess(access, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let result = work(handle);
    // SAFETY: `handle` was opened above.
    unsafe { CloseHandle(handle) };
    Some(result)
}

/// Asks Windows to page out the client's unused memory. Roblox takes back what it needs.
pub fn trim_memory(pid: u32) -> bool {
    with_process(
        pid,
        PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION,
        |handle| {
            // SAFETY: `handle` is open with PROCESS_SET_QUOTA.
            unsafe { K32EmptyWorkingSet(handle) != 0 }
        },
    )
    .unwrap_or(false)
}

/// Memory the client uses right now (working set), in megabytes.
pub fn memory_mb(pid: u32) -> Option<u64> {
    with_process(pid, PROCESS_QUERY_LIMITED_INFORMATION, |handle| {
        let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        // SAFETY: `counters` is valid with its size set.
        (unsafe { K32GetProcessMemoryInfo(handle, &mut counters, counters.cb) } != 0)
            .then_some(counters.WorkingSetSize as u64 / (1024 * 1024))
    })
    .flatten()
}

/// RAM and commit (RAM plus page file) of the whole computer.
pub fn system_memory() -> Option<crate::models::SystemMemory> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: `status` is valid with its length set.
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return None;
    }
    let mb = |bytes: u64| (bytes / (1024 * 1024)).min(u32::MAX as u64) as u32;
    Some(crate::models::SystemMemory {
        ram_total_mb: mb(status.ullTotalPhys),
        ram_free_mb: mb(status.ullAvailPhys),
        commit_limit_mb: mb(status.ullTotalPageFile),
        commit_used_mb: mb(status
            .ullTotalPageFile
            .saturating_sub(status.ullAvailPageFile)),
    })
}

/// Memory the client has allocated for itself (its commit), in megabytes. Unlike the working
/// set this does not shrink when RAM is capped or trimmed, so it is what grows when a game leaks.
pub fn committed_mb(pid: u32) -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS_EX;
    with_process(pid, PROCESS_QUERY_LIMITED_INFORMATION, |handle| {
        let mut counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        // SAFETY: the extended counters start with the basic ones; the size says which.
        (unsafe { K32GetProcessMemoryInfo(handle, &mut counters as *mut _ as *mut _, counters.cb) }
            != 0)
            .then_some(counters.PrivateUsage as u64 / (1024 * 1024))
    })
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_trims_the_memory_of_a_real_process() {
        let mut child = std::process::Command::new("cmd")
            .args(["/C", "ping -n 5 127.0.0.1 >NUL"])
            .spawn()
            .unwrap();
        let pid = child.id();
        let before = memory_mb(pid);
        assert!(before.is_some(), "the working set can be read");
        assert!(trim_memory(pid), "the working set can be trimmed");
        assert!(memory_mb(std::process::id()).unwrap() > 0);
        assert!(committed_mb(std::process::id()).unwrap() > 0);
        let system = system_memory().unwrap();
        assert!(system.ram_total_mb > 0 && system.ram_free_mb <= system.ram_total_mb);
        assert!(system.commit_limit_mb >= system.ram_total_mb / 2);
        assert!(system.commit_used_mb <= system.commit_limit_mb);
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(memory_mb(u32::MAX - 1), None, "no such process");
        assert!(user_idle_secs().is_some());
    }
}

#[cfg(test)]
mod real_window {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use windows_sys::Win32::{
        Foundation::{LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, PeekMessageW,
            RegisterClassW, TranslateMessage, MSG, PM_REMOVE, WM_KEYDOWN, WM_LBUTTONDOWN,
            WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
        },
    };

    static KEY: AtomicU32 = AtomicU32::new(0);
    static CLICK: AtomicU32 = AtomicU32::new(u32::MAX);

    unsafe extern "system" fn procedure(
        window: HWND,
        message: u32,
        w: WPARAM,
        l: LPARAM,
    ) -> LRESULT {
        if message == WM_KEYDOWN {
            KEY.store(w as u32, Ordering::SeqCst);
        }
        if message == WM_LBUTTONDOWN {
            // Client coordinates: x in the low word, y in the high word.
            CLICK.store(l as u32, Ordering::SeqCst);
        }
        unsafe { DefWindowProcW(window, message, w, l) }
    }

    /// Opens a real window, lets anti-AFK press its key in it, and checks the key arrived and
    /// the previously focused window got focus back. It takes focus for a moment.
    #[test]
    #[ignore = "opens a window and takes keyboard focus for a moment"]
    fn the_key_reaches_a_real_window_and_focus_goes_back() {
        let class: Vec<u16> = "NoctaAntiAfkTest\0".encode_utf16().collect();
        let previous = unsafe { GetForegroundWindow() };
        let window = unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let mut definition: WNDCLASSW = std::mem::zeroed();
            definition.lpfnWndProc = Some(procedure);
            definition.hInstance = instance;
            definition.lpszClassName = class.as_ptr();
            RegisterClassW(&definition);
            CreateWindowExW(
                0,
                class.as_ptr(),
                class.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                100,
                100,
                300,
                200,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            )
        };
        assert!(!window.is_null());
        // A new window takes focus; give it back so anti-AFK starts from the user's window.
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(focus(previous), "focus went back to the original window");
        let pid = std::process::id();
        let pump = || {
            let started = std::time::Instant::now();
            while started.elapsed() < std::time::Duration::from_secs(3) {
                unsafe {
                    let mut message: MSG = std::mem::zeroed();
                    while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        };
        let presser = std::thread::spawn(move || nudge(pid, AfkInput::Key(AntiAfkAction::Turn)));
        pump();
        assert!(presser.join().unwrap(), "the window was found");
        assert_eq!(
            KEY.load(Ordering::SeqCst),
            VK_RIGHT as u32,
            "Right arrow arrived"
        );
        let now = unsafe { GetForegroundWindow() };
        println!("focus before {previous:?}, after {now:?}, test window {window:?}");
        assert_ne!(now, window, "focus went back to the previous window");

        // A click at a point of the window, with the mouse put back afterwards.
        let mut cursor = POINT { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut cursor) };
        let point = ClickPoint {
            x_percent: 25,
            y_percent: 75,
        };
        let clicker = std::thread::spawn(move || nudge(pid, AfkInput::Click(point)));
        pump();
        assert!(clicker.join().unwrap());
        let mut area = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe { GetClientRect(window, &mut area) };
        let packed = CLICK.load(Ordering::SeqCst);
        let (x, y) = ((packed & 0xffff) as i32, (packed >> 16) as i32);
        println!(
            "clicked at {x},{y} in a {}x{} client area",
            area.right, area.bottom
        );
        assert_eq!(
            (x, y),
            (area.right / 4, area.bottom * 3 / 4),
            "the click landed on the point"
        );
        // The mouse is put back where it was; the user may have moved it since, but it does not
        // stay on the clicked point.
        let mut clicked = POINT { x, y };
        unsafe { ClientToScreen(window, &mut clicked) };
        let mut after = POINT { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut after) };
        println!(
            "mouse before {},{}, after {},{}",
            cursor.x, cursor.y, after.x, after.y
        );
        assert_ne!(
            (after.x, after.y),
            (clicked.x, clicked.y),
            "the mouse went back"
        );
        assert_ne!(
            unsafe { GetForegroundWindow() },
            window,
            "focus went back again"
        );
        unsafe { DestroyWindow(window) };
    }
}

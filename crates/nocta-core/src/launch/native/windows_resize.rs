//! Dragging the border of a Roblox window to any size, down to a single pixel.
//!
//! Roblox keeps its window at least 800×600 by answering the size-limits question Windows asks
//! while a border is dragged. Nothing is injected into Roblox to change that. Instead Nocta watches
//! for a border drag starting on a Roblox window (a system event any program can receive), ends
//! Windows' own drag, and sizes the window after the mouse itself until the button is released,
//! with `SWP_NOSENDCHANGING` so the question is never asked.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Once, OnceLock,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT},
    System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::{
        Accessibility::{SetWinEventHook, HWINEVENTHOOK},
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON},
        WindowsAndMessaging::{
            DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowRect,
            GetWindowThreadProcessId, PostMessageW, SendMessageTimeoutW, SetWindowPos,
            TranslateMessage, EVENT_SYSTEM_MOVESIZESTART, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT,
            HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, MSG, OBJID_WINDOW, SMTO_ABORTIFHUNG,
            SM_SWAPBUTTON, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOSENDCHANGING, SWP_NOZORDER,
            WINEVENT_OUTOFCONTEXT, WM_CANCELMODE, WM_NCHITTEST,
        },
    },
};

const ROBLOX: &str = "robloxplayerbeta.exe";

static ENABLED: AtomicBool = AtomicBool::new(false);
static WATCHER: Once = Once::new();
/// Which windows are taken over; Roblox's in the app, a test's own window in tests.
static TARGET: OnceLock<fn(HWND) -> bool> = OnceLock::new();

/// Turns free border dragging of Roblox windows on or off. The watcher starts on first use.
pub fn set_enabled(on: bool) {
    set_enabled_for(on, is_roblox);
}

fn set_enabled_for(on: bool, target: fn(HWND) -> bool) {
    ENABLED.store(on, Ordering::SeqCst);
    if on {
        let _ = TARGET.set(target);
        WATCHER.call_once(|| {
            std::thread::Builder::new()
                .name("nocta-window-resize".into())
                .spawn(watch)
                .expect("window resize thread");
        });
    }
}

/// The edges a drag moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edges {
    pub left: bool,
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
}

/// The edges of a non-client hit-test result; `None` for anything but a sizing border.
pub fn edges(hit: u32) -> Option<Edges> {
    let (left, top, right, bottom) = match hit {
        HTLEFT => (true, false, false, false),
        HTRIGHT => (false, false, true, false),
        HTTOP => (false, true, false, false),
        HTBOTTOM => (false, false, false, true),
        HTTOPLEFT => (true, true, false, false),
        HTTOPRIGHT => (false, true, true, false),
        HTBOTTOMLEFT => (true, false, false, true),
        HTBOTTOMRIGHT => (false, false, true, true),
        _ => return None,
    };
    Some(Edges {
        left,
        top,
        right,
        bottom,
    })
}

/// The window's rectangle after the mouse moved by `dx`, `dy` from where the drag started. Moved
/// edges never cross the opposite one: the window stays at least 1×1.
pub fn dragged(start: RECT, edges: Edges, dx: i32, dy: i32) -> RECT {
    let mut rect = start;
    if edges.left {
        rect.left = (start.left + dx).min(start.right - 1);
    }
    if edges.right {
        rect.right = (start.right + dx).max(start.left + 1);
    }
    if edges.top {
        rect.top = (start.top + dy).min(start.bottom - 1);
    }
    if edges.bottom {
        rect.bottom = (start.bottom + dy).max(start.top + 1);
    }
    rect
}

fn is_roblox(window: HWND) -> bool {
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid out pointer; a stale handle leaves it 0.
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    if pid == 0 {
        return false;
    }
    // SAFETY: the handle is closed below; a failed open returns null.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return false;
    }
    let mut buffer = [0u16; 1024];
    let mut size = buffer.len() as u32;
    // SAFETY: `buffer` holds `size` characters.
    let ok = unsafe {
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut size)
    };
    // SAFETY: opened above.
    unsafe { CloseHandle(process) };
    ok != 0
        && String::from_utf16_lossy(&buffer[..size as usize])
            .to_ascii_lowercase()
            .ends_with(ROBLOX)
}

fn watch() {
    // SAFETY: the hook calls back on this thread, which pumps messages below for as long as the
    // app runs.
    unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_MOVESIZESTART,
            EVENT_SYSTEM_MOVESIZESTART,
            std::ptr::null_mut(),
            Some(on_drag_start),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        );
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

unsafe extern "system" fn on_drag_start(
    _hook: HWINEVENTHOOK,
    _event: u32,
    window: HWND,
    object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    if object != OBJID_WINDOW || !ENABLED.load(Ordering::SeqCst) {
        return;
    }
    if !TARGET.get().is_some_and(|target| target(window)) {
        return;
    }
    let mut cursor = POINT { x: 0, y: 0 };
    let mut start = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: valid out pointers; a stale window makes the calls fail.
    if unsafe { GetCursorPos(&mut cursor) } == 0
        || unsafe { GetWindowRect(window, &mut start) } == 0
    {
        return;
    }
    let mut hit = 0usize;
    let point = ((cursor.y as u16 as u32) << 16 | cursor.x as u16 as u32) as LPARAM;
    // SAFETY: plain message; waits at most 200 ms for a busy window.
    let answered = unsafe {
        SendMessageTimeoutW(
            window,
            WM_NCHITTEST,
            0,
            point,
            SMTO_ABORTIFHUNG,
            200,
            &mut hit,
        )
    };
    // Moving the window (its title bar) is left to Windows.
    let Some(edges) = (answered != 0).then(|| edges(hit as u32)).flatten() else {
        return;
    };
    // Ends Windows' own drag, which stops at Roblox's minimum.
    // SAFETY: plain message.
    unsafe { PostMessageW(window, WM_CANCELMODE, 0, 0) };
    follow(window, start, edges, cursor);
}

/// Sizes the window after the mouse until the button that started the drag is released.
fn follow(window: HWND, start: RECT, edges: Edges, origin: POINT) {
    // SAFETY: plain queries.
    let button = if unsafe { GetSystemMetrics(SM_SWAPBUTTON) } != 0 {
        VK_RBUTTON
    } else {
        VK_LBUTTON
    };
    let mut last = start;
    // SAFETY: plain query; the high bit is set while the button is down.
    while unsafe { GetAsyncKeyState(button as i32) } as u16 & 0x8000 != 0 {
        let mut cursor = origin;
        // SAFETY: valid out pointer.
        unsafe { GetCursorPos(&mut cursor) };
        let rect = dragged(start, edges, cursor.x - origin.x, cursor.y - origin.y);
        if (rect.left, rect.top, rect.right, rect.bottom)
            != (last.left, last.top, last.right, last.bottom)
        {
            // SAFETY: a stale handle makes the call fail harmlessly.
            let placed = unsafe {
                SetWindowPos(
                    window,
                    std::ptr::null_mut(),
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS | SWP_NOSENDCHANGING,
                )
            };
            if placed == 0 {
                return;
            }
            last = rect;
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: RECT = RECT {
        left: 100,
        top: 100,
        right: 900,
        bottom: 700,
    };

    #[test]
    fn edges_follow_the_mouse_down_to_one_pixel() {
        let bottom_right = edges(HTBOTTOMRIGHT).unwrap();
        let rect = dragged(START, bottom_right, -700, -500);
        assert_eq!((rect.right - rect.left, rect.bottom - rect.top), (100, 100));
        let rect = dragged(START, bottom_right, -5000, -5000);
        assert_eq!((rect.right - rect.left, rect.bottom - rect.top), (1, 1));
        assert_eq!((rect.left, rect.top), (100, 100), "the other corner stays");
        let top_left = edges(HTTOPLEFT).unwrap();
        let rect = dragged(START, top_left, 750, 590);
        assert_eq!(
            (rect.left, rect.top, rect.right, rect.bottom),
            (850, 690, 900, 700)
        );
        let rect = dragged(START, edges(HTLEFT).unwrap(), 9000, 9000);
        assert_eq!((rect.left, rect.top, rect.bottom), (899, 100, 700));
        assert!(edges(windows_sys::Win32::UI::WindowsAndMessaging::HTCAPTION).is_none());
    }
}

#[cfg(test)]
mod real_window {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use windows_sys::Win32::{
        Foundation::{LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Input::KeyboardAndMouse::{
                SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
                MOUSEINPUT,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, PeekMessageW, RegisterClassW,
                SetCursorPos, MINMAXINFO, PM_REMOVE, WM_GETMINMAXINFO, WNDCLASSW,
                WS_OVERLAPPEDWINDOW, WS_VISIBLE,
            },
        },
    };

    static OURS: AtomicU32 = AtomicU32::new(0);

    /// A window that keeps itself at least 800×600 the way Roblox does.
    unsafe extern "system" fn procedure(
        window: HWND,
        message: u32,
        w: WPARAM,
        l: LPARAM,
    ) -> LRESULT {
        if message == WM_GETMINMAXINFO {
            let info = unsafe { &mut *(l as *mut MINMAXINFO) };
            info.ptMinTrackSize = POINT { x: 800, y: 600 };
            return 0;
        }
        unsafe { DefWindowProcW(window, message, w, l) }
    }

    fn ours(window: HWND) -> bool {
        window as usize as u32 == OURS.load(Ordering::SeqCst)
    }

    fn mouse(flags: u32) {
        let input = INPUT {
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
        };
        // SAFETY: one well-formed input.
        unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
    }

    fn pump(window: HWND, millis: u64) {
        let started = std::time::Instant::now();
        while started.elapsed() < std::time::Duration::from_millis(millis) {
            unsafe {
                let mut message: MSG = std::mem::zeroed();
                while PeekMessageW(&mut message, window, 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn size(window: HWND) -> (i32, i32) {
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        unsafe { GetWindowRect(window, &mut rect) };
        (rect.right - rect.left, rect.bottom - rect.top)
    }

    /// Drags the bottom-right corner of a real window that keeps a minimum of 800×600, the way a
    /// user does, first without and then with free resizing. It moves the mouse for a moment.
    #[test]
    #[ignore = "opens a window and moves the mouse for a moment"]
    fn a_border_drag_goes_below_the_windows_own_minimum() {
        let class: Vec<u16> = "NoctaFreeResizeTest\0".encode_utf16().collect();
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
                900,
                700,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            )
        };
        assert!(!window.is_null());
        OURS.store(window as usize as u32, Ordering::SeqCst);
        pump(window, 300);
        let mut before = POINT { x: 0, y: 0 };
        unsafe { GetCursorPos(&mut before) };

        // The bottom-right corner, dragged up and left by `by` pixels.
        let drag = |by: i32| {
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            unsafe { GetWindowRect(window, &mut rect) };
            let (x, y) = (rect.right - 3, rect.bottom - 3);
            unsafe { SetCursorPos(x, y) };
            pump(window, 100);
            // The size loop runs inside the window's own message handling, so the drag is
            // played from another thread while this one pumps messages.
            let mover = std::thread::spawn(move || {
                mouse(MOUSEEVENTF_LEFTDOWN);
                std::thread::sleep(std::time::Duration::from_millis(150));
                for step in 1..=20 {
                    unsafe { SetCursorPos(x - by * step / 20, y - by * step / 20) };
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                std::thread::sleep(std::time::Duration::from_millis(150));
                mouse(MOUSEEVENTF_LEFTUP);
            });
            while !mover.is_finished() {
                pump(window, 20);
            }
            pump(window, 300);
            size(window)
        };

        let held = drag(500);
        println!("Windows' own drag: {held:?}");
        assert_eq!(held, (800, 600), "the window's minimum holds without Nocta");

        set_enabled_for(true, ours);
        pump(window, 200);
        let free = drag(700);
        println!("with free resizing: {free:?}");
        set_enabled(false);
        unsafe { SetCursorPos(before.x, before.y) };
        unsafe { DestroyWindow(window) };
        assert_eq!(free, (100, 1), "the drag went below the minimum");
    }
}

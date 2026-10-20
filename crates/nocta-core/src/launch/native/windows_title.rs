//! Keeping a window's title while its program keeps changing it: an account's browser shows the
//! account's name instead of the page title. Chromium ignores `--window-name` here and renames its
//! window on every page, so Nocta listens for those renames (a system event for that one process)
//! and puts the name back right away.
use std::{
    collections::HashMap,
    sync::{mpsc, Mutex, OnceLock},
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM},
    System::Threading::GetCurrentThreadId,
    UI::{
        Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK},
        WindowsAndMessaging::{
            DispatchMessageW, GetAncestor, GetMessageW, GetWindowTextW, GetWindowThreadProcessId,
            PeekMessageW, PostThreadMessageW, SendMessageTimeoutW, TranslateMessage, CHILDID_SELF,
            EVENT_OBJECT_NAMECHANGE, GA_ROOT, MSG, OBJID_WINDOW, PM_NOREMOVE, SMTO_ABORTIFHUNG,
            WINEVENT_OUTOFCONTEXT, WM_APP, WM_SETTEXT,
        },
    },
};

/// Titles kept per process.
static KEPT: Mutex<Option<HashMap<u32, String>>> = Mutex::new(None);
/// The thread that holds the listeners, by its thread ID.
static WATCHER: OnceLock<u32> = OnceLock::new();
/// Tells the watcher that the kept processes changed.
const SYNC: u32 = WM_APP + 1;

/// The process's windows keep `title` until this is dropped.
pub struct KeptTitle {
    pid: u32,
}

impl Drop for KeptTitle {
    fn drop(&mut self) {
        if let Some(kept) = KEPT.lock().unwrap().as_mut() {
            kept.remove(&self.pid);
        }
        sync();
    }
}

/// Gives the windows of `pid` the title `title` and keeps it there.
pub fn keep(pid: u32, title: &str) -> KeptTitle {
    KEPT.lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(pid, title.to_owned());
    super::windows_control::set_title(pid, title);
    sync();
    KeptTitle { pid }
}

fn sync() {
    let thread = *WATCHER.get_or_init(|| {
        let (ready, started) = mpsc::channel();
        std::thread::Builder::new()
            .name("nocta-window-titles".into())
            .spawn(move || watch(ready))
            .expect("window title thread");
        started.recv().expect("window title thread started")
    });
    // SAFETY: plain message to the watcher thread, which has a queue before its ID is known.
    unsafe { PostThreadMessageW(thread, SYNC, 0, 0) };
}

fn kept_title(pid: u32) -> Option<String> {
    KEPT.lock().unwrap().as_ref()?.get(&pid).cloned()
}

fn watch(ready: mpsc::Sender<u32>) {
    let mut hooks: HashMap<u32, HWINEVENTHOOK> = HashMap::new();
    // SAFETY: creates this thread's message queue so messages posted to it are kept.
    unsafe {
        let mut message: MSG = std::mem::zeroed();
        PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
        let _ = ready.send(GetCurrentThreadId());
    }
    let mut message: MSG = unsafe { std::mem::zeroed() };
    // SAFETY: a plain message loop; the hooks call back on this thread.
    while unsafe { GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) } > 0 {
        if message.hwnd.is_null() && message.message == SYNC {
            let wanted: Vec<u32> = KEPT
                .lock()
                .unwrap()
                .as_ref()
                .map(|kept| kept.keys().copied().collect())
                .unwrap_or_default();
            hooks.retain(|pid, hook| {
                let keep = wanted.contains(pid);
                if !keep {
                    // SAFETY: installed below on this thread.
                    unsafe { UnhookWinEvent(*hook) };
                }
                keep
            });
            for pid in wanted {
                hooks.entry(pid).or_insert_with(|| {
                    // SAFETY: listens to one process only; calls back on this thread.
                    unsafe {
                        SetWinEventHook(
                            EVENT_OBJECT_NAMECHANGE,
                            EVENT_OBJECT_NAMECHANGE,
                            std::ptr::null_mut(),
                            Some(on_rename),
                            pid,
                            0,
                            WINEVENT_OUTOFCONTEXT,
                        )
                    }
                });
            }
            continue;
        }
        // SAFETY: standard dispatch of this thread's messages.
        unsafe {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

unsafe extern "system" fn on_rename(
    _hook: HWINEVENTHOOK,
    _event: u32,
    window: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    // Only the title of a top-level window, not the names of things inside it.
    if object != OBJID_WINDOW || child != CHILDID_SELF as i32 || window.is_null() {
        return;
    }
    // SAFETY: plain query on the window the event is about.
    if unsafe { GetAncestor(window, GA_ROOT) } != window {
        return;
    }
    let mut pid = 0u32;
    // SAFETY: valid out pointer.
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    let Some(title) = kept_title(pid) else {
        return;
    };
    let mut buffer = [0u16; 512];
    // SAFETY: `buffer` holds its length in characters.
    let length = unsafe { GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32) };
    if String::from_utf16_lossy(&buffer[..length.max(0) as usize]) == title {
        return;
    }
    let text: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let mut result = 0usize;
    // SAFETY: `text` is NUL-terminated and outlives the call, which waits at most half a second.
    unsafe {
        SendMessageTimeoutW(
            window,
            WM_SETTEXT,
            0,
            text.as_ptr() as LPARAM,
            SMTO_ABORTIFHUNG,
            500,
            &mut result,
        )
    };
}

#[cfg(test)]
mod real_window {
    use super::*;
    use windows_sys::Win32::{
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, SetWindowTextW,
            PM_REMOVE, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
        },
    };

    fn pump(millis: u64) {
        let started = std::time::Instant::now();
        while started.elapsed() < std::time::Duration::from_millis(millis) {
            unsafe {
                let mut message: MSG = std::mem::zeroed();
                while PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn title(window: HWND) -> String {
        let mut buffer = [0u16; 256];
        let length = unsafe { GetWindowTextW(window, buffer.as_mut_ptr(), 256) };
        String::from_utf16_lossy(&buffer[..length as usize])
    }

    fn rename(window: HWND, text: &str) {
        let text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe { SetWindowTextW(window, text.as_ptr()) };
    }

    /// A window that renames itself the way a browser does on every page gets the kept title
    /// back, and keeps its own once the title is no longer kept.
    #[test]
    #[ignore = "opens a window for a moment"]
    fn a_renamed_window_gets_the_kept_title_back() {
        let class: Vec<u16> = "NoctaTitleTest\0".encode_utf16().collect();
        let window = unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let mut definition: WNDCLASSW = std::mem::zeroed();
            definition.lpfnWndProc = Some(DefWindowProcW);
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
                400,
                300,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            )
        };
        assert!(!window.is_null());
        pump(200);
        // Setting the title waits for this thread, so it pumps messages meanwhile.
        let keeper = std::thread::spawn(|| keep(std::process::id(), "Alice_1 (#7)"));
        while !keeper.is_finished() {
            pump(20);
        }
        let kept = keeper.join().unwrap();
        pump(300);
        assert_eq!(title(window), "Alice_1 (#7)");
        for page in ["Roblox – Home", "Charts – Roblox", "Settings"] {
            rename(window, page);
            pump(300);
            assert_eq!(title(window), "Alice_1 (#7)", "after the page {page}");
        }
        drop(kept);
        pump(300);
        rename(window, "Roblox – Home");
        pump(300);
        assert_eq!(title(window), "Roblox – Home", "no longer kept");
        unsafe { DestroyWindow(window) };
    }
}

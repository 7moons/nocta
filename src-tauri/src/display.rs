//! How the window is drawn, and the WebView settings that keep its memory low.
//!
//! By default the window is drawn without the graphics card: WebView2's GPU process alone holds
//! about 90 MB, more than the rest of Nocta, and Nocta's interface does not need it. GPU
//! acceleration can be turned on in Settings (a marker file in the data folder keeps that). With
//! it on, a window that stays blank after a start (some graphics drivers) restarts Nocta without
//! it again ("safe display").
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tauri::AppHandle;

/// How long the interface may take to appear before Nocta restarts without the GPU.
const UI_TIMEOUT: Duration = Duration::from_secs(25);
/// Present when the user turned GPU acceleration on.
const GPU_MARKER: &str = "gpu-acceleration";
/// The marker of earlier versions, where drawing without the GPU was the exception.
const OLD_MARKER: &str = "safe-display";
/// Command-line switch for one start without GPU acceleration.
pub const SAFE_DISPLAY_ARG: &str = "--safe-display";
/// WebView2's own defaults (as Tauri sets them), then fewer and lighter processes: the GPU and
/// network services run inside the browser process, one renderer, no background services.
/// Measured with 241 accounts: about 145 MB in 5 processes instead of about 280 MB in 7.
const LEAN_BROWSER_ARGS: &str = concat!(
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection",
    " --in-process-gpu",
    " --enable-features=NetworkServiceInProcess2,NetworkServiceInProcess",
    " --renderer-process-limit=1",
    " --disable-background-networking --disable-component-update --disable-sync",
);
/// Drawing without the graphics card.
const SOFTWARE_ARGS: &str = "--disable-gpu --disable-gpu-compositing";

static UI_READY: AtomicBool = AtomicBool::new(false);

/// The data folder, found the same way as [`crate::state::DesktopState::open`] but before Tauri
/// starts: `data` next to the program in portable mode, otherwise the app data folder.
pub(crate) fn data_directory() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    let directory = executable.parent()?;
    if cfg!(debug_assertions) {
        if let Some(path) = std::env::var_os("NOCTA_DATA_DIR") {
            return Some(PathBuf::from(path));
        }
    }
    if directory.join("portable.flag").exists() {
        return Some(directory.join("data"));
    }
    let base = if cfg!(target_os = "windows") {
        PathBuf::from(std::env::var_os("APPDATA")?)
    } else {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support")
    };
    Some(base.join("app.nocta.desktop"))
}

fn marker(name: &str) -> Option<PathBuf> {
    data_directory().map(|directory| directory.join(name))
}

/// Whether this start draws without the GPU: the default, unless the user turned acceleration
/// on (and did not ask for one start without it).
pub fn requested() -> bool {
    if let Some(old) = marker(OLD_MARKER) {
        // Without the GPU is the default now; the old marker means nothing more.
        let _ = std::fs::remove_file(old);
    }
    std::env::args().any(|arg| arg == SAFE_DISPLAY_ARG)
        || !marker(GPU_MARKER).is_some_and(|path| path.exists())
}

/// Sets up the WebView. Must run before the window is created. Arguments set in the
/// environment by the user win.
pub fn apply(safe: bool) {
    if std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_none() {
        let args = if safe {
            format!("{LEAN_BROWSER_ARGS} {SOFTWARE_ARGS}")
        } else {
            LEAN_BROWSER_ARGS.to_owned()
        };
        std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", args);
    }
}

/// Remembers whether later starts draw without the GPU (`safe`) or with it.
pub fn remember(safe: bool) -> std::io::Result<()> {
    let Some(path) = marker(GPU_MARKER) else {
        return Err(std::io::Error::other("The data folder was not found."));
    };
    if safe {
        match std::fs::remove_file(&path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, b"The window is drawn with GPU acceleration.\n")
    }
}

/// Called by the interface once it has drawn itself.
pub fn ready() {
    // A debug switch to try the fallback: the interface's signal is ignored.
    if cfg!(debug_assertions) && std::env::var("NOCTA_IGNORE_UI_READY").as_deref() == Ok("1") {
        return;
    }
    UI_READY.store(true, Ordering::SeqCst);
}

/// Waits for the interface; without it, restarts Nocta without the GPU. Already without it
/// there is nothing more to try, so it is only logged.
pub fn watch(app: AppHandle, safe: bool) {
    // Development builds load the interface from a dev server that may still be compiling, and a
    // restart would end `tauri dev`; there the watch only runs when a timeout is given.
    let timeout = if cfg!(debug_assertions) {
        match std::env::var("NOCTA_UI_TIMEOUT_SECS")
            .ok()
            .and_then(|secs| secs.parse().ok())
        {
            Some(secs) => Duration::from_secs(secs),
            None => return,
        }
    } else {
        UI_TIMEOUT
    };
    std::thread::spawn(move || {
        std::thread::sleep(timeout);
        if UI_READY.load(Ordering::SeqCst) {
            return;
        }
        if safe {
            tracing::error!("The interface did not appear, even without the GPU");
            return;
        }
        tracing::warn!(
            secs = timeout.as_secs(),
            "The interface did not appear; restarting without the GPU"
        );
        match remember(true) {
            Ok(()) => app.restart(),
            Err(error) => tracing::error!(%error, "Drawing without the GPU not saved"),
        }
    });
}

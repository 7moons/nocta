//! Keeping the window's memory low while nobody looks at it. A hidden or minimized window asks
//! WebView2 to use as little memory as it can (it drops caches and trims its processes); it
//! goes back to normal as soon as the window is shown.
use tauri::{Runtime, WebviewWindow};

/// Low memory while the window is hidden or minimized, normal while it is shown.
pub fn follow_visibility<R: Runtime>(window: &WebviewWindow<R>) {
    let low = !window.is_visible().unwrap_or(true) || window.is_minimized().unwrap_or(false);
    set_low(window, low);
}

fn set_low<R: Runtime>(window: &WebviewWindow<R>, low: bool) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows_core::Interface;
    let result = window.with_webview(move |webview| {
        // SAFETY: COM calls on the WebView2 objects Tauri created, on its own thread.
        let applied = unsafe {
            webview
                .controller()
                .CoreWebView2()
                .and_then(|core| core.cast::<ICoreWebView2_19>())
                .and_then(|core| {
                    core.SetMemoryUsageTargetLevel(if low {
                        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
                    } else {
                        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
                    })
                })
        };
        if let Err(error) = applied {
            tracing::debug!(%error, "WebView2 memory target not set");
        }
    });
    if let Err(error) = result {
        tracing::debug!(%error, "WebView not reachable");
    }
}

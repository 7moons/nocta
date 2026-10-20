//! Platform adapters for URI dispatch and owned-process identity.
use super::instances::NativePlatform;
use std::sync::Arc;

pub fn platform() -> Arc<dyn NativePlatform> {
    Arc::new(windows::WindowsPlatform)
}

pub mod windows;
pub mod windows_control;
pub mod windows_resize;
pub mod windows_title;
pub mod windows_window;

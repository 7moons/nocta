#[cfg(not(windows))]
compile_error!("Nocta runs on Windows only.");

pub mod accounts;
pub mod backup;
pub mod browser;
pub mod cli;
pub mod error;
pub mod hotkeys;
pub mod launch;
pub mod legacy;
pub mod metrics;
pub mod models;
pub mod ram_import;
pub mod roblox;
pub mod secret;
pub mod sessions;
pub mod storage;
pub mod tasks;
pub mod totp;
pub mod vault;
pub mod webhook;

pub use error::{CoreError, CoreResult};

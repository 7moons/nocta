//! Nocta's own Roblox install.
#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientGraphicsApi {
    #[default]
    Auto,
    D3d11,
    Vulkan,
    Opengl,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientUpdateMode {
    /// Install a newer Roblox version before launching.
    #[default]
    Auto,
    /// Report a newer version and wait for the user to update.
    Ask,
    /// Never check; update only on request.
    Manual,
}

/// A Roblox client version installed in Nocta's own folder.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InstalledClient {
    pub guid: String,
    /// Display version such as 0.741.0.7411058, when known.
    pub version: Option<String>,
    pub installed_at: String,
    pub size_mb: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LatestClient {
    pub guid: String,
    pub version: String,
    pub checked_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClientInstallProgress {
    pub guid: String,
    /// "preparing", "downloading", "extracting" or "finishing".
    pub phase: String,
    pub done_mb: u32,
    pub total_mb: u32,
    pub packages_done: u32,
    pub packages_total: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
pub struct ClientState {
    pub installed: Vec<InstalledClient>,
    /// GUID used for the next launch.
    pub active: Option<String>,
    pub latest: Option<LatestClient>,
    pub progress: Option<ClientInstallProgress>,
    pub last_error: Option<String>,
    pub directory: String,
}

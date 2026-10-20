//! The snapshot the UI renders, with logs and metrics.
#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: u32,
    pub timestamp: String,
    pub level: String,
    pub module: String,
    pub message: String,
    pub account_id: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    pub platform: String,
    pub version: String,
    pub data_directory: String,
    pub portable: bool,
    pub browser_ready: bool,
    pub browser_path: Option<String>,
    pub capabilities: Vec<String>,
    /// The window is drawn without GPU acceleration (safe display mode).
    #[serde(default)]
    pub safe_display: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub revision: u32,
    pub accounts: Vec<Account>,
    pub folders: Vec<Folder>,
    pub config: AppConfig,
    pub logs: Vec<LogEntry>,
    pub runtime: RuntimeInfo,
    pub tasks: Vec<AccountTask>,
    pub metrics: MetricsSummary,
    #[serde(default)]
    pub instances: Vec<GameInstance>,
    #[serde(default)]
    pub external_instances: Vec<ExternalInstance>,
    #[serde(default)]
    pub client: ClientState,
    #[serde(default)]
    pub launcher: LauncherState,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DurationSummary {
    pub samples: u32,
    pub average_ms: f64,
    pub p95_ms: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MetricErrors {
    pub check_timeout: u32,
    pub verify_timeout: u32,
    pub ambiguous_precheck: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSummary {
    pub checks_total: u32,
    pub captchas_detected_total: u32,
    pub launches_total: u32,
    pub launches_success_total: u32,
    pub check: DurationSummary,
    pub launch: DurationSummary,
    pub errors: MetricErrors,
}

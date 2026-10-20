//! Operations run on accounts and their progress.
#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    CheckHealth,
    ValidateCookie,
    OpenBrowser,
    RefreshCookie,
    LaunchGame,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Running,
    WaitingUser,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AccountTask {
    pub id: u32,
    pub account_id: u32,
    pub kind: TaskKind,
    pub status: TaskStatus,
    pub detail: String,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<u32>,
}

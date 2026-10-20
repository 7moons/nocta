//! Bulk import reports and old Nocta databases.
#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// An account database of the earlier Nocta app (or its predecessors) found on this computer.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LegacySource {
    pub path: String,
    pub app: String,
    pub accounts: u32,
    pub in_trash: u32,
    pub folders: u32,
    pub modified_at: Option<String>,
    /// The file is encrypted with the other app's password, which the import asks for.
    #[serde(default)]
    pub needs_password: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub added: u32,
    pub updated: u32,
    pub skipped: u32,
    pub issues: Vec<ImportIssue>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub processed: u32,
    pub total: u32,
    pub added: u32,
    pub updated: u32,
    pub failed: u32,
    pub committed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ImportIssue {
    pub line: u32,
    pub reason: String,
}

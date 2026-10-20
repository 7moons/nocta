//! Accounts, folders, presence and the account input forms.
#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccountHealth {
    #[default]
    Unchecked,
    Healthy,
    CaptchaRequired,
    InvalidSession,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AccountPresence {
    pub user_presence_type: u32,
    pub last_location: Option<String>,
    pub last_online: Option<String>,
    pub place_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: u32,
    pub username: String,
    pub folder_id: Option<u32>,
    pub health: AccountHealth,
    pub checked_at: Option<String>,
    pub created_at: String,
    pub deleted_at: Option<String>,
    pub has_password: bool,
    pub has_cookie: bool,
    pub user_id: Option<String>,
    #[serde(default)]
    pub cookie_valid: Option<bool>,
    #[serde(default)]
    pub last_started_at: Option<String>,
    #[serde(default)]
    pub presence: Option<AccountPresence>,
    #[serde(default)]
    pub last_task: Option<AccountTask>,
    /// The user's own name for the account, shown next to the username.
    #[serde(default)]
    pub alias: String,
    #[serde(default)]
    pub note: String,
    /// Color tag, `#rrggbb`.
    #[serde(default)]
    pub color: Option<String>,
    /// Pinned accounts are listed first.
    #[serde(default)]
    pub pinned: bool,
    /// Whether an authenticator key is saved for two-step verification codes.
    #[serde(default)]
    pub has_totp: bool,
    /// Time spent in games launched by Nocta.
    #[serde(default)]
    pub playtime_secs: u32,
    /// Robux balance when last checked.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub robux: Option<i64>,
    #[serde(default)]
    pub robux_checked_at: Option<String>,
    /// Main accounts keep running; periodic restarts and the alt profile skip them.
    #[serde(default)]
    pub is_main: bool,
    /// Position in the user's own order (dragged in the list); `None` comes after, by ID.
    #[serde(default)]
    pub sort_order: Option<u32>,
    /// The user's own fields, like "Server: EU".
    #[serde(default)]
    pub fields: Vec<AccountField>,
    /// Where anti-AFK clicks in this account's client, instead of the general action.
    #[serde(default)]
    pub afk_point: Option<ClickPoint>,
}

/// A named value the user keeps with an account.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AccountField {
    pub name: String,
    pub value: String,
}

/// The user's own details of an account.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AccountDetails {
    pub alias: String,
    pub note: String,
    pub color: Option<String>,
    pub pinned: bool,
    #[serde(default)]
    pub is_main: bool,
    #[serde(default)]
    pub fields: Vec<AccountField>,
    #[serde(default)]
    pub afk_point: Option<ClickPoint>,
}

/// A two-step verification code and how long it stays valid.
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TotpCode {
    pub code: String,
    pub remaining_secs: u32,
}

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct AccountSecrets {
    pub password: String,
    pub cookie: String,
    /// Authenticator key (base32) for two-step verification codes; empty when none.
    #[serde(default)]
    pub totp: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: u32,
    pub name: String,
    pub color: String,
    pub group: Option<String>,
    pub sort_order: u32,
}

#[derive(Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct FolderInput {
    pub name: String,
    pub color: String,
    pub group: Option<String>,
}

#[derive(Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AccountUpdate {
    pub username: String,
    pub folder_id: Option<u32>,
    pub password: Option<String>,
    pub cookie: Option<String>,
}

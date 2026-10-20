//! Other Roblox users: lookups, presence and friends.
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A Roblox user. IDs are decimal strings so JavaScript never rounds them.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RobloxUser {
    pub id: String,
    pub name: String,
    pub display_name: String,
}

/// Where a user is right now. `kind`: 0 offline, 1 on the website, 2 in a game, 3 in Studio.
/// `job_id` is only known when the asking account may join that server.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserPresence {
    pub user_id: String,
    pub kind: u32,
    pub place_id: Option<String>,
    pub job_id: Option<String>,
    pub location: Option<String>,
}

/// A friend of an account with where they are.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FriendEntry {
    pub user: RobloxUser,
    pub presence: Option<UserPresence>,
    pub avatar_url: Option<String>,
}

/// One Roblox privacy setting of an account, with the values Roblox offers for it.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrivacySetting {
    /// Roblox's name, like `whoCanJoinMeInExperiences`.
    pub key: String,
    pub value: String,
    pub options: Vec<String>,
    /// Options Roblox only allows after a check of its own (like an age check), with what it
    /// wants. They cannot be chosen from Nocta, so leaving one of them is one-way.
    #[serde(default)]
    pub restricted: Vec<RestrictedOption>,
}

/// An option Roblox only allows after its own check.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RestrictedOption {
    pub value: String,
    /// Roblox's name for the check, like `FacialAgeEstimation`.
    pub requirement: String,
}

/// The device asking to sign in with a Quick Login code.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QuickLoginDevice {
    pub device: String,
    pub location: String,
}

/// Who to befriend among the user's own accounts.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum BefriendMode {
    /// Every account with every other one.
    Everyone,
    /// Every account with this main account.
    WithMain { main: u32 },
}

/// What befriending the user's own accounts did.
#[derive(Clone, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BefriendSummary {
    pub pairs: u32,
    /// Became friends now.
    pub added: u32,
    pub already: u32,
    /// Sent, but Roblox wants a captcha or the other side could not accept.
    pub pending: u32,
    pub failed: u32,
}

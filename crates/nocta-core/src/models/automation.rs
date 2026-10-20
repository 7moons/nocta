//! Launch presets and schedules.
use super::JoinStrategy;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Where a preset sends its accounts.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum PresetTarget {
    /// Any public server of the game.
    Place { place_id: String },
    /// A server, private server or share link.
    Link { url: String },
    /// A public server picked by strategy (squad, lowest ping, hop).
    Pick {
        place_id: String,
        strategy: JoinStrategy,
    },
    /// Wherever this player is.
    Follow { username: String },
}

/// A named group of accounts launched together into one destination.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPreset {
    pub id: String,
    pub name: String,
    pub account_ids: Vec<u32>,
    pub target: PresetTarget,
    /// Launch with the AFK profile (frame-rate cap, lowest graphics, minimized windows).
    pub afk: bool,
    /// System-wide shortcut that runs the preset; empty means none.
    #[serde(default)]
    pub hotkey: String,
}

/// Runs a preset at a time of day on chosen weekdays, and optionally closes its clients later.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub id: String,
    pub preset_id: String,
    /// Weekdays, 0 = Monday … 6 = Sunday.
    pub days: Vec<u8>,
    /// Local time, `HH:MM`.
    pub time: String,
    /// Close the preset's clients this many minutes after launching; 0 keeps them running.
    pub close_after_minutes: u32,
    pub enabled: bool,
}

//! Launch targets, servers, game info and running game clients.
#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LaunchMode {
    Public,
    Job,
    Private,
}

/// How a server is picked for the selected accounts.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JoinStrategy {
    /// One server with room for every selected account.
    Squad,
    LowestPing,
    /// A random server, other than `exclude_job` when given.
    Hop,
    /// The server with the most players that still has a free slot.
    Fullest,
    /// The server with the fewest players.
    Emptiest,
}

/// Frontend launch target. Private servers are selected by an opaque `targetId`; access codes typed by the
/// user may be sent once but are never returned in any response or snapshot.
#[derive(Clone, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum LaunchTargetInput {
    Configured,
    Public {
        place_id: String,
    },
    Job {
        place_id: String,
        job_id: String,
    },
    Private {
        place_id: String,
        access_code: String,
        link_code: Option<String>,
    },
    Server {
        target_id: String,
    },
    Invite {
        url: String,
    },
    /// A public server picked by `strategy` when the launch starts.
    Pick {
        place_id: String,
        strategy: JoinStrategy,
        #[serde(default)]
        exclude_job: Option<String>,
    },
    /// The server a player is in, asked with the first selected account.
    Follow {
        username: String,
    },
}

impl std::fmt::Debug for LaunchTargetInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Configured => "LaunchTargetInput::Configured",
            Self::Public { .. } => "LaunchTargetInput::Public",
            Self::Job { .. } => "LaunchTargetInput::Job",
            Self::Private { .. } => "LaunchTargetInput::Private([REDACTED])",
            Self::Server { .. } => "LaunchTargetInput::Server",
            Self::Invite { .. } => "LaunchTargetInput::Invite([REDACTED])",
            Self::Pick { .. } => "LaunchTargetInput::Pick",
            Self::Follow { .. } => "LaunchTargetInput::Follow",
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchTargetSummary {
    pub mode: LaunchMode,
    pub place_id: String,
    pub job_id: Option<String>,
    pub used_fallback: bool,
    /// Opaque backend reference for private targets; never contains the access code.
    pub target_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServerKind {
    Public,
    Vip,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ServerRow {
    pub id: String,
    pub kind: ServerKind,
    pub playing: u32,
    pub max_players: u32,
    pub player_tokens: Vec<String>,
    pub name: Option<String>,
    pub target_id: Option<String>,
    pub joinable: bool,
    /// Server ping in ms as reported by Roblox (public servers only).
    #[serde(default)]
    pub ping: Option<u32>,
    /// Server frame rate as reported by Roblox (public servers only).
    #[serde(default)]
    pub fps: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ServerListResponse {
    pub place_id: String,
    pub items: Vec<ServerRow>,
    pub warnings: Vec<String>,
    pub cancelled: bool,
    /// Cursor for the next page of public servers; pass it back to load more.
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// How public servers are fetched. Sorting happens on Roblox's side, so "most players"
/// returns the fullest servers of the whole game, not just of the first page.
#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
pub struct ServerListOptions {
    pub most_players_first: bool,
    pub exclude_full: bool,
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaceInfo {
    pub place_id: String,
    pub name: String,
    pub universe_id: Option<String>,
    pub icon_url: Option<String>,
}

/// A game found by name through Roblox search.
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameSearchResult {
    pub place_id: String,
    pub universe_id: String,
    pub name: String,
    pub creator: Option<String>,
    pub playing: u32,
    pub icon_url: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstanceStatus {
    Starting,
    Running,
    Exited,
    Unknown,
}

/// The computer's memory: RAM, and the commit limit (RAM plus page file) that every program's
/// allocations count against. With RAM capped per client, the commit limit is what stops more
/// clients from starting.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SystemMemory {
    pub ram_total_mb: u32,
    pub ram_free_mb: u32,
    pub commit_limit_mb: u32,
    pub commit_used_mb: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GameInstance {
    pub id: String,
    pub account_id: u32,
    pub username: String,
    pub place_id: String,
    pub mode: LaunchMode,
    pub status: InstanceStatus,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
    pub started_at: String,
    pub exited_at: Option<String>,
    pub detail: String,
    /// What the client's own log says: still loading, joining, in its game, or disconnected.
    #[serde(default)]
    pub stage: ClientStage,
    /// Why Roblox disconnected the client, when it did.
    #[serde(default)]
    pub disconnect: Option<String>,
    /// Memory the client holds right now (its working set), in megabytes.
    #[serde(default)]
    pub memory_mb: Option<u32>,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClientStage {
    #[default]
    Loading,
    Joining,
    InGame,
    Disconnected,
}

/// A running Roblox game client that Nocta did not launch (started before Nocta, or outside it).
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalInstance {
    /// Identity token `pid:creation-ms`; closing re-verifies it so a reused PID is never targeted.
    pub id: String,
    pub pid: u32,
    pub process_name: String,
    pub started_at: String,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MultiInstanceState {
    #[default]
    Disabled,
    Acquired,
    Abandoned,
    Busy,
    Unsupported,
    Failed,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LauncherState {
    pub handler_registered: bool,
    pub handler_path: Option<String>,
    pub multi_instance: MultiInstanceState,
    pub multi_instance_supported: bool,
    pub detail: Option<String>,
}

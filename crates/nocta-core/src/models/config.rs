//! App settings (`AppConfig`) and the types they use.
#[allow(unused_imports)]
use super::*;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GamePreset {
    pub id: String,
    pub name: String,
    pub place_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
pub struct LogPreferences {
    pub level: String,
    pub module: String,
    pub show_timestamp: bool,
    pub show_account: bool,
    pub show_module: bool,
    pub show_level: bool,
}

impl Default for LogPreferences {
    fn default() -> Self {
        Self {
            level: "all".into(),
            module: "all".into(),
            show_timestamp: true,
            show_account: true,
            show_module: true,
            show_level: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
pub struct AppConfig {
    pub checker_concurrency: u32,
    pub precheck_retries: u32,
    pub browser_timeout_secs: u32,
    pub checker_delay_secs: u32,
    pub bulk_action_delay_secs: u32,
    pub browser_app_mode: bool,
    pub launch_precheck: bool,
    pub pre_launch_delay_secs: u32,
    pub multi_instance: bool,
    pub auto_fix_private_invite_privacy: bool,
    pub place_id: String,
    pub invite_url: String,
    pub presets: Vec<GamePreset>,
    pub always_on_top: bool,
    pub privacy_mode: bool,
    pub interface_scale: u32,
    pub show_status_bar: bool,
    pub default_folder_color: String,
    pub search_separators: String,
    pub log_preferences: LogPreferences,
    /// Launch accounts with Nocta's own Roblox client instead of the system `roblox-player` handler.
    pub managed_client: bool,
    pub client_update_mode: ClientUpdateMode,
    /// Previous client versions kept for rollback, besides the active one.
    pub client_keep_versions: u32,
    /// Version GUID to launch with; `None` launches the newest installed version.
    pub client_pinned_version: Option<String>,
    /// Lowest rendering, texture and detail quality for Nocta's client (allowlisted flags only).
    pub client_low_graphics: bool,
    pub client_graphics_api: ClientGraphicsApi,
    /// Roblox's frame-rate cap written before launches; 0 leaves Roblox's own setting.
    pub fps_cap: u32,
    /// AFK profile: its frame-rate cap, lowest graphics and minimized windows.
    pub afk_mode: bool,
    pub afk_fps_cap: u32,
    pub afk_minimize: bool,
    /// Relaunch a client into the same server when it closes on its own.
    pub auto_rejoin: bool,
    /// Discord webhook told about crashes and rejoins; empty for none.
    pub discord_webhook: String,
    /// How the launch panel picks a server; `None` is any server or the saved link.
    pub join_strategy: Option<JoinStrategy>,
    /// Player followed when launching with "follow a player".
    pub follow_username: String,
    pub launch_presets: Vec<LaunchPreset>,
    pub schedules: Vec<Schedule>,
    /// Press a key in idle clients now and then so Roblox does not kick them after 20 minutes.
    pub anti_afk: bool,
    pub anti_afk_minutes: u32,
    /// Vary each client's interval a little, so they do not all act at the same second.
    pub anti_afk_random: bool,
    pub anti_afk_action: AntiAfkAction,
    /// Page out the clients' unused memory every few minutes.
    pub trim_memory: bool,
    /// Close (and with auto-rejoin, restart) a client using more memory than this; 0 is off.
    pub memory_limit_mb: u32,
    /// The window's close button hides Nocta in the tray instead of quitting.
    pub close_to_tray: bool,
    pub start_with_windows: bool,
    /// Started with Windows: stay in the tray instead of opening the window.
    pub start_minimized: bool,
    pub hotkeys: Hotkeys,
    /// Roblox FastFlags for Nocta's own client, limited to the flags Roblox allows.
    pub fast_flags: Vec<FastFlag>,
    /// Shorter account rows with one line each, so more fit on screen.
    pub compact_list: bool,
    /// When the next account of a launch starts.
    pub launch_pacing: LaunchPacing,
    /// With `InGame` pacing, the longest wait for a client to join before the next one starts.
    pub in_game_timeout_secs: u32,
    /// Title each client window with its account's name.
    pub name_windows: bool,
    pub arrange_monitors: ArrangeMonitors,
    /// Arrange the windows again whenever a client appears.
    pub auto_arrange: bool,
    /// Close (and with auto-rejoin, restart) clients whose window stopped responding.
    pub close_hung: bool,
    /// Close (and with auto-rejoin, restart) clients that Roblox disconnected.
    pub close_disconnected: bool,
    /// Restart clients of accounts not marked as main every so many minutes; 0 is off.
    pub restart_alts_minutes: u32,
    /// Resource profile for clients of main accounts.
    pub main_profile: ClientProfile,
    /// Resource profile for every other client.
    pub alt_profile: ClientProfile,
    /// Keep the computer awake while clients run with anti-AFK, auto-rejoin or schedules.
    pub keep_awake: bool,
    /// Dragging a Roblox window's border goes below Roblox's minimum (800×600), down to 1×1.
    pub free_window_resize: bool,
    /// Colors the user saved for folders, `#rrggbb`, shown next to the built-in ones.
    pub folder_colors: Vec<String>,
    /// Where anti-AFK clicks when its action is `Click` (accounts can have their own point).
    pub anti_afk_click: ClickPoint,
    /// Hide account names (aliases stay) so the screen can be shared or streamed.
    pub streamer_mode: bool,
    /// Picked servers (squad, fullest, hop…) must be in this part of the world.
    pub server_region: Option<ServerRegion>,
}

/// Where a Roblox server runs, by continent of its datacenter.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServerRegion {
    NorthAmerica,
    SouthAmerica,
    Europe,
    Asia,
    Oceania,
    Africa,
}

impl ServerRegion {
    /// From a two-letter continent code (`EU`, `NA`…).
    pub fn from_continent(code: &str) -> Option<Self> {
        Some(match code {
            "NA" => Self::NorthAmerica,
            "SA" => Self::SouthAmerica,
            "EU" => Self::Europe,
            "AS" => Self::Asia,
            "OC" => Self::Oceania,
            "AF" => Self::Africa,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::NorthAmerica => "North America",
            Self::SouthAmerica => "South America",
            Self::Europe => "Europe",
            Self::Asia => "Asia",
            Self::Oceania => "Oceania",
            Self::Africa => "Africa",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LaunchPacing {
    /// The next account starts once the previous client is in its game.
    #[default]
    InGame,
    /// A fixed pause between accounts (`bulk_action_delay_secs`).
    Fixed,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArrangeMonitors {
    #[default]
    All,
    Primary,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProcessPriority {
    #[default]
    Normal,
    BelowNormal,
    Idle,
}

/// How much of the computer a client may use.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct ClientProfile {
    pub priority: ProcessPriority,
    /// Windows' efficiency mode (EcoQoS): lower clocks and efficient cores.
    pub eco_mode: bool,
    /// Share of all CPU cores the client may use, in percent; 0 is unlimited.
    pub cpu_limit_percent: u32,
    /// Window size in pixels; 0 leaves the size Roblox chooses.
    pub window_width: u32,
    pub window_height: u32,
    /// Windows takes memory from these clients first when it runs short.
    pub low_memory_priority: bool,
    /// Gives back the client's unused memory this often, in seconds; 0 is off. A client in a
    /// game touches only part of what it holds: measured, a client at 1.7 GB of RAM went down to
    /// 0.1 GB and settled near 0.8 GB.
    pub trim_every_secs: u32,
    /// Most RAM the client may hold, in megabytes; 0 is no cap. Windows keeps the rest of what
    /// it allocated compressed or in the page file. Measured in a game: 939 MB without a cap,
    /// 350 MB with one, for 0.3% more CPU; at 250 MB reloading pages starts to cost CPU.
    pub ram_cap_mb: u32,
}

/// What anti-AFK presses in a client.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AntiAfkAction {
    /// Right arrow: turns the camera a little. Harmless in almost every game.
    #[default]
    Turn,
    /// Space: jumps.
    Jump,
    /// A left click at `anti_afk_click` in the client window.
    Click,
}

/// A point in a client window, in percent of its width and height from the top left.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClickPoint {
    pub x_percent: u32,
    pub y_percent: u32,
}

impl Default for ClickPoint {
    fn default() -> Self {
        Self {
            x_percent: 50,
            y_percent: 50,
        }
    }
}

/// System-wide shortcuts (`Ctrl+Alt+N` style); empty means none.
#[derive(Clone, Debug, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Hotkeys {
    pub show: String,
    pub close_roblox: String,
    pub toggle_afk: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FastFlag {
    pub name: String,
    pub value: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            checker_concurrency: 5,
            precheck_retries: 1,
            browser_timeout_secs: 180,
            checker_delay_secs: 0,
            bulk_action_delay_secs: 30,
            browser_app_mode: true,
            launch_precheck: true,
            pre_launch_delay_secs: 5,
            multi_instance: true,
            auto_fix_private_invite_privacy: false,
            place_id: String::new(),
            invite_url: String::new(),
            presets: Vec::new(),
            always_on_top: false,
            privacy_mode: true,
            interface_scale: 100,
            show_status_bar: true,
            default_folder_color: "#a3a3a3".to_owned(),
            search_separators: ",".into(),
            log_preferences: LogPreferences::default(),
            managed_client: false,
            client_update_mode: ClientUpdateMode::Auto,
            client_keep_versions: 1,
            client_pinned_version: None,
            client_low_graphics: false,
            client_graphics_api: ClientGraphicsApi::Auto,
            fps_cap: 0,
            afk_mode: false,
            afk_fps_cap: 15,
            afk_minimize: true,
            auto_rejoin: false,
            discord_webhook: String::new(),
            join_strategy: None,
            follow_username: String::new(),
            launch_presets: Vec::new(),
            schedules: Vec::new(),
            anti_afk: false,
            anti_afk_minutes: 15,
            anti_afk_random: true,
            anti_afk_action: AntiAfkAction::Turn,
            trim_memory: false,
            memory_limit_mb: 0,
            close_to_tray: false,
            start_with_windows: false,
            start_minimized: true,
            hotkeys: Hotkeys::default(),
            fast_flags: Vec::new(),
            compact_list: false,
            launch_pacing: LaunchPacing::InGame,
            in_game_timeout_secs: 120,
            name_windows: true,
            arrange_monitors: ArrangeMonitors::All,
            auto_arrange: false,
            close_hung: true,
            close_disconnected: true,
            restart_alts_minutes: 0,
            main_profile: ClientProfile::default(),
            alt_profile: ClientProfile::default(),
            keep_awake: true,
            free_window_resize: true,
            folder_colors: Vec::new(),
            anti_afk_click: ClickPoint::default(),
            streamer_mode: false,
            server_region: None,
        }
    }
}

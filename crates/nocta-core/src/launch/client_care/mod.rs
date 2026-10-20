//! What the upkeep loop does for running clients besides anti-AFK and memory: names their
//! windows, applies resource profiles, arranges windows, restarts frozen or disconnected
//! clients and alts on a timer, and keeps the computer awake.
use super::Launcher;

mod upkeep;
pub mod window_grid;
use crate::models::{AppConfig, ArrangeMonitors, ClientStage, GameInstance, InstanceStatus};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
pub use upkeep::Upkeep;

/// A window counts as frozen after this many upkeep passes in a row ("Not responding").
const HUNG_PASSES: u32 = 4;
/// A disconnected client gets this long to rejoin on its own (a teleport) before it is closed.
const DISCONNECT_GRACE: Duration = Duration::from_secs(20);
/// Titles are set again now and then, since Roblox may rename its window.
const RETITLE_EVERY: Duration = Duration::from_secs(60);
/// Profiles and window sizes are set again this often, in case Roblox changed them (it sets
/// its own RAM limits and window size).
const RESIZE_EVERY: Duration = Duration::from_secs(60);
/// How often "Trim client memory" gives memory back when a profile does not ask for more.
pub const TRIM_EVERY: Duration = Duration::from_secs(5 * 60);

#[derive(Default)]
pub struct CareState {
    titled: HashMap<String, Instant>,
    /// The profile each client got, so a changed setting reaches running clients.
    profiled: HashMap<String, (crate::models::ClientProfile, Instant)>,
    /// The window size each client got and when; set again now and then in case Roblox changed it.
    sized: HashMap<String, ((u32, u32), Instant)>,
    hung: HashMap<String, u32>,
    disconnected: HashMap<String, Instant>,
    arranged: Vec<String>,
    /// When each client last gave back its unused memory.
    trimmed: HashMap<String, Instant>,
    /// Clients already trimmed once after joining their game.
    trimmed_in_game: HashSet<String>,
}

struct AccountInfo {
    name: String,
    main: bool,
}

/// When a client started, from its RFC 3339 timestamp.
fn running_for(client: &GameInstance) -> Option<Duration> {
    let started = chrono::DateTime::parse_from_rfc3339(&client.started_at).ok()?;
    (chrono::Utc::now() - started.with_timezone(&chrono::Utc))
        .to_std()
        .ok()
}

/// The account part of a client window title. In streamer mode the username is left out: the
/// alias, or the account number, is shown instead.
pub fn window_name(id: u32, alias: &str, username: &str, streamer: bool) -> String {
    match (alias.is_empty(), streamer) {
        (true, true) => format!("#{id}"),
        (false, true) => alias.to_owned(),
        (true, false) => username.to_owned(),
        (false, false) => format!("{alias} ({username})"),
    }
}

/// Looks after running clients: playtime, anti-AFK, memory, windows and restarts.
#[derive(Clone)]
pub struct ClientCare {
    pub(super) launcher: Arc<Launcher>,
}

impl ClientCare {
    pub fn new(launcher: Arc<Launcher>) -> Self {
        Self { launcher }
    }

    pub(super) async fn care_for_clients(
        &self,
        state: &mut CareState,
        config: &AppConfig,
        clients: &[(GameInstance, u32)],
        now: Instant,
    ) {
        let alive: HashSet<&String> = clients.iter().map(|(client, _)| &client.id).collect();
        state.titled.retain(|id, _| alive.contains(id));
        state.profiled.retain(|id, _| alive.contains(id));
        state.sized.retain(|id, _| alive.contains(id));
        state.hung.retain(|id, _| alive.contains(id));
        state.disconnected.retain(|id, _| alive.contains(id));
        state.trimmed.retain(|id, _| alive.contains(id));
        state.trimmed_in_game.retain(|id| alive.contains(id));
        self.launcher
            .instances
            .platform()
            .free_resize(config.free_window_resize);
        self.launcher.instances.platform().keep_awake(
            config.keep_awake
                && !clients.is_empty()
                && (config.anti_afk || config.auto_rejoin || !config.schedules.is_empty()),
        );
        if clients.is_empty() {
            return;
        }
        let accounts: HashMap<u32, AccountInfo> = match self.launcher.accounts.list().await {
            Ok(list) => list
                .into_iter()
                .map(|account| {
                    let name = window_name(
                        account.id,
                        &account.alias,
                        &account.username,
                        config.streamer_mode,
                    );
                    (
                        account.id,
                        AccountInfo {
                            name,
                            main: account.is_main,
                        },
                    )
                })
                .collect(),
            Err(_) => HashMap::new(),
        };
        let platform = self.launcher.instances.platform();
        for (client, pid) in clients {
            let info = accounts.get(&client.account_id);
            if config.name_windows
                && state
                    .titled
                    .get(&client.id)
                    .is_none_or(|at| now.duration_since(*at) >= RETITLE_EVERY)
            {
                let title = format!(
                    "Roblox · {}",
                    info.map_or(client.username.as_str(), |info| info.name.as_str())
                );
                let (platform, pid) = (platform.clone(), *pid);
                let named = tokio::task::spawn_blocking(move || platform.set_title(pid, &title))
                    .await
                    .unwrap_or(false);
                if named {
                    state.titled.insert(client.id.clone(), now);
                }
            }
            let profile = if info.is_some_and(|info| info.main) {
                config.main_profile
            } else {
                config.alt_profile
            };
            let due = state.profiled.get(&client.id).is_none_or(|(given, at)| {
                *given != profile || now.duration_since(*at) >= RESIZE_EVERY
            });
            if due {
                let (platform, pid) = (platform.clone(), *pid);
                let applied =
                    tokio::task::spawn_blocking(move || platform.apply_profile(pid, &profile))
                        .await
                        .is_ok_and(|result| result.is_ok());
                if applied {
                    state.profiled.insert(client.id.clone(), (profile, now));
                }
            }
            // The profile's window size, unless auto-arrange sizes the windows itself.
            let size = (profile.window_width, profile.window_height);
            if size.0 > 0 && !config.auto_arrange {
                let due = state.sized.get(&client.id).is_none_or(|(given, at)| {
                    *given != size || now.duration_since(*at) >= RESIZE_EVERY
                });
                if due {
                    let (platform, pid) = (platform.clone(), *pid);
                    let resized = tokio::task::spawn_blocking(move || {
                        platform.resize_window(pid, size.0, size.1)
                    })
                    .await
                    .unwrap_or(false);
                    if resized {
                        state.sized.insert(client.id.clone(), (size, now));
                    }
                }
            }
        }

        // Memory: what each client holds, and giving back what it does not use. Loading a game
        // leaves the most behind, so a client is also trimmed once right after it joins.
        for (client, pid) in clients {
            let main = accounts
                .get(&client.account_id)
                .is_some_and(|info| info.main);
            let profile = if main {
                config.main_profile
            } else {
                config.alt_profile
            };
            let own = (profile.trim_every_secs > 0)
                .then(|| Duration::from_secs(profile.trim_every_secs as u64));
            let every = match (own, config.trim_memory) {
                (Some(own), true) => Some(own.min(TRIM_EVERY)),
                (Some(own), false) => Some(own),
                (None, true) => Some(TRIM_EVERY),
                (None, false) => None,
            };
            if let Some(every) = every {
                let joined = client.stage == ClientStage::InGame
                    && state.trimmed_in_game.insert(client.id.clone());
                let last = *state.trimmed.entry(client.id.clone()).or_insert(now);
                if joined || now.duration_since(last) >= every {
                    self.launcher.instances.trim_memory(*pid).await;
                    state.trimmed.insert(client.id.clone(), now);
                }
            }
            if let Some(megabytes) = self.launcher.instances.memory_mb(*pid) {
                self.launcher
                    .instances
                    .set_memory(&client.id, megabytes.min(u32::MAX as u64) as u32);
            }
        }

        if config.auto_arrange {
            let mut ids: Vec<String> = clients
                .iter()
                .map(|(client, _)| client.id.clone())
                .collect();
            ids.sort();
            if ids != state.arranged && self.arrange_windows(config).await as usize == ids.len() {
                state.arranged = ids;
            }
        }

        // Frozen, disconnected and timed restarts, at most one decision per client.
        for (client, pid) in clients {
            let main = accounts
                .get(&client.account_id)
                .is_some_and(|info| info.main);
            let (platform, pid) = (platform.clone(), *pid);
            let hung = tokio::task::spawn_blocking(move || platform.hung(pid))
                .await
                .unwrap_or(None)
                == Some(true);
            let passes = state.hung.entry(client.id.clone()).or_default();
            *passes = if hung { *passes + 1 } else { 0 };
            if config.close_hung && *passes >= HUNG_PASSES {
                state.hung.remove(&client.id);
                self.restart_client(client, "stopped responding", config.auto_rejoin, config)
                    .await;
                continue;
            }
            if client.stage == ClientStage::Disconnected {
                let since = *state.disconnected.entry(client.id.clone()).or_insert(now);
                if config.close_disconnected && now.duration_since(since) >= DISCONNECT_GRACE {
                    state.disconnected.remove(&client.id);
                    let why = format!(
                        "was disconnected ({})",
                        client.disconnect.clone().unwrap_or_default()
                    );
                    self.restart_client(client, &why, config.auto_rejoin, config)
                        .await;
                    continue;
                }
            } else {
                state.disconnected.remove(&client.id);
            }
            if config.restart_alts_minutes > 0
                && !main
                && running_for(client).is_some_and(|age| {
                    age >= Duration::from_secs(config.restart_alts_minutes as u64 * 60)
                })
            {
                let why = format!("ran for {} minutes", config.restart_alts_minutes);
                self.restart_client(client, &why, true, config).await;
            }
        }
    }

    /// Closes a client and, when `relaunch` is set, starts its account again into the same
    /// destination. `why` goes to the log and the webhook.
    pub(super) async fn restart_client(
        &self,
        client: &GameInstance,
        why: &str,
        relaunch: bool,
        config: &AppConfig,
    ) {
        let message = format!(
            "{}'s Roblox client {why}{}",
            client.username,
            if relaunch {
                "; restarting it."
            } else {
                "; it was closed."
            }
        );
        let _ = self
            .launcher
            .accounts
            .log("WARNING", "LAUNCH", &message, Some(client.account_id))
            .await;
        self.launcher
            .webhook
            .notify(&config.discord_webhook, &message)
            .await;
        let run = self.launcher.runs.run_of(&client.id);
        if self.launcher.instances.close(&client.id).await.is_err() || !relaunch {
            return;
        }
        let target = self.launcher.rejoin_target(client.account_id);
        match self
            .launcher
            .start(&[client.account_id], target, config.clone())
            .await
        {
            Ok(tasks) => {
                if let Some(run) = run {
                    self.launcher.runs.tag(&run, &tasks);
                }
            }
            Err(error) => {
                let _ = self
                    .launcher
                    .accounts
                    .log(
                        "ERROR",
                        "LAUNCH",
                        &format!("Restarting {} failed: {error}", client.username),
                        Some(client.account_id),
                    )
                    .await;
            }
        }
    }

    /// Puts the windows of running clients in a grid on the chosen monitors. Returns how many
    /// windows were placed.
    pub async fn arrange_windows(&self, config: &AppConfig) -> u32 {
        let platform = self.launcher.instances.platform();
        let mut clients: Vec<(u32, String)> = self
            .launcher
            .instances
            .snapshot()
            .into_iter()
            .filter(|client| client.status == InstanceStatus::Running)
            .filter_map(|client| Some((client.pid?, client.username)))
            .collect();
        clients.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        let scope = config.arrange_monitors;
        tokio::task::spawn_blocking(move || {
            let mut monitors = platform.monitors();
            if scope == ArrangeMonitors::Primary {
                monitors.retain(|monitor| monitor.primary);
            }
            let cells = window_grid::grid(&monitors, clients.len());
            clients
                .iter()
                .zip(cells)
                .filter(|((pid, _), cell)| {
                    platform.place_window(*pid, cell.x, cell.y, cell.width, cell.height)
                })
                .count() as u32
        })
        .await
        .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::window_name;

    #[test]
    fn streamer_mode_keeps_usernames_out_of_window_titles() {
        assert_eq!(window_name(3, "", "RealName", false), "RealName");
        assert_eq!(window_name(3, "Farm", "RealName", false), "Farm (RealName)");
        assert_eq!(window_name(3, "", "RealName", true), "#3");
        assert_eq!(window_name(3, "Farm", "RealName", true), "Farm");
    }
}

//! Looking after running clients: playtime per account, anti-AFK key presses, memory trimming
//! and the memory limit. One loop does all of it every few seconds.
use super::ClientCare;
use crate::models::{AppConfig, GameInstance, InstanceStatus};
use rand::Rng;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

const TICK: Duration = Duration::from_secs(15);
/// Longer gaps (a sleeping computer) are not counted as playtime.
const MAX_TICK_GAP: Duration = Duration::from_secs(120);
/// While the user is typing or moving the mouse, anti-AFK waits so it does not steal focus...
const USER_ACTIVE_SECS: u64 = 5;
const WAIT_FOR_USER: Duration = Duration::from_secs(20);
/// ...but not longer than this, or the client would be kicked.
const MAX_WAIT_FOR_USER: Duration = Duration::from_secs(120);

#[derive(Default)]
pub struct Upkeep {
    last_tick: Option<Instant>,
    /// When each client's next anti-AFK press is due.
    afk_due: HashMap<String, Instant>,
    /// Since when a press has been waiting for the user to stop using the computer.
    afk_waiting: HashMap<String, Instant>,
    care: super::CareState,
}

/// The time until the next anti-AFK press, varied by up to a fifth when `random` is on.
pub fn afk_interval(config: &AppConfig, random: f64) -> Duration {
    let base = config.anti_afk_minutes.clamp(1, 19) as f64 * 60.0;
    let factor = if config.anti_afk_random {
        0.8 + random.clamp(0.0, 1.0) * 0.4
    } else {
        1.0
    };
    Duration::from_secs_f64((base * factor).min(19.0 * 60.0))
}

fn running(instances: Vec<GameInstance>) -> Vec<(GameInstance, u32)> {
    instances
        .into_iter()
        .filter(|instance| instance.status == InstanceStatus::Running)
        .filter_map(|instance| Some((instance.clone(), instance.pid?)))
        .collect()
}

impl ClientCare {
    /// Starts the upkeep loop. Call once, after start-up.
    pub fn spawn_upkeep(&self) {
        // A cap left behind by an earlier run goes back first.
        if let Err(error) = self.launcher.frame_rate.settle() {
            tracing::warn!(%error, "The user's frame-rate cap was not put back");
        }
        let service = self.clone();
        self.launcher.tracker.spawn(async move {
            let mut state = Upkeep::default();
            loop {
                tokio::select! {
                    _ = service.launcher.stop.cancelled() => break,
                    _ = tokio::time::sleep(TICK) => {}
                }
                service.upkeep(&mut state, Instant::now()).await;
            }
        });
    }

    /// One pass over the running clients at `now`.
    pub async fn upkeep(&self, state: &mut Upkeep, now: Instant) {
        let Ok(config) = self.launcher.accounts.config().await else {
            return;
        };
        self.launcher.refresh_client_logs().await;
        // A capped client the user closed, or one from before Nocta started, saved Nocta's cap.
        if let Err(error) = self.launcher.frame_rate.settle() {
            tracing::warn!(%error, "The user's frame-rate cap was not put back");
        }
        let clients = running(self.launcher.instances.snapshot());
        self.count_playtime(state, &clients, now).await;
        self.anti_afk(state, &config, &clients, now).await;
        if config.memory_limit_mb > 0 {
            self.enforce_memory_limit(&config, &clients).await;
        }
        let clients = running(self.launcher.instances.snapshot());
        self.care_for_clients(&mut state.care, &config, &clients, now)
            .await;
    }

    async fn count_playtime(
        &self,
        state: &mut Upkeep,
        clients: &[(GameInstance, u32)],
        now: Instant,
    ) {
        let elapsed = state
            .last_tick
            .map(|last| now.saturating_duration_since(last))
            .filter(|gap| *gap <= MAX_TICK_GAP);
        state.last_tick = Some(now);
        let Some(elapsed) = elapsed else {
            return;
        };
        let mut per_account: HashMap<u32, u64> = HashMap::new();
        for (client, _) in clients {
            // Two clients of one account count once.
            per_account.insert(client.account_id, elapsed.as_secs());
        }
        let updates: Vec<(u32, u64)> = per_account
            .into_iter()
            .filter(|(_, secs)| *secs > 0)
            .collect();
        if let Err(error) = self.launcher.accounts.add_playtime(&updates).await {
            tracing::warn!(%error, "Playtime not saved");
        }
    }

    async fn anti_afk(
        &self,
        state: &mut Upkeep,
        config: &AppConfig,
        clients: &[(GameInstance, u32)],
        now: Instant,
    ) {
        if !config.anti_afk {
            state.afk_due.clear();
            state.afk_waiting.clear();
            return;
        }
        state
            .afk_due
            .retain(|id, _| clients.iter().any(|(client, _)| &client.id == id));
        state
            .afk_waiting
            .retain(|id, _| clients.iter().any(|(client, _)| &client.id == id));
        let user_active = self
            .launcher
            .instances
            .user_idle_secs()
            .is_some_and(|idle| idle < USER_ACTIVE_SECS);
        // Accounts' own click points, read only once a press is due.
        let mut points: Option<HashMap<u32, crate::models::ClickPoint>> = None;
        let mut pressed = 0;
        for (client, pid) in clients {
            let due = *state
                .afk_due
                .entry(client.id.clone())
                .or_insert_with(|| now + afk_interval(config, rand::thread_rng().gen()));
            if now < due {
                continue;
            }
            let waiting_since = *state.afk_waiting.entry(client.id.clone()).or_insert(now);
            if user_active && now.duration_since(waiting_since) < MAX_WAIT_FOR_USER {
                state.afk_due.insert(client.id.clone(), now + WAIT_FOR_USER);
                continue;
            }
            state.afk_waiting.remove(&client.id);
            if points.is_none() {
                points = Some(match self.launcher.accounts.list().await {
                    Ok(accounts) => accounts
                        .into_iter()
                        .filter_map(|account| Some((account.id, account.afk_point?)))
                        .collect(),
                    Err(_) => HashMap::new(),
                });
            }
            let point = points
                .as_ref()
                .and_then(|points| points.get(&client.account_id).copied());
            let input = crate::launch::instances::AfkInput::choose(config, point);
            if self.launcher.instances.nudge(*pid, input).await {
                pressed += 1;
            }
            state.afk_due.insert(
                client.id.clone(),
                now + afk_interval(config, rand::thread_rng().gen()),
            );
        }
        if pressed > 0 {
            let message = format!("Anti-AFK kept {pressed} Roblox clients active.");
            let _ = self
                .launcher
                .accounts
                .log("INFO", "LAUNCH", &message, None)
                .await;
        }
    }

    async fn enforce_memory_limit(&self, config: &AppConfig, clients: &[(GameInstance, u32)]) {
        for (client, pid) in clients {
            // Allocated memory, not RAM: with a RAM cap the working set never grows past it,
            // while a leaking game keeps allocating.
            let Some(used) = self.launcher.instances.committed_mb(*pid) else {
                continue;
            };
            if used <= config.memory_limit_mb as u64 {
                continue;
            }
            let why = format!("used {used} MB (limit {} MB)", config.memory_limit_mb);
            self.restart_client(client, &why, config.auto_rejoin, config)
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_stay_under_roblox_idle_kick_and_vary_when_asked() {
        let config = AppConfig {
            anti_afk_minutes: 15,
            anti_afk_random: false,
            ..Default::default()
        };
        assert_eq!(afk_interval(&config, 0.9), Duration::from_secs(900));
        let random = AppConfig {
            anti_afk_random: true,
            ..config.clone()
        };
        assert_eq!(afk_interval(&random, 0.0), Duration::from_secs(720));
        assert_eq!(afk_interval(&random, 0.5), Duration::from_secs(900));
        let longest = AppConfig {
            anti_afk_minutes: 19,
            ..random
        };
        assert!(afk_interval(&longest, 1.0) <= Duration::from_secs(19 * 60));
    }
}

//! Auto-rejoin: when a client Nocta started closes by itself, the account goes back to where it
//! was launched (the same server, private server or followed player), and the Discord webhook
//! hears about it. Rejoins slow down when a game keeps crashing, and stop for an hour after
//! [`MAX_PER_HOUR`] attempts.
use super::{instances::ExitEvent, target::ResolvedTarget, Launcher};
use crate::models::LaunchTargetInput;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

pub const MAX_PER_HOUR: usize = 10;
const HOUR: Duration = Duration::from_secs(60 * 60);

/// Where an account was last launched, kept for a rejoin.
#[derive(Clone)]
pub(super) enum Destination {
    Input(LaunchTargetInput),
    /// A private server from the list, kept resolved: its opaque ID expires after 30 minutes.
    Private(ResolvedTarget),
}

/// The wait before a rejoin, given how many rejoins this account had in the last hour.
pub fn rejoin_delay(recent: usize) -> Option<Duration> {
    let secs = match recent {
        0 => 5,
        1 => 15,
        2 => 60,
        3 => 180,
        n if n < MAX_PER_HOUR => 600,
        _ => return None,
    };
    Some(Duration::from_secs(secs))
}

impl Launcher {
    pub(super) fn remember_destination(&self, accounts: &[u32], input: &LaunchTargetInput) {
        let destination = match input {
            LaunchTargetInput::Server { target_id } => match self.servers.target(target_id) {
                Ok(target) => Destination::Private(target),
                Err(_) => return,
            },
            other => Destination::Input(other.clone()),
        };
        let mut destinations = self.destinations.lock().unwrap();
        for account in accounts {
            destinations.insert(*account, destination.clone());
        }
    }

    /// The launch target that brings `account` back to its last destination.
    pub fn rejoin_target(&self, account: u32) -> LaunchTargetInput {
        match self.destinations.lock().unwrap().get(&account).cloned() {
            Some(Destination::Input(input)) => input,
            Some(Destination::Private(target)) => LaunchTargetInput::Server {
                target_id: self.servers.register(target),
            },
            None => LaunchTargetInput::Configured,
        }
    }
}

/// Starts clients again after they close by themselves.
#[derive(Clone)]
pub struct Rejoin {
    launcher: Arc<Launcher>,
}

impl Rejoin {
    pub fn new(launcher: Arc<Launcher>) -> Self {
        Self { launcher }
    }

    /// Starts listening for clients that close by themselves. Call once, after start-up.
    pub fn spawn(&self) {
        let mut exits = self.launcher.instances.subscribe_exits();
        let service = self.clone();
        self.launcher.tracker.spawn(async move {
            let mut history: HashMap<u32, Vec<Instant>> = HashMap::new();
            loop {
                let event = tokio::select! {
                    _ = service.launcher.stop.cancelled() => break,
                    event = exits.recv() => match event {
                        Some(event) => event,
                        None => break,
                    },
                };
                // A closing client saves its settings, Nocta's cap included.
                let _ = service.launcher.frame_rate.settle();
                let recent = history.entry(event.account_id).or_default();
                recent.retain(|at| at.elapsed() < HOUR);
                let attempts = recent.len();
                if let Some(delay) = service.on_exit(&event, attempts).await {
                    recent.push(Instant::now());
                    // Tests wait milliseconds instead of seconds.
                    let delay = if cfg!(test) {
                        Duration::from_millis(delay.as_secs())
                    } else {
                        delay
                    };
                    let service = service.clone();
                    service.launcher.clone().tracker.spawn(async move {
                        tokio::select! {
                            _ = service.launcher.stop.cancelled() => {}
                            _ = tokio::time::sleep(delay) => service.rejoin(&event).await,
                        }
                    });
                }
            }
        });
    }

    /// Logs and reports an exit; returns the wait before rejoining, or `None` for no rejoin.
    async fn on_exit(&self, event: &ExitEvent, attempts: usize) -> Option<Duration> {
        let config = self.launcher.accounts.config().await.ok()?;
        // Why Roblox let it go, when its log said so.
        let reason = self
            .launcher
            .instances
            .snapshot()
            .into_iter()
            .find(|instance| instance.id == event.instance_id)
            .and_then(|instance| instance.disconnect)
            .map(|why| format!(" Roblox disconnected it: {why}."))
            .unwrap_or_default();
        let name = &event.username;
        let (delay, message) = if !config.auto_rejoin {
            (None, format!("{name}'s Roblox client closed.{reason}"))
        } else {
            match rejoin_delay(attempts) {
                Some(delay) => (
                    Some(delay),
                    format!(
                        "{name}'s Roblox client closed.{reason} Rejoining in {} s.",
                        delay.as_secs()
                    ),
                ),
                None => (
                    None,
                    format!(
                        "{name}'s Roblox client keeps closing; auto-rejoin paused for an hour."
                    ),
                ),
            }
        };
        let _ = self
            .launcher
            .accounts
            .log("WARNING", "LAUNCH", &message, Some(event.account_id))
            .await;
        self.launcher
            .webhook
            .notify(&config.discord_webhook, &message)
            .await;
        delay
    }

    async fn rejoin(&self, event: &ExitEvent) {
        let Ok(config) = self.launcher.accounts.config().await else {
            return;
        };
        // Turned off while waiting, or the account is busy with something else.
        if !config.auto_rejoin {
            return;
        }
        let target = self.launcher.rejoin_target(event.account_id);
        // A client a schedule started stays the schedule's after rejoining.
        let run = self.launcher.runs.run_of(&event.instance_id);
        match self
            .launcher
            .start(&[event.account_id], target, config.clone())
            .await
        {
            Ok(tasks) => {
                if let Some(run) = run {
                    self.launcher.runs.tag(&run, &tasks);
                }
            }
            Err(error) => {
                let message = format!("Rejoining {} failed: {error}", event.username);
                let _ = self
                    .launcher
                    .accounts
                    .log("ERROR", "LAUNCH", &message, Some(event.account_id))
                    .await;
                self.launcher
                    .webhook
                    .notify(&config.discord_webhook, &message)
                    .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejoins_slow_down_and_pause_after_too_many() {
        let delays: Vec<_> = (0..MAX_PER_HOUR + 1).map(rejoin_delay).collect();
        assert_eq!(delays[0], Some(Duration::from_secs(5)));
        assert!(delays.windows(2).all(|pair| match pair {
            [Some(a), Some(b)] => a <= b,
            [Some(_), None] => true,
            _ => false,
        }));
        assert_eq!(delays[MAX_PER_HOUR], None);
    }
}

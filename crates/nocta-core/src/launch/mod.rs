//! Game launcher: target resolution, launch precheck/solve, authentication ticket, PlaceLauncher URI dispatch and
//! owned instance tracking.
pub mod automation;
pub mod client;
pub mod client_care;
pub mod client_logs;
pub mod client_watch;
pub mod fps;
pub mod instances;
pub mod join;
pub mod multi;
pub mod native;
pub mod place_info;
pub mod placelauncher;
pub mod region;
pub mod rejoin;
pub mod resolver;
pub mod servers;
pub mod target;

pub use placelauncher::JoinIds;

use crate::roblox::{
    invite,
    web::{self, RobloxWeb},
};
use crate::{
    accounts::Accounts,
    models::*,
    secret::SecretString,
    sessions::{sleep, verify_identity, Sessions},
    CoreError, CoreResult,
};
use instances::{InstanceManager, LaunchRecord};
use place_info::PlaceInfos;
use resolver::InviteResolver;
use servers::{ListRequest, ServerLists};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
    time::Instant,
};
use target::{LaunchTarget, ResolvedKind, ResolvedTarget};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

pub struct Launcher {
    accounts: Accounts,
    sessions: Arc<Sessions>,
    pub web: Arc<RobloxWeb>,
    pub resolver: InviteResolver,
    pub servers: ServerLists,
    pub places: PlaceInfos,
    pub social: Arc<crate::roblox::social::Social>,
    pub account_tools: crate::roblox::account_tools::AccountTools,
    pub presence: crate::roblox::presence::Presence,
    pub frame_rate: fps::FrameRate,
    webhook: crate::webhook::Webhook,
    /// Last destination of each account, for auto-rejoin.
    destinations: Mutex<HashMap<u32, rejoin::Destination>>,
    /// Clients started by schedules, so "close after" closes only those.
    runs: automation::ScheduleRuns,
    pub(crate) client_logs: Arc<client_watch::ClientLogs>,
    /// The client each launch task started, for pacing the next account.
    task_instances: Mutex<HashMap<u32, String>>,
    pub instances: Arc<InstanceManager>,
    /// Nocta's own Roblox install, used when `managed_client` is on.
    pub client: Arc<client::ClientInstall>,
    tracker: TaskTracker,
    /// Last server list per request key, used to keep VIP rows on partial failures.
    last_lists: Mutex<HashMap<String, ServerListResponse>>,
    stop: CancellationToken,
    me: Weak<Launcher>,
}

/// Longest wait for a client to read the frame-rate cap before the user's value goes back anyway.
const READ_CAP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Precheck outcome. Generic 403 is never treated as ready.
enum Precheck {
    Ready,
    Expired,
}

impl Launcher {
    pub fn new(
        accounts: Accounts,
        sessions: Arc<Sessions>,
        web: Arc<RobloxWeb>,
        instances: Arc<InstanceManager>,
        client: Arc<client::ClientInstall>,
    ) -> Arc<Self> {
        let social = Arc::new(crate::roblox::social::Social::new(web.clone()));
        Arc::new_cyclic(|me| Self {
            account_tools: crate::roblox::account_tools::AccountTools::new(
                accounts.clone(),
                web.clone(),
            ),
            presence: crate::roblox::presence::Presence::new(accounts.clone(), social.clone()),
            // Tests never change the real Roblox settings of the computer they run on.
            frame_rate: fps::FrameRate::new(
                if cfg!(test) {
                    None
                } else {
                    fps::settings_dir()
                },
                client.state_file(".fps-cap-original"),
            ),
            social,
            resolver: InviteResolver::new(web.clone(), Some(sessions.browsers.clone())),
            servers: ServerLists::new(web.clone()),
            places: PlaceInfos::new(web.clone()),
            webhook: crate::webhook::Webhook::default(),
            destinations: Mutex::new(HashMap::new()),
            runs: automation::ScheduleRuns::default(),
            // Tests never read the real Roblox logs of the computer they run on.
            client_logs: client_watch::ClientLogs::new(if cfg!(test) {
                None
            } else {
                client_logs::logs_dir()
            }),
            task_instances: Mutex::new(HashMap::new()),
            accounts,
            sessions,
            web,
            instances,
            client,
            tracker: TaskTracker::new(),
            last_lists: Mutex::new(HashMap::new()),
            stop: CancellationToken::new(),
            me: me.clone(),
        })
    }

    pub async fn list_servers(
        &self,
        request_key: &str,
        place_id: &str,
        account_id: Option<u32>,
        include_vip: bool,
        options: &crate::models::ServerListOptions,
    ) -> CoreResult<ServerListResponse> {
        let _tracked = self.tracker.token();
        if self.stop.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let secrets = match account_id.filter(|_| include_vip) {
            Some(id) => Some(self.accounts.secrets(id).await?),
            None => None,
        };
        // Private rows from a different selected account must not be reused after a partial failure.
        let cache_key = format!("{request_key}|{place_id}|{account_id:?}|{include_vip}");
        let previous = self.last_lists.lock().unwrap().get(&cache_key).cloned();
        let operation = self.servers.list(ListRequest {
            request_key,
            place_id,
            vip_cookie: secrets
                .as_ref()
                .map(|secrets| secrets.cookie.as_str())
                .filter(|cookie| !cookie.is_empty()),
            include_vip,
            previous: previous.as_ref(),
            options,
        });
        tokio::pin!(operation);
        let response = tokio::select! {
            result = &mut operation => result?,
            _ = self.stop.cancelled() => {
                self.servers.cancel(request_key);
                let _ = operation.await;
                return Err(CoreError::Cancelled);
            }
        };
        if !response.cancelled {
            let mut lists = self.last_lists.lock().unwrap();
            if lists.len() > 64 {
                lists.clear();
            }
            lists.insert(cache_key, response.clone());
        }
        Ok(response)
    }

    /// Starts launch tasks. Accounts are launched one after another with the configured bulk interval.
    pub async fn start(
        self: &Arc<Self>,
        ids: &[u32],
        input: LaunchTargetInput,
        config: AppConfig,
    ) -> CoreResult<Vec<u32>> {
        if ids.is_empty() {
            return Err(CoreError::Validation("Select at least one account.".into()));
        }
        if let Some(target) = Self::input_target(&input) {
            // Explicit targets are validated before any task starts.
            target.resolve_explicit()?;
        }
        if let LaunchTargetInput::Server { target_id } = &input {
            self.servers.target(target_id)?;
        }
        if config.multi_instance {
            self.instances.ensure_multi_instance()?;
        }
        if config.managed_client {
            // Automatic mode installs on demand; other modes need an installed version.
            if config.client_update_mode != crate::models::ClientUpdateMode::Auto
                && self.client.active(&config).is_none()
            {
                return Err(CoreError::Validation(
                    "Nocta's Roblox client is not installed. Install it in Settings → Roblox client.".into(),
                ));
            }
        }
        for id in ids {
            self.accounts.get(*id).await?;
        }
        // Squad, ping, hop and follow targets become one exact server for every account.
        let (input, note) = self
            .settle_target(input, ids, &CancellationToken::new())
            .await?;
        if let Some(note) = &note {
            let _ = self.accounts.log("INFO", "LAUNCH", note, None).await;
        }
        let jobs = self.sessions.tasks.reserve(ids, TaskKind::LaunchGame)?;
        self.remember_destination(ids, &input);
        let task_ids = jobs.iter().map(|(id, _, _)| *id).collect();
        let service = self.clone();
        self.tracker.spawn(async move {
            let mut previous: Option<(u32, String)> = None;
            for (index, (task_id, account_id, cancel)) in jobs.into_iter().enumerate() {
                let paced = match &previous {
                    Some((_, instance)) if index > 0 && !cancel.is_cancelled() => {
                        service
                            .pace_after(task_id, instance, &config, &cancel)
                            .await
                    }
                    _ => false,
                };
                if !paced
                    && index > 0
                    && config.bulk_action_delay_secs > 0
                    && !cancel.is_cancelled()
                {
                    service.sessions.tasks.set(
                        task_id,
                        TaskStatus::Queued,
                        format!(
                            "Waiting {} s before launching (bulk interval).",
                            config.bulk_action_delay_secs
                        ),
                    );
                    let _ = sleep(config.bulk_action_delay_secs as u64, &cancel).await;
                }
                let result = if cancel.is_cancelled() {
                    Err(CoreError::Cancelled)
                } else {
                    service
                        .launch_account(task_id, account_id, &input, &config, &cancel)
                        .await
                };
                previous = service
                    .task_instances
                    .lock()
                    .unwrap()
                    .remove(&task_id)
                    .map(|instance| (account_id, instance));
                let cleanup = service.sessions.browsers.close(account_id).await;
                let result = match cleanup {
                    Ok(()) => result,
                    Err(error) => Err(error),
                };
                let (status, detail, level) = match result {
                    Ok(detail) => (TaskStatus::Succeeded, detail, "SUCCESS"),
                    Err(CoreError::Cancelled) => (
                        TaskStatus::Cancelled,
                        "Launch cancelled. Local cleanup completed.".into(),
                        "INFO",
                    ),
                    Err(error) => (TaskStatus::Failed, error.to_string(), "ERROR"),
                };
                if !service.sessions.tasks.cancelled_in_queue(task_id) {
                    let _ = service
                        .accounts
                        .log(level, "LAUNCH", &detail, Some(account_id))
                        .await;
                }
                service.sessions.finish_task(task_id, status, detail).await;
            }
        });
        Ok(task_ids)
    }

    async fn launch_account(
        &self,
        task_id: u32,
        account_id: u32,
        input: &LaunchTargetInput,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> CoreResult<String> {
        let started = Instant::now();
        let result = self
            .launch_inner(task_id, account_id, input, config, cancel)
            .await;
        if !matches!(result, Err(CoreError::Cancelled)) {
            let _ = self
                .accounts
                .record_operation(
                    "launch",
                    if result.is_ok() { "success" } else { "failed" },
                    started.elapsed().as_millis().min(u32::MAX as u128) as u32,
                    match result {
                        Err(CoreError::VerificationTimeout) => Some("verify_timeout"),
                        _ => None,
                    },
                )
                .await;
        }
        result
    }

    fn stage(&self, task_id: u32, detail: &str) {
        self.sessions
            .tasks
            .set(task_id, TaskStatus::Running, detail.to_owned());
    }

    /// Keeps `hold` until the client has read the cap (it is in its game, or gave up), then drops it.
    fn release_when_read(&self, instance: String, hold: fps::Hold) {
        let Some(launcher) = self.me.upgrade() else {
            return;
        };
        self.tracker.spawn(async move {
            let _ = launcher
                .wait_until_in_game(&instance, READ_CAP_TIMEOUT, &launcher.stop)
                .await;
            drop(hold);
        });
    }

    pub async fn test_webhook(&self, raw: &str) -> CoreResult<()> {
        self.webhook
            .send(
                raw,
                "Nocta is connected. Crashes and rejoins will be posted here.",
            )
            .await
    }

    pub async fn shutdown(&self) {
        self.stop.cancel();
        self.tracker.close();
        self.tracker.wait().await;
        self.instances.shutdown();
    }
}

mod pipeline;
mod targets;
#[cfg(test)]
mod tests;

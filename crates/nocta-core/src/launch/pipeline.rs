//! Launching one account: session cookie, join precheck, authentication ticket and handing the
//! URI to the Roblox client.
use super::*;

pub const CAPTCHA_REQUIRED: &str = "Roblox wants a captcha for this account before it can join. Open the account's browser, clear the captcha there and launch again.";

impl Launcher {
    /// Returns a usable cookie, running the visible refresh flow once when the stored session is invalid.
    pub(super) async fn active_cookie(
        &self,
        task_id: u32,
        account: &Account,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> CoreResult<zeroize::Zeroizing<String>> {
        let secrets = self.accounts.secrets(account.id).await?;
        if let Some(identity) = self
            .sessions
            .client
            .identity(&secrets.cookie, cancel)
            .await?
        {
            verify_identity(account, &identity)?;
            self.accounts
                .mark_cookie(account.id, true, Some(&identity.id.to_string()))
                .await?;
            return Ok(zeroize::Zeroizing::new(secrets.cookie.clone()));
        }
        self.accounts.mark_cookie(account.id, false, None).await?;
        self.resolver.invalidate_account(account.id);
        self.stage(
            task_id,
            "Session expired. Opening the login browser to refresh it.",
        );
        let cookie = self
            .sessions
            .refresh_cookie(task_id, account, &secrets, config, cancel)
            .await;
        self.sessions.browsers.close(account.id).await?;
        let cookie = cookie?;
        self.stage(task_id, "Session refreshed. Continuing the launch.");
        Ok(cookie)
    }

    pub(super) async fn launch_inner(
        &self,
        task_id: u32,
        account_id: u32,
        input: &LaunchTargetInput,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> CoreResult<String> {
        self.stage(task_id, "Preparing launch.");
        if config.pre_launch_delay_secs > 0 {
            self.stage(task_id, "Waiting for the configured pre-launch delay.");
            sleep(config.pre_launch_delay_secs.into(), cancel).await?;
        }
        self.accounts.mark_started(account_id).await?;
        if config.multi_instance {
            self.instances.ensure_multi_instance()?;
        }
        let account = self.accounts.get(account_id).await?;
        let mut cookie = self
            .active_cookie(task_id, &account, config, cancel)
            .await?;

        self.stage(task_id, "Resolving launch target.");
        let target = self
            .resolve(Some(account_id), Some(&cookie), input, config, cancel)
            .await?;
        let ids = JoinIds::generate();

        if config.launch_precheck {
            self.stage(task_id, "Running launch precheck.");
            let mut outcome = self
                .precheck(task_id, &account, &cookie, &target, &ids, config, cancel)
                .await?;
            if matches!(outcome, Precheck::Expired) {
                self.accounts.mark_cookie(account_id, false, None).await?;
                cookie = self
                    .active_cookie(task_id, &account, config, cancel)
                    .await?;
                outcome = self
                    .precheck(task_id, &account, &cookie, &target, &ids, config, cancel)
                    .await?;
                if matches!(outcome, Precheck::Expired) {
                    return Err(CoreError::Validation(
                        "Session expired again after refreshing. Launch aborted.".into(),
                    ));
                }
            }
        } else {
            let _ = self
                .accounts
                .log(
                    "INFO",
                    "LAUNCH",
                    "Launch precheck skipped by configuration.",
                    Some(account_id),
                )
                .await;
        }

        self.stage(task_id, "Requesting authentication ticket.");
        let (ticket, csrf) = self
            .ticket(task_id, &account, &mut cookie, &target, config, cancel)
            .await?;

        if target.mode() == LaunchMode::Private && config.auto_fix_private_invite_privacy {
            self.stage(task_id, "Checking private server invite privacy.");
            let (level, message) = match self
                .web
                .ensure_invite_privacy(&cookie, &csrf, &target.place_id, cancel)
                .await
            {
                Ok(web::PrivacyOutcome::AlreadyAllUsers) => (
                    "INFO",
                    "Private server invite privacy already allows all users.".to_owned(),
                ),
                Ok(web::PrivacyOutcome::Updated) => (
                    "INFO",
                    "Private server invite privacy was set to all users.".to_owned(),
                ),
                Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
                Err(error) => (
                    "WARNING",
                    format!("Private server invite privacy was not changed: {error}"),
                ),
            };
            let _ = self
                .accounts
                .log(level, "LAUNCH", &message, Some(account_id))
                .await;
        }

        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let client = if config.managed_client {
            self.stage(task_id, "Preparing Nocta's Roblox client.");
            Some(self.client.ready(config, cancel).await?)
        } else {
            None
        };
        // Roblox reads the cap from a file shared with the user's own clients, so it is there only
        // until this client has read it.
        let frame_rate = match self.frame_rate.hold(config) {
            Ok(hold) => hold,
            Err(error) => {
                let _ = self
                    .accounts
                    .log(
                        "WARNING",
                        "LAUNCH",
                        &format!("The frame-rate cap was not changed: {error}"),
                        Some(account_id),
                    )
                    .await;
                None
            }
        };
        self.stage(task_id, "Handing the launch link to Roblox.");
        let uri = placelauncher::player_uri(
            &target,
            &ticket,
            &ids,
            chrono::Utc::now().timestamp_millis(),
        );
        drop(ticket);
        // The client's log is created after this moment.
        let dispatched_at = std::time::SystemTime::now() - std::time::Duration::from_secs(2);
        let instance = self
            .instances
            .launch(
                LaunchRecord {
                    account_id,
                    username: &account.username,
                    place_id: &target.place_id,
                    mode: target.mode(),
                },
                uri,
                client.as_deref(),
                cancel,
            )
            .await?;
        self.runs.adopt(task_id, &instance.id);
        self.client_logs.track(&instance.id, dispatched_at);
        if let Some(hold) = frame_rate {
            self.release_when_read(instance.id.clone(), hold);
        }
        self.task_instances
            .lock()
            .unwrap()
            .insert(task_id, instance.id.clone());
        if config.afk_mode && config.afk_minimize {
            if let Some(pid) = instance.pid {
                self.instances.minimize_when_shown(pid);
            }
        }
        let mode = match target.mode() {
            LaunchMode::Public => "public",
            LaunchMode::Job => "server",
            LaunchMode::Private => "private server",
        };
        let fallback = if target.used_fallback {
            " (configured place fallback)"
        } else {
            ""
        };
        Ok(match instance.status {
            InstanceStatus::Running => format!(
                "Launch dispatched ({mode}, place {}){fallback}. Roblox client is running.",
                target.place_id
            ),
            InstanceStatus::Starting => format!(
                "Launch dispatched ({mode}, place {}){fallback}. Roblox launcher is starting the client.",
                target.place_id
            ),
            _ => format!(
                "Launch dispatched ({mode}, place {}){fallback}. No client process was confirmed yet.",
                target.place_id
            ),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn precheck(
        &self,
        task_id: u32,
        account: &Account,
        cookie: &str,
        target: &ResolvedTarget,
        ids: &JoinIds,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> CoreResult<Precheck> {
        let mut probe = Err(CoreError::Network);
        for attempt in 0..=config.precheck_retries.min(10) {
            probe = self
                .sessions
                .client
                .check_join(cookie, &target.place_id, Some(ids), cancel)
                .await;
            if probe.is_ok() || matches!(probe, Err(CoreError::Cancelled)) {
                break;
            }
            if attempt < config.precheck_retries.min(10) {
                self.stage(task_id, "Ambiguous launch precheck. Waiting to retry.");
                sleep(5, cancel).await?;
            }
        }
        let probe = probe?;
        match probe.health {
            AccountHealth::Healthy => {
                self.accounts
                    .mark_health(account.id, AccountHealth::Healthy, None)
                    .await?;
                Ok(Precheck::Ready)
            }
            AccountHealth::InvalidSession => Ok(Precheck::Expired),
            AccountHealth::CaptchaRequired => {
                self.accounts
                    .mark_health(account.id, AccountHealth::CaptchaRequired, None)
                    .await?;
                // Captchas are not solved here: the account's own browser (Open browser) is where
                // the user clears it.
                Err(CoreError::Validation(CAPTCHA_REQUIRED.into()))
            }
            _ => Err(CoreError::Validation("The launch precheck failed.".into())),
        }
    }

    /// At most two ticket attempts. A missing ticket re-validates (and if needed refreshes) the session once.
    pub(super) async fn ticket(
        &self,
        task_id: u32,
        account: &Account,
        cookie: &mut zeroize::Zeroizing<String>,
        target: &ResolvedTarget,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> CoreResult<(SecretString, SecretString)> {
        let first = self
            .web
            .authentication_ticket(cookie, &target.place_id, None, cancel)
            .await?;
        if let (Some(ticket), Some(csrf)) = (first.ticket.clone(), first.csrf.clone()) {
            return Ok((ticket, csrf));
        }
        let _ = self
            .accounts
            .log(
                "WARNING",
                "LAUNCH",
                "Authentication ticket missing; validating the session and retrying once.",
                Some(account.id),
            )
            .await;
        if first.unauthorized {
            self.accounts.mark_cookie(account.id, false, None).await?;
        }
        *cookie = self.active_cookie(task_id, account, config, cancel).await?;
        self.stage(task_id, "Retrying authentication ticket.");
        let second = self
            .web
            .authentication_ticket(cookie, &target.place_id, first.csrf.as_ref(), cancel)
            .await?;
        match (second.ticket, second.csrf) {
            (Some(ticket), Some(csrf)) => Ok((ticket, csrf)),
            _ => Err(CoreError::Validation(
                "Roblox did not issue an authentication ticket. Try again later.".into(),
            )),
        }
    }
}

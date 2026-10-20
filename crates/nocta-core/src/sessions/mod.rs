pub mod http;
use crate::{
    accounts::Accounts,
    browser::{BrowserManager, BrowserOptions, BrowserPurpose},
    models::*,
    tasks::{ConcurrencyGate, Tasks},
    CoreError, CoreResult,
};
use http::RobloxClient;
use std::{sync::Arc, time::Duration};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

pub struct Sessions {
    pub tasks: Arc<Tasks>,
    pub browsers: Arc<BrowserManager>,
    pub client: Arc<RobloxClient>,
    accounts: Accounts,
    tracker: TaskTracker,
    gate: Arc<ConcurrencyGate>,
}

impl Sessions {
    pub fn new(
        accounts: Accounts,
        browsers: Arc<BrowserManager>,
        config: &AppConfig,
    ) -> CoreResult<Arc<Self>> {
        Ok(Arc::new(Self {
            accounts,
            client: Arc::new(RobloxClient::new()?),
            tasks: Arc::new(Tasks::new(browsers.quarantine.clone())),
            tracker: TaskTracker::new(),
            gate: ConcurrencyGate::new(config.checker_concurrency),
            browsers,
        }))
    }
    #[cfg(test)]
    pub(crate) fn with_client(base: &Arc<Self>, client: RobloxClient) -> Self {
        Self {
            tasks: base.tasks.clone(),
            browsers: base.browsers.clone(),
            client: Arc::new(client),
            accounts: base.accounts.clone(),
            tracker: TaskTracker::new(),
            gate: base.gate.clone(),
        }
    }
    pub fn configure(&self, config: &AppConfig) {
        self.gate.set_limit(config.checker_concurrency);
    }
    pub(crate) async fn finish_task(
        &self,
        id: u32,
        status: TaskStatus,
        detail: String,
    ) -> Option<AccountTask> {
        let quiet = self.tasks.cancelled_in_queue(id);
        let task = self.tasks.finish(id, status, detail);
        if let Some(task) = &task {
            if !quiet && self.accounts.record_task(task).await.is_err() {
                tracing::warn!(
                    account_id = task.account_id,
                    "Could not persist the last operation metadata"
                );
            }
            self.tasks.release_finished(task);
        }
        self.tasks.changed.notify_one();
        task
    }
    async fn utility_browser(
        &self,
        account_id: u32,
        purpose: BrowserPurpose,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> CoreResult<Arc<crate::browser::BrowserSession>> {
        let browser = self
            .browsers
            .start_with_options(
                account_id,
                purpose,
                BrowserOptions {
                    visible: true,
                    app_mode: config.browser_app_mode,
                    ..Default::default()
                },
                cancel,
            )
            .await?;
        Ok(browser)
    }
    pub async fn start(
        self: &Arc<Self>,
        ids: &[u32],
        kind: TaskKind,
        config: AppConfig,
    ) -> CoreResult<Vec<u32>> {
        if !matches!(
            kind,
            TaskKind::CheckHealth
                | TaskKind::ValidateCookie
                | TaskKind::OpenBrowser
                | TaskKind::RefreshCookie
        ) {
            return Err(CoreError::Validation(
                "Unsupported session operation.".into(),
            ));
        }
        if ids.is_empty() {
            return Err(CoreError::Validation("Select at least one account.".into()));
        }
        if kind == TaskKind::CheckHealth
            && config
                .place_id
                .parse::<u64>()
                .ok()
                .filter(|id| *id > 0)
                .is_none()
        {
            return Err(CoreError::Validation(
                "Enter a valid Place ID before checking account health.".into(),
            ));
        }
        for id in ids {
            self.accounts.get(*id).await?;
        }
        let jobs = self.tasks.reserve(ids, kind)?;
        let task_ids = jobs.iter().map(|(id, _, _)| *id).collect();
        let cadence = Arc::new(tokio::sync::Mutex::new(tokio::time::Instant::now()));
        for (id, account_id, cancel) in jobs {
            let service = self.clone();
            let config = config.clone();
            let cadence = cadence.clone();
            self.tracker.spawn(async move {
                let result = async {
                    if matches!(kind, TaskKind::OpenBrowser | TaskKind::RefreshCookie) {
                        let mut next = tokio::select! { _ = cancel.cancelled() => return Err(CoreError::Cancelled), next = cadence.lock() => next };
                        service.tasks.set(id, TaskStatus::Queued, "Waiting for the bulk account interval.");
                        tokio::select! { _ = cancel.cancelled() => return Err(CoreError::Cancelled), _ = tokio::time::sleep_until(*next) => {} }
                        *next = tokio::time::Instant::now() + Duration::from_secs(config.bulk_action_delay_secs.into());
                    }
                    service.run(id, account_id, kind, &config, &cancel).await
                }.await;
                // Browser cleanup must finish before releasing the account reservation.
                let cleanup = service.browsers.close(account_id).await;
                let result = match cleanup {
                    Ok(()) => result,
                    Err(error) => Err(error),
                };
                let (status, detail, level) = match result {
                    Ok(detail) => (TaskStatus::Succeeded, detail, "SUCCESS"),
                    Err(CoreError::Cancelled) => (
                        TaskStatus::Cancelled,
                        "Cancelled. Local cleanup completed.".into(),
                        "INFO",
                    ),
                    Err(error) => (TaskStatus::Failed, error.to_string(), "ERROR"),
                };
                if !service.tasks.cancelled_in_queue(id) {
                    let _ = service
                        .accounts
                        .log(level, "SESSIONS", &detail, Some(account_id))
                        .await;
                }
                service.finish_task(id, status, detail).await;
            });
        }
        Ok(task_ids)
    }
    pub(crate) async fn run(
        &self,
        id: u32,
        account_id: u32,
        kind: TaskKind,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> CoreResult<String> {
        let started = std::time::Instant::now();
        let result = self.run_inner(id, account_id, kind, config, cancel).await;
        if kind == TaskKind::CheckHealth && !matches!(result, Err(CoreError::Cancelled)) {
            let health = self
                .accounts
                .get(account_id)
                .await
                .ok()
                .map(|account| account.health);
            let outcome = if result.is_err() {
                "failed"
            } else {
                match health {
                    Some(AccountHealth::Healthy) => "healthy",
                    Some(AccountHealth::CaptchaRequired) => "captcha",
                    Some(AccountHealth::InvalidSession) => "invalid",
                    _ => "failed",
                }
            };
            let error = match result {
                Err(CoreError::Timeout) => Some("check_timeout"),
                Err(_) => Some("ambiguous_precheck"),
                _ => None,
            };
            let _ = self
                .accounts
                .record_operation(
                    "check",
                    outcome,
                    started.elapsed().as_millis().min(u32::MAX as u128) as u32,
                    error,
                )
                .await;
        }
        result
    }
    async fn run_inner(
        &self,
        id: u32,
        account_id: u32,
        kind: TaskKind,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> CoreResult<String> {
        let permit = self.gate.acquire(cancel).await?;
        if config.checker_delay_secs > 0 {
            sleep(config.checker_delay_secs as u64, cancel).await?;
        }
        self.tasks
            .set(id, TaskStatus::Running, "Starting account operation.");
        let account = self.accounts.get(account_id).await?;
        let secrets = self.accounts.secrets(account_id).await?;
        if matches!(kind, TaskKind::CheckHealth | TaskKind::ValidateCookie) {
            let identity = self.client.identity(&secrets.cookie, cancel).await?;
            let Some(identity) = identity else {
                self.accounts.mark_cookie(account_id, false, None).await?;
                if kind == TaskKind::CheckHealth {
                    self.accounts
                        .mark_health(account_id, AccountHealth::InvalidSession, None)
                        .await?;
                }
                return Ok("The account session is invalid or expired.".into());
            };
            verify_identity(&account, &identity)?;
            let user_id = identity.id.to_string();
            self.accounts
                .mark_cookie(account_id, true, Some(&user_id))
                .await?;
            if kind == TaskKind::ValidateCookie {
                return Ok("Session cookie validated.".into());
            }
            self.tasks
                .set(id, TaskStatus::Running, "Checking game-join challenge.");
            let mut result = Err(CoreError::Network);
            for attempt in 0..=config.precheck_retries.min(100) {
                result = self
                    .client
                    .check(&secrets.cookie, &config.place_id, cancel)
                    .await;
                if result.is_ok() || matches!(result, Err(CoreError::Cancelled)) {
                    break;
                }
                if attempt < config.precheck_retries {
                    self.tasks.set(
                        id,
                        TaskStatus::Running,
                        "Ambiguous precheck. Waiting to retry.",
                    );
                    sleep(5, cancel).await?;
                }
            }
            match result {
                Ok(probe) => {
                    let detail = match probe.health {
                        AccountHealth::Healthy => "Join precheck passed. No challenge required.",
                        AccountHealth::CaptchaRequired => "Captcha challenge detected.",
                        AccountHealth::InvalidSession => "Session expired during precheck.",
                        _ => "Account check finished.",
                    };
                    self.accounts
                        .mark_health(account_id, probe.health, Some(&user_id))
                        .await?;
                    Ok(detail.into())
                }
                Err(error) => {
                    if !matches!(error, CoreError::Cancelled) {
                        self.accounts
                            .mark_health(account_id, AccountHealth::Error, Some(&user_id))
                            .await?;
                    }
                    Err(error)
                }
            }
        } else {
            self.accounts.mark_started(account_id).await?;
            if kind == TaskKind::RefreshCookie {
                drop(permit);
                self.refresh_cookie(id, &account, &secrets, config, cancel)
                    .await?;
                return Ok("A validated session cookie was captured and encrypted.".into());
            }
            let browser = self
                .utility_browser(account_id, BrowserPurpose::Manual, config, cancel)
                .await?;
            // The window shows whose browser it is, like the account's game windows.
            let _title = if config.name_windows {
                let name = crate::launch::client_care::window_name(
                    account.id,
                    &account.alias,
                    &account.username,
                    config.streamer_mode,
                );
                let pid = browser.process_id().await;
                Some(crate::launch::native::windows_title::keep(pid, &name))
            } else {
                None
            };
            if !secrets.cookie.is_empty() {
                browser.set_roblox_cookie(&secrets.cookie, cancel).await?;
            }
            browser
                .navigate("https://www.roblox.com/home", cancel)
                .await?;
            drop(permit);
            self.tasks.set(
                id,
                TaskStatus::WaitingUser,
                "Account browser is open. Close its window when finished.",
            );
            loop {
                if !browser.running().await? {
                    return Ok("Account browser closed.".into());
                }
                sleep(1, cancel).await?;
            }
        }
    }

    /// Visible login flow: navigates to the Roblox login page, autofills and submits only on the trusted
    /// `https://www.roblox.com` origin, leaves 2FA/verification to the user, and returns a cookie only after it is
    /// independently validated and stored. The caller owns browser cleanup.
    pub(crate) async fn refresh_cookie(
        &self,
        id: u32,
        account: &Account,
        secrets: &AccountSecrets,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> CoreResult<zeroize::Zeroizing<String>> {
        let browser = match self
            .utility_browser(account.id, BrowserPurpose::Task, config, cancel)
            .await
        {
            Ok(browser) => browser,
            Err(CoreError::Busy) => {
                return Err(CoreError::Validation(
                    "Another browser is open for this account. Close it and try again.".into(),
                ))
            }
            Err(error) => return Err(error),
        };
        browser
            .navigate("https://www.roblox.com/login", cancel)
            .await?;
        self.tasks.set(
            id,
            TaskStatus::WaitingUser,
            "Complete login and any verification in the account browser.",
        );
        let can_submit = !account.username.is_empty() && !secrets.password.is_empty();
        let script = |submit: bool| -> CoreResult<zeroize::Zeroizing<String>> {
            Ok(zeroize::Zeroizing::new(login_script(
                &account.username,
                &secrets.password,
                submit,
            )?))
        };
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(config.browser_timeout_secs as u64);
        let mut filled = false;
        let mut submitted = false;
        let mut verification_hint = false;
        loop {
            if cancel.is_cancelled() {
                return Err(CoreError::Cancelled);
            }
            if !browser.running().await? {
                return Err(CoreError::Browser(
                    "Login browser was closed before a valid session was captured.".into(),
                ));
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(CoreError::Timeout);
            }
            if !submitted && !secrets.password.is_empty() {
                let submit = filled && can_submit;
                let state = browser
                    .evaluate(&script(submit)?, cancel)
                    .await
                    .unwrap_or(serde_json::Value::Null);
                match state.as_str() {
                    Some("filled") => filled = true,
                    Some("submitted") => {
                        submitted = true;
                        self.tasks.set(
                            id,
                            TaskStatus::WaitingUser,
                            "Login submitted. Complete two-step verification or any challenge in the browser.",
                        );
                    }
                    _ => {}
                }
            } else if submitted && !verification_hint {
                let two_step = browser
                    .evaluate(TWO_STEP_SCRIPT, cancel)
                    .await
                    .unwrap_or(serde_json::Value::Bool(false));
                if two_step == true {
                    verification_hint = true;
                    self.tasks.set(
                        id,
                        TaskStatus::WaitingUser,
                        "Two-step verification required. Enter the code in the account browser.",
                    );
                }
            }
            if let Some(cookie) = browser.roblox_cookie(cancel).await? {
                if let Some(identity) = self.client.identity(&cookie, cancel).await? {
                    verify_identity(account, &identity)?;
                    self.accounts
                        .replace_cookie(account.id, &cookie, &identity.id.to_string())
                        .await?;
                    return Ok(cookie);
                }
            }
            sleep(2, cancel).await?;
        }
    }
    /// Adds an account by signing in on Roblox's own login page in a fresh browser, so captchas,
    /// two-step verification and passkeys work as usual and the password never passes through
    /// Nocta. The session is saved as a new account, or replaces the cookie of the account with
    /// that username. Closing the browser cancels.
    pub async fn sign_in_new(
        &self,
        folder_id: Option<u32>,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> CoreResult<u32> {
        // Account IDs start at 1, so 0 is free for the sign-in browser.
        const SIGN_IN: u32 = 0;
        let browser = self
            .utility_browser(SIGN_IN, BrowserPurpose::Task, config, cancel)
            .await?;
        let result = async {
            browser
                .navigate("https://www.roblox.com/login", cancel)
                .await?;
            let deadline = tokio::time::Instant::now() + Duration::from_secs(15 * 60);
            loop {
                if !browser.running().await? {
                    return Err(CoreError::Validation(
                        "The sign-in window was closed before signing in.".into(),
                    ));
                }
                if tokio::time::Instant::now() >= deadline {
                    return Err(CoreError::Timeout);
                }
                if let Some(cookie) = browser.roblox_cookie(cancel).await? {
                    if let Some(identity) = self.client.identity(&cookie, cancel).await? {
                        return self.save_signed_in(&identity, &cookie, folder_id).await;
                    }
                }
                sleep(2, cancel).await?;
            }
        }
        .await;
        let _ = self.browsers.close(SIGN_IN).await;
        result
    }

    pub(crate) async fn save_signed_in(
        &self,
        identity: &http::Identity,
        cookie: &str,
        folder_id: Option<u32>,
    ) -> CoreResult<u32> {
        let find = |accounts: Vec<Account>| {
            accounts
                .into_iter()
                .find(|account| account.username.eq_ignore_ascii_case(&identity.name))
                .map(|account| account.id)
        };
        if let Some(id) = find(self.accounts.list().await?) {
            self.accounts
                .replace_cookie(id, cookie, &identity.id.to_string())
                .await?;
            return Ok(id);
        }
        let line = zeroize::Zeroizing::new(format!("{}::{cookie}", identity.name));
        self.accounts
            .import(line.to_string(), folder_id, false)
            .await?;
        let id = find(self.accounts.list().await?).ok_or(CoreError::NotFound)?;
        self.accounts
            .replace_cookie(id, cookie, &identity.id.to_string())
            .await?;
        Ok(id)
    }

    pub async fn shutdown(&self) {
        self.tasks.shutdown();
        self.tracker.close();
        self.tracker.wait().await;
        if let Err(error) = self.browsers.close_all().await {
            tracing::warn!(%error, "Browser cleanup failed");
        }
    }
}

const TWO_STEP_SCRIPT: &str = r#"(() => location.origin === 'https://www.roblox.com' && !!document.querySelector('#two-step-verification-code-input, input[autocomplete="one-time-code"], [class*="two-step"], [class*="TwoStep"]'))()"#;

/// Autofill (and optionally submit) only matching login inputs on the trusted Roblox origin.
fn login_script(username: &str, password: &str, submit: bool) -> CoreResult<String> {
    let username = serde_json::to_string(username)?;
    let password = zeroize::Zeroizing::new(serde_json::to_string(password)?);
    Ok(format!(
        r#"(() => {{ if (location.origin !== 'https://www.roblox.com') return 'origin'; const u = document.querySelector('#login-username, input[name="username"]'); const p = document.querySelector('#login-password, input[name="password"]'); if (!u || !p) return 'missing'; const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set; if (u.value !== {username}) {{ setter.call(u, {username}); u.dispatchEvent(new Event('input', {{bubbles:true}})); }} if (!p.value) {{ setter.call(p, {password}); p.dispatchEvent(new Event('input', {{bubbles:true}})); }} if (!{submit}) return 'filled'; const b = document.querySelector('#login-button') || (p.form && p.form.querySelector('button[type="submit"]')); if (!b || b.disabled) return 'filled'; b.click(); return 'submitted'; }})()"#,
        password = password.as_str()
    ))
}

pub(crate) fn verify_identity(account: &Account, identity: &http::Identity) -> CoreResult<()> {
    if let Some(user_id) = &account.user_id {
        if user_id != &identity.id.to_string() {
            return Err(CoreError::Validation(
                "The cookie belongs to a different Roblox account.".into(),
            ));
        }
    } else if !account.username.eq_ignore_ascii_case(&identity.name) {
        return Err(CoreError::Validation(
            "The cookie username does not match this account.".into(),
        ));
    }
    Ok(())
}
pub async fn sleep(seconds: u64, cancel: &CancellationToken) -> CoreResult<()> {
    tokio::select! { biased; _ = cancel.cancelled() => Err(CoreError::Cancelled), _ = tokio::time::sleep(Duration::from_secs(seconds)) => Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_script_is_origin_gated_and_escapes_credentials() {
        let script = login_script("Ali\"ce", "p'w\\x</script>", true).unwrap();
        assert!(script.starts_with(
            "(() => { if (location.origin !== 'https://www.roblox.com') return 'origin';"
        ));
        assert!(script.contains(r#""Ali\"ce""#));
        assert!(script.contains("if (!true) return 'filled'"));
        assert!(login_script("a", "b", false)
            .unwrap()
            .contains("if (!false) return 'filled'"));
        assert!(TWO_STEP_SCRIPT.contains("location.origin === 'https://www.roblox.com'"));
    }
}

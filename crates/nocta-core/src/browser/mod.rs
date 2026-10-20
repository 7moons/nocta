use std::path::{Path, PathBuf};
pub mod cdp;
mod process;
use crate::{CoreError, CoreResult};
use process::OwnedProcess;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BrowserPurpose {
    Manual,
    Task,
}
#[derive(Default)]
pub struct BrowserOptions {
    pub visible: bool,
    pub hidden: bool,
    pub app_mode: bool,
    pub bounds: Option<(i32, i32, u32, u32)>,
    pub offline: bool,
}

pub struct BrowserSession {
    pub account_id: u32,
    pub purpose: BrowserPurpose,
    pub client: cdp::Cdp,
    pub page: String,
    process: Mutex<OwnedProcess>,
    profile: PathBuf,
}

impl BrowserSession {
    pub async fn navigate(&self, url: &str, cancel: &CancellationToken) -> CoreResult<()> {
        let response = self
            .client
            .call(
                "Page.navigate",
                json!({"url": url}),
                Some(&self.page),
                cancel,
            )
            .await?;
        if response.get("errorText").is_some() {
            return Err(CoreError::Browser("The page could not be opened.".into()));
        }
        Ok(())
    }
    pub async fn set_roblox_cookie(
        &self,
        cookie: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<()> {
        let response = self.client.call("Network.setCookie", json!({"name": ".ROBLOSECURITY", "value": cookie, "domain": ".roblox.com", "path": "/", "secure": true, "httpOnly": true}), Some(&self.page), cancel).await?;
        if response["success"] != true {
            return Err(CoreError::Browser(
                "The session cookie could not be set.".into(),
            ));
        }
        Ok(())
    }
    pub async fn roblox_cookie(
        &self,
        cancel: &CancellationToken,
    ) -> CoreResult<Option<zeroize::Zeroizing<String>>> {
        let result = self
            .client
            .call(
                "Network.getCookies",
                json!({"urls": ["https://www.roblox.com/"]}),
                Some(&self.page),
                cancel,
            )
            .await?;
        Ok(result["cookies"].as_array().and_then(|cookies| {
            cookies
                .iter()
                .find(|cookie| cookie["name"] == ".ROBLOSECURITY")
                .and_then(|cookie| cookie["value"].as_str())
                .map(|value| zeroize::Zeroizing::new(value.to_owned()))
        }))
    }
    pub async fn evaluate(
        &self,
        expression: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<Value> {
        let result = self
            .client
            .call(
                "Runtime.evaluate",
                json!({"expression": expression, "returnByValue": true, "awaitPromise": true}),
                Some(&self.page),
                cancel,
            )
            .await?;
        if result.get("exceptionDetails").is_some() {
            return Err(CoreError::Browser(
                "The browser operation could not complete.".into(),
            ));
        }
        Ok(result["result"]["value"].clone())
    }
    pub async fn running(&self) -> CoreResult<bool> {
        self.process.lock().await.running()
    }
    pub async fn process_id(&self) -> u32 {
        self.process.lock().await.id()
    }
    pub async fn close(&self) -> CoreResult<()> {
        let _ = self
            .client
            .call_timeout(
                "Browser.close",
                json!({}),
                None,
                &CancellationToken::new(),
                Duration::from_secs(1),
            )
            .await;
        let result = self.process.lock().await.shutdown().await;
        if result.is_ok() {
            for _ in 0..20 {
                match tokio::fs::remove_dir_all(&self.profile).await {
                    Ok(()) => return Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                    Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
                }
            }
            tracing::warn!(
                "A closed browser profile could not be removed; startup cleanup will retry."
            );
            return Err(CoreError::Browser(
                "Browser profile cleanup failed; retrying is required.".into(),
            ));
        }
        result
    }
}

/// Accounts whose browser process or profile cleanup failed. New operations on them are refused until a
/// retry succeeds, so a lease is never handed out while a previous browser may still be alive.
#[derive(Default)]
pub struct Quarantine(std::sync::Mutex<HashSet<u32>>);
impl Quarantine {
    pub fn insert(&self, account_id: u32) {
        self.0.lock().unwrap().insert(account_id);
    }
    pub fn remove(&self, account_id: u32) {
        self.0.lock().unwrap().remove(&account_id);
    }
    pub fn contains_any(&self, ids: &[u32]) -> bool {
        let set = self.0.lock().unwrap();
        ids.iter().any(|id| set.contains(id))
    }
    pub fn list(&self) -> Vec<u32> {
        let mut ids = self.0.lock().unwrap().iter().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        ids
    }
}

pub struct BrowserManager {
    executable: Option<PathBuf>,
    profiles: PathBuf,
    sessions: Mutex<BTreeMap<u32, Arc<BrowserSession>>>,
    pending: Mutex<BTreeMap<u32, PendingBrowser>>,
    pub quarantine: Arc<Quarantine>,
}

struct PendingBrowser {
    process: Option<OwnedProcess>,
    profile: PathBuf,
}
impl PendingBrowser {
    async fn close(&mut self) -> CoreResult<()> {
        if let Some(process) = self.process.as_mut() {
            process.shutdown().await?;
        }
        match tokio::fs::remove_dir_all(&self.profile).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

async fn create_private_directory(path: &Path) -> CoreResult<()> {
    tokio::fs::create_dir_all(path).await?;
    Ok(())
}

impl BrowserManager {
    pub fn new(executable: Option<PathBuf>, profiles: PathBuf) -> Self {
        Self {
            executable,
            profiles,
            sessions: Mutex::new(BTreeMap::new()),
            pending: Mutex::new(BTreeMap::new()),
            quarantine: Arc::new(Quarantine::default()),
        }
    }
    pub async fn sweep(&self) -> CoreResult<()> {
        create_private_directory(&self.profiles).await?;
        let mut entries = tokio::fs::read_dir(&self.profiles).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir()
                && uuid::Uuid::parse_str(&entry.file_name().to_string_lossy()).is_ok()
            {
                // Directory names are generated by this manager; never follow symlinks.
                let _ = tokio::fs::remove_dir_all(entry.path()).await;
            }
        }
        Ok(())
    }
    pub async fn start(
        &self,
        account_id: u32,
        purpose: BrowserPurpose,
        visible: bool,
        cancel: &CancellationToken,
    ) -> CoreResult<Arc<BrowserSession>> {
        self.start_with_options(
            account_id,
            purpose,
            BrowserOptions {
                visible,
                ..Default::default()
            },
            cancel,
        )
        .await
    }
    pub async fn start_with_options(
        &self,
        account_id: u32,
        purpose: BrowserPurpose,
        options: BrowserOptions,
        cancel: &CancellationToken,
    ) -> CoreResult<Arc<BrowserSession>> {
        let executable = self.executable.as_deref().ok_or_else(|| {
            CoreError::Browser(
                "Managed Chromium is not installed. Stage the app browser runtime first.".into(),
            )
        })?;
        let mut sessions = self.sessions.lock().await;
        if sessions.contains_key(&account_id) || self.quarantine.contains_any(&[account_id]) {
            return Err(CoreError::Busy);
        }
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        create_private_directory(&self.profiles).await?;
        let profile = self.profiles.join(uuid::Uuid::new_v4().to_string());
        create_private_directory(&profile).await?;
        let mut args = vec![
            "--remote-debugging-port=0".into(),
            "--no-first-run".into(),
            "--no-default-browser-check".into(),
            "--disable-background-networking".into(),
            "--window-size=1280,800".into(),
            format!("--user-data-dir={}", profile.display()),
        ];
        if !options.visible {
            args.push("--headless=new".into());
        }
        if options.offline {
            args.push("--host-resolver-rules=MAP * 127.0.0.1,EXCLUDE localhost".into());
        }
        if options.hidden {
            args.push("--window-position=-32000,-32000".into());
            args.push("--start-minimized".into());
        } else if let Some((x, y, width, height)) = options.bounds {
            args.push(format!("--window-position={x},{y}"));
            args.push(format!("--window-size={width},{height}"));
        }
        args.push(if options.app_mode {
            "--app=about:blank".into()
        } else {
            "about:blank".into()
        });
        let mut process =
            match OwnedProcess::spawn(executable, &args, options.visible && !options.hidden) {
                Ok(process) => process,
                Err(error) => {
                    self.clean_startup(
                        account_id,
                        PendingBrowser {
                            process: None,
                            profile,
                        },
                    )
                    .await?;
                    return Err(error);
                }
            };
        let setup = async {
            let active_port = profile.join("DevToolsActivePort");
            let port = tokio::time::timeout(Duration::from_secs(20), async {
                loop {
                    if !process.running()? {
                        return Err(CoreError::Browser("Chromium exited during startup.".into()));
                    }
                    if let Ok(contents) = tokio::fs::read_to_string(&active_port).await {
                        if let Some(port) = contents
                            .lines()
                            .next()
                            .and_then(|line| line.parse::<u16>().ok())
                        {
                            return Ok(port);
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            })
            .await
            .map_err(|_| CoreError::Timeout)??;
            let http = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|_| CoreError::Network)?;
            let version: Value = http
                .get(format!("http://127.0.0.1:{port}/json/version"))
                .send()
                .await
                .map_err(|_| CoreError::Browser("Cannot reach the managed browser.".into()))?
                .json()
                .await
                .map_err(|_| CoreError::Browser("Invalid browser endpoint.".into()))?;
            let url = version["webSocketDebuggerUrl"]
                .as_str()
                .ok_or_else(|| CoreError::Browser("Missing browser endpoint.".into()))?;
            let client = cdp::Cdp::connect(url, cancel).await?;
            // The debugging endpoint can become ready before Chrome creates its initial page.
            let target = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let targets = client
                        .call("Target.getTargets", json!({}), None, cancel)
                        .await?;
                    if let Some(target) = targets["targetInfos"]
                        .as_array()
                        .and_then(|targets| targets.iter().find(|target| target["type"] == "page"))
                        .and_then(|target| target["targetId"].as_str())
                    {
                        return Ok::<_, CoreError>(target.to_owned());
                    }
                    tokio::select! {
                        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                        _ = tokio::time::sleep(Duration::from_millis(50)) => {}
                    }
                }
            })
            .await
            .map_err(|_| CoreError::Timeout)??;
            let attached = client
                .call(
                    "Target.attachToTarget",
                    json!({"targetId": target, "flatten": true}),
                    None,
                    cancel,
                )
                .await?;
            let page = attached["sessionId"]
                .as_str()
                .ok_or_else(|| CoreError::Browser("Cannot attach to browser page.".into()))?
                .to_owned();
            for command in ["Page.enable", "Network.enable", "Runtime.enable"] {
                client.call(command, json!({}), Some(&page), cancel).await?;
            }
            Ok::<_, CoreError>((client, page))
        };
        let result = tokio::select! { biased; _ = cancel.cancelled() => Err(CoreError::Cancelled), result = setup => result };
        match result {
            Ok((client, page)) => {
                let session = Arc::new(BrowserSession {
                    account_id,
                    purpose,
                    client,
                    page,
                    process: Mutex::new(process),
                    profile,
                });
                sessions.insert(account_id, session.clone());
                Ok(session)
            }
            Err(error) => {
                self.clean_startup(
                    account_id,
                    PendingBrowser {
                        process: Some(process),
                        profile,
                    },
                )
                .await?;
                Err(error)
            }
        }
    }
    async fn clean_startup(&self, account_id: u32, mut browser: PendingBrowser) -> CoreResult<()> {
        if browser.close().await.is_err() {
            self.pending.lock().await.insert(account_id, browser);
            self.quarantine.insert(account_id);
            return Err(CoreError::Browser("Browser startup cleanup is pending. The account is blocked until cleanup succeeds.".into()));
        }
        Ok(())
    }
    pub async fn close(&self, account_id: u32) -> CoreResult<()> {
        // Keep the entry reserved until its process tree and profile are cleaned up.
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(&account_id) {
            if let Err(error) = session.close().await {
                // The session stays registered, so `start` refuses this account, and the quarantine blocks new
                // task leases until a retry succeeds.
                self.quarantine.insert(account_id);
                tracing::warn!(
                    account_id,
                    "Browser cleanup failed; the account is quarantined"
                );
                return Err(CoreError::Browser(format!(
                    "Browser cleanup is pending ({error}). The account is blocked until cleanup succeeds."
                )));
            }
        }
        sessions.remove(&account_id);
        let mut pending = self.pending.lock().await;
        if let Some(browser) = pending.get_mut(&account_id) {
            browser.close().await?;
        }
        pending.remove(&account_id);
        self.quarantine.remove(account_id);
        Ok(())
    }
    /// Retries cleanup for quarantined accounts. Returns the accounts still pending.
    pub async fn retry_pending(&self) -> Vec<u32> {
        for account_id in self.quarantine.list() {
            let _ = self.close(account_id).await;
        }
        self.quarantine.list()
    }
    pub fn start_cleanup_loop(self: &Arc<Self>, stop: CancellationToken) {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {
                        if !manager.quarantine.list().is_empty() {
                            manager.retry_pending().await;
                        }
                    }
                }
            }
        });
    }
    pub async fn close_all(&self) -> CoreResult<()> {
        let mut ids = self
            .sessions
            .lock()
            .await
            .keys()
            .copied()
            .collect::<Vec<_>>();
        ids.extend(self.pending.lock().await.keys().copied());
        ids.sort_unstable();
        ids.dedup();
        let mut first_error = None;
        for id in ids {
            if let Err(error) = self.close(id).await {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

/// Only app-managed runtime locations are used; never the user's system Chrome.
pub fn locate(executable_directory: &Path, resource_directory: &Path) -> Option<PathBuf> {
    let names = ["chrome/chrome.exe", "chrome-win64/chrome.exe"];
    for root in [
        executable_directory.join("runtime"),
        resource_directory.join("runtime"),
    ] {
        for name in names {
            let path = root.join(name);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn failed_startup_cleanup_blocks_reuse_and_shutdown_attempts_other_accounts() {
        let directory = tempfile::tempdir().unwrap();
        let manager = BrowserManager::new(
            Some(directory.path().join("missing-browser")),
            directory.path().join("profiles"),
        );
        // A file in place of a directory simulates a cleanup error without leaving any process alive.
        let failed = directory.path().join("failed-profile");
        tokio::fs::write(&failed, "fixture").await.unwrap();
        assert!(manager
            .clean_startup(
                1,
                PendingBrowser {
                    process: None,
                    profile: failed.clone()
                }
            )
            .await
            .is_err());
        assert_eq!(manager.quarantine.list(), vec![1]);
        assert!(matches!(
            manager
                .start(1, BrowserPurpose::Task, false, &CancellationToken::new())
                .await,
            Err(CoreError::Busy)
        ));
        let other = directory.path().join("other-profile");
        tokio::fs::create_dir(&other).await.unwrap();
        manager.pending.lock().await.insert(
            2,
            PendingBrowser {
                process: None,
                profile: other.clone(),
            },
        );
        assert!(manager.close_all().await.is_err());
        assert!(!other.exists());
        assert_eq!(manager.quarantine.list(), vec![1]);
        tokio::fs::remove_file(&failed).await.unwrap();
        assert!(manager.retry_pending().await.is_empty());
        assert!(manager.pending.lock().await.is_empty());
    }
}

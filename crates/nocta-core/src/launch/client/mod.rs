//! Nocta's own Roblox client install (Windows), separate from the system `roblox-player` handler.
//!
//! Versions are downloaded from Roblox's setup CDN the same way official bootstrappers do it: the current
//! version GUID comes from the client-settings API, `{guid}-rbxPkgManifest.txt` lists the packages with their
//! MD5, and each zip is extracted into its mapped folder. A version is assembled under `.staging/` and moved
//! into `versions/` only when complete, so a listed version is always whole. The system Roblox install and its
//! link handler are never touched.
use crate::{
    models::{
        AppConfig, ClientGraphicsApi, ClientInstallProgress, ClientState, ClientUpdateMode,
        InstalledClient, LatestClient,
    },
    CoreError, CoreResult,
};
use chrono::Utc;
use futures::{stream, TryStreamExt};
use md5::{Digest, Md5};
use serde_json::Value;
use std::{
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::{io::AsyncWriteExt, sync::Notify};
use tokio_util::sync::CancellationToken;

pub const EXECUTABLE: &str = "RobloxPlayerBeta.exe";
const MARKER: &str = ".nocta-client.json";
const LATEST_TTL: Duration = Duration::from_secs(5 * 60);

pub struct Endpoints {
    /// Client-version endpoints, tried in order.
    pub versions: Vec<String>,
    /// Setup CDN mirrors, tried in order.
    pub cdns: Vec<String>,
}
impl Default for Endpoints {
    fn default() -> Self {
        Self {
            versions: vec![
                "https://clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer".into(),
                "https://clientsettings.roblox.com/v2/client-version/WindowsPlayer".into(),
            ],
            cdns: vec![
                "https://setup.rbxcdn.com".into(),
                "https://setup-aws.rbxcdn.com".into(),
                "https://setup-ak.rbxcdn.com".into(),
                "https://roblox-setup.cachefly.net".into(),
                "https://s3.amazonaws.com/setup.roblox.com".into(),
            ],
        }
    }
}

#[derive(Default)]
struct State {
    latest: Option<(LatestClient, Instant)>,
    progress: Option<ClientInstallProgress>,
    last_error: Option<String>,
    cancel: Option<CancellationToken>,
}

pub struct ClientInstall {
    root: PathBuf,
    http: reqwest::Client,
    endpoints: Endpoints,
    state: Mutex<State>,
    install_lock: tokio::sync::Mutex<()>,
    pub changed: Arc<Notify>,
}

pub fn validate_guid(guid: &str) -> CoreResult<()> {
    let valid = guid.len() == 24
        && guid.starts_with("version-")
        && guid[8..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if valid {
        Ok(())
    } else {
        Err(CoreError::Validation(
            "A Roblox version looks like version- followed by 16 lowercase hex characters.".into(),
        ))
    }
}

fn mb(bytes: u64) -> u32 {
    (bytes / (1024 * 1024)).min(u32::MAX as u64) as u32
}

impl ClientInstall {
    pub fn new(root: PathBuf, changed: Arc<Notify>) -> CoreResult<Self> {
        Self::with_endpoints(root, changed, Endpoints::default())
    }

    pub fn with_endpoints(
        root: PathBuf,
        changed: Arc<Notify>,
        endpoints: Endpoints,
    ) -> CoreResult<Self> {
        let http = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(60))
            .user_agent(concat!("Nocta/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| CoreError::Network)?;
        Ok(Self {
            root,
            http,
            endpoints,
            state: Mutex::new(State::default()),
            install_lock: tokio::sync::Mutex::new(()),
            changed,
        })
    }

    /// A file in Nocta's Roblox data folder, for small bits of state such as the user's own
    /// frame-rate cap.
    pub fn state_file(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn versions_dir(&self) -> PathBuf {
        self.root.join("versions")
    }

    pub fn version_dir(&self, guid: &str) -> PathBuf {
        self.versions_dir().join(guid)
    }

    /// Complete installs, newest first. A folder without its marker is ignored.
    pub fn installed(&self) -> Vec<InstalledClient> {
        let mut list = std::fs::read_dir(self.versions_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let dir = entry.path();
                let marker = std::fs::read(dir.join(MARKER)).ok()?;
                let client: InstalledClient = serde_json::from_slice(&marker).ok()?;
                (dir.join(EXECUTABLE).is_file()
                    && entry.file_name().to_str() == Some(client.guid.as_str()))
                .then_some(client)
            })
            .collect::<Vec<_>>();
        list.sort_by(|a, b| b.installed_at.cmp(&a.installed_at));
        list
    }

    /// The version the next launch uses: the pinned one if installed, otherwise the newest.
    pub fn active(&self, config: &AppConfig) -> Option<InstalledClient> {
        let installed = self.installed();
        config
            .client_pinned_version
            .as_ref()
            .and_then(|pinned| installed.iter().find(|client| &client.guid == pinned))
            .or_else(|| installed.first())
            .cloned()
    }

    pub fn snapshot(&self, config: &AppConfig) -> ClientState {
        let state = self.state.lock().unwrap();
        ClientState {
            installed: self.installed(),
            active: self.active(config).map(|client| client.guid),
            latest: state.latest.as_ref().map(|(latest, _)| latest.clone()),
            progress: state.progress.clone(),
            last_error: state.last_error.clone(),
            directory: self.root.to_string_lossy().into_owned(),
        }
    }

    fn set_progress(&self, progress: Option<ClientInstallProgress>) {
        self.state.lock().unwrap().progress = progress;
        self.changed.notify_one();
    }

    /// Current Roblox version, cached for a few minutes. `force` skips the cache.
    pub async fn latest(&self, force: bool) -> CoreResult<LatestClient> {
        if !force {
            if let Some((latest, at)) = &self.state.lock().unwrap().latest {
                if at.elapsed() < LATEST_TTL {
                    return Ok(latest.clone());
                }
            }
        }
        let mut failure = CoreError::Network;
        for url in &self.endpoints.versions {
            let response = match self.http.get(url).send().await {
                Ok(response) if response.status().is_success() => response,
                Ok(response) => {
                    failure = CoreError::Validation(format!(
                        "The Roblox version service returned HTTP {}.",
                        response.status().as_u16()
                    ));
                    continue;
                }
                Err(_) => continue,
            };
            let Ok(body) = response.json::<Value>().await else {
                continue;
            };
            let guid = body["clientVersionUpload"].as_str().unwrap_or_default();
            if validate_guid(guid).is_err() {
                failure = CoreError::Validation(
                    "The Roblox version service returned an unexpected version.".into(),
                );
                continue;
            }
            let latest = LatestClient {
                guid: guid.to_owned(),
                version: body["version"].as_str().unwrap_or_default().to_owned(),
                checked_at: Utc::now().to_rfc3339(),
            };
            self.state.lock().unwrap().latest = Some((latest.clone(), Instant::now()));
            self.changed.notify_one();
            return Ok(latest);
        }
        Err(failure)
    }

    pub fn cancel(&self) {
        if let Some(cancel) = &self.state.lock().unwrap().cancel {
            cancel.cancel();
        }
    }

    /// Deletes one installed version. The active version and versions with a running client stay.
    pub fn remove(&self, guid: &str, config: &AppConfig, running: &[String]) -> CoreResult<()> {
        validate_guid(guid)?;
        if self
            .active(config)
            .is_some_and(|client| client.guid == guid)
        {
            return Err(CoreError::Validation(
                "This is the version Nocta launches with. Choose another version first.".into(),
            ));
        }
        let dir = self.version_dir(guid);
        if in_use(&dir, running) {
            return Err(CoreError::Validation(
                "A Roblox client from this version is running. Close it first.".into(),
            ));
        }
        std::fs::remove_dir_all(&dir)?;
        self.changed.notify_one();
        Ok(())
    }

    /// Removes versions beyond the active one plus `client_keep_versions` newer-first others.
    /// Versions with a running client are always kept. Returns how many were removed.
    pub fn cleanup(&self, config: &AppConfig, running: &[String]) -> u32 {
        let active = self.active(config).map(|client| client.guid);
        let mut kept = 0;
        let mut removed = 0;
        for client in self.installed() {
            if Some(&client.guid) == active.as_ref() {
                continue;
            }
            if kept < config.client_keep_versions {
                kept += 1;
                continue;
            }
            let dir = self.version_dir(&client.guid);
            if !in_use(&dir, running) && std::fs::remove_dir_all(&dir).is_ok() {
                removed += 1;
            }
        }
        if removed > 0 {
            self.changed.notify_one();
        }
        removed
    }

    /// Resolves the executable for a launch, updating first in automatic mode.
    pub async fn ready(
        self: &Arc<Self>,
        config: &AppConfig,
        cancel: &CancellationToken,
    ) -> CoreResult<PathBuf> {
        let mut active = self.active(config);
        if config.client_update_mode == ClientUpdateMode::Auto
            && config.client_pinned_version.is_none()
        {
            match self.latest(false).await {
                Ok(latest) if active.as_ref().map(|client| &client.guid) != Some(&latest.guid) => {
                    let install = self.install(&latest.guid, Some(latest.version.clone()));
                    active = Some(tokio::select! {
                        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                        installed = install => installed?,
                    });
                }
                Ok(_) => {}
                // Offline or the service is down: an installed version is still usable.
                Err(error) if active.is_some() => {
                    tracing::warn!(%error, "Roblox version check failed; launching the installed client")
                }
                Err(error) => return Err(error),
            }
        } else if config.client_update_mode == ClientUpdateMode::Ask {
            // Only refreshes what the UI shows as "update available"; installing is the user's call.
            let _ = self.latest(false).await;
        }
        let active = active.ok_or_else(|| {
            CoreError::Validation(
                "Nocta's Roblox client is not installed. Install it in Settings → Roblox client, or turn off \"Launch with Nocta's client\".".into(),
            )
        })?;
        let executable = self.version_dir(&active.guid).join(EXECUTABLE);
        if !executable.is_file() {
            return Err(CoreError::Validation(format!(
                "Roblox {} is incomplete. Reinstall it in Settings → Roblox client.",
                active.guid
            )));
        }
        self.apply_settings(&active.guid, config)?;
        Ok(executable)
    }
}

fn in_use(dir: &Path, running: &[String]) -> bool {
    let prefix = dir.to_string_lossy().to_lowercase().replace('/', "\\");
    running.iter().any(|path| {
        path.to_lowercase()
            .replace('/', "\\")
            .starts_with(&format!("{prefix}\\"))
    })
}

pub mod fast_flags;
mod flags;
mod install;
#[cfg(test)]
mod tests;

use nocta_core::launch::{client::ClientInstall, instances::InstanceManager, Launcher};
use nocta_core::roblox::web::RobloxWeb;
use nocta_core::{accounts::Accounts, models::*, storage::Storage, vault::Vault, CoreResult};
use nocta_core::{browser::BrowserManager, sessions::Sessions};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};
use tauri::{AppHandle, Emitter, Manager};

pub struct DesktopState {
    pub storage: Storage,
    pub accounts: Accounts,
    pub sessions: Arc<Sessions>,
    pub launcher: Arc<Launcher>,
    pub automation: nocta_core::launch::automation::Automation,
    pub client_care: nocta_core::launch::client_care::ClientCare,
    pub monitor_stop: tokio_util::sync::CancellationToken,
    pub monitor: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub mutation: tokio::sync::Mutex<()>,
    revision: AtomicU32,
    pub runtime: RuntimeInfo,
    /// Fingerprints of the large parts last sent to the window; unchanged parts are left out.
    /// Held for a whole publish, so snapshots reach the window one at a time and in order.
    sent: tokio::sync::Mutex<SentParts>,
}

/// Fingerprints of the accounts and logs in the last snapshot event.
#[derive(Default)]
struct SentParts {
    accounts: Option<u64>,
    logs: Option<u64>,
}

fn fingerprint(value: &serde_json::Value) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.to_string().hash(&mut hasher);
    hasher.finish()
}

impl DesktopState {
    pub async fn open(app: &AppHandle) -> CoreResult<Self> {
        let executable_directory = std::env::current_exe()?.parent().unwrap().to_path_buf();
        let portable = executable_directory.join("portable.flag").exists();
        let mut directory = if portable {
            executable_directory.join("data")
        } else {
            app.path()
                .app_data_dir()
                .map_err(|error| nocta_core::CoreError::Validation(error.to_string()))?
        };
        if cfg!(debug_assertions) {
            if let Some(override_directory) = std::env::var_os("NOCTA_DATA_DIR") {
                directory = PathBuf::from(override_directory);
            }
        }
        let storage = Storage::open(&directory.join("nocta.sqlite")).await?;
        let vault = Arc::new(Vault::open(
            &storage.vault_id().await?,
            !storage.has_secrets().await?,
        )?);
        let accounts = Accounts::new(storage.clone(), vault);
        let resources = app
            .path()
            .resource_dir()
            .unwrap_or_else(|_| executable_directory.clone());
        let mut browser_path = nocta_core::browser::locate(&executable_directory, &resources);
        if cfg!(debug_assertions) && browser_path.is_none() {
            browser_path = nocta_core::browser::locate(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap(),
                &resources,
            );
        }
        let browsers = Arc::new(BrowserManager::new(
            browser_path.clone(),
            directory.join("profiles"),
        ));
        browsers.sweep().await?;
        // Browser extension copies with keys written in, left by older versions; nothing
        // creates them now, and no key should stay on disk.
        let _ = tokio::fs::remove_dir_all(directory.join("private-extensions")).await;
        let sessions = Sessions::new(accounts.clone(), browsers, &storage.config().await?)?;
        let instances = InstanceManager::new(
            InstanceManager::native(),
            Arc::new(tokio::sync::Notify::new()),
        );
        if storage.config().await?.multi_instance {
            if let Err(error) = instances.set_multi_instance(true) {
                accounts
                    .log(
                        "WARNING",
                        "LAUNCH",
                        &format!("Multi-instance could not be enabled: {error}"),
                        None,
                    )
                    .await?;
            }
        }
        let monitor_stop = tokio_util::sync::CancellationToken::new();
        let monitor = instances.start_monitor(monitor_stop.clone());
        sessions.browsers.start_cleanup_loop(monitor_stop.clone());
        let launcher = Launcher::new(
            accounts.clone(),
            sessions.clone(),
            Arc::new(RobloxWeb::new()?),
            instances,
            Arc::new(ClientInstall::new(
                directory.join("roblox"),
                Arc::new(tokio::sync::Notify::new()),
            )?),
        );
        let runtime = RuntimeInfo {
            platform: std::env::consts::OS.to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            data_directory: directory.to_string_lossy().into_owned(),
            portable,
            browser_ready: browser_path.is_some(),
            browser_path: browser_path.map(|path| path.to_string_lossy().into_owned()),
            capabilities: vec![
                "accounts".into(),
                "folders".into(),
                "settings".into(),
                "encrypted_secrets".into(),
                "sessions".into(),
                "tasks".into(),
                "launch".into(),
                "instances".into(),
            ],
            safe_display: false,
        };
        accounts
            .log(
                "INFO",
                "SYSTEM",
                "Nocta started. Local storage is ready.",
                None,
            )
            .await?;
        Ok(Self {
            storage,
            accounts,
            sessions,
            automation: nocta_core::launch::automation::Automation::new(launcher.clone()),
            client_care: nocta_core::launch::client_care::ClientCare::new(launcher.clone()),
            launcher,
            monitor_stop,
            monitor: tokio::sync::Mutex::new(Some(monitor)),
            mutation: tokio::sync::Mutex::new(()),
            revision: AtomicU32::new(1),
            runtime,
            sent: Default::default(),
        })
    }

    pub async fn snapshot(&self) -> CoreResult<AppSnapshot> {
        let config = self.storage.config().await?;
        Ok(AppSnapshot {
            revision: self.revision.load(Ordering::SeqCst),
            accounts: self.accounts.list().await?,
            folders: self.accounts.folders().await?,
            config: config.clone(),
            logs: self.accounts.logs().await?,
            runtime: self.runtime.clone(),
            tasks: self.sessions.tasks.snapshot(),
            metrics: self.storage.metrics().await?,
            launcher: self.launcher.instances.state(),
            instances: self.launcher.instances.snapshot(),
            external_instances: self.launcher.instances.external_snapshot(),
            client: self.launcher.client.snapshot(&config),
        })
    }

    /// Sends the snapshot to the window. Accounts and logs are by far its largest parts and
    /// rarely change between updates (task progress, clients), so they are only sent when they
    /// did; the window keeps the ones it has. With thousands of accounts this saves the window
    /// from parsing and rebuilding megabytes of data several times a second.
    pub async fn publish(&self, app: &AppHandle) -> CoreResult<()> {
        let mut guard = self.sent.lock().await;
        let sent = &mut *guard;
        self.revision.fetch_add(1, Ordering::SeqCst);
        let snapshot = self.snapshot().await?;
        let mut value = serde_json::to_value(&snapshot)?;
        let object = value.as_object_mut().expect("a snapshot is an object");
        for (field, last) in [("accounts", &mut sent.accounts), ("logs", &mut sent.logs)] {
            let print = object.get(field).map(fingerprint);
            if print.is_some() && print == *last {
                object.remove(field);
            }
            *last = print;
        }
        if let Err(error) = app.emit("nocta:snapshot", value) {
            // The window may have missed parts; send everything next time.
            *sent = SentParts::default();
            tracing::warn!(%error, "Snapshot delivery failed");
        }
        Ok(())
    }
}

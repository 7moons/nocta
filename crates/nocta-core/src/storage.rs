use crate::{models::AppConfig, CoreResult};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
    SqlitePool,
};
use std::{path::Path, str::FromStr};

#[derive(Clone)]
pub struct Storage {
    /// The one connection that writes, so writes never wait on a busy database.
    pub(crate) pool: SqlitePool,
    /// Read-only connections for the snapshot. In WAL mode they read the last committed state
    /// while a write is in progress, so the window is not held up by bulk work.
    pub(crate) read: SqlitePool,
}

/// Most folder colors the user can save.
pub const MAX_FOLDER_COLORS: usize = 32;

fn is_hex_color(color: &str) -> bool {
    let bytes = color.as_bytes();
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(u8::is_ascii_hexdigit)
}

impl Storage {
    pub async fn open(path: &Path) -> CoreResult<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            // In WAL mode this cannot corrupt the database; a power cut may lose only the last
            // commits, and it saves an fsync on each of the many small writes bulk checks make.
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(std::time::Duration::from_secs(10));
        let mut storage = Self::from_options(options.clone()).await?;
        storage.read = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(options.read_only(true))
            .await?;
        Ok(storage)
    }

    pub async fn memory() -> CoreResult<Self> {
        Self::from_options(SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true))
            .await
    }

    async fn from_options(options: SqliteConnectOptions) -> CoreResult<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|error| {
                crate::CoreError::Validation(format!("Schema migration failed: {error}"))
            })?;
        sqlx::query("INSERT OR IGNORE INTO app_meta (key, value) VALUES ('vault_id', ?)")
            .bind(uuid::Uuid::new_v4().to_string())
            .execute(&pool)
            .await?;
        Ok(Self {
            read: pool.clone(),
            pool,
        })
    }

    pub async fn vault_id(&self) -> CoreResult<String> {
        Ok(
            sqlx::query_scalar("SELECT value FROM app_meta WHERE key = 'vault_id'")
                .fetch_one(&self.pool)
                .await?,
        )
    }

    pub async fn config(&self) -> CoreResult<AppConfig> {
        let json: Option<String> =
            sqlx::query_scalar("SELECT value FROM app_meta WHERE key = 'config'")
                .fetch_optional(&self.read)
                .await?;
        Ok(match json {
            Some(value) => serde_json::from_str(&value)?,
            None => AppConfig::default(),
        })
    }

    pub async fn has_secrets(&self) -> CoreResult<bool> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM accounts) OR EXISTS(SELECT 1 FROM provider_secrets)",
        )
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn patched_config(&self, patch: serde_json::Value) -> CoreResult<AppConfig> {
        let mut current = serde_json::to_value(self.config().await?)?;
        let fields = current.as_object_mut().unwrap();
        let patch = patch
            .as_object()
            .ok_or_else(|| crate::CoreError::Validation("Expected a settings patch.".into()))?;
        for (key, value) in patch {
            if !fields.contains_key(key) {
                return Err(crate::CoreError::Validation("Unknown setting.".into()));
            }
            fields.insert(key.clone(), value.clone());
        }
        let config: AppConfig = serde_json::from_value(current)?;
        Self::validate_config(&config)?;
        Ok(config)
    }

    pub async fn save_config(&self, config: &AppConfig) -> CoreResult<()> {
        Self::validate_config(config)?;
        sqlx::query("INSERT INTO app_meta (key, value) VALUES ('config', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
            .bind(serde_json::to_string(config)?).execute(&self.pool).await?;
        Ok(())
    }

    pub fn validate_config(config: &AppConfig) -> CoreResult<()> {
        if !(1..=100).contains(&config.checker_concurrency)
            || !(10..=3600).contains(&config.browser_timeout_secs)
            || !(75..=150).contains(&config.interface_scale)
            || config.precheck_retries > 10
            || [
                config.checker_delay_secs,
                config.bulk_action_delay_secs,
                config.pre_launch_delay_secs,
            ]
            .iter()
            .any(|delay| *delay > 3600)
        {
            return Err(crate::CoreError::Validation(
                "Check concurrency, timeouts, delays and interface scale.".to_owned(),
            ));
        }
        if !config.place_id.is_empty() {
            crate::launch::target::validate_place_id(&config.place_id)?;
        }
        if (config.fps_cap != 0
            && !(crate::launch::fps::MIN_CAP..=crate::launch::fps::MAX_CAP)
                .contains(&config.fps_cap))
            || !(crate::launch::fps::MIN_CAP..=240).contains(&config.afk_fps_cap)
        {
            return Err(crate::CoreError::Validation(
                "Use a frame-rate cap between 5 and 1000 (AFK: 5 to 240), or 0 for no cap.".into(),
            ));
        }
        if !config.discord_webhook.is_empty() {
            crate::webhook::validate(&config.discord_webhook)?;
        }
        crate::launch::automation::validate(&config.launch_presets, &config.schedules)?;
        if !(10..=600).contains(&config.in_game_timeout_secs) {
            return Err(crate::CoreError::Validation(
                "Wait between 10 and 600 seconds for a client to join.".into(),
            ));
        }
        if config.restart_alts_minutes != 0 && !(10..=1440).contains(&config.restart_alts_minutes) {
            return Err(crate::CoreError::Validation(
                "Restart clients every 10 to 1440 minutes, or 0 for never.".into(),
            ));
        }
        if config.anti_afk_click.x_percent > 100 || config.anti_afk_click.y_percent > 100 {
            return Err(crate::CoreError::Validation(
                "A click point is given in percent, from 0 to 100.".into(),
            ));
        }
        for profile in [&config.main_profile, &config.alt_profile] {
            if profile.ram_cap_mb != 0 && !(100..=16384).contains(&profile.ram_cap_mb) {
                return Err(crate::CoreError::Validation(
                    "Cap a client's RAM at 100 to 16384 MB, or 0 for no cap.".into(),
                ));
            }
            if profile.trim_every_secs != 0 && !(30..=3600).contains(&profile.trim_every_secs) {
                return Err(crate::CoreError::Validation(
                    "Give memory back every 30 to 3600 seconds, or 0 for never.".into(),
                ));
            }
            if profile.cpu_limit_percent != 0 && !(5..=100).contains(&profile.cpu_limit_percent) {
                return Err(crate::CoreError::Validation(
                    "Limit a client's CPU to 5–100 %, or 0 for no limit.".into(),
                ));
            }
            let width_ok = profile.window_width == 0 || (1..=7680).contains(&profile.window_width);
            let height_ok =
                profile.window_height == 0 || (1..=4320).contains(&profile.window_height);
            if !width_ok
                || !height_ok
                || (profile.window_width == 0) != (profile.window_height == 0)
            {
                return Err(crate::CoreError::Validation(
                    "Set both window width (1–7680) and height (1–4320), or both to 0.".into(),
                ));
            }
        }
        if !(1..=19).contains(&config.anti_afk_minutes) {
            return Err(crate::CoreError::Validation(
                "Anti-AFK must act every 1 to 19 minutes; Roblox kicks idle players after 20."
                    .into(),
            ));
        }
        if config.memory_limit_mb != 0 && !(512..=16384).contains(&config.memory_limit_mb) {
            return Err(crate::CoreError::Validation(
                "Use a memory limit between 512 and 16384 MB, or 0 for none.".into(),
            ));
        }
        for hotkey in [
            &config.hotkeys.show,
            &config.hotkeys.close_roblox,
            &config.hotkeys.toggle_afk,
        ]
        .into_iter()
        .chain(config.launch_presets.iter().map(|preset| &preset.hotkey))
        {
            crate::hotkeys::validate(hotkey)?;
        }
        crate::launch::client::fast_flags::validate(&config.fast_flags)?;
        if config.follow_username.len() > 40 {
            return Err(crate::CoreError::Validation(
                "Enter a Roblox username.".into(),
            ));
        }
        if config.client_keep_versions > 5 {
            return Err(crate::CoreError::Validation(
                "Keep at most 5 previous Roblox versions.".into(),
            ));
        }
        if let Some(guid) = &config.client_pinned_version {
            crate::launch::client::validate_guid(guid)?;
        }
        if !is_hex_color(&config.default_folder_color)
            || config.folder_colors.len() > MAX_FOLDER_COLORS
            || !config.folder_colors.iter().all(|color| is_hex_color(color))
        {
            return Err(crate::CoreError::Validation(format!(
                "Folder colors are six-digit hex codes, at most {MAX_FOLDER_COLORS} saved."
            )));
        }
        if config.search_separators.len() > 64
            || config.search_separators.chars().any(char::is_control)
        {
            return Err(crate::CoreError::Validation(
                "Search separators must be at most 64 bytes and contain no control characters."
                    .into(),
            ));
        }
        if !config.invite_url.is_empty() {
            if config.invite_url.len() > 4096 {
                return Err(crate::CoreError::Validation(
                    "The invite link is too long.".into(),
                ));
            }
            let url = reqwest::Url::parse(&config.invite_url).map_err(|_| {
                crate::CoreError::Validation("Enter a valid HTTPS Roblox invite link.".into())
            })?;
            if url.scheme() != "https"
                || !matches!(
                    url.host_str(),
                    Some("roblox.com" | "www.roblox.com" | "web.roblox.com")
                )
                || !url.username().is_empty()
                || url.password().is_some()
                || url.port_or_known_default() != Some(443)
            {
                return Err(crate::CoreError::Validation(
                    "Invite links must use an approved HTTPS Roblox host.".into(),
                ));
            }
        }
        if config.presets.len() > 20 {
            return Err(crate::CoreError::Validation(
                "Save at most 20 place presets.".into(),
            ));
        }
        let mut places = std::collections::HashSet::new();
        if !matches!(
            config.log_preferences.level.as_str(),
            "all" | "INFO" | "WARN" | "WARNING" | "SUCCESS" | "ERROR" | "DEBUG"
        ) || config.log_preferences.module.len() > 80
            || config.log_preferences.module.chars().any(char::is_control)
        {
            return Err(crate::CoreError::Validation(
                "Invalid log filter preferences.".into(),
            ));
        }
        let mut identifiers = std::collections::HashSet::new();
        for preset in &config.presets {
            let place = crate::launch::target::validate_place_id(&preset.place_id)?;
            if preset.id.is_empty()
                || preset.id.len() > 128
                || preset.name.trim().is_empty()
                || preset.name.len() > 200
                || preset.name.chars().any(char::is_control)
                || !places.insert(place)
                || !identifiers.insert(&preset.id)
            {
                return Err(crate::CoreError::Validation(
                    "Saved places need unique IDs, unique place IDs and a valid name.".into(),
                ));
            }
        }
        Ok(())
    }

    pub async fn close(&self) {
        self.read.close().await;
        self.pool.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn multi_instance_default_matches_platform_and_keeps_explicit_choice() {
        let storage = Storage::memory().await.unwrap();
        let mut config = storage.config().await.unwrap();
        assert!(config.multi_instance);
        config.multi_instance = false;
        storage.save_config(&config).await.unwrap();
        assert!(!storage.config().await.unwrap().multi_instance);
    }

    #[tokio::test]
    async fn saved_folder_colors_are_hex_codes_and_limited() {
        let storage = Storage::memory().await.unwrap();
        let mut config = storage.config().await.unwrap();
        assert!(config.folder_colors.is_empty());
        config.folder_colors = vec!["#123abc".into(), "#FFAA00".into()];
        storage.save_config(&config).await.unwrap();
        assert_eq!(
            storage.config().await.unwrap().folder_colors,
            ["#123abc", "#FFAA00"]
        );
        for wrong in [vec!["red".to_owned()], vec!["#12345".to_owned()]] {
            config.folder_colors = wrong;
            assert!(storage.save_config(&config).await.is_err());
        }
        config.folder_colors = vec!["#000000".into(); MAX_FOLDER_COLORS + 1];
        assert!(storage.save_config(&config).await.is_err());
    }

    #[tokio::test]
    async fn reads_see_commits_and_are_not_held_up_by_a_write() {
        let temp = tempfile::tempdir().unwrap();
        let storage = Storage::open(&temp.path().join("nocta.sqlite"))
            .await
            .unwrap();
        let mut config = storage.config().await.unwrap();
        config.place_id = "42".into();
        storage.save_config(&config).await.unwrap();
        assert_eq!(storage.config().await.unwrap().place_id, "42");

        let mut write = storage.pool.begin().await.unwrap();
        sqlx::query("UPDATE app_meta SET value = 'x' WHERE key = 'vault_id'")
            .execute(&mut *write)
            .await
            .unwrap();
        let read = tokio::time::timeout(std::time::Duration::from_secs(1), storage.config());
        assert_eq!(read.await.unwrap().unwrap().place_id, "42");
        write.rollback().await.unwrap();

        assert!(sqlx::query("DELETE FROM app_meta")
            .execute(&storage.read)
            .await
            .is_err());
        storage.close().await;
    }
}

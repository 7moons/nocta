//! Password-protected backup export and restore.
use super::*;

impl Accounts {
    pub async fn export_backup(&self, password: String) -> CoreResult<Vec<u8>> {
        let password = Zeroizing::new(password);
        let mut rows = Vec::new();
        for metadata in self.list().await? {
            let secrets = self.secrets(metadata.id).await?;
            rows.push(crate::backup::BackupAccount { metadata, secrets });
        }
        let payload = crate::backup::BackupData {
            version: 1,
            config: self.storage.config().await?,
            folders: self.folders().await?,
            accounts: rows,
        };
        let plaintext = Zeroizing::new(serde_json::to_vec(&payload)?);
        crate::backup::encrypt(&plaintext, &password)
    }

    pub async fn restore_backup(&self, bytes: Vec<u8>, password: String) -> CoreResult<u32> {
        self.restore_backup_with_settings(bytes, password, |_| Ok(()))
            .await
    }

    /// Apply native settings immediately before commit. A rejected native setting rolls back all imported data.
    pub async fn restore_backup_with_settings(
        &self,
        bytes: Vec<u8>,
        password: String,
        apply_settings: impl FnOnce(&AppConfig) -> CoreResult<()> + Send,
    ) -> CoreResult<u32> {
        let password = Zeroizing::new(password);
        if bytes.len() > 100 * 1024 * 1024 {
            return Err(CoreError::Validation("Backup exceeds 100 MB.".into()));
        }
        let plaintext = crate::backup::decrypt(&bytes, &password)?;
        let data: crate::backup::BackupData = serde_json::from_slice(&plaintext)?;
        if data.version != 1 {
            return Err(CoreError::Validation("Unsupported backup version.".into()));
        }
        Storage::validate_config(&data.config)?;
        let mut transaction = self.storage.pool.begin().await?;
        let occupied: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM accounts) OR EXISTS(SELECT 1 FROM folders) OR EXISTS(SELECT 1 FROM provider_secrets)")
            .fetch_one(&mut *transaction).await?;
        if occupied {
            return Err(CoreError::Validation("Restore requires an empty workspace, including Trash and folders. Existing data has been kept.".into()));
        }
        for folder in data.folders {
            validate_folder(&folder.name, &folder.color, folder.group.as_deref())?;
            sqlx::query("INSERT INTO folders (id, name, normalized_name, color, group_name, sort_order) VALUES (?, ?, ?, ?, ?, ?)")
                .bind(folder.id).bind(folder.name.trim()).bind(folder.name.trim().to_lowercase())
                .bind(folder.color).bind(folder.group).bind(folder.sort_order).execute(&mut *transaction).await?;
        }
        let count = data.accounts.len() as u32;
        for row in data.accounts {
            let account = row.metadata;
            validate_username(&account.username)?;
            let plain = Zeroizing::new(serde_json::to_vec(&row.secrets)?);
            let sealed = self
                .vault
                .seal(&plain, format!("account:{}", account.id).as_bytes())?;
            let health = serde_json::to_value(&account.health)?;
            let presence = account
                .presence
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?
                .as_deref()
                .and_then(super::presence::parse)
                .map(|value| serde_json::to_string(&value))
                .transpose()?;
            let last_task = account
                .last_task
                .filter(|task| task.status.terminal() && task.account_id == account.id)
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;
            sqlx::query("INSERT INTO accounts (id, username, normalized_username, folder_id, health, checked_at, created_at, deleted_at, has_password, has_cookie, user_id, secrets, cookie_valid, last_started_at, presence, last_task, alias, note, color, pinned, has_totp, playtime_secs, robux, robux_checked_at, is_main, sort_order, fields, afk_point) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
                .bind(account.id).bind(account.username.trim()).bind(account.username.trim().to_lowercase())
                .bind(account.folder_id).bind(health.as_str().unwrap()).bind(account.checked_at)
                .bind(account.created_at).bind(account.deleted_at).bind(!row.secrets.password.is_empty())
                .bind(!row.secrets.cookie.is_empty()).bind(account.user_id).bind(sealed)
                .bind(account.cookie_valid).bind(account.last_started_at).bind(presence).bind(last_task)
                .bind(account.alias).bind(account.note).bind(account.color).bind(account.pinned)
                .bind(!row.secrets.totp.is_empty())
                .bind(account.playtime_secs as i64).bind(account.robux).bind(account.robux_checked_at).bind(account.is_main)
                .bind(account.sort_order).bind(serde_json::to_string(&account.fields)?)
                .bind(account.afk_point.as_ref().map(serde_json::to_string).transpose()?)
                .execute(&mut *transaction).await?;
        }
        sqlx::query("INSERT INTO app_meta (key, value) VALUES ('config', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
            .bind(serde_json::to_string(&data.config)?).execute(&mut *transaction).await?;
        apply_settings(&data.config)?;
        transaction.commit().await?;
        self.log(
            "INFO",
            "BACKUP",
            &format!("Restored {count} accounts from an encrypted backup."),
            None,
        )
        .await?;
        Ok(count)
    }
}

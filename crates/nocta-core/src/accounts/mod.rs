//! Accounts, folders and everything stored with them: secrets (sealed in the vault),
//! the clipboard, backups and the activity log. Each concern lives in
//! its own file as an `impl Accounts` block.
use crate::{models::*, storage::Storage, vault::Vault, CoreError, CoreResult};
use chrono::Utc;
use rand::{distributions::Alphanumeric, Rng};
use sqlx::FromRow;
use std::collections::HashSet;
use std::sync::Arc;
use zeroize::Zeroizing;

mod backup;
mod clipboard;
mod details;
mod export;
pub use export::ExportFormat;
mod folders;
use folders::validate_folder;
mod import;
mod logs;
mod presence;
mod share_links;
#[cfg(test)]
mod tests;

#[derive(Clone)]
pub struct Accounts {
    storage: Storage,
    vault: Arc<Vault>,
}

#[derive(FromRow)]
struct AccountRow {
    id: i64,
    username: String,
    folder_id: Option<i64>,
    health: String,
    checked_at: Option<String>,
    created_at: String,
    deleted_at: Option<String>,
    has_password: bool,
    has_cookie: bool,
    user_id: Option<String>,
    cookie_valid: Option<bool>,
    last_started_at: Option<String>,
    presence: Option<String>,
    last_task: Option<String>,
    alias: String,
    note: String,
    color: Option<String>,
    pinned: bool,
    has_totp: bool,
    playtime_secs: i64,
    robux: Option<i64>,
    robux_checked_at: Option<String>,
    is_main: bool,
    sort_order: Option<i64>,
    fields: String,
    afk_point: Option<String>,
}

impl From<AccountRow> for Account {
    fn from(row: AccountRow) -> Self {
        Self {
            id: row.id as u32,
            username: row.username,
            folder_id: row.folder_id.map(|id| id as u32),
            health: serde_json::from_value(serde_json::Value::String(row.health))
                .unwrap_or_default(),
            checked_at: row.checked_at,
            created_at: row.created_at,
            deleted_at: row.deleted_at,
            has_password: row.has_password,
            has_cookie: row.has_cookie,
            user_id: row.user_id,
            cookie_valid: row.cookie_valid,
            last_started_at: row.last_started_at,
            presence: row.presence.as_deref().and_then(presence::parse),
            last_task: row
                .last_task
                .as_deref()
                .and_then(|raw| serde_json::from_str::<AccountTask>(raw).ok())
                .filter(|task| task.status.terminal() && task.account_id == row.id as u32),
            alias: row.alias,
            note: row.note,
            color: row.color,
            pinned: row.pinned,
            has_totp: row.has_totp,
            playtime_secs: row.playtime_secs.clamp(0, u32::MAX as i64) as u32,
            robux: row.robux,
            robux_checked_at: row.robux_checked_at,
            is_main: row.is_main,
            sort_order: row
                .sort_order
                .map(|order| order.clamp(0, u32::MAX as i64) as u32),
            fields: serde_json::from_str(&row.fields).unwrap_or_default(),
            afk_point: row
                .afk_point
                .as_deref()
                .and_then(|raw| serde_json::from_str(raw).ok()),
        }
    }
}

impl Accounts {
    pub fn new(storage: Storage, vault: Arc<Vault>) -> Self {
        Self { storage, vault }
    }

    pub async fn list(&self) -> CoreResult<Vec<Account>> {
        let rows = sqlx::query_as::<_, AccountRow>("SELECT id, username, folder_id, health, checked_at, created_at, deleted_at, has_password, has_cookie, user_id, cookie_valid, last_started_at, presence, last_task, alias, note, color, pinned, has_totp, playtime_secs, robux, robux_checked_at, is_main, sort_order, fields, afk_point FROM accounts ORDER BY id")
            .fetch_all(&self.storage.read).await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn get(&self, id: u32) -> CoreResult<Account> {
        let row = sqlx::query_as::<_, AccountRow>("SELECT id, username, folder_id, health, checked_at, created_at, deleted_at, has_password, has_cookie, user_id, cookie_valid, last_started_at, presence, last_task, alias, note, color, pinned, has_totp, playtime_secs, robux, robux_checked_at, is_main, sort_order, fields, afk_point FROM accounts WHERE id = ? AND deleted_at IS NULL")
            .bind(id).fetch_optional(&self.storage.pool).await?.ok_or(CoreError::NotFound)?;
        Ok(row.into())
    }

    pub async fn mark_health(
        &self,
        id: u32,
        health: AccountHealth,
        user_id: Option<&str>,
    ) -> CoreResult<()> {
        let health = serde_json::to_value(health)?;
        map_constraint(sqlx::query("UPDATE accounts SET health = ?, checked_at = ?, user_id = COALESCE(?, user_id) WHERE id = ? AND deleted_at IS NULL")
            .bind(health.as_str().unwrap()).bind(Utc::now().to_rfc3339()).bind(user_id).bind(id).execute(&self.storage.pool).await)
    }

    pub async fn mark_cookie(&self, id: u32, valid: bool, user_id: Option<&str>) -> CoreResult<()> {
        map_constraint(sqlx::query("UPDATE accounts SET cookie_valid = ?, user_id = COALESCE(?, user_id), health = CASE WHEN ? = 0 THEN 'invalid_session' WHEN health = 'invalid_session' THEN 'unchecked' ELSE health END WHERE id = ? AND deleted_at IS NULL")
            .bind(valid).bind(user_id).bind(valid).bind(id).execute(&self.storage.pool).await)
    }

    pub async fn mark_started(&self, id: u32) -> CoreResult<()> {
        map_constraint(
            sqlx::query(
                "UPDATE accounts SET last_started_at = ? WHERE id = ? AND deleted_at IS NULL",
            )
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(&self.storage.pool)
            .await,
        )
    }

    pub async fn record_task(&self, task: &AccountTask) -> CoreResult<()> {
        if !task.status.terminal() {
            return Err(CoreError::Validation(
                "Only completed operations can be persisted.".into(),
            ));
        }
        map_constraint(
            sqlx::query("UPDATE accounts SET last_task = ? WHERE id = ?")
                .bind(serde_json::to_string(task)?)
                .bind(task.account_id)
                .execute(&self.storage.pool)
                .await,
        )
    }

    pub async fn record_tasks(&self, tasks: &[AccountTask]) -> CoreResult<()> {
        if tasks.is_empty() {
            return Ok(());
        }
        if tasks.iter().any(|task| !task.status.terminal()) {
            return Err(CoreError::Validation(
                "Only completed operations can be persisted.".into(),
            ));
        }
        let mut transaction = self.storage.pool.begin().await?;
        for task in tasks {
            sqlx::query("UPDATE accounts SET last_task = ? WHERE id = ?")
                .bind(serde_json::to_string(task)?)
                .bind(task.account_id)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn replace_cookie(&self, id: u32, cookie: &str, user_id: &str) -> CoreResult<()> {
        let mut secrets = self.secrets(id).await?;
        secrets.cookie = cookie.to_owned();
        let plain = Zeroizing::new(serde_json::to_vec(&secrets)?);
        let sealed = self
            .vault
            .seal(&plain, format!("account:{id}").as_bytes())?;
        map_constraint(sqlx::query("UPDATE accounts SET secrets = ?, has_cookie = 1, cookie_valid = 1, user_id = ?, health = 'unchecked', checked_at = NULL WHERE id = ? AND deleted_at IS NULL")
            .bind(sealed).bind(user_id).bind(id).execute(&self.storage.pool).await)
    }

    /// The saved settings, for background work that runs outside a command.
    pub async fn config(&self) -> CoreResult<AppConfig> {
        self.storage.config().await
    }

    pub async fn save_config(&self, config: &AppConfig) -> CoreResult<()> {
        self.storage.save_config(config).await
    }

    pub async fn secrets(&self, id: u32) -> CoreResult<AccountSecrets> {
        let sealed: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT secrets FROM accounts WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.storage.pool)
                .await?;
        let plain = self.vault.open_secret(
            &sealed.ok_or(CoreError::NotFound)?,
            format!("account:{id}").as_bytes(),
        )?;
        Ok(serde_json::from_slice(&plain)?)
    }

    pub async fn update(&self, id: u32, input: AccountUpdate) -> CoreResult<()> {
        validate_username(&input.username)?;
        let mut secrets = self.secrets(id).await?;
        let previous_username: String =
            sqlx::query_scalar("SELECT username FROM accounts WHERE id = ?")
                .bind(id)
                .fetch_one(&self.storage.pool)
                .await?;
        let identity_changed = previous_username != input.username.trim()
            || input
                .cookie
                .as_ref()
                .is_some_and(|cookie| cookie != &secrets.cookie);
        if let Some(password) = input.password {
            secrets.password = password;
        }
        if let Some(cookie) = input.cookie {
            secrets.cookie = cookie;
        }
        let plain = Zeroizing::new(serde_json::to_vec(&secrets)?);
        let sealed = self
            .vault
            .seal(&plain, format!("account:{id}").as_bytes())?;
        let result = sqlx::query("UPDATE accounts SET username = ?, normalized_username = ?, folder_id = ?, secrets = ?, has_password = ?, has_cookie = ?, health = CASE WHEN ? THEN 'unchecked' ELSE health END, checked_at = CASE WHEN ? THEN NULL ELSE checked_at END, cookie_valid = CASE WHEN ? THEN NULL ELSE cookie_valid END, user_id = CASE WHEN ? THEN NULL ELSE user_id END WHERE id = ?")
            .bind(input.username.trim()).bind(input.username.trim().to_lowercase()).bind(input.folder_id)
            .bind(sealed).bind(!secrets.password.is_empty()).bind(!secrets.cookie.is_empty())
            .bind(identity_changed).bind(identity_changed).bind(identity_changed).bind(identity_changed)
            .bind(id).execute(&self.storage.pool).await;
        map_constraint(result)?;
        Ok(())
    }

    pub async fn action(
        &self,
        ids: &[u32],
        action: &str,
        folder_id: Option<u32>,
    ) -> CoreResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut transaction = self.storage.pool.begin().await?;
        if let Some(id) = folder_id {
            let exists: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM folders WHERE id = ?)")
                    .bind(id)
                    .fetch_one(&mut *transaction)
                    .await?;
            if !exists {
                return Err(CoreError::NotFound);
            }
        }
        for id in ids {
            match action {
                "trash" => {
                    sqlx::query(
                        "UPDATE accounts SET deleted_at = ? WHERE id = ? AND deleted_at IS NULL",
                    )
                    .bind(Utc::now().to_rfc3339())
                    .bind(id)
                    .execute(&mut *transaction)
                    .await?;
                }
                "restore" => {
                    sqlx::query("UPDATE accounts SET deleted_at = NULL, folder_id = COALESCE(?, folder_id) WHERE id = ? AND deleted_at IS NOT NULL")
                        .bind(folder_id).bind(id)
                        .execute(&mut *transaction)
                        .await?;
                }
                "restore_unfiled" => {
                    sqlx::query("UPDATE accounts SET deleted_at = NULL, folder_id = NULL WHERE id = ? AND deleted_at IS NOT NULL")
                        .bind(id).execute(&mut *transaction).await?;
                }
                "purge" => {
                    sqlx::query("DELETE FROM accounts WHERE id = ? AND deleted_at IS NOT NULL")
                        .bind(id)
                        .execute(&mut *transaction)
                        .await?;
                }
                "move" => {
                    sqlx::query(
                        "UPDATE accounts SET folder_id = ? WHERE id = ? AND deleted_at IS NULL",
                    )
                    .bind(folder_id)
                    .bind(id)
                    .execute(&mut *transaction)
                    .await?;
                }
                _ => return Err(CoreError::Validation("Unknown account action.".into())),
            }
        }
        transaction.commit().await?;
        self.log(
            "INFO",
            "ACCOUNTS",
            &format!("{action}: {} selected accounts.", ids.len()),
            None,
        )
        .await?;
        Ok(())
    }
}

fn validate_username(username: &str) -> CoreResult<()> {
    let name = username.trim();
    if name.is_empty() || name.len() > 40 || name.chars().any(|c| c.is_control() || c == ':') {
        return Err(CoreError::Validation(
            "Enter a valid username (1–40 characters).".into(),
        ));
    }
    Ok(())
}

fn map_constraint(result: Result<sqlx::sqlite::SqliteQueryResult, sqlx::Error>) -> CoreResult<()> {
    match result {
        Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
            Err(CoreError::Duplicate)
        }
        Err(error) => Err(error.into()),
        Ok(result) if result.rows_affected() == 0 => Err(CoreError::NotFound),
        Ok(_) => Ok(()),
    }
}

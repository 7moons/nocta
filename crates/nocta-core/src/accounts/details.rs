//! Account details that are only for the user: alias, note, color tag and pinning, plus the
//! two-step verification key (sealed with the other secrets).
use super::*;
use crate::totp;

/// Hex colors offered for tags; anything else is refused.
fn valid_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Most fields an account can have.
pub const MAX_FIELDS: usize = 10;

/// Trimmed fields without empty rows; names are unique (ignoring case) and short.
fn valid_fields(fields: &[AccountField]) -> CoreResult<Vec<AccountField>> {
    let fields: Vec<AccountField> = fields
        .iter()
        .map(|field| AccountField {
            name: field.name.trim().to_owned(),
            value: field.value.trim().to_owned(),
        })
        .filter(|field| !field.name.is_empty() || !field.value.is_empty())
        .collect();
    if fields.len() > MAX_FIELDS {
        return Err(CoreError::Validation(format!(
            "An account can have at most {MAX_FIELDS} fields."
        )));
    }
    for (index, field) in fields.iter().enumerate() {
        if field.name.is_empty() {
            return Err(CoreError::Validation("Give every field a name.".into()));
        }
        if field.name.chars().count() > 30 || field.value.chars().count() > 200 {
            return Err(CoreError::Validation(
                "Field names can have 30 characters and values 200.".into(),
            ));
        }
        if field
            .name
            .chars()
            .chain(field.value.chars())
            .any(char::is_control)
        {
            return Err(CoreError::Validation(
                "Fields cannot contain line breaks or control characters.".into(),
            ));
        }
        if fields[..index]
            .iter()
            .any(|other| other.name.eq_ignore_ascii_case(&field.name))
        {
            return Err(CoreError::Validation(format!(
                "There are two fields named {}.",
                field.name
            )));
        }
    }
    Ok(fields)
}

impl Accounts {
    /// Saves the user's own order: `ids[0]` first. Accounts not listed keep no position and
    /// come after, by ID.
    pub async fn reorder(&self, ids: &[u32]) -> CoreResult<()> {
        let mut transaction = self.storage.pool.begin().await?;
        sqlx::query("UPDATE accounts SET sort_order = NULL")
            .execute(&mut *transaction)
            .await?;
        for (position, id) in ids.iter().enumerate() {
            sqlx::query("UPDATE accounts SET sort_order = ? WHERE id = ?")
                .bind(position as i64)
                .bind(id)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn update_details(&self, id: u32, details: AccountDetails) -> CoreResult<()> {
        let alias = details.alias.trim();
        let note = details.note.trim();
        if alias.chars().count() > 40 || alias.chars().any(char::is_control) {
            return Err(CoreError::Validation(
                "An alias can have at most 40 characters.".into(),
            ));
        }
        if note.chars().count() > 1000 {
            return Err(CoreError::Validation(
                "A note can have at most 1000 characters.".into(),
            ));
        }
        if details
            .color
            .as_deref()
            .is_some_and(|color| !valid_color(color))
        {
            return Err(CoreError::Validation("Use a six-digit hex color.".into()));
        }
        let fields = valid_fields(&details.fields)?;
        if details
            .afk_point
            .is_some_and(|point| point.x_percent > 100 || point.y_percent > 100)
        {
            return Err(CoreError::Validation(
                "A click point is given in percent, from 0 to 100.".into(),
            ));
        }
        let result = sqlx::query(
            "UPDATE accounts SET alias = ?, note = ?, color = ?, pinned = ?, is_main = ?, fields = ?, afk_point = ? WHERE id = ?",
        )
        .bind(alias)
        .bind(note)
        .bind(details.color)
        .bind(details.pinned)
        .bind(details.is_main)
        .bind(serde_json::to_string(&fields)?)
        .bind(
            details
                .afk_point
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
        )
        .bind(id)
        .execute(&self.storage.pool)
        .await;
        map_constraint(result)
    }

    pub async fn add_playtime(&self, updates: &[(u32, u64)]) -> CoreResult<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let mut transaction = self.storage.pool.begin().await?;
        for (id, secs) in updates {
            sqlx::query("UPDATE accounts SET playtime_secs = playtime_secs + ? WHERE id = ?")
                .bind(*secs as i64)
                .bind(id)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn store_robux(&self, id: u32, robux: i64) -> CoreResult<()> {
        sqlx::query("UPDATE accounts SET robux = ?, robux_checked_at = ? WHERE id = ?")
            .bind(robux)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(&self.storage.pool)
            .await?;
        Ok(())
    }

    /// Stores live presence fetched from Roblox for these accounts.
    pub async fn store_presence(&self, updates: &[(u32, AccountPresence)]) -> CoreResult<()> {
        let mut transaction = self.storage.pool.begin().await?;
        for (id, presence) in updates {
            sqlx::query("UPDATE accounts SET presence = ? WHERE id = ?")
                .bind(serde_json::to_string(presence)?)
                .bind(id)
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Saves (`Some`) or removes (`None`) the account's authenticator key.
    pub async fn set_totp(&self, id: u32, key: Option<String>) -> CoreResult<()> {
        if let Some(key) = &key {
            totp::decode_secret(key)?;
        }
        let mut secrets = self.secrets(id).await?;
        secrets.totp = key.map(|key| key.trim().to_owned()).unwrap_or_default();
        let plain = Zeroizing::new(serde_json::to_vec(&secrets)?);
        let sealed = self
            .vault
            .seal(&plain, format!("account:{id}").as_bytes())?;
        let result = sqlx::query("UPDATE accounts SET secrets = ?, has_totp = ? WHERE id = ?")
            .bind(sealed)
            .bind(!secrets.totp.is_empty())
            .bind(id)
            .execute(&self.storage.pool)
            .await;
        map_constraint(result)
    }

    /// The current two-step verification code of an account with a saved key.
    pub async fn totp_code(&self, id: u32) -> CoreResult<TotpCode> {
        let secrets = self.secrets(id).await?;
        if secrets.totp.is_empty() {
            return Err(CoreError::Validation(
                "This account has no authenticator key saved.".into(),
            ));
        }
        let key = Zeroizing::new(totp::decode_secret(&secrets.totp)?);
        let now = Utc::now().timestamp().max(0) as u64;
        Ok(TotpCode {
            code: totp::code_at(&key, now, totp::DIGITS),
            remaining_secs: totp::remaining(now),
        })
    }
}

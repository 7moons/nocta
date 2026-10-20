//! Bulk account import from `username:password:cookie` lines and from old Nocta databases.
use super::*;

impl Accounts {
    pub async fn import(
        &self,
        text: String,
        folder_id: Option<u32>,
        update_existing: bool,
    ) -> CoreResult<ImportReport> {
        self.import_with_progress(text, folder_id, update_existing, |_| {})
            .await
    }

    pub async fn import_with_progress(
        &self,
        text: String,
        folder_id: Option<u32>,
        update_existing: bool,
        progress: impl Fn(ImportProgress) + Send,
    ) -> CoreResult<ImportReport> {
        let text = Zeroizing::new(text);
        if text.len() > 64 * 1024 * 1024 {
            return Err(CoreError::Validation(
                "Import is larger than 64 MB. Split it into smaller batches.".into(),
            ));
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
        let mut report = ImportReport::default();
        let mut seen = HashSet::new();
        let total = text.lines().count() as u32;
        for (index, line) in text.lines().enumerate() {
            if index % 25 == 0 {
                progress(ImportProgress {
                    processed: index as u32,
                    total,
                    added: report.added,
                    updated: report.updated,
                    failed: report.skipped,
                    committed: false,
                });
            }
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let parts = line.splitn(3, ':').collect::<Vec<_>>();
            let issue = |reason: &str| ImportIssue {
                line: (index + 1) as u32,
                reason: reason.into(),
            };
            if parts.len() != 3
                || validate_username(parts[0]).is_err()
                || parts[2].trim().is_empty()
            {
                report.skipped += 1;
                report
                    .issues
                    .push(issue("Expected username:password:cookie."));
                continue;
            }
            let username = parts[0].trim();
            if !seen.insert(username.to_lowercase()) {
                report.skipped += 1;
                report
                    .issues
                    .push(issue("Duplicate username within this import."));
                continue;
            }
            let existing: Option<i64> =
                sqlx::query_scalar("SELECT id FROM accounts WHERE normalized_username = ?")
                    .bind(username.to_lowercase())
                    .fetch_optional(&mut *transaction)
                    .await?;
            let id = match (existing, update_existing) {
                (Some(_), false) => { report.skipped += 1; report.issues.push(issue("Username already exists, including in Trash.")); continue; }
                (None, true) => { report.skipped += 1; report.issues.push(issue("Username not found.")); continue; }
                (Some(id), true) => id,
                (None, false) => sqlx::query("INSERT INTO accounts (username, normalized_username, folder_id, created_at, secrets) VALUES (?, ?, ?, ?, X'')")
                    .bind(username).bind(username.to_lowercase()).bind(folder_id).bind(Utc::now().to_rfc3339())
                    .execute(&mut *transaction).await?.last_insert_rowid(),
            };
            let secrets = AccountSecrets {
                password: parts[1].to_owned(),
                cookie: parts[2].trim().to_owned(),
                totp: String::new(),
            };
            let plain = Zeroizing::new(serde_json::to_vec(&secrets)?);
            let sealed = self
                .vault
                .seal(&plain, format!("account:{id}").as_bytes())?;
            sqlx::query("UPDATE accounts SET secrets = ?, has_password = ?, has_cookie = ?, health = 'unchecked', checked_at = NULL, cookie_valid = NULL, user_id = NULL WHERE id = ?")
                .bind(sealed).bind(!secrets.password.is_empty()).bind(!secrets.cookie.is_empty()).bind(id).execute(&mut *transaction).await?;
            if update_existing {
                report.updated += 1;
            } else {
                report.added += 1;
            }
        }
        transaction.commit().await?;
        progress(ImportProgress {
            processed: total,
            total,
            added: report.added,
            updated: report.updated,
            failed: report.skipped,
            committed: true,
        });
        self.log(
            "INFO",
            "ACCOUNTS",
            &format!(
                "Import complete: {} added, {} updated, {} skipped.",
                report.added, report.updated, report.skipped
            ),
            None,
        )
        .await?;
        Ok(report)
    }

    /// Imports accounts and folders read from an old app's database in one transaction.
    /// Folders are matched by name (case-insensitive) or created; accounts follow the same rules as a
    /// text import (valid username, a cookie, no duplicates). Accounts from the old Trash go to Trash
    /// when `include_trash` is set and are skipped otherwise.
    pub async fn import_legacy(
        &self,
        data: crate::legacy::LegacyData,
        include_trash: bool,
        progress: impl Fn(ImportProgress) + Send,
    ) -> CoreResult<ImportReport> {
        let mut transaction = self.storage.pool.begin().await?;
        let mut folders = std::collections::HashMap::new();
        for folder in &data.folders {
            let name = folder.name.trim();
            let group = folder
                .group
                .as_deref()
                .map(str::trim)
                .filter(|group| !group.is_empty());
            let color = folder
                .color
                .as_deref()
                .map(str::trim)
                .filter(|color| validate_folder(name, color, group).is_ok())
                .unwrap_or("#a3a3a3")
                .to_ascii_lowercase();
            if validate_folder(name, &color, group).is_err() {
                continue;
            }
            let existing: Option<i64> =
                sqlx::query_scalar("SELECT id FROM folders WHERE normalized_name = ?")
                    .bind(name.to_lowercase())
                    .fetch_optional(&mut *transaction)
                    .await?;
            let id = match existing {
                Some(id) => id,
                None => sqlx::query("INSERT INTO folders (name, normalized_name, color, group_name, sort_order) VALUES (?, ?, ?, ?, (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM folders))")
                    .bind(name).bind(name.to_lowercase()).bind(&color).bind(group)
                    .execute(&mut *transaction).await?.last_insert_rowid(),
            };
            folders.insert(folder.id, id);
        }

        let mut report = ImportReport::default();
        let mut seen = HashSet::new();
        let total = data.accounts.len() as u32;
        let now = Utc::now().to_rfc3339();
        for (index, account) in data.accounts.into_iter().enumerate() {
            if index % 50 == 0 {
                progress(ImportProgress {
                    processed: index as u32,
                    total,
                    added: report.added,
                    updated: 0,
                    failed: report.skipped,
                    committed: false,
                });
            }
            let username = account.username.trim().to_owned();
            if username.is_empty() {
                continue;
            }
            let mut skip = |reason: &str| {
                report.skipped += 1;
                report.issues.push(ImportIssue {
                    line: (index + 1) as u32,
                    reason: format!("{username}: {reason}"),
                });
            };
            if account.deleted && !include_trash {
                skip("In the old Trash (not included).");
                continue;
            }
            if validate_username(&username).is_err() {
                skip("The username is not valid.");
                continue;
            }
            if account.cookie.trim().is_empty() {
                skip("No cookie saved.");
                continue;
            }
            if !seen.insert(username.to_lowercase()) {
                skip("Listed twice in the old database.");
                continue;
            }
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM accounts WHERE normalized_username = ?)",
            )
            .bind(username.to_lowercase())
            .fetch_one(&mut *transaction)
            .await?;
            if exists {
                skip("Already in Nocta (or in Trash).");
                continue;
            }
            let folder_id = account.folder_id.and_then(|id| folders.get(&id).copied());
            let alias: String = account.alias.trim().chars().take(40).collect();
            let note: String = account.note.trim().chars().take(1000).collect();
            let id = sqlx::query("INSERT INTO accounts (username, normalized_username, folder_id, created_at, deleted_at, secrets, alias, note, user_id) VALUES (?, ?, ?, ?, ?, X'', ?, ?, ?)")
                .bind(&username).bind(username.to_lowercase()).bind(folder_id).bind(&now)
                .bind(account.deleted.then_some(now.as_str()))
                .bind(alias.trim()).bind(note.trim()).bind(&account.user_id)
                .execute(&mut *transaction).await?.last_insert_rowid();
            let secrets = AccountSecrets {
                password: account.password,
                cookie: account.cookie.trim().to_owned(),
                totp: String::new(),
            };
            let plain = Zeroizing::new(serde_json::to_vec(&secrets)?);
            let sealed = self
                .vault
                .seal(&plain, format!("account:{id}").as_bytes())?;
            sqlx::query(
                "UPDATE accounts SET secrets = ?, has_password = ?, has_cookie = 1 WHERE id = ?",
            )
            .bind(sealed)
            .bind(!secrets.password.is_empty())
            .bind(id)
            .execute(&mut *transaction)
            .await?;
            report.added += 1;
        }
        transaction.commit().await?;
        progress(ImportProgress {
            processed: total,
            total,
            added: report.added,
            updated: 0,
            failed: report.skipped,
            committed: true,
        });
        self.log(
            "INFO",
            "ACCOUNTS",
            &format!(
                "Imported from the old app: {} added, {} skipped, {} folders.",
                report.added,
                report.skipped,
                folders.len()
            ),
            None,
        )
        .await?;
        Ok(report)
    }
}

//! Live online status of the user's own accounts, asked with one of their sessions.
use crate::{
    accounts::Accounts,
    models::{AccountHealth, AccountPresence, FriendEntry},
    roblox::social::Social,
    CoreError, CoreResult,
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Where the user's accounts are, their Robux and their friends.
pub struct Presence {
    accounts: Accounts,
    social: Arc<Social>,
}

impl Presence {
    pub fn new(accounts: Accounts, social: Arc<Social>) -> Self {
        Self { accounts, social }
    }

    /// Refreshes where every account with a known Roblox ID is (offline, website, in game,
    /// Studio). Returns how many accounts were updated.
    pub async fn refresh_presence(&self, cancel: &CancellationToken) -> CoreResult<u32> {
        let accounts: Vec<_> = self
            .accounts
            .list()
            .await?
            .into_iter()
            .filter(|account| account.deleted_at.is_none())
            .collect();
        let known: Vec<(u32, u64)> = accounts
            .iter()
            .filter_map(|account| Some((account.id, account.user_id.as_ref()?.parse().ok()?)))
            .collect();
        if known.is_empty() {
            return Ok(0);
        }
        // Any working session can ask; healthy accounts are tried first.
        let mut askers: Vec<_> = accounts
            .iter()
            .filter(|account| account.has_cookie && account.cookie_valid != Some(false))
            .collect();
        askers.sort_by_key(|account| account.health != AccountHealth::Healthy);
        let ids: Vec<u64> = known.iter().map(|(_, user)| *user).collect();
        let mut found = None;
        for asker in askers.into_iter().take(3) {
            let cookie = self.accounts.secrets(asker.id).await?;
            match self
                .social
                .presence(Some(&cookie.cookie), &ids, cancel)
                .await
            {
                Ok(presence) => {
                    found = Some(presence);
                    break;
                }
                // A signed-out or banned session cannot ask; another one may.
                Err(CoreError::Validation(message))
                    if message == super::web::SIGNED_OUT || message == super::web::MODERATED =>
                {
                    continue
                }
                Err(error) => return Err(error),
            }
        }
        // Without a working session Roblox still says who is online, just not where.
        let presence = match found {
            Some(presence) => presence,
            None => self.social.presence(None, &ids, cancel).await?,
        };
        let now = chrono::Utc::now().to_rfc3339();
        let updates: Vec<(u32, AccountPresence)> = known
            .iter()
            .filter_map(|(id, user)| {
                let entry = presence
                    .iter()
                    .find(|item| item.user_id == user.to_string())?;
                Some((
                    *id,
                    AccountPresence {
                        user_presence_type: entry.kind,
                        last_location: entry.location.clone(),
                        last_online: (entry.kind > 0).then(|| now.clone()),
                        place_id: entry.place_id.clone(),
                    },
                ))
            })
            .collect();
        self.accounts.store_presence(&updates).await?;
        Ok(updates.len() as u32)
    }

    /// Checks the Robux balance of these accounts, a few at a time. Returns how many were
    /// updated; accounts that fail (no session, banned) are logged and skipped.
    pub async fn refresh_robux(&self, ids: &[u32], cancel: &CancellationToken) -> CoreResult<u32> {
        use futures::StreamExt;
        // Results are collected before anything is written: writing while other checks are
        // paused mid-query could wait on a database connection one of them holds.
        let results: Vec<(u32, CoreResult<i64>)> = futures::stream::iter(ids.iter().copied())
            .map(|id| async move { (id, self.robux_of(id, cancel).await) })
            .buffer_unordered(3)
            .collect()
            .await;
        let mut updated = 0;
        for (id, result) in results {
            match result {
                Ok(robux) => {
                    self.accounts.store_robux(id, robux).await?;
                    updated += 1;
                }
                Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
                Err(error) => {
                    let _ = self
                        .accounts
                        .log(
                            "WARNING",
                            "ACCOUNTS",
                            &format!("Robux not checked: {error}"),
                            Some(id),
                        )
                        .await;
                }
            }
        }
        Ok(updated)
    }

    async fn robux_of(&self, id: u32, cancel: &CancellationToken) -> CoreResult<i64> {
        let account = self.accounts.get(id).await?;
        let user_id: u64 = account
            .user_id
            .as_deref()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| {
                CoreError::Validation(
                    "Check this account's session first so Nocta knows its Roblox ID.".into(),
                )
            })?;
        let secrets = self.accounts.secrets(id).await?;
        self.social.robux(&secrets.cookie, user_id, cancel).await
    }

    /// The friends of an account with where they are and their avatars, people in a game first.
    pub async fn friends_of(
        &self,
        account_id: u32,
        cancel: &CancellationToken,
    ) -> CoreResult<Vec<FriendEntry>> {
        let account = self.accounts.get(account_id).await?;
        let user_id: u64 = account
            .user_id
            .as_deref()
            .and_then(|id| id.parse().ok())
            .ok_or_else(|| {
                CoreError::Validation(
                    "Check this account's session first so Nocta knows its Roblox ID.".into(),
                )
            })?;
        let cookie = self.accounts.secrets(account_id).await?;
        let friends = self.social.friends(&cookie.cookie, user_id, cancel).await?;
        let ids: Vec<u64> = friends
            .iter()
            .filter_map(|friend| friend.id.parse().ok())
            .collect();
        let presence = if ids.is_empty() {
            Vec::new()
        } else {
            self.social
                .presence(Some(&cookie.cookie), &ids, cancel)
                .await?
        };
        let avatars = self.social.avatars(&ids, cancel).await.unwrap_or_default();
        let mut entries: Vec<FriendEntry> = friends
            .into_iter()
            .map(|user| FriendEntry {
                presence: presence
                    .iter()
                    .find(|item| item.user_id == user.id)
                    .cloned(),
                avatar_url: avatars.get(&user.id).cloned(),
                user,
            })
            .collect();
        // In a game, then online, then offline; by name within each.
        entries.sort_by_key(|entry| {
            let kind = entry.presence.as_ref().map_or(0, |presence| presence.kind);
            (
                match kind {
                    2 => 0,
                    1 | 3 => 1,
                    _ => 2,
                },
                entry.user.name.to_lowercase(),
            )
        });
        Ok(entries)
    }
}

//! Direct game links remembered for Roblox share links (see `Launcher::expand_share_link`).
use super::*;

impl Accounts {
    /// Direct game link remembered for a share link (see `Launcher::expand_share_link`).
    pub async fn share_link(&self, key: &str) -> CoreResult<Option<String>> {
        Ok(self.share_links().await?.remove(key))
    }

    pub async fn remember_share_link(&self, key: &str, direct: &str) -> CoreResult<()> {
        let mut links = self.share_links().await?;
        // Old entries are dropped first so the map stays small.
        while links.len() >= 200 {
            let Some(first) = links.keys().next().cloned() else {
                break;
            };
            links.remove(&first);
        }
        links.insert(key.to_owned(), direct.to_owned());
        sqlx::query("INSERT INTO app_meta (key, value) VALUES ('share_links', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
            .bind(serde_json::to_string(&links)?)
            .execute(&self.storage.pool)
            .await?;
        Ok(())
    }

    async fn share_links(&self) -> CoreResult<std::collections::BTreeMap<String, String>> {
        let json: Option<String> =
            sqlx::query_scalar("SELECT value FROM app_meta WHERE key = 'share_links'")
                .fetch_optional(&self.storage.pool)
                .await?;
        Ok(json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default())
    }
}

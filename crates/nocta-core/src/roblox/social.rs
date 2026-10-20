//! Roblox users, presence, friends and avatar headshots.
//!
//! Lookups by name and avatars need no account. Presence and friends are asked with an account's
//! cookie: Roblox shows a player's server (`gameId`) only to accounts allowed to join them.
use crate::{
    models::{RobloxUser, UserPresence},
    roblox::web::RobloxWeb,
    CoreError, CoreResult,
};
use reqwest::Url;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

/// Roblox accepts at most this many IDs per presence, user or thumbnail request.
pub const BATCH: usize = 50;
const AVATAR_TTL: Duration = Duration::from_secs(6 * 60 * 60);

pub struct Social {
    web: Arc<RobloxWeb>,
    avatars: Mutex<HashMap<u64, (Instant, Option<String>)>>,
}

fn id(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        .filter(|id| *id > 0)
}

fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(|text| {
            text.chars()
                .filter(|c| !c.is_control())
                .take(200)
                .collect::<String>()
        })
        .filter(|text| !text.trim().is_empty())
}

fn url(base: &str, path: &str) -> CoreResult<Url> {
    Url::parse(&format!("{base}{path}")).map_err(|_| CoreError::Network)
}

fn parse_user(value: &Value) -> Option<RobloxUser> {
    let name = text(&value["name"])?;
    Some(RobloxUser {
        id: id(&value["id"])?.to_string(),
        display_name: text(&value["displayName"]).unwrap_or_else(|| name.clone()),
        name,
    })
}

/// Roblox presence types: 0 offline, 1 website, 2 in game, 3 in Studio.
fn parse_presence(value: &Value) -> Option<UserPresence> {
    Some(UserPresence {
        user_id: id(&value["userId"])?.to_string(),
        kind: value["userPresenceType"]
            .as_u64()
            .filter(|kind| *kind <= 3)
            .unwrap_or(0) as u32,
        place_id: id(&value["placeId"])
            .or_else(|| id(&value["rootPlaceId"]))
            .map(|id| id.to_string()),
        job_id: text(&value["gameId"]),
        location: text(&value["lastLocation"]),
    })
}

impl Social {
    pub fn new(web: Arc<RobloxWeb>) -> Self {
        Self {
            web,
            avatars: Mutex::new(HashMap::new()),
        }
    }

    /// The account with this exact username, or a validation error naming it.
    pub async fn user_by_name(
        &self,
        username: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<RobloxUser> {
        let username = username.trim();
        if username.is_empty()
            || username.len() > 40
            || !username
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(CoreError::Validation("Enter a Roblox username.".into()));
        }
        let body = json!({ "usernames": [username], "excludeBannedUsers": true });
        let response = self
            .web
            .post_json_raw(
                url(&self.web.endpoints.users, "/v1/usernames/users")?,
                &body,
                cancel,
            )
            .await?;
        if !(200..300).contains(&response.status) {
            return Err(CoreError::Validation(
                "Roblox could not look up that username.".into(),
            ));
        }
        response.json()?["data"]
            .as_array()
            .and_then(|users| users.first())
            .and_then(parse_user)
            .ok_or_else(|| CoreError::Validation(format!("No Roblox user is named {username}.")))
    }

    /// Names of users by ID, for friends lists that only return IDs.
    async fn users_by_id(
        &self,
        ids: &[u64],
        cancel: &CancellationToken,
    ) -> CoreResult<Vec<RobloxUser>> {
        let mut users = Vec::new();
        for chunk in ids.chunks(BATCH) {
            let body = json!({ "userIds": chunk, "excludeBannedUsers": false });
            let response = self
                .web
                .post_json_raw(url(&self.web.endpoints.users, "/v1/users")?, &body, cancel)
                .await?;
            if (200..300).contains(&response.status) {
                users.extend(
                    response.json()?["data"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(parse_user),
                );
            }
        }
        Ok(users)
    }

    /// Where each user is right now, asked as the account behind `cookie`. Without a cookie
    /// Roblox still says who is online, but not which game or server they are in.
    pub async fn presence(
        &self,
        cookie: Option<&str>,
        user_ids: &[u64],
        cancel: &CancellationToken,
    ) -> CoreResult<Vec<UserPresence>> {
        let mut found = Vec::new();
        for chunk in user_ids.chunks(BATCH) {
            let endpoint = url(&self.web.endpoints.presence, "/v1/presence/users")?;
            let body = json!({ "userIds": chunk });
            let response = match cookie {
                Some(cookie) => {
                    self.web
                        .post_json_authed(endpoint, cookie, &body, cancel)
                        .await?
                }
                None => self.web.post_json_raw(endpoint, &body, cancel).await?,
            };
            if response.status == 429 {
                return Err(CoreError::Validation(
                    crate::roblox::web::RATE_LIMITED.into(),
                ));
            }
            if !(200..300).contains(&response.status) {
                return Err(CoreError::Validation(
                    "Roblox presence is unavailable right now.".into(),
                ));
            }
            found.extend(
                response.json()?["userPresences"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(parse_presence),
            );
        }
        Ok(found)
    }

    /// The Robux balance of the account behind `cookie` (whose Roblox ID is `user_id`).
    pub async fn robux(
        &self,
        cookie: &str,
        user_id: u64,
        cancel: &CancellationToken,
    ) -> CoreResult<i64> {
        let value = self
            .web
            .get_json_authed(
                url(
                    &self.web.endpoints.economy,
                    &format!("/v1/users/{user_id}/currency"),
                )?,
                cookie,
                cancel,
            )
            .await?;
        value["robux"].as_i64().ok_or(CoreError::Network)
    }

    /// The friends of `user_id`, as seen by the account behind `cookie`, sorted by name.
    pub async fn friends(
        &self,
        cookie: &str,
        user_id: u64,
        cancel: &CancellationToken,
    ) -> CoreResult<Vec<RobloxUser>> {
        let value = self
            .web
            .get_json_authed(
                url(
                    &self.web.endpoints.friends,
                    &format!("/v1/users/{user_id}/friends"),
                )?,
                cookie,
                cancel,
            )
            .await?;
        let entries: Vec<&Value> = value["data"].as_array().into_iter().flatten().collect();
        let mut friends: Vec<RobloxUser> = entries
            .iter()
            .filter_map(|entry| parse_user(entry))
            .collect();
        // Newer responses carry only IDs; names come from the users API.
        let missing: Vec<u64> = entries
            .iter()
            .filter(|entry| parse_user(entry).is_none())
            .filter_map(|entry| id(&entry["id"]))
            .collect();
        if !missing.is_empty() {
            friends.extend(self.users_by_id(&missing, cancel).await?);
        }
        friends.sort_by_key(|friend| friend.name.to_lowercase());
        friends.dedup_by(|a, b| a.id == b.id);
        Ok(friends)
    }

    /// Headshot URLs by user ID, cached for a few hours. Users without one are left out.
    pub async fn avatars(
        &self,
        user_ids: &[u64],
        cancel: &CancellationToken,
    ) -> CoreResult<HashMap<String, String>> {
        let mut result = HashMap::new();
        let mut wanted = Vec::new();
        {
            let cache = self.avatars.lock().unwrap();
            for id in user_ids {
                match cache.get(id) {
                    Some((at, url)) if at.elapsed() < AVATAR_TTL => {
                        if let Some(url) = url {
                            result.insert(id.to_string(), url.clone());
                        }
                    }
                    _ => wanted.push(*id),
                }
            }
        }
        wanted.sort_unstable();
        wanted.dedup();
        for chunk in wanted.chunks(BATCH) {
            let ids = chunk
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let address = url(
                &self.web.endpoints.thumbnails,
                &format!("/v1/users/avatar-headshot?userIds={ids}&size=48x48&format=Png&isCircular=false"),
            )?;
            let response = self.web.get_raw(address, None, None, cancel).await?;
            if !(200..300).contains(&response.status) {
                continue;
            }
            let value = response.json()?;
            let mut cache = self.avatars.lock().unwrap();
            for id in chunk {
                cache.insert(*id, (Instant::now(), None));
            }
            for item in value["data"].as_array().into_iter().flatten() {
                let (Some(target), Some(image)) =
                    (id(&item["targetId"]), cdn_image(&item["imageUrl"]))
                else {
                    continue;
                };
                cache.insert(target, (Instant::now(), Some(image.clone())));
                result.insert(target.to_string(), image);
            }
        }
        Ok(result)
    }
}

/// Only Roblox CDN images reach the interface.
fn cdn_image(value: &Value) -> Option<String> {
    let url = Url::parse(value.as_str()?).ok()?;
    (url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| host == "rbxcdn.com" || host.ends_with(".rbxcdn.com")))
    .then(|| url.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roblox::web::fixture::{response, serve};

    fn json(body: &str) -> String {
        response("200 OK", &[("Content-Type", "application/json")], body)
    }

    #[tokio::test]
    async fn looks_up_users_presence_friends_and_avatars() {
        let (base, server) = serve(vec![
            json(r#"{"data":[{"requestedUsername":"builder","id":156,"name":"builderman","displayName":"Builder"}]}"#),
            // Presence: CSRF challenge, then the answer.
            response("403 Forbidden", &[("x-csrf-token", "csrf")], ""),
            json(r#"{"userPresences":[{"userPresenceType":2,"userId":156,"placeId":1818,"gameId":"job-1","lastLocation":"Crossroads"},{"userPresenceType":0,"userId":1}]}"#),
            // Friends with names, plus one that needs a lookup.
            json(r#"{"data":[{"id":2,"name":"zed","displayName":"Zed"},{"id":3}]}"#),
            json(r#"{"data":[{"id":3,"name":"amy","displayName":"Amy"}]}"#),
            json(r#"{"data":[{"targetId":156,"imageUrl":"https://tr.rbxcdn.com/a.png"},{"targetId":2,"imageUrl":"https://evil.test/b.png"}]}"#),
        ])
        .await;
        let social = Social::new(Arc::new(RobloxWeb::fixture(&base)));
        let cancel = CancellationToken::new();
        let user = social.user_by_name(" builder ", &cancel).await.unwrap();
        assert_eq!(
            (user.id.as_str(), user.name.as_str()),
            ("156", "builderman")
        );

        let presence = social
            .presence(Some("synthetic-cookie"), &[156, 1], &cancel)
            .await
            .unwrap();
        assert_eq!(presence[0].kind, 2);
        assert_eq!(presence[0].place_id.as_deref(), Some("1818"));
        assert_eq!(presence[0].job_id.as_deref(), Some("job-1"));
        assert_eq!(presence[1].kind, 0);

        let friends = social
            .friends("synthetic-cookie", 156, &cancel)
            .await
            .unwrap();
        assert_eq!(
            friends.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            ["amy", "zed"]
        );

        let avatars = social.avatars(&[156, 2], &cancel).await.unwrap();
        assert_eq!(
            avatars.get("156").map(String::as_str),
            Some("https://tr.rbxcdn.com/a.png")
        );
        assert!(!avatars.contains_key("2"), "non-CDN images are dropped");
        // Cached: the fixture is done, so a second lookup must not need the network.
        assert_eq!(social.avatars(&[156], &cancel).await.unwrap().len(), 1);

        let requests = server.await.unwrap();
        assert!(requests[1].contains("synthetic-cookie"));
        assert!(requests[2].to_lowercase().contains("x-csrf-token: csrf"));
        assert!(requests[0].contains(r#""usernames":["builder"]"#));
        assert!(
            !requests[0].contains("synthetic-cookie"),
            "name lookups need no account"
        );
        assert!(social.user_by_name("not a name!", &cancel).await.is_err());
    }
}

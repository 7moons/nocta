//! Changes to the user's accounts on Roblox itself: signing out other sessions, privacy, display
//! name, approving a Quick Login code, and friendships among the user's own accounts.
//!
//! Every request is made with the account's own cookie. Roblox may ask the owner to confirm a
//! change (password, 2-step code or captcha); that is reported, never worked around.
use super::web::{AuthedWrite, RobloxWeb, RATE_LIMITED, SIGNED_OUT};
use crate::{
    accounts::Accounts,
    models::{BefriendMode, BefriendSummary, PrivacySetting, QuickLoginDevice, RestrictedOption},
    CoreError, CoreResult,
};
use reqwest::{Method, Url};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

/// Most friendships one run makes; everyone with everyone grows quickly.
pub const MAX_BEFRIEND_PAIRS: usize = 300;
/// Pause between friendships, so Roblox does not start refusing them.
const BEFRIEND_PAUSE: Duration = Duration::from_millis(600);
/// Pause between accounts when one change is made to many.
const BULK_PAUSE: Duration = Duration::from_millis(300);

fn url(base: &str, path: &str) -> CoreResult<Url> {
    Url::parse(&format!("{base}{path}")).map_err(|_| CoreError::Network)
}

/// Roblox setting names and values are short identifiers.
fn identifier(text: &str) -> bool {
    !text.is_empty() && text.len() <= 60 && text.chars().all(|c| c.is_ascii_alphanumeric())
}

/// The privacy settings (`whoCan…`) from Roblox's settings-and-options answer, sorted by name.
/// Each entry has its current value and the values Roblox offers.
pub fn parse_privacy(value: &Value) -> Vec<PrivacySetting> {
    let Some(entries) = value.as_object() else {
        return Vec::new();
    };
    let mut settings: Vec<PrivacySetting> = entries
        .iter()
        .filter(|(key, _)| key.starts_with("whoCan") && identifier(key))
        .filter_map(|(key, entry)| {
            let value = entry["currentValue"].as_str().filter(|v| identifier(v))?;
            let mut options = Vec::new();
            let mut restricted = Vec::new();
            for option in entry["options"].as_array()? {
                let Some(name) = option
                    .as_str()
                    .or_else(|| option["option"]["optionValue"].as_str())
                    .or_else(|| option["optionValue"].as_str())
                    .filter(|name| identifier(name))
                else {
                    continue;
                };
                // "SelfUpdateSetting": the owner may choose it, as Nocta does. Anything else is
                // a check only Roblox can do (like an age check), or "Inherited": the option
                // follows another setting and cannot be chosen on its own.
                if let Some(requirement) = option["requirement"]
                    .as_str()
                    .filter(|requirement| identifier(requirement) && *requirement != SELF_UPDATE)
                {
                    restricted.push(RestrictedOption {
                        value: name.to_owned(),
                        requirement: requirement.to_owned(),
                    });
                }
                options.push(name.to_owned());
            }
            (!options.is_empty()).then(|| PrivacySetting {
                key: key.clone(),
                value: value.to_owned(),
                options,
                restricted,
            })
        })
        .collect();
    settings.sort_by(|a, b| a.key.cmp(&b.key));
    settings
}

/// Roblox's mark on options the account owner may choose themselves.
const SELF_UPDATE: &str = "SelfUpdateSetting";

/// A Quick Login code as Roblox shows it on the other device: six letters or digits.
fn quick_code(code: &str) -> CoreResult<String> {
    let code: String = code
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect::<String>()
        .to_ascii_uppercase();
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(CoreError::Validation(
            "Enter the six-character code shown on the other device.".into(),
        ));
    }
    Ok(code)
}

fn rate_limited(write: &AuthedWrite) -> CoreResult<()> {
    if write.status == 429 {
        return Err(CoreError::Validation(RATE_LIMITED.into()));
    }
    Ok(())
}

/// Roblox answers a friend request that needs a captcha with this flag or error code 14.
fn captcha_required(write: &AuthedWrite) -> bool {
    let body = write.json().unwrap_or(Value::Null);
    body["isCaptchaRequired"].as_bool() == Some(true)
        || body["errors"]
            .as_array()
            .is_some_and(|errors| errors.iter().any(|error| error["code"] == 14))
}

struct Session {
    id: u32,
    cookie: String,
    user_id: u64,
}

/// Changes made on Roblox with each account's own session.
pub struct AccountTools {
    accounts: Accounts,
    web: Arc<RobloxWeb>,
}

impl AccountTools {
    pub fn new(accounts: Accounts, web: Arc<RobloxWeb>) -> Self {
        Self { accounts, web }
    }

    async fn session(&self, id: u32) -> CoreResult<Session> {
        let account = self.accounts.get(id).await?;
        let user_id = account
            .user_id
            .as_deref()
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| {
                CoreError::Validation(
                    "Check this account's session first so Nocta knows its Roblox ID.".into(),
                )
            })?;
        let secrets = self.accounts.secrets(id).await?;
        if secrets.cookie.is_empty() {
            return Err(CoreError::Validation(SIGNED_OUT.into()));
        }
        Ok(Session {
            id,
            cookie: secrets.cookie.clone(),
            user_id,
        })
    }

    async fn note(&self, level: &str, message: &str, id: u32) {
        let _ = self
            .accounts
            .log(level, "ACCOUNTS", message, Some(id))
            .await;
    }

    /// Signs the account out everywhere else (other browsers, devices, stolen cookies) and keeps
    /// Nocta signed in with the new cookie Roblox hands back.
    pub async fn sign_out_other_sessions(
        &self,
        id: u32,
        cancel: &CancellationToken,
    ) -> CoreResult<()> {
        let session = self.session(id).await?;
        let write = self
            .web
            .write_authed(
                Method::POST,
                url(
                    &self.web.endpoints.auth,
                    "/v1/logoutfromallsessionsandreauthenticate",
                )?,
                &session.cookie,
                None,
                cancel,
            )
            .await?;
        rate_limited(&write)?;
        if !write.ok() {
            return Err(write.error("Roblox did not sign out the other sessions."));
        }
        let Some(cookie) = write.new_cookie else {
            return Err(CoreError::Validation(
                "Roblox signed out the other sessions but sent no new cookie; refresh this account's cookie.".into(),
            ));
        };
        self.accounts
            .replace_cookie(id, cookie.expose(), &session.user_id.to_string())
            .await?;
        self.note("INFO", "Signed out all other sessions.", id)
            .await;
        Ok(())
    }

    /// The account's Roblox privacy settings with the values Roblox offers for each.
    pub async fn privacy_settings(
        &self,
        id: u32,
        cancel: &CancellationToken,
    ) -> CoreResult<Vec<PrivacySetting>> {
        let session = self.session(id).await?;
        let value = self
            .web
            .get_json_authed(
                url(
                    &self.web.endpoints.apis,
                    "/user-settings-api/v1/user-settings/settings-and-options",
                )?,
                &session.cookie,
                cancel,
            )
            .await?;
        let settings = parse_privacy(&value);
        if settings.is_empty() {
            return Err(CoreError::Validation(
                "Roblox did not return any privacy settings.".into(),
            ));
        }
        Ok(settings)
    }

    /// Sets one privacy setting on these accounts. Returns how many were changed; the others are
    /// logged with Roblox's reason.
    pub async fn set_privacy(
        &self,
        ids: &[u32],
        key: &str,
        value: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<u32> {
        if !key.starts_with("whoCan") || !identifier(key) || !identifier(value) {
            return Err(CoreError::Validation("Choose a privacy setting.".into()));
        }
        let mut changed = 0;
        for (index, id) in ids.iter().enumerate() {
            if index > 0 {
                tokio::select! {
                    _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                    _ = tokio::time::sleep(BULK_PAUSE) => {}
                }
            }
            let result = async {
                let session = self.session(*id).await?;
                // Roblox refuses options behind its own checks; say so instead of a server error.
                let settings = self.privacy_settings(*id, cancel).await?;
                let setting = settings
                    .iter()
                    .find(|setting| setting.key == key)
                    .ok_or_else(|| {
                        CoreError::Validation("Roblox has no such setting for this account.".into())
                    })?;
                if !setting.options.iter().any(|option| option == value) {
                    return Err(CoreError::Validation(format!(
                        "Roblox does not offer {value} for this setting."
                    )));
                }
                if let Some(locked) = setting.restricted.iter().find(|item| item.value == value) {
                    return Err(CoreError::Validation(format!(
                        "Roblox only allows {value} after its own check ({}); change it on roblox.com.",
                        locked.requirement
                    )));
                }
                if setting.value == value {
                    return Ok(());
                }
                let write = self
                    .web
                    .write_authed(
                        Method::POST,
                        url(
                            &self.web.endpoints.apis,
                            "/user-settings-api/v1/user-settings",
                        )?,
                        &session.cookie,
                        Some(&json!({ key: value })),
                        cancel,
                    )
                    .await?;
                rate_limited(&write)?;
                if write.ok() {
                    Ok(())
                } else {
                    Err(write.error("Roblox did not change this setting."))
                }
            }
            .await;
            match result {
                Ok(()) => {
                    changed += 1;
                    self.note("INFO", &format!("Privacy: {key} set to {value}."), *id)
                        .await;
                }
                Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
                Err(error) => {
                    self.note("WARNING", &format!("Privacy not changed: {error}"), *id)
                        .await
                }
            }
        }
        Ok(changed)
    }

    /// Changes the name other players see. Roblox allows this once a week.
    pub async fn set_display_name(
        &self,
        id: u32,
        name: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<()> {
        let name = name.trim();
        let length = name.chars().count();
        if !(3..=20).contains(&length) || name.chars().any(char::is_control) {
            return Err(CoreError::Validation(
                "A display name has 3 to 20 characters.".into(),
            ));
        }
        let session = self.session(id).await?;
        let write = self
            .web
            .write_authed(
                Method::PATCH,
                url(
                    &self.web.endpoints.users,
                    &format!("/v1/users/{}/display-names", session.user_id),
                )?,
                &session.cookie,
                Some(&json!({ "newDisplayName": name })),
                cancel,
            )
            .await?;
        rate_limited(&write)?;
        if !write.ok() {
            return Err(write.error("Roblox did not change the display name."));
        }
        self.note("INFO", &format!("Display name changed to {name}."), id)
            .await;
        Ok(())
    }

    /// Looks up a Quick Login code: which device wants to sign in as this account, and from
    /// where. Nothing is approved yet.
    pub async fn quick_login_check(
        &self,
        id: u32,
        code: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<QuickLoginDevice> {
        let code = quick_code(code)?;
        let session = self.session(id).await?;
        let write = self
            .web
            .write_authed(
                Method::POST,
                url(
                    &self.web.endpoints.apis,
                    "/auth-token-service/v1/login/enterCode",
                )?,
                &session.cookie,
                Some(&json!({ "code": code })),
                cancel,
            )
            .await?;
        rate_limited(&write)?;
        if !write.ok() {
            return Err(write.error("Roblox did not recognise this code."));
        }
        let body = write.json()?;
        let text = |value: &Value| {
            value
                .as_str()
                .map(|text| text.chars().filter(|c| !c.is_control()).take(120).collect())
                .unwrap_or_default()
        };
        Ok(QuickLoginDevice {
            device: text(&body["deviceInfo"]),
            location: text(&body["location"]),
        })
    }

    /// Approves a Quick Login code: the other device is signed in as this account.
    pub async fn quick_login_approve(
        &self,
        id: u32,
        code: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<()> {
        let code = quick_code(code)?;
        let session = self.session(id).await?;
        let write = self
            .web
            .write_authed(
                Method::POST,
                url(
                    &self.web.endpoints.apis,
                    "/auth-token-service/v1/login/validateCode",
                )?,
                &session.cookie,
                Some(&json!({ "code": code })),
                cancel,
            )
            .await?;
        rate_limited(&write)?;
        if !write.ok() {
            return Err(write.error("Roblox did not approve this code."));
        }
        self.note("INFO", "Approved a Quick Login on another device.", id)
            .await;
        Ok(())
    }

    /// Makes the user's own accounts friends: everyone with everyone, or everyone with one main
    /// account. Each pair is checked first, then one side asks and the other accepts.
    pub async fn befriend_accounts(
        &self,
        ids: &[u32],
        mode: BefriendMode,
        cancel: &CancellationToken,
    ) -> CoreResult<BefriendSummary> {
        let mut wanted: Vec<u32> = ids.to_vec();
        if let BefriendMode::WithMain { main } = mode {
            wanted.retain(|id| *id != main);
            wanted.insert(0, main);
        }
        wanted.dedup();
        let mut sessions = Vec::new();
        for id in wanted {
            match self.session(id).await {
                Ok(session) => sessions.push(session),
                Err(error) => {
                    if matches!(mode, BefriendMode::WithMain { main } if main == id) {
                        return Err(error);
                    }
                    self.note("WARNING", &format!("Not befriended: {error}"), id)
                        .await;
                }
            }
        }
        let mut pairs = Vec::new();
        match mode {
            BefriendMode::Everyone => {
                for a in 0..sessions.len() {
                    for b in a + 1..sessions.len() {
                        pairs.push((a, b));
                    }
                }
            }
            BefriendMode::WithMain { .. } => pairs.extend((1..sessions.len()).map(|b| (0, b))),
        }
        if pairs.is_empty() {
            return Err(CoreError::Validation(
                "Select at least two accounts with a working session.".into(),
            ));
        }
        if pairs.len() > MAX_BEFRIEND_PAIRS {
            return Err(CoreError::Validation(format!(
                "That would be {} friendships; do at most {MAX_BEFRIEND_PAIRS} at a time (fewer accounts, or friends with one main account).",
                pairs.len()
            )));
        }
        let mut summary = BefriendSummary {
            pairs: pairs.len() as u32,
            ..Default::default()
        };
        for (index, (a, b)) in pairs.into_iter().enumerate() {
            if index > 0 {
                tokio::select! {
                    _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                    _ = tokio::time::sleep(BEFRIEND_PAUSE) => {}
                }
            }
            let (a, b) = (&sessions[a], &sessions[b]);
            match self.befriend(a, b, cancel).await {
                Ok(Friendship::Added) => summary.added += 1,
                Ok(Friendship::Already) => summary.already += 1,
                Ok(Friendship::Pending) => summary.pending += 1,
                Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
                // A check of Roblox's own: the pair waits; running it again finishes it.
                Err(CoreError::Validation(message)) if super::web::is_challenge(&message) => {
                    summary.pending += 1;
                    self.note(
                        "WARNING",
                        &format!("Friendship with account #{} waits: {message}", b.id),
                        a.id,
                    )
                    .await;
                }
                Err(error) => {
                    summary.failed += 1;
                    self.note(
                        "WARNING",
                        &format!("Friendship with account #{} not made: {error}", b.id),
                        a.id,
                    )
                    .await;
                }
            }
        }
        Ok(summary)
    }

    async fn befriend(
        &self,
        a: &Session,
        b: &Session,
        cancel: &CancellationToken,
    ) -> CoreResult<Friendship> {
        let friends = &self.web.endpoints.friends;
        let status = self
            .web
            .get_json_authed(
                url(
                    friends,
                    &format!(
                        "/v1/users/{}/friends/statuses?userIds={}",
                        a.user_id, b.user_id
                    ),
                )?,
                &a.cookie,
                cancel,
            )
            .await?;
        let status = status["data"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item["status"].as_str())
            .unwrap_or("NotFriends")
            .to_owned();
        if status == "Friends" {
            return Ok(Friendship::Already);
        }
        // `b` already asked `a`: `a` only accepts.
        let (asker, accepter) = if status == "RequestReceived" {
            (b, a)
        } else {
            (a, b)
        };
        if status != "RequestReceived" && status != "RequestSent" {
            let request = self
                .web
                .write_authed(
                    Method::POST,
                    url(
                        friends,
                        &format!("/v1/users/{}/request-friendship", b.user_id),
                    )?,
                    &a.cookie,
                    Some(&json!({ "friendshipOriginSourceType": 0 })),
                    cancel,
                )
                .await?;
            rate_limited(&request)?;
            if captcha_required(&request) {
                return Ok(Friendship::Pending);
            }
            if !request.ok() {
                return Err(request.error("Roblox did not send the friend request."));
            }
        }
        let accept = self
            .web
            .write_authed(
                Method::POST,
                url(
                    friends,
                    &format!("/v1/users/{}/accept-friend-request", asker.user_id),
                )?,
                &accepter.cookie,
                None,
                cancel,
            )
            .await?;
        rate_limited(&accept)?;
        if accept.ok() {
            Ok(Friendship::Added)
        } else if captcha_required(&accept) {
            Ok(Friendship::Pending)
        } else {
            Err(accept.error("Roblox did not accept the friend request."))
        }
    }
}

enum Friendship {
    Added,
    Already,
    Pending,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_settings_come_with_the_values_roblox_offers() {
        let value = json!({
            "whoCanJoinMeInExperiences": {
                "currentValue": "Friends",
                "options": [
                    { "option": { "optionValue": "AllUsers" }, "requirement": null },
                    { "option": { "optionValue": "Friends" } },
                    { "option": { "optionValue": "NoOne" } }
                ]
            },
            "whoCanSeeMyInventory": { "currentValue": "NoOne", "options": ["AllUsers", "NoOne"] },
            "allowVoice": { "currentValue": "On", "options": ["On", "Off"] },
            "whoCanBroken": { "currentValue": "x y", "options": ["A"] }
        });
        let settings = parse_privacy(&value);
        assert_eq!(settings.len(), 2, "only who-can settings with clean values");
        assert_eq!(settings[0].key, "whoCanJoinMeInExperiences");
        assert_eq!(settings[0].value, "Friends");
        assert_eq!(settings[0].options, ["AllUsers", "Friends", "NoOne"]);
        assert_eq!(settings[1].options, ["AllUsers", "NoOne"]);
        assert!(settings[0].restricted.is_empty());

        // Roblox's own answer for in-game chat: everyone needs an age check, whispers follow.
        let chat = parse_privacy(&json!({
            "whoCanChatWithMeInExperiences": {"currentValue": "AllUsers", "options": [
                {"option": {"optionValue": "AllUsers"}, "requirement": "FacialAgeEstimation"},
                {"option": {"optionValue": "NoOne"}}
            ]},
            "whoCanSeeMyInventory": {"currentValue": "AllUsers", "options": [
                {"option": {"optionValue": "AllUsers"}},
                {"option": {"optionValue": "Friends"}, "requirement": "SelfUpdateSetting"}
            ]},
            "whoCanWhisperChatWithMeInExperiences": {"currentValue": "AllUsers", "options": [
                {"option": {"optionValue": "AllUsers"}, "requirement": "Inherited"},
                {"option": {"optionValue": "NoOne"}}
            ]}
        }));
        assert_eq!(
            chat[0].restricted,
            [RestrictedOption {
                value: "AllUsers".into(),
                requirement: "FacialAgeEstimation".into()
            }]
        );
        assert!(chat[1].restricted.is_empty(), "the owner may choose these");
        assert_eq!(chat[2].restricted[0].requirement, "Inherited");
    }

    #[test]
    fn quick_login_codes_are_checked_before_anything_is_sent() {
        assert_eq!(quick_code(" ab3-d9z ").unwrap(), "AB3D9Z");
        assert!(quick_code("ABC12").is_err());
        assert!(quick_code("ABC12!").is_err());
    }
}

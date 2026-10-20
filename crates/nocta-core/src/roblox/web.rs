//! Bounded HTTP access to Roblox web endpoints used by the launcher.
//!
//! Every request has a timeout, a body limit and cancellation. Redirects are followed manually so each hop is
//! re-checked against the host allowlist, and the session cookie is attached only to HTTPS `roblox.com` hosts.
use crate::{secret::SecretString, sessions::http::RobloxClient, CoreError, CoreResult};
use reqwest::{
    header::{HeaderMap, HeaderValue, CONTENT_TYPE, COOKIE, LOCATION, REFERER, SET_COOKIE},
    Client, Method, RequestBuilder, Response, Url,
};
use serde_json::Value;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub const MAX_BODY: usize = 2 * 1024 * 1024;
pub const MAX_REDIRECTS: usize = 5;
pub const SIGNED_OUT: &str = "This account is signed out. Refresh its cookie and try again.";
pub const RATE_LIMITED: &str = "Roblox is limiting requests right now. Try again in a minute.";
pub const MODERATED: &str =
    "Roblox has moderated (banned) this account, so it cannot be used for this.";

/// Roblox answers requests made with a banned account's session with 403 "User is moderated".
/// Roblox wants the account owner to confirm the change (password or 2-step code).
pub const CHALLENGED: &str =
    "Roblox wants to confirm it's you for this change. Make it on roblox.com, signed in as this account.";
/// Roblox wants a captcha first. It asks for one only now and then, mostly after many changes in
/// a short time, so a later try usually goes through.
pub const CHALLENGED_CAPTCHA: &str =
    "Roblox wants a captcha for this right now. Try again in a few minutes, or do it on roblox.com.";

/// Whether an error is one of Roblox's checks rather than a refusal.
pub fn is_challenge(message: &str) -> bool {
    message == CHALLENGED || message == CHALLENGED_CAPTCHA
}

/// The first error message in a Roblox `{"errors": [{"message": …}]}` body.
pub fn roblox_error(body: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(body).ok()?;
    value["errors"]
        .as_array()?
        .iter()
        .find_map(|error| error["message"].as_str())
        .map(|message| {
            message
                .chars()
                .filter(|c| !c.is_control())
                .take(300)
                .collect()
        })
        .filter(|message: &String| !message.trim().is_empty())
}

/// The new session cookie when Roblox rotated it in `Set-Cookie`.
fn rotated_cookie(headers: &HeaderMap) -> Option<SecretString> {
    headers.get_all(SET_COOKIE).iter().find_map(|header| {
        let value = header.to_str().ok()?.strip_prefix(".ROBLOSECURITY=")?;
        let cookie = value.split(';').next()?.trim();
        (!cookie.is_empty()).then(|| SecretString::new(cookie))
    })
}

/// The answer to a write made with an account's cookie.
pub struct AuthedWrite {
    pub status: u16,
    pub body: Vec<u8>,
    /// The session's new cookie, when Roblox replaced it.
    pub new_cookie: Option<SecretString>,
}

impl AuthedWrite {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
    pub fn json(&self) -> CoreResult<Value> {
        serde_json::from_slice(&self.body)
            .map_err(|_| CoreError::Validation("Roblox returned an unexpected response.".into()))
    }
    /// Roblox's own error message, or `fallback`.
    pub fn error(&self, fallback: &str) -> CoreError {
        CoreError::Validation(roblox_error(&self.body).unwrap_or_else(|| fallback.to_owned()))
    }
}

fn moderated(status: u16, body: &[u8]) -> bool {
    status == 403
        && String::from_utf8_lossy(body)
            .to_ascii_lowercase()
            .contains("user is moderated")
}

#[derive(Clone)]
pub struct Endpoints {
    pub auth: String,
    pub games: String,
    pub www: String,
    pub web: String,
    pub economy: String,
    pub thumbnails: String,
    pub apis: String,
    pub users: String,
    pub presence: String,
    pub friends: String,
    pub gamejoin: String,
    /// IP geolocation for server regions (no cookie is ever sent there).
    pub geo: String,
}
impl Endpoints {
    pub fn production() -> Self {
        Self {
            auth: "https://auth.roblox.com".into(),
            games: "https://games.roblox.com".into(),
            www: "https://www.roblox.com".into(),
            web: "https://web.roblox.com".into(),
            economy: "https://economy.roblox.com".into(),
            thumbnails: "https://thumbnails.roblox.com".into(),
            apis: "https://apis.roblox.com".into(),
            users: "https://users.roblox.com".into(),
            presence: "https://presence.roblox.com".into(),
            friends: "https://friends.roblox.com".into(),
            gamejoin: "https://gamejoin.roblox.com".into(),
            geo: "https://ipwho.is".into(),
        }
    }
    pub fn fixture(base: &str) -> Self {
        Self {
            auth: base.into(),
            games: base.into(),
            www: base.into(),
            web: base.into(),
            economy: base.into(),
            thumbnails: base.into(),
            apis: base.into(),
            users: base.into(),
            presence: base.into(),
            friends: base.into(),
            gamejoin: base.into(),
            geo: base.into(),
        }
    }
}

pub struct Page {
    pub final_url: Url,
    pub status: u16,
    pub body: String,
}
pub struct RawResponse {
    pub status: u16,
    pub retry_after: Option<String>,
    pub body: Vec<u8>,
}
impl RawResponse {
    pub fn json(&self) -> CoreResult<Value> {
        serde_json::from_slice(&self.body)
            .map_err(|_| CoreError::Validation("Roblox returned an unexpected response.".into()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareLinkType {
    Server,
    ExperienceInvite,
}
impl ShareLinkType {
    pub fn from_url(url: &Url) -> Option<(Self, String)> {
        let code = super::invite::query(url, "code")
            .map(|code| code.trim().to_owned())
            .filter(|code| !code.is_empty() && code.len() <= 200)?;
        let kind = match super::invite::query(url, "type")?.to_lowercase().as_str() {
            "server" => Self::Server,
            "experienceinvite" => Self::ExperienceInvite,
            _ => return None,
        };
        Some((kind, code))
    }
    fn api_name(self) -> &'static str {
        match self {
            Self::Server => "Server",
            Self::ExperienceInvite => "ExperienceInvite",
        }
    }
}

fn share_id(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => number.as_u64().map(|id| id.to_string()),
        Value::String(text) => Some(text.trim().to_owned()).filter(|text| !text.is_empty()),
        _ => None,
    }
}

/// The direct link for a resolved share link, or why it cannot be joined.
fn parse_share_link(value: &Value, kind: ShareLinkType) -> CoreResult<Option<String>> {
    let data = match kind {
        ShareLinkType::Server => &value["privateServerInviteData"],
        ShareLinkType::ExperienceInvite => &value["experienceInviteData"],
    };
    if data.is_null() {
        return Ok(None);
    }
    if let Some(status) = data["status"].as_str() {
        if !status.eq_ignore_ascii_case("valid") {
            return Err(CoreError::Validation(format!(
                "Roblox says this share link is not usable ({status}). Ask for a new link."
            )));
        }
    }
    let Some(place) =
        share_id(&data["placeId"]).filter(|id| id.bytes().all(|b| b.is_ascii_digit()))
    else {
        return Ok(None);
    };
    let mut url = Url::parse(&format!("https://www.roblox.com/games/{place}/"))
        .map_err(|_| CoreError::Network)?;
    match kind {
        ShareLinkType::Server => {
            let Some(code) = share_id(&data["linkCode"]) else {
                return Ok(None);
            };
            url.query_pairs_mut()
                .append_pair("privateServerLinkCode", &code);
        }
        ShareLinkType::ExperienceInvite => {
            if let Some(job) = share_id(&data["instanceId"]) {
                url.query_pairs_mut().append_pair("gameInstanceId", &job);
            }
        }
    }
    Ok(Some(url.into()))
}

#[derive(Default)]
pub struct TicketAttempt {
    pub ticket: Option<SecretString>,
    pub csrf: Option<SecretString>,
    pub unauthorized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyOutcome {
    AlreadyAllUsers,
    Updated,
}

pub struct RobloxWeb {
    client: Client,
    pub endpoints: Endpoints,
    allow_loopback: bool,
    /// The browser ID a browser keeps in `RBXEventTrackerV2`; some settings writes need one.
    tracker: String,
}

/// A browser ID like the ones Roblox hands out, made once per run.
fn new_tracker() -> String {
    (100_000_000_000u64 + rand::random::<u32>() as u64).to_string()
}

impl RobloxWeb {
    pub fn new() -> CoreResult<Self> {
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .connect_timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("Mozilla/5.0")
                .build()
                .map_err(|_| CoreError::Network)?,
            endpoints: Endpoints::production(),
            allow_loopback: false,
            tracker: new_tracker(),
        })
    }
    /// Test constructor: every endpoint points to a loopback fixture and plain HTTP loopback is allowed.
    pub fn fixture(base: &str) -> Self {
        Self {
            client: Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(5))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            endpoints: Endpoints::fixture(base),
            allow_loopback: true,
            tracker: new_tracker(),
        }
    }

    fn loopback(&self, url: &Url) -> bool {
        self.allow_loopback
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
    }
    /// Hosts that may receive the account cookie.
    pub fn cookie_host(&self, url: &Url) -> bool {
        if self.loopback(url) {
            return true;
        }
        url.scheme() == "https"
            && url.port().is_none()
            && url.username().is_empty()
            && url.password().is_none()
            && url
                .host_str()
                .is_some_and(|host| host == "roblox.com" || host.ends_with(".roblox.com"))
    }
    /// Hosts that may be visited during invite resolution. `ro.blox.com` short links are followed without a cookie.
    pub fn hop_host(&self, url: &Url) -> bool {
        self.cookie_host(url)
            || (url.scheme() == "https"
                && url.port().is_none()
                && url.host_str() == Some("ro.blox.com"))
    }
    pub fn validate_invite_url(&self, raw: &str) -> CoreResult<Url> {
        let raw = raw.trim();
        if raw.len() > 2048 {
            return Err(CoreError::Validation("The invite link is too long.".into()));
        }
        let url = Url::parse(raw)
            .map_err(|_| CoreError::Validation("Enter a valid invite or game link.".into()))?;
        if !self.hop_host(&url) {
            return Err(CoreError::Validation(
                "Invite links must be HTTPS roblox.com links.".into(),
            ));
        }
        Ok(url)
    }

    async fn send(
        &self,
        request: RequestBuilder,
        cancel: &CancellationToken,
    ) -> CoreResult<Response> {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(CoreError::Cancelled),
            result = request.send() => result.map_err(|error| if error.is_timeout() { CoreError::Timeout } else { CoreError::Network }),
        }
    }
    async fn body(response: Response, cancel: &CancellationToken) -> CoreResult<Vec<u8>> {
        if response
            .content_length()
            .is_some_and(|length| length as usize > MAX_BODY)
        {
            return Err(CoreError::Validation(
                "Roblox returned a response that is too large.".into(),
            ));
        }
        let mut response = response;
        let mut body = Vec::new();
        loop {
            let chunk = tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                chunk = response.chunk() => chunk.map_err(|error| if error.is_timeout() { CoreError::Timeout } else { CoreError::Network })?,
            };
            let Some(chunk) = chunk else { break };
            if body.len() + chunk.len() > MAX_BODY {
                return Err(CoreError::Validation(
                    "Roblox returned a response that is too large.".into(),
                ));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }

    /// Authenticated GET with manually checked redirects. Used for invite and private-server pages.
    pub async fn get_page(
        &self,
        url: Url,
        cookie: &str,
        referer: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<Page> {
        let cookie_value = RobloxClient::cookie(cookie)?;
        let mut current = url;
        for _ in 0..=MAX_REDIRECTS {
            if !self.hop_host(&current) {
                return Err(CoreError::Validation(
                    "The link redirected outside roblox.com and was not followed.".into(),
                ));
            }
            let mut request = self.client.get(current.clone()).header(REFERER, referer);
            if self.cookie_host(&current) {
                request = request.header(COOKIE, cookie_value.clone());
            }
            let response = self.send(request, cancel).await?;
            let status = response.status();
            if status.is_redirection() {
                let location = response
                    .headers()
                    .get(LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| {
                        CoreError::Validation("Roblox returned an invalid redirect.".into())
                    })?;
                current = current.join(location).map_err(|_| {
                    CoreError::Validation("Roblox returned an invalid redirect.".into())
                })?;
                continue;
            }
            let body = Self::body(response, cancel).await?;
            return Ok(Page {
                final_url: current,
                status: status.as_u16(),
                body: String::from_utf8_lossy(&body).into_owned(),
            });
        }
        Err(CoreError::Validation(
            "The link redirected too many times.".into(),
        ))
    }

    /// API GET without redirects. The cookie is sent only to allowed Roblox hosts.
    pub async fn get_raw(
        &self,
        url: Url,
        cookie: Option<&str>,
        referer: Option<&str>,
        cancel: &CancellationToken,
    ) -> CoreResult<RawResponse> {
        let mut request = self.client.get(url.clone());
        if let Some(referer) = referer {
            request = request.header(REFERER, referer);
        }
        if let Some(cookie) = cookie {
            if !self.cookie_host(&url) {
                return Err(CoreError::Validation(
                    "Refusing to send account credentials to a non-Roblox host.".into(),
                ));
            }
            request = request.header(COOKIE, RobloxClient::cookie(cookie)?);
        }
        self.raw(request, cancel).await
    }
    pub async fn post_json_raw(
        &self,
        url: Url,
        body: &Value,
        cancel: &CancellationToken,
    ) -> CoreResult<RawResponse> {
        self.raw(self.client.post(url).json(body), cancel).await
    }
    async fn raw(
        &self,
        request: RequestBuilder,
        cancel: &CancellationToken,
    ) -> CoreResult<RawResponse> {
        let response = self.send(request, cancel).await?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = if status == 429 {
            Vec::new()
        } else {
            Self::body(response, cancel).await?
        };
        Ok(RawResponse {
            status,
            retry_after,
            body,
        })
    }

    fn ticket_url(&self) -> CoreResult<Url> {
        Url::parse(&format!("{}/v1/authentication-ticket", self.endpoints.auth))
            .map_err(|_| CoreError::Network)
    }

    /// One legacy ticket attempt: a CSRF request, then the ticket request with `x-csrf-token`. No JSON body.
    pub async fn authentication_ticket(
        &self,
        cookie: &str,
        place_id: &str,
        previous_csrf: Option<&SecretString>,
        cancel: &CancellationToken,
    ) -> CoreResult<TicketAttempt> {
        let url = self.ticket_url()?;
        if !self.cookie_host(&url) {
            return Err(CoreError::Validation(
                "Refusing to send account credentials to a non-Roblox host.".into(),
            ));
        }
        let cookie = RobloxClient::cookie(cookie)?;
        let referer = format!("https://www.roblox.com/games/{place_id}/");
        let first = self
            .send(
                self.client
                    .post(url.clone())
                    .header(COOKIE, cookie.clone())
                    .header(REFERER, &referer)
                    .body(Vec::new()),
                cancel,
            )
            .await?;
        if first.status() == 401 {
            return Ok(TicketAttempt {
                unauthorized: true,
                ..Default::default()
            });
        }
        let csrf = first
            .headers()
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty())
            .map(SecretString::new)
            .or_else(|| previous_csrf.cloned());
        // The first request is only used for its token header.
        drop(first);
        let Some(csrf) = csrf else {
            return Ok(TicketAttempt::default());
        };
        let mut token = HeaderValue::from_str(csrf.expose())
            .map_err(|_| CoreError::Validation("Roblox returned an invalid CSRF token.".into()))?;
        token.set_sensitive(true);
        let second = self
            .send(
                self.client
                    .post(url)
                    .header(COOKIE, cookie)
                    .header("x-csrf-token", token)
                    .header(REFERER, &referer)
                    .header(CONTENT_TYPE, "application/json")
                    .body(Vec::new()),
                cancel,
            )
            .await?;
        if second.status() == 401 {
            return Ok(TicketAttempt {
                csrf: Some(csrf),
                unauthorized: true,
                ..Default::default()
            });
        }
        let ticket = second
            .headers()
            .get("rbx-authentication-ticket")
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty())
            .map(SecretString::new);
        Ok(TicketAttempt {
            ticket,
            csrf: Some(csrf),
            unauthorized: false,
        })
    }

    /// POST with the account cookie. Roblox answers the first write with 403 and an
    /// `x-csrf-token`; the request is repeated once with that token. A 401 means the session
    /// is signed out and is reported as [`SIGNED_OUT`].
    pub async fn post_json_authed(
        &self,
        url: Url,
        cookie: &str,
        body: &Value,
        cancel: &CancellationToken,
    ) -> CoreResult<RawResponse> {
        let write = self
            .write_authed(Method::POST, url, cookie, Some(body), cancel)
            .await?;
        Ok(RawResponse {
            status: write.status,
            retry_after: None,
            body: write.body,
        })
    }

    /// A write (`POST`, `PATCH`, `DELETE`…) with the account cookie and the CSRF round trip of
    /// [`Self::post_json_authed`]. A refused write that needs verification is [`CHALLENGED`]. A cookie Roblox rotated comes back in `new_cookie`.
    pub async fn write_authed(
        &self,
        method: Method,
        url: Url,
        cookie: &str,
        body: Option<&Value>,
        cancel: &CancellationToken,
    ) -> CoreResult<AuthedWrite> {
        self.write_authed_as(method, url, cookie, body, None, cancel)
            .await
    }

    /// [`Self::write_authed`] with another `User-Agent`: Roblox's join service only answers its
    /// own client (`Roblox/WinInet`).
    pub async fn write_authed_as(
        &self,
        method: Method,
        url: Url,
        cookie: &str,
        body: Option<&Value>,
        user_agent: Option<&str>,
        cancel: &CancellationToken,
    ) -> CoreResult<AuthedWrite> {
        if !self.cookie_host(&url) {
            return Err(CoreError::Validation(
                "Refusing to send account credentials to a non-Roblox host.".into(),
            ));
        }
        // Like a browser, writes carry the session and the browser tracker cookie.
        let session = RobloxClient::cookie(cookie)?;
        let mut cookie = HeaderValue::from_str(&format!(
            "{}; RBXEventTrackerV2=CreateDate={}&rbxid=&browserid={}",
            session.to_str().map_err(|_| CoreError::Network)?,
            chrono::Utc::now().format("%-m/%-d/%Y %-I:%M:%S %p"),
            self.tracker
        ))
        .map_err(|_| CoreError::Network)?;
        cookie.set_sensitive(true);
        let request = |token: Option<&HeaderValue>| {
            let mut request = self
                .client
                .request(method.clone(), url.clone())
                .header(COOKIE, cookie.clone())
                .header(REFERER, "https://www.roblox.com/");
            if let Some(agent) = user_agent {
                request = request.header(reqwest::header::USER_AGENT, agent);
            }
            request = match body {
                Some(body) => request.json(body),
                None => request.header(CONTENT_TYPE, "application/json").body("{}"),
            };
            if let Some(token) = token {
                request = request.header("x-csrf-token", token.clone());
            }
            request
        };
        let mut response = self.send(request(None), cancel).await?;
        if response.status() == 403 {
            if let Some(mut token) = response.headers().get("x-csrf-token").cloned() {
                token.set_sensitive(true);
                drop(response);
                response = self.send(request(Some(&token)), cancel).await?;
            }
        }
        let status = response.status().as_u16();
        if status == 401 {
            return Err(CoreError::Validation(SIGNED_OUT.into()));
        }
        let challenge = response
            .headers()
            .contains_key("rblx-challenge-id")
            .then(|| {
                response
                    .headers()
                    .get("rblx-challenge-type")
                    .and_then(|kind| kind.to_str().ok())
                    .unwrap_or("")
                    .to_ascii_lowercase()
            });
        let new_cookie = rotated_cookie(response.headers());
        let body = if status == 429 {
            Vec::new()
        } else {
            Self::body(response, cancel).await?
        };
        if moderated(status, &body) {
            return Err(CoreError::Validation(MODERATED.into()));
        }
        if let Some(kind) = challenge.filter(|_| !(200..300).contains(&status)) {
            return Err(CoreError::Validation(
                if kind.contains("captcha") {
                    CHALLENGED_CAPTCHA
                } else {
                    CHALLENGED
                }
                .into(),
            ));
        }
        Ok(AuthedWrite {
            status,
            body,
            new_cookie,
        })
    }

    /// GET with the account cookie; a 401 is reported as [`SIGNED_OUT`].
    pub async fn get_json_authed(
        &self,
        url: Url,
        cookie: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<Value> {
        let response = self
            .get_raw(url, Some(cookie), Some("https://www.roblox.com/"), cancel)
            .await?;
        if response.status == 401 {
            return Err(CoreError::Validation(SIGNED_OUT.into()));
        }
        if response.status == 429 {
            return Err(CoreError::Validation(RATE_LIMITED.into()));
        }
        if moderated(response.status, &response.body) {
            return Err(CoreError::Validation(MODERATED.into()));
        }
        if !(200..300).contains(&response.status) {
            return Err(CoreError::Validation(
                "Roblox could not answer this request right now.".into(),
            ));
        }
        response.json()
    }

    /// Turns a share link (`/share?code=…&type=Server|ExperienceInvite`) into a direct game link
    /// through Roblox's share-links API. Roblox only answers signed-in users, so a cookie is
    /// needed, but any account works. `Ok(None)` means Roblox did not recognise the link.
    pub async fn resolve_share_link(
        &self,
        cookie: &str,
        code: &str,
        link_type: ShareLinkType,
        cancel: &CancellationToken,
    ) -> CoreResult<Option<String>> {
        let url = Url::parse(&format!(
            "{}/sharelinks/v1/resolve-link",
            self.endpoints.apis
        ))
        .map_err(|_| CoreError::Network)?;
        let body = serde_json::json!({ "linkId": code, "linkType": link_type.api_name() });
        let response = self.post_json_authed(url, cookie, &body, cancel).await?;
        if !(200..300).contains(&response.status) {
            tracing::debug!(status = response.status, "Share link lookup failed");
            return Ok(None);
        }
        parse_share_link(&response.json()?, link_type)
    }

    /// Optional account preference write, only called when explicitly enabled for private invites.
    pub async fn ensure_invite_privacy(
        &self,
        cookie: &str,
        csrf: &SecretString,
        place_id: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<PrivacyOutcome> {
        let url = Url::parse(&format!(
            "{}/account/settings/private-server-invite-privacy",
            self.endpoints.www
        ))
        .map_err(|_| CoreError::Network)?;
        let referer = format!("https://www.roblox.com/games/{place_id}/");
        let page = self.get_page(url.clone(), cookie, &referer, cancel).await?;
        if page.status == 200 && privacy_is_all_users(&page.body) {
            return Ok(PrivacyOutcome::AlreadyAllUsers);
        }
        if !self.cookie_host(&url) {
            return Err(CoreError::Validation(
                "Refusing to send account credentials to a non-Roblox host.".into(),
            ));
        }
        let mut token = HeaderValue::from_str(csrf.expose())
            .map_err(|_| CoreError::Validation("Roblox returned an invalid CSRF token.".into()))?;
        token.set_sensitive(true);
        let response = self
            .send(
                self.client
                    .post(url)
                    .header(COOKIE, RobloxClient::cookie(cookie)?)
                    .header("x-csrf-token", token)
                    .header(REFERER, &referer)
                    .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                    .body("PrivateServerInvitePrivacy=AllUsers"),
                cancel,
            )
            .await?;
        if response.status().is_success() {
            Ok(PrivacyOutcome::Updated)
        } else {
            Err(CoreError::Validation(format!(
                "Private invite privacy update returned HTTP {}.",
                response.status().as_u16()
            )))
        }
    }
}

/// Stricter than the legacy `includes('AllUsers')`: the value must be attached to the privacy setting.
fn privacy_is_all_users(body: &str) -> bool {
    static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    PATTERN
        .get_or_init(|| {
            regex::Regex::new(r#"(?i)privateserverinviteprivacy["']?\s*[:=]\s*["']?allusers\b"#)
                .unwrap()
        })
        .is_match(body)
}

/// Parses `Retry-After` seconds or HTTP date into milliseconds.
pub fn retry_after_ms(value: &str) -> Option<u64> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(seconds.saturating_mul(1000));
    }
    chrono::DateTime::parse_from_rfc2822(value)
        .ok()
        .map(|date| (date.timestamp_millis() - chrono::Utc::now().timestamp_millis()).max(0) as u64)
}

#[cfg(test)]
pub(crate) mod fixture {
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    /// Minimal HTTP/1.1 fixture: answers queued responses in order and records raw requests.
    pub async fn serve(responses: Vec<String>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut buffer = [0u8; 4096];
                    let read = socket.read(&mut buffer).await.unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buffer[..read]);
                    if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                        let length: usize = headers
                            .lines()
                            .find_map(|line| {
                                line.strip_prefix("content-length:")
                                    .and_then(|value| value.trim().parse().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8_lossy(&bytes).into_owned());
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            }
            requests
        });
        (base, server)
    }
    pub fn response(status: &str, headers: &[(&str, &str)], body: &str) -> String {
        let mut text = format!("HTTP/1.1 {status}\r\n");
        for (name, value) in headers {
            text.push_str(&format!("{name}: {value}\r\n"));
        }
        text.push_str(&format!(
            "Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ));
        text
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::{response, serve};
    use super::*;

    #[test]
    fn cookie_and_hop_allowlists() {
        let web = RobloxWeb::new().unwrap();
        for allowed in [
            "https://www.roblox.com/games/1",
            "https://web.roblox.com/x",
            "https://roblox.com/",
        ] {
            assert!(web.cookie_host(&Url::parse(allowed).unwrap()), "{allowed}");
        }
        for rejected in [
            "http://www.roblox.com/",
            "https://roblox.com.evil.test/",
            "https://evilroblox.com/",
            "https://www.roblox.com:8443/",
            "https://user@www.roblox.com/",
            "http://127.0.0.1/",
            "https://ro.blox.com/abc",
        ] {
            assert!(
                !web.cookie_host(&Url::parse(rejected).unwrap()),
                "{rejected}"
            );
        }
        assert!(web.hop_host(&Url::parse("https://ro.blox.com/abc").unwrap()));
        assert!(web
            .validate_invite_url("https://evil.test/games/1")
            .is_err());
        assert!(web.validate_invite_url("not a url").is_err());
        assert!(web
            .validate_invite_url(&format!("https://www.roblox.com/{}", "a".repeat(3000)))
            .is_err());
    }

    #[test]
    fn retry_after_seconds_and_dates() {
        assert_eq!(retry_after_ms("3"), Some(3000));
        assert_eq!(retry_after_ms("Wed, 21 Oct 2015 07:28:00 GMT"), Some(0));
        assert_eq!(retry_after_ms("soon"), None);
    }

    #[tokio::test]
    async fn redirects_are_rechecked_and_bodies_bounded() {
        let (base, server) = serve(vec![
            response(
                "302 Found",
                &[("Location", "/games/7?privateServerLinkCode=abc")],
                "",
            ),
            response("200 OK", &[], "page"),
        ])
        .await;
        let web = RobloxWeb::fixture(&base);
        let page = web
            .get_page(
                Url::parse(&format!("{base}/share?code=1&type=Server")).unwrap(),
                "synthetic-cookie",
                "https://www.roblox.com/",
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(page.body, "page");
        assert!(page
            .final_url
            .as_str()
            .ends_with("/games/7?privateServerLinkCode=abc"));
        let requests = server.await.unwrap();
        assert!(requests[1].contains(".ROBLOSECURITY=synthetic-cookie"));

        let (base, server) = serve(vec![response(
            "302 Found",
            &[("Location", "https://evil.test/steal")],
            "",
        )])
        .await;
        let error = RobloxWeb::fixture(&base)
            .get_page(
                Url::parse(&base).unwrap(),
                "synthetic-cookie",
                "https://www.roblox.com/",
                &CancellationToken::new(),
            )
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("outside roblox.com"));
        server.await.unwrap();

        let redirects = (0..=MAX_REDIRECTS)
            .map(|_| response("302 Found", &[("Location", "/again")], ""))
            .collect();
        let (base, server) = serve(redirects).await;
        assert!(RobloxWeb::fixture(&base)
            .get_page(
                Url::parse(&base).unwrap(),
                "c",
                "https://www.roblox.com/",
                &CancellationToken::new()
            )
            .await
            .is_err());
        server.await.unwrap();

        let large = "x".repeat(MAX_BODY + 1);
        let (base, server) = serve(vec![response("200 OK", &[], &large)]).await;
        let error = RobloxWeb::fixture(&base)
            .get_raw(
                Url::parse(&base).unwrap(),
                None,
                None,
                &CancellationToken::new(),
            )
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("too large"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn share_links_resolve_after_the_csrf_challenge() {
        let json = |body: &str| response("200 OK", &[("Content-Type", "application/json")], body);
        let (base, server) = serve(vec![
            response("403 Forbidden", &[("x-csrf-token", "synthetic-csrf")], ""),
            json(r#"{"privateServerInviteData":{"status":"Valid","placeId":118637423917462,"linkCode":"abc 123"}}"#),
            // A signed-out cookie.
            response("401 Unauthorized", &[], ""),
            // An expired link.
            json(r#"{"privateServerInviteData":{"status":"Expired","placeId":1,"linkCode":"x"}}"#),
            // An experience invite with its server.
            json(r#"{"experienceInviteData":{"status":"Valid","placeId":"42","instanceId":"job-1"}}"#),
        ])
        .await;
        let web = RobloxWeb::fixture(&base);
        let cancel = CancellationToken::new();
        let link = Url::parse("https://www.roblox.com/share?code=c0de&type=Server").unwrap();
        let (kind, code) = ShareLinkType::from_url(&link).unwrap();
        assert_eq!((kind, code.as_str()), (ShareLinkType::Server, "c0de"));
        assert_eq!(
            web.resolve_share_link("synthetic-cookie", &code, kind, &cancel)
                .await
                .unwrap()
                .as_deref(),
            Some("https://www.roblox.com/games/118637423917462/?privateServerLinkCode=abc+123")
        );
        let signed_out = web
            .resolve_share_link("synthetic-cookie", &code, kind, &cancel)
            .await
            .unwrap_err();
        assert_eq!(signed_out.to_string(), SIGNED_OUT);
        assert!(web
            .resolve_share_link("synthetic-cookie", &code, kind, &cancel)
            .await
            .unwrap_err()
            .to_string()
            .contains("Expired"));
        assert_eq!(
            web.resolve_share_link(
                "synthetic-cookie",
                "c",
                ShareLinkType::ExperienceInvite,
                &cancel
            )
            .await
            .unwrap()
            .as_deref(),
            Some("https://www.roblox.com/games/42/?gameInstanceId=job-1")
        );
        let requests = server.await.unwrap();
        assert!(requests[0].starts_with("POST /sharelinks/v1/resolve-link"));
        assert!(requests[1]
            .to_lowercase()
            .contains("x-csrf-token: synthetic-csrf"));
        assert!(requests[1].ends_with(r#"{"linkId":"c0de","linkType":"Server"}"#));
        assert!(ShareLinkType::from_url(
            &Url::parse("https://www.roblox.com/share?code=c&type=Profile").unwrap()
        )
        .is_none());
    }

    #[tokio::test]
    async fn ticket_uses_csrf_rotation_without_json_body() {
        let (base, server) = serve(vec![
            response("403 Forbidden", &[("x-csrf-token", "synthetic-csrf")], ""),
            response(
                "200 OK",
                &[("rbx-authentication-ticket", "synthetic-ticket")],
                "",
            ),
        ])
        .await;
        let attempt = RobloxWeb::fixture(&base)
            .authentication_ticket("synthetic-cookie", "42", None, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(attempt.ticket.unwrap().expose(), "synthetic-ticket");
        let requests = server.await.unwrap();
        assert!(requests[0].starts_with("POST /v1/authentication-ticket"));
        assert!(!requests[0].to_lowercase().contains("x-csrf-token"));
        assert!(requests[0]
            .to_lowercase()
            .contains("referer: https://www.roblox.com/games/42/"));
        assert!(requests[1]
            .to_lowercase()
            .contains("x-csrf-token: synthetic-csrf"));
        assert!(requests[1].ends_with("\r\n\r\n"), "no request body");

        // A missing ticket is reported, and a previous token is reused when the challenge returns none.
        let (base, server) = serve(vec![
            response("403 Forbidden", &[], ""),
            response("403 Forbidden", &[], ""),
        ])
        .await;
        let attempt = RobloxWeb::fixture(&base)
            .authentication_ticket(
                "synthetic-cookie",
                "42",
                Some(&SecretString::new("old-token")),
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(attempt.ticket.is_none());
        assert!(server.await.unwrap()[1]
            .to_lowercase()
            .contains("x-csrf-token: old-token"));

        let (base, server) = serve(vec![response("401 Unauthorized", &[], "")]).await;
        assert!(
            RobloxWeb::fixture(&base)
                .authentication_ticket("synthetic-cookie", "42", None, &CancellationToken::new())
                .await
                .unwrap()
                .unauthorized
        );
        server.await.unwrap();
    }

    #[tokio::test]
    async fn privacy_update_posts_exact_form_body() {
        let (base, server) = serve(vec![
            response(
                "200 OK",
                &[],
                "<select name=\"PrivateServerInvitePrivacy\"><option value=\"NoOne\" selected>",
            ),
            response("200 OK", &[], ""),
        ])
        .await;
        let outcome = RobloxWeb::fixture(&base)
            .ensure_invite_privacy(
                "synthetic-cookie",
                &SecretString::new("csrf"),
                "1",
                &CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(outcome, PrivacyOutcome::Updated);
        let requests = server.await.unwrap();
        assert!(requests[1].ends_with("PrivateServerInvitePrivacy=AllUsers"));
        assert!(privacy_is_all_users(
            "{\"PrivateServerInvitePrivacy\":\"AllUsers\"}"
        ));
        assert!(!privacy_is_all_users("AllUsers appears elsewhere"));
    }

    #[tokio::test]
    async fn cancellation_stops_requests() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let web = RobloxWeb::fixture("http://127.0.0.1:9");
        assert!(matches!(
            web.authentication_ticket("c", "1", None, &cancel).await,
            Err(CoreError::Cancelled)
        ));
    }
}

use crate::{models::AccountHealth, CoreError, CoreResult};
use reqwest::{
    header::{HeaderValue, COOKIE},
    Client, Response,
};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
pub struct Identity {
    pub id: u64,
    pub name: String,
}
pub struct JoinProbe {
    pub health: AccountHealth,
    pub challenge_url: Option<String>,
}
pub struct RobloxClient {
    client: Client,
    users: String,
    auth: String,
    join: String,
}

impl RobloxClient {
    pub fn new() -> CoreResult<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(12))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("Mozilla/5.0")
            .build()
            .map_err(|_| CoreError::Network)?;
        Ok(Self {
            client,
            users: "https://users.roblox.com".into(),
            auth: "https://auth.roblox.com".into(),
            join: "https://gamejoin.roblox.com".into(),
        })
    }
    #[cfg(test)]
    pub(crate) fn use_fixture(&mut self, base: &str) {
        self.client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        self.users = base.into();
        self.auth = base.into();
        self.join = base.into();
    }
    pub(crate) fn cookie(cookie: &str) -> CoreResult<HeaderValue> {
        if cookie.is_empty() || cookie.contains(';') {
            return Err(CoreError::Validation(
                "The saved cookie is empty or malformed.".into(),
            ));
        }
        let mut value = HeaderValue::from_str(&format!(".ROBLOSECURITY={cookie}"))
            .map_err(|_| CoreError::Validation("The saved cookie is malformed.".into()))?;
        value.set_sensitive(true);
        Ok(value)
    }
    async fn send(
        &self,
        request: reqwest::RequestBuilder,
        cancel: &CancellationToken,
    ) -> CoreResult<Response> {
        tokio::select! { biased; _ = cancel.cancelled() => Err(CoreError::Cancelled), result = request.send() => result.map_err(|error| if error.is_timeout() { CoreError::Timeout } else { CoreError::Network }) }
    }
    pub async fn identity(
        &self,
        cookie: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<Option<Identity>> {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        if cookie.is_empty() {
            return Ok(None);
        }
        let response = self
            .send(
                self.client
                    .get(format!("{}/v1/users/authenticated", self.users))
                    .header(COOKIE, Self::cookie(cookie)?),
                cancel,
            )
            .await?;
        if response.status() == 401 {
            return Ok(None);
        }
        if response.status() != 200 {
            return Err(CoreError::Validation(format!(
                "Session validation returned HTTP {}. Try again later.",
                response.status().as_u16()
            )));
        }
        tokio::select! { biased; _ = cancel.cancelled() => Err(CoreError::Cancelled), identity = response.json::<Identity>() => identity.map(Some).map_err(|_| CoreError::Network) }
    }
    pub async fn check(
        &self,
        cookie: &str,
        place_id: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<JoinProbe> {
        self.check_join(cookie, place_id, None, cancel).await
    }
    /// Join precheck. Launches pass their own join IDs so the precheck and PlaceLauncher share them.
    pub async fn check_join(
        &self,
        cookie: &str,
        place_id: &str,
        ids: Option<&crate::launch::JoinIds>,
        cancel: &CancellationToken,
    ) -> CoreResult<JoinProbe> {
        let generated;
        let ids = match ids {
            Some(ids) => ids,
            None => {
                generated = crate::launch::JoinIds::generate();
                &generated
            }
        };
        let place = place_id
            .parse::<u64>()
            .ok()
            .filter(|id| *id > 0)
            .ok_or_else(|| {
                CoreError::Validation(
                    "Enter a valid Place ID before checking account health.".into(),
                )
            })?;
        let cookie = Self::cookie(cookie)?;
        // Intentionally omit X-CSRF-TOKEN: this retrieves the challenge token without logging out.
        let csrf = self
            .send(
                self.client
                    .post(format!("{}/v2/logout", self.auth))
                    .header(COOKIE, cookie.clone())
                    .header("Origin", "https://www.roblox.com"),
                cancel,
            )
            .await?;
        if csrf.status() == 401 {
            return Ok(JoinProbe {
                health: AccountHealth::InvalidSession,
                challenge_url: None,
            });
        }
        let mut token = csrf.headers().get("x-csrf-token").cloned().ok_or_else(|| {
            CoreError::Validation(
                "The service did not return a CSRF token. Try again later.".into(),
            )
        })?;
        token.set_sensitive(true);
        let response = self.send(self.client.post(format!("{}/v1/join-game", self.join)).header(COOKIE, cookie).header("X-CSRF-TOKEN", token)
            .header("Origin", "https://www.roblox.com").header("Referer", format!("https://www.roblox.com/games/{place}"))
            .json(&json!({"placeId": place, "gamerTag": "", "isPartyLeader": false, "isPlayTogetherGame": false, "browserTrackerId": ids.tracker_id, "isTeleport": false, "gameJoinAttemptId": ids.attempt_id, "eventId": ids.event_id, "isQueueAllowedOverride": true, "isImmersiveAdsTeleport": true, "channelName": null, "isoContext": "", "joinOrigin": "web", "partyId": null})), cancel).await?;
        match response.status().as_u16() {
            200 => Ok(JoinProbe {
                health: AccountHealth::Healthy,
                challenge_url: None,
            }),
            401 => Ok(JoinProbe {
                health: AccountHealth::InvalidSession,
                challenge_url: None,
            }),
            403 => {
                let id = response
                    .headers()
                    .get("rblx-challenge-id")
                    .and_then(|v| v.to_str().ok());
                let metadata = response
                    .headers()
                    .get("rblx-challenge-metadata")
                    .and_then(|v| v.to_str().ok());
                let kind = response
                    .headers()
                    .get("rblx-challenge-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("captcha");
                if let (Some(id), Some(metadata)) = (id, metadata) {
                    let mut url =
                        reqwest::Url::parse("https://www.roblox.com/challenge/cdn/hybrid").unwrap();
                    url.query_pairs_mut().extend_pairs([
                        ("generic-challenge-type", kind),
                        ("app-type", "windows"),
                        ("generic-challenge-id", id),
                        ("challenge-metadata-json", metadata),
                        ("challenge-type", "generic"),
                        ("dark-mode", "false"),
                    ]);
                    Ok(JoinProbe {
                        health: AccountHealth::CaptchaRequired,
                        challenge_url: Some(url.into()),
                    })
                } else {
                    Err(CoreError::Validation(
                        "Join precheck returned 403 without challenge headers.".into(),
                    ))
                }
            }
            status => Err(CoreError::Validation(format!(
                "Join precheck returned HTTP {status}. Try again later."
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn fixture(
        responses: Vec<&'static str>,
    ) -> (RobloxClient, tokio::task::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut buffer = [0u8; 4096];
                    let read = socket.read(&mut buffer).await.unwrap();
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
                                    .and_then(|s| s.trim().parse().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });
        let mut client = RobloxClient::new().unwrap();
        client.client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        client.users = base.clone();
        client.auth = base.clone();
        client.join = base;
        (client, server)
    }
    const CSRF: &str = "HTTP/1.1 403 Forbidden\r\nx-csrf-token: synthetic-token\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

    #[tokio::test]
    async fn identity_and_join_classification_are_distinct() {
        let (client, server) = fixture(vec!["HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 23\r\nConnection: close\r\n\r\n{\"id\":1,\"name\":\"Alice\"}"]).await;
        assert_eq!(
            client
                .identity("synthetic-cookie", &CancellationToken::new())
                .await
                .unwrap()
                .unwrap()
                .name,
            "Alice"
        );
        assert!(server.await.unwrap()[0].starts_with("GET /v1/users/authenticated"));
        for (response, health) in [
            ("HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", AccountHealth::Healthy),
            ("HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", AccountHealth::InvalidSession),
            ("HTTP/1.1 403 Forbidden\r\nrblx-challenge-id: test-id\r\nrblx-challenge-metadata: test/metadata+value\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", AccountHealth::CaptchaRequired),
        ] {
            let (client, server) = fixture(vec![CSRF, response]).await;
            let probe = client.check("synthetic-cookie", "123", &CancellationToken::new()).await.unwrap();
            assert_eq!(probe.health, health);
            if let Some(url) = probe.challenge_url { assert!(url.contains("test%2Fmetadata%2Bvalue")); }
            let requests = server.await.unwrap();
            assert!(!requests[0].to_lowercase().contains("x-csrf-token:"));
            assert!(requests[1].to_lowercase().contains("x-csrf-token: synthetic-token"));
            assert!(requests[1].contains("\"placeId\":123"));
        }
    }

    #[tokio::test]
    async fn ambiguous_statuses_never_become_captcha_and_errors_exclude_credentials() {
        for response in [
            "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        ] {
            let (client, server) = fixture(vec![CSRF, response]).await;
            let error = client
                .check("synthetic-secret-cookie", "123", &CancellationToken::new())
                .await
                .err()
                .unwrap();
            assert!(!error.to_string().contains("synthetic-secret"));
            server.await.unwrap();
        }
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            RobloxClient::new().unwrap().identity("", &cancel).await,
            Err(CoreError::Cancelled)
        ));
        assert!(RobloxClient::cookie("injected\r\nHeader: secret").is_err());
        assert!(RobloxClient::cookie("cookie;other=value").is_err());
    }
}

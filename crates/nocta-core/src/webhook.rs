//! Discord webhook notifications (crashes and rejoins). Only Discord's webhook URLs are accepted,
//! and a failed delivery never affects what triggered it.
use crate::{CoreError, CoreResult};
use reqwest::Url;
use serde_json::json;
use std::time::Duration;

const HOSTS: &[&str] = &[
    "discord.com",
    "ptb.discord.com",
    "canary.discord.com",
    "discordapp.com",
];

/// Accepts `https://discord.com/api/webhooks/<id>/<token>` and its Discord variants.
pub fn validate(raw: &str) -> CoreResult<Url> {
    let invalid = || {
        CoreError::Validation(
            "Paste a Discord webhook link (https://discord.com/api/webhooks/…).".into(),
        )
    };
    let url = Url::parse(raw.trim()).map_err(|_| invalid())?;
    let valid = url.scheme() == "https"
        && url.port().is_none()
        && url.username().is_empty()
        && url.host_str().is_some_and(|host| HOSTS.contains(&host))
        && url.path().starts_with("/api/webhooks/")
        && url.path().trim_end_matches('/').split('/').count() == 5;
    if valid {
        Ok(url)
    } else {
        Err(invalid())
    }
}

pub struct Webhook {
    client: reqwest::Client,
}

impl Default for Webhook {
    fn default() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_default(),
        }
    }
}

impl Webhook {
    /// Posts `message` to the webhook in `raw`. Empty means no webhook. Errors are logged only.
    pub async fn notify(&self, raw: &str, message: &str) {
        if raw.trim().is_empty() {
            return;
        }
        if let Err(error) = self.send(raw, message).await {
            tracing::warn!(%error, "Webhook failed");
        }
    }

    /// Posts `message` to the webhook in `raw` and reports whether Discord accepted it.
    pub async fn send(&self, raw: &str, message: &str) -> CoreResult<()> {
        self.post(validate(raw)?, message).await
    }

    async fn post(&self, url: Url, message: &str) -> CoreResult<()> {
        let content: String = message.chars().take(1900).collect();
        let body = json!({ "username": "Nocta", "content": content });
        let response = self
            .client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|_| CoreError::Validation("Discord could not be reached.".into()))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(CoreError::Validation(format!(
                "Discord refused the message ({}). Check the webhook link.",
                response.status().as_u16()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roblox::web::fixture::{response, serve};

    #[test]
    fn accepts_only_discord_webhook_links() {
        assert!(validate("https://discord.com/api/webhooks/123/abc-DEF").is_ok());
        assert!(validate(" https://canary.discord.com/api/webhooks/1/x ").is_ok());
        for bad in [
            "http://discord.com/api/webhooks/1/x",
            "https://evil.test/api/webhooks/1/x",
            "https://discord.com/api/other/1/x",
            "https://discord.com/api/webhooks/1",
            "https://discord.com:8443/api/webhooks/1/x",
            "not a link",
        ] {
            assert!(validate(bad).is_err(), "{bad}");
        }
    }

    #[tokio::test]
    async fn posts_the_message_as_nocta() {
        let (base, server) = serve(vec![response("204 No Content", &[], "")]).await;
        let webhook = Webhook {
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
        };
        webhook
            .post(
                Url::parse(&format!("{base}/api/webhooks/1/x")).unwrap(),
                "Alice crashed",
            )
            .await
            .unwrap();
        let request = &server.await.unwrap()[0];
        assert!(request.contains(r#""content":"Alice crashed""#));
        assert!(request.contains(r#""username":"Nocta""#));
        // No webhook configured: nothing is sent.
        webhook.notify("", "ignored").await;
        // Links that are not Discord's are refused before anything is sent.
        assert!(webhook
            .send("https://example.com/api/webhooks/1/x", "test")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn a_rejected_message_is_reported_to_the_test_button() {
        let (base, server) = serve(vec![response("404 Not Found", &[], "")]).await;
        let webhook = Webhook {
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
        };
        let error = webhook
            .post(
                Url::parse(&format!("{base}/api/webhooks/1/x")).unwrap(),
                "test",
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("404"), "{error}");
        server.await.unwrap();
    }
}

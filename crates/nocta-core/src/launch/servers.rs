//! Public/VIP server lists with 429 handling, per-request cancellation and an opaque private-target registry.
use super::target::{validate_place_id, ResolvedKind, ResolvedTarget};
use crate::roblox::web::{retry_after_ms, RobloxWeb};
use crate::{
    models::{ServerKind, ServerListOptions, ServerListResponse, ServerRow},
    secret::SecretString,
    CoreError, CoreResult,
};
use reqwest::Url;
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

pub const PUBLIC_PAGE: u32 = 100;
pub const VIP_PAGE: u32 = 25;
pub const PUBLIC_PAGES: usize = 1;
pub const VIP_PAGES: usize = 8;
pub const RATE_LIMIT_RETRIES: u32 = 4;
const TARGET_TTL: Duration = Duration::from_secs(30 * 60);

struct Registered {
    target: ResolvedTarget,
    created: Instant,
}
struct Active {
    generation: u64,
    cancel: CancellationToken,
}

pub struct ServerLists {
    web: Arc<RobloxWeb>,
    active: Mutex<HashMap<String, Active>>,
    targets: Mutex<HashMap<String, Registered>>,
    serial: AtomicU64,
    base_delay_ms: u64,
}

pub struct ListRequest<'a> {
    pub request_key: &'a str,
    pub place_id: &'a str,
    pub vip_cookie: Option<&'a str>,
    pub include_vip: bool,
    pub previous: Option<&'a ServerListResponse>,
    /// Sorting, full-server filter and page cursor for public servers.
    pub options: &'a ServerListOptions,
}

impl ServerLists {
    pub fn new(web: Arc<RobloxWeb>) -> Self {
        Self {
            web,
            active: Mutex::new(HashMap::new()),
            targets: Mutex::new(HashMap::new()),
            serial: AtomicU64::new(0),
            base_delay_ms: 1200,
        }
    }
    #[cfg(test)]
    fn fast(mut self) -> Self {
        self.base_delay_ms = 1;
        self
    }

    /// Stores a target in the backend registry and returns an opaque ID usable by the frontend.
    pub fn register(&self, target: ResolvedTarget) -> String {
        let id = format!("target-{}", uuid::Uuid::new_v4());
        let mut targets = self.targets.lock().unwrap();
        targets.retain(|_, entry| entry.created.elapsed() < TARGET_TTL);
        if targets.len() >= 5000 {
            if let Some(oldest) = targets
                .iter()
                .min_by_key(|(_, entry)| entry.created)
                .map(|(key, _)| key.clone())
            {
                targets.remove(&oldest);
            }
        }
        targets.insert(
            id.clone(),
            Registered {
                target,
                created: Instant::now(),
            },
        );
        id
    }
    pub fn target(&self, id: &str) -> CoreResult<ResolvedTarget> {
        let targets = self.targets.lock().unwrap();
        targets
            .get(id)
            .filter(|entry| entry.created.elapsed() < TARGET_TTL)
            .map(|entry| entry.target.clone())
            .ok_or_else(|| {
                CoreError::Validation(
                    "The selected server target expired. Refresh the server list.".into(),
                )
            })
    }

    pub fn cancel(&self, request_key: &str) {
        if let Some(active) = self.active.lock().unwrap().remove(request_key) {
            active.cancel.cancel();
        }
    }

    /// Lists servers. A newer request with the same key cancels the older one, and an older response that
    /// completes late is reported as cancelled instead of replacing newer data.
    pub async fn list(&self, request: ListRequest<'_>) -> CoreResult<ServerListResponse> {
        let key = request.request_key.trim();
        if key.is_empty() || key.len() > 128 {
            return Err(CoreError::Validation(
                "Missing server list request key.".into(),
            ));
        }
        let place_id = validate_place_id(request.place_id)?;
        let generation = self.serial.fetch_add(1, Ordering::SeqCst) + 1;
        let cancel = CancellationToken::new();
        if let Some(previous) = self.active.lock().unwrap().insert(
            key.to_owned(),
            Active {
                generation,
                cancel: cancel.clone(),
            },
        ) {
            previous.cancel.cancel();
        }
        let result = self.list_inner(&place_id, &request, &cancel).await;
        let current = {
            let mut active = self.active.lock().unwrap();
            let current = active
                .get(key)
                .is_some_and(|entry| entry.generation == generation);
            if current {
                active.remove(key);
            }
            current
        };
        let cancelled = || ServerListResponse {
            place_id: place_id.clone(),
            items: Vec::new(),
            warnings: vec!["Server list request was cancelled.".into()],
            cancelled: true,
            next_cursor: None,
        };
        match result {
            _ if !current => Ok(cancelled()),
            Err(CoreError::Cancelled) => Ok(cancelled()),
            other => other,
        }
    }

    async fn list_inner(
        &self,
        place_id: &str,
        request: &ListRequest<'_>,
        cancel: &CancellationToken,
    ) -> CoreResult<ServerListResponse> {
        let mut warnings = Vec::new();
        // Further public pages are fetched on request through `next_cursor`, not as a warning.
        let (mut items, next_cursor) = self
            .pages(place_id, ServerKind::Public, None, request.options, cancel)
            .await?;
        // VIP servers belong to the first page only; a "load more" request continues public servers.
        if request.include_vip && request.options.cursor.is_none() {
            match request.vip_cookie {
                None => warnings.push("No account selected. VIP servers were skipped.".into()),
                Some(cookie) => match self
                    .pages(
                        place_id,
                        ServerKind::Vip,
                        Some(cookie),
                        &ServerListOptions::default(),
                        cancel,
                    )
                    .await
                {
                    Ok((vip, more)) => {
                        if more.is_some() {
                            warnings.push(format!(
                                "VIP server list was limited to {} rows.",
                                vip.len()
                            ));
                        }
                        items.extend(vip);
                    }
                    Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
                    Err(_) => {
                        warnings.push(
                            "Failed to load VIP servers for the selected account. Previously loaded private servers are kept."
                                .into(),
                        );
                        // Keep previously listed private servers for the same place instead of dropping them.
                        if let Some(previous) = request
                            .previous
                            .filter(|previous| previous.place_id == place_id)
                        {
                            items.extend(
                                previous
                                    .items
                                    .iter()
                                    .filter(|row| row.kind == ServerKind::Vip)
                                    .filter(|row| {
                                        row.target_id
                                            .as_deref()
                                            .is_none_or(|id| self.target(id).is_ok())
                                    })
                                    .cloned(),
                            );
                        }
                    }
                },
            }
        }
        Ok(ServerListResponse {
            place_id: place_id.to_owned(),
            items,
            warnings,
            cancelled: false,
            next_cursor,
        })
    }

    async fn pages(
        &self,
        place_id: &str,
        kind: ServerKind,
        cookie: Option<&str>,
        options: &ServerListOptions,
        cancel: &CancellationToken,
    ) -> CoreResult<(Vec<ServerRow>, Option<String>)> {
        let (limit, max_pages, segment) = match kind {
            ServerKind::Public => (PUBLIC_PAGE, PUBLIC_PAGES, "public"),
            ServerKind::Vip => (VIP_PAGE, VIP_PAGES, "VIP"),
        };
        let mut rows = Vec::new();
        let mut cursor: Option<String> = options
            .cursor
            .as_deref()
            .map(str::trim)
            .filter(|cursor| !cursor.is_empty() && cursor.len() <= 1024)
            .map(str::to_owned);
        let mut row_index = 0usize;
        for page in 0..max_pages {
            let mut url = Url::parse(&format!(
                "{}/v1/games/{place_id}/servers/{segment}",
                self.web.endpoints.games
            ))
            .map_err(|_| CoreError::Network)?;
            url.query_pairs_mut()
                .append_pair(
                    "sortOrder",
                    if options.most_players_first {
                        "Desc"
                    } else {
                        "Asc"
                    },
                )
                .append_pair("limit", &limit.to_string());
            if options.exclude_full {
                url.query_pairs_mut()
                    .append_pair("excludeFullGames", "true");
            }
            if let Some(cursor) = &cursor {
                url.query_pairs_mut().append_pair("cursor", cursor);
            }
            let referer = format!("https://www.roblox.com/games/{place_id}/");
            let payload = self
                .get_with_retry(url, cookie, cookie.map(|_| referer.as_str()), cancel)
                .await?;
            for raw in payload["data"].as_array().into_iter().flatten() {
                if let Some(row) = self.row(raw, kind, place_id, row_index) {
                    rows.push(row);
                }
                row_index += 1;
            }
            cursor = payload["nextPageCursor"]
                .as_str()
                .filter(|cursor| !cursor.is_empty() && cursor.len() <= 1024)
                .map(str::to_owned);
            if cursor.is_none() || page + 1 == max_pages {
                break;
            }
        }
        // A cursor is returned only when another page is actually available.
        Ok((rows, cursor))
    }

    fn row(
        &self,
        raw: &Value,
        kind: ServerKind,
        place_id: &str,
        index: usize,
    ) -> Option<ServerRow> {
        let text = |value: &Value| match value {
            Value::String(value) => Some(value.trim().to_owned()).filter(|value| !value.is_empty()),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        };
        let access_code = raw["accessCode"].as_str().map(str::trim).filter(|code| {
            !code.is_empty()
                && code.len() <= 256
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        });
        let id = text(&raw["id"])
            .or_else(|| text(&raw["name"]))
            .or_else(|| {
                (kind == ServerKind::Vip && access_code.is_some()).then(|| format!("vip-{index}"))
            })?;
        let number = |value: &Value| {
            value
                .as_f64()
                .filter(|value| value.is_finite() && *value >= 0.0)
                .map(|value| value.min(u32::MAX as f64) as u32)
                .unwrap_or(0)
        };
        let target_id = match kind {
            ServerKind::Public => {
                // Public rows carry the instance GUID; the frontend selects them through a job target.
                None
            }
            ServerKind::Vip => access_code.map(|code| {
                self.register(ResolvedTarget {
                    place_id: place_id.to_owned(),
                    kind: ResolvedKind::Private {
                        access_code: SecretString::new(code),
                        link_code: None,
                    },
                    used_fallback: false,
                })
            }),
        };
        Some(ServerRow {
            id,
            kind,
            playing: number(&raw["playing"]),
            max_players: number(&raw["maxPlayers"]),
            player_tokens: raw["playerTokens"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|token| token.as_str())
                .filter(|token| !token.is_empty() && token.len() <= 128)
                .take(100)
                .map(str::to_owned)
                .collect(),
            name: raw["name"].as_str().map(str::to_owned),
            target_id,
            joinable: kind == ServerKind::Public || access_code.is_some(),
            ping: raw["ping"]
                .as_f64()
                .filter(|value| value.is_finite() && *value >= 0.0)
                .map(|value| value.round().min(99_999.0) as u32),
            fps: raw["fps"]
                .as_f64()
                .filter(|value| value.is_finite() && *value >= 0.0)
                .map(|value| value.round().min(1_000.0) as u32),
        })
    }

    async fn get_with_retry(
        &self,
        url: Url,
        cookie: Option<&str>,
        referer: Option<&str>,
        cancel: &CancellationToken,
    ) -> CoreResult<Value> {
        for attempt in 1..=RATE_LIMIT_RETRIES + 1 {
            let response = self
                .web
                .get_raw(url.clone(), cookie, referer, cancel)
                .await?;
            if response.status == 429 {
                if attempt > RATE_LIMIT_RETRIES {
                    break;
                }
                let delay = retry_delay_ms(
                    attempt,
                    response.retry_after.as_deref(),
                    self.base_delay_ms,
                    rand::random::<u64>() % 400,
                );
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                    _ = tokio::time::sleep(Duration::from_millis(delay)) => {}
                }
                continue;
            }
            if !(200..300).contains(&response.status) {
                return Err(CoreError::Validation(format!(
                    "The servers API returned HTTP {}.",
                    response.status
                )));
            }
            return response.json();
        }
        Err(CoreError::Validation(
            "The servers API is rate limited. Try again shortly.".into(),
        ))
    }
}

/// `Retry-After` clamped to 750..15000 ms; otherwise exponential backoff with jitter, capped at 15 s.
pub fn retry_delay_ms(
    attempt: u32,
    retry_after: Option<&str>,
    base_ms: u64,
    jitter_ms: u64,
) -> u64 {
    if let Some(hint) = retry_after.and_then(retry_after_ms) {
        return hint.clamp(750, 15_000).min(base_ms.max(1) * 15_000 / 1200);
    }
    (base_ms.saturating_mul(1 << attempt.saturating_sub(1).min(16)) + jitter_ms).min(15_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roblox::web::fixture::{response, serve};

    fn json(body: &str) -> String {
        response("200 OK", &[("Content-Type", "application/json")], body)
    }

    #[test]
    fn retry_delays_respect_hints_and_caps() {
        assert_eq!(retry_delay_ms(1, Some("0"), 1200, 0), 750);
        assert_eq!(retry_delay_ms(1, Some("2"), 1200, 0), 2000);
        assert_eq!(retry_delay_ms(1, Some("120"), 1200, 0), 15_000);
        assert_eq!(retry_delay_ms(1, None, 1200, 399), 1599);
        assert_eq!(retry_delay_ms(3, None, 1200, 0), 4800);
        assert_eq!(retry_delay_ms(10, None, 1200, 0), 15_000);
    }

    #[tokio::test]
    async fn public_and_vip_rows_hide_access_codes_behind_opaque_ids() {
        let (base, server) = serve(vec![
            json(r#"{"data":[{"id":"job-1","playing":3,"maxPlayers":10,"playerTokens":["t1",""]},{"playing":1}],"nextPageCursor":null}"#),
            json(r#"{"data":[{"name":"Friends","accessCode":"synthetic-access","playing":"x","maxPlayers":8},{"id":5}],"nextPageCursor":"next"}"#),
            json(r#"{"data":[{"accessCode":"second-code"}],"nextPageCursor":""}"#),
        ])
        .await;
        let lists = ServerLists::new(Arc::new(RobloxWeb::fixture(&base))).fast();
        let response = lists
            .list(ListRequest {
                request_key: "account-1",
                place_id: "77",
                vip_cookie: Some("synthetic-cookie"),
                include_vip: true,
                previous: None,
                options: &ServerListOptions::default(),
            })
            .await
            .unwrap();
        assert!(!response.cancelled);
        assert_eq!(response.items.len(), 4);
        assert_eq!(response.items[0].id, "job-1");
        assert_eq!(response.items[0].player_tokens, vec!["t1"]);
        let vip = &response.items[1];
        assert_eq!(vip.kind, ServerKind::Vip);
        assert_eq!(vip.playing, 0);
        assert!(vip.joinable);
        assert!(
            !response.items[2].joinable,
            "VIP row without code cannot be joined"
        );
        assert_eq!(response.items[3].id, "vip-2");
        let serialized = serde_json::to_string(&response).unwrap();
        assert!(!serialized.contains("synthetic-access"));
        assert!(!serialized.contains("second-code"));
        let target = lists.target(vip.target_id.as_ref().unwrap()).unwrap();
        match target.kind {
            ResolvedKind::Private { access_code, .. } => {
                assert_eq!(access_code.expose(), "synthetic-access")
            }
            _ => panic!("expected private target"),
        }
        let requests = server.await.unwrap();
        assert!(requests[0].starts_with("GET /v1/games/77/servers/public?sortOrder=Asc&limit=100 "));
        assert!(!requests[0].contains("ROBLOSECURITY"));
        assert!(requests[1].starts_with("GET /v1/games/77/servers/VIP?sortOrder=Asc&limit=25 "));
        assert!(requests[1].contains(".ROBLOSECURITY=synthetic-cookie"));
        assert!(requests[2].contains("cursor=next"));
        assert!(lists.target("target-missing").is_err());
    }

    #[tokio::test]
    async fn options_sort_filter_and_page_public_servers() {
        let (base, server) = serve(vec![
            json(r#"{"data":[{"id":"busy","playing":29,"maxPlayers":30,"ping":81.6,"fps":59.9}],"nextPageCursor":"page-2"}"#),
            json(r#"{"data":[{"id":"quiet","playing":2,"maxPlayers":30}],"nextPageCursor":null}"#),
        ])
        .await;
        let lists = ServerLists::new(Arc::new(RobloxWeb::fixture(&base))).fast();
        let mut options = ServerListOptions {
            most_players_first: true,
            exclude_full: true,
            cursor: None,
        };
        let first = lists
            .list(ListRequest {
                request_key: "k",
                place_id: "77",
                vip_cookie: None,
                include_vip: false,
                previous: None,
                options: &options,
            })
            .await
            .unwrap();
        assert_eq!(first.next_cursor.as_deref(), Some("page-2"));
        assert_eq!(
            (first.items[0].ping, first.items[0].fps),
            (Some(82), Some(60))
        );
        options.cursor = first.next_cursor.clone();
        let second = lists
            .list(ListRequest {
                request_key: "k",
                place_id: "77",
                vip_cookie: Some("c"),
                include_vip: true,
                previous: None,
                options: &options,
            })
            .await
            .unwrap();
        assert_eq!(second.items[0].id, "quiet");
        assert_eq!(second.next_cursor, None);
        let requests = server.await.unwrap();
        // The second call asked for VIP too, but a "load more" page never fetches VIP servers.
        assert_eq!(requests.len(), 2);
        assert!(requests[0].contains("sortOrder=Desc"));
        assert!(requests[0].contains("excludeFullGames=true"));
        assert!(requests[1].contains("cursor=page-2"));
    }

    #[tokio::test]
    async fn rate_limits_retry_and_vip_failure_keeps_previous_rows() {
        let (base, server) = serve(vec![
            response("429 Too Many Requests", &[("Retry-After", "0")], ""),
            json(r#"{"data":[{"id":"job-1"}]}"#),
            response("500 Internal Server Error", &[], ""),
        ])
        .await;
        let lists = ServerLists::new(Arc::new(RobloxWeb::fixture(&base))).fast();
        let previous = ServerListResponse {
            place_id: "77".into(),
            items: vec![ServerRow {
                id: "old-vip".into(),
                kind: ServerKind::Vip,
                playing: 1,
                max_players: 2,
                player_tokens: vec![],
                name: None,
                target_id: None,
                joinable: false,
                ping: None,
                fps: None,
            }],
            warnings: vec![],
            cancelled: false,
            next_cursor: None,
        };
        let listed = lists
            .list(ListRequest {
                request_key: "k",
                place_id: "77",
                vip_cookie: Some("c"),
                include_vip: true,
                previous: Some(&previous),
                options: &ServerListOptions::default(),
            })
            .await
            .unwrap();
        assert_eq!(listed.items.len(), 2);
        assert_eq!(listed.items[1].id, "old-vip");
        assert!(listed.warnings[0].contains("VIP"));
        server.await.unwrap();

        let responses = (0..=RATE_LIMIT_RETRIES)
            .map(|_| response("429 Too Many Requests", &[], ""))
            .collect();
        let (base, server) = serve(responses).await;
        let lists = ServerLists::new(Arc::new(RobloxWeb::fixture(&base))).fast();
        let error = lists
            .list(ListRequest {
                request_key: "k",
                place_id: "77",
                vip_cookie: None,
                include_vip: false,
                previous: None,
                options: &ServerListOptions::default(),
            })
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("rate limited"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn newer_request_cancels_older_and_backoff_is_cancellable() {
        let (base, server) = serve(vec![
            response("429 Too Many Requests", &[("Retry-After", "10")], ""),
            json(r#"{"data":[{"id":"fresh"}]}"#),
        ])
        .await;
        let mut lists = ServerLists::new(Arc::new(RobloxWeb::fixture(&base)));
        lists.base_delay_ms = 1200;
        let lists = Arc::new(lists);
        let older = {
            let lists = lists.clone();
            tokio::spawn(async move {
                lists
                    .list(ListRequest {
                        request_key: "same",
                        place_id: "1",
                        vip_cookie: None,
                        include_vip: false,
                        previous: None,
                        options: &ServerListOptions::default(),
                    })
                    .await
            })
        };
        // Wait until the older request is sleeping on its Retry-After backoff.
        tokio::time::sleep(Duration::from_millis(200)).await;
        let started = Instant::now();
        let newer = lists
            .list(ListRequest {
                request_key: "same",
                place_id: "1",
                vip_cookie: None,
                include_vip: false,
                previous: None,
                options: &ServerListOptions::default(),
            })
            .await
            .unwrap();
        assert_eq!(newer.items[0].id, "fresh");
        let older = older.await.unwrap().unwrap();
        assert!(older.cancelled);
        assert!(older.items.is_empty());
        assert!(started.elapsed() < Duration::from_secs(5));
        server.await.unwrap();
        lists.cancel("unknown");
    }
}

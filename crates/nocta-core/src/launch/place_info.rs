//! Place name, universe ID and icon lookups with a five-minute cache and request coalescing,
//! plus game search by name.
use super::target::validate_place_id;
use crate::roblox::web::RobloxWeb;
use crate::{
    models::{GameSearchResult, PlaceInfo},
    CoreError, CoreResult,
};
use reqwest::Url;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::OnceCell;
use tokio_util::sync::CancellationToken;

pub const PLACE_INFO_TTL: Duration = Duration::from_secs(5 * 60);
/// Results shown for one search; Roblox returns many more loosely related games.
pub const SEARCH_LIMIT: usize = 12;

struct Entry {
    cell: Arc<OnceCell<PlaceInfo>>,
    created: Instant,
}
pub struct PlaceInfos {
    web: Arc<RobloxWeb>,
    cache: Mutex<HashMap<String, Entry>>,
    searches: Mutex<HashMap<String, (Instant, Vec<GameSearchResult>)>>,
    #[cfg(test)]
    calls: std::sync::atomic::AtomicU32,
}

impl PlaceInfos {
    pub fn new(web: Arc<RobloxWeb>) -> Self {
        Self {
            web,
            cache: Mutex::new(HashMap::new()),
            searches: Mutex::new(HashMap::new()),
            #[cfg(test)]
            calls: Default::default(),
        }
    }
    pub async fn get(&self, place_id: &str, cancel: &CancellationToken) -> CoreResult<PlaceInfo> {
        let place_id = validate_place_id(place_id)?;
        let cell = {
            let mut cache = self.cache.lock().unwrap();
            cache.retain(|_, entry| {
                entry.created.elapsed() < PLACE_INFO_TTL || !entry.cell.initialized()
            });
            if cache.len() > 1000 {
                cache.retain(|_, entry| !entry.cell.initialized());
            }
            cache
                .entry(place_id.clone())
                .or_insert_with(|| Entry {
                    cell: Arc::new(OnceCell::new()),
                    created: Instant::now(),
                })
                .cell
                .clone()
        };
        Ok(cell
            .get_or_try_init(|| async {
                #[cfg(test)]
                self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                self.fetch(&place_id, cancel).await
            })
            .await?
            .clone())
    }

    /// Games matching `query`, most relevant first, with icons. Repeated searches are served
    /// from a short cache so typing back and forth does not hit Roblox again.
    pub async fn search(
        &self,
        query: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<Vec<GameSearchResult>> {
        let query: String = query.trim().chars().take(100).collect();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let key = query.to_lowercase();
        if let Some((at, results)) = self.searches.lock().unwrap().get(&key) {
            if at.elapsed() < PLACE_INFO_TTL {
                return Ok(results.clone());
            }
        }
        let mut url = Url::parse(&format!(
            "{}/search-api/omni-search",
            self.web.endpoints.apis
        ))
        .map_err(|_| CoreError::Network)?;
        url.query_pairs_mut()
            .append_pair("searchQuery", &query)
            .append_pair("sessionId", &uuid::Uuid::new_v4().to_string())
            .append_pair("pageType", "all");
        let response = self.web.get_raw(url, None, None, cancel).await?;
        if response.status == 429 {
            return Err(CoreError::Validation(
                "Roblox is limiting searches right now. Try again in a moment.".into(),
            ));
        }
        if !(200..300).contains(&response.status) {
            return Err(CoreError::Validation(
                "Roblox game search is unavailable right now.".into(),
            ));
        }
        let mut results = parse_search(&response.json()?);
        self.attach_icons(&mut results, cancel).await;
        let mut searches = self.searches.lock().unwrap();
        searches.retain(|_, (at, _)| at.elapsed() < PLACE_INFO_TTL);
        if searches.len() < 200 {
            searches.insert(key, (Instant::now(), results.clone()));
        }
        Ok(results)
    }

    /// Icons are optional; a failed lookup leaves results without them.
    async fn attach_icons(&self, results: &mut [GameSearchResult], cancel: &CancellationToken) {
        if results.is_empty() {
            return;
        }
        let ids: Vec<&str> = results
            .iter()
            .map(|game| game.universe_id.as_str())
            .collect();
        let Ok(url) = Url::parse(&format!(
            "{}/v1/games/icons?universeIds={}&returnPolicy=PlaceHolder&size=150x150&format=Png&isCircular=false",
            self.web.endpoints.thumbnails,
            ids.join(",")
        )) else {
            return;
        };
        let Ok(Some(value)) = self.json(url, cancel).await else {
            return;
        };
        for item in value["data"].as_array().into_iter().flatten() {
            let Some(target) = id_text(&item["targetId"]) else {
                continue;
            };
            if let Some(game) = results.iter_mut().find(|game| game.universe_id == target) {
                game.icon_url = safe_image(&item["imageUrl"]);
            }
        }
    }

    async fn json(&self, url: Url, cancel: &CancellationToken) -> CoreResult<Option<Value>> {
        let response = self.web.get_raw(url, None, None, cancel).await?;
        if !(200..300).contains(&response.status) {
            return Ok(None);
        }
        Ok(response.json().ok())
    }

    async fn fetch(&self, place_id: &str, cancel: &CancellationToken) -> CoreResult<PlaceInfo> {
        let endpoints = &self.web.endpoints;
        let details_url = Url::parse(&format!(
            "{}/v1/games/multiget-place-details?placeIds={place_id}",
            endpoints.games
        ))
        .map_err(|_| CoreError::Network)?;
        let economy_url = Url::parse(&format!(
            "{}/v2/assets/{place_id}/details",
            endpoints.economy
        ))
        .map_err(|_| CoreError::Network)?;
        // Sequential requests keep ordering deterministic; both are bounded by the client timeout.
        let details = self.json(details_url, cancel).await;
        if matches!(details, Err(CoreError::Cancelled)) {
            return Err(CoreError::Cancelled);
        }
        let economy = self.json(economy_url, cancel).await;
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        // Partial data is accepted: either source may fail independently.
        let details = details.ok().flatten();
        let detail = details
            .as_ref()
            .and_then(Value::as_array)
            .and_then(|entries| {
                entries
                    .iter()
                    .find(|entry| id_text(&entry["placeId"]).as_deref() == Some(place_id))
            });
        let detail_name = detail.and_then(|entry| clean_name(&entry["name"]));
        let universe_id = detail
            .and_then(|entry| id_text(&entry["universeId"]))
            .filter(|id| id != "0");
        let economy_name = economy
            .ok()
            .flatten()
            .and_then(|value| clean_name(&value["Name"]));
        let name = economy_name.or(detail_name).ok_or_else(|| {
            CoreError::Validation("Place not found or its details are unavailable.".into())
        })?;
        let mut icon_url = None;
        if let Some(universe) = &universe_id {
            let url = Url::parse(&format!("{}/v1/batch", endpoints.thumbnails))
                .map_err(|_| CoreError::Network)?;
            let body = json!([{ "requestId": format!("{universe}:GameIcon:512x512:png:regular"), "targetId": universe.parse::<u64>().unwrap_or(0), "type": "GameIcon", "size": "512x512", "format": "png", "isCircular": false }]);
            if let Ok(response) = self.web.post_json_raw(url, &body, cancel).await {
                if (200..300).contains(&response.status) {
                    if let Ok(value) = response.json() {
                        let data = value["data"].as_array();
                        icon_url = data
                            .and_then(|items| {
                                items
                                    .iter()
                                    .find(|item| {
                                        id_text(&item["targetId"]).as_deref() == Some(universe)
                                    })
                                    .or_else(|| items.first())
                            })
                            .and_then(|item| safe_image(&item["imageUrl"]));
                    }
                }
            }
        }
        if icon_url.is_none() {
            let url = Url::parse(&format!(
                "{}/v1/assets?assetIds={place_id}&returnPolicy=PlaceHolder&size=150x150&format=Png&isCircular=false",
                endpoints.thumbnails
            ))
            .map_err(|_| CoreError::Network)?;
            icon_url = self
                .json(url, cancel)
                .await
                .ok()
                .flatten()
                .and_then(|value| safe_image(&value["data"][0]["imageUrl"]));
        }
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        Ok(PlaceInfo {
            place_id: place_id.to_owned(),
            name,
            universe_id,
            icon_url,
        })
    }
}

/// Game entries from every result group, without duplicates, in the order Roblox ranks them.
fn parse_search(value: &Value) -> Vec<GameSearchResult> {
    let mut results: Vec<GameSearchResult> = Vec::new();
    let groups = value["searchResults"].as_array().into_iter().flatten();
    for group in groups.filter(|group| group["contentGroupType"] == "Game") {
        for game in group["contents"].as_array().into_iter().flatten() {
            let (Some(place_id), Some(universe_id), Some(name)) = (
                id_text(&game["rootPlaceId"]).filter(|id| id != "0"),
                id_text(&game["universeId"]).filter(|id| id != "0"),
                clean_name(&game["name"]),
            ) else {
                continue;
            };
            if results.iter().any(|known| known.place_id == place_id) {
                continue;
            }
            results.push(GameSearchResult {
                place_id,
                universe_id,
                name,
                creator: clean_name(&game["creatorName"]),
                playing: game["playerCount"]
                    .as_u64()
                    .map_or(0, |count| count.min(u32::MAX as u64) as u32),
                icon_url: None,
            });
            if results.len() == SEARCH_LIMIT {
                return results;
            }
        }
    }
    results
}

fn id_text(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => number.as_u64().map(|id| id.to_string()),
        Value::String(text) => text.parse::<u64>().ok().map(|id| id.to_string()),
        _ => None,
    }
}
fn clean_name(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| name.chars().filter(|c| !c.is_control()).take(200).collect())
}
/// Only Roblox CDN HTTPS images are passed to the UI (also enforced by the app CSP).
fn safe_image(value: &Value) -> Option<String> {
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
    use std::sync::atomic::Ordering;

    fn json(body: &str) -> String {
        response("200 OK", &[("Content-Type", "application/json")], body)
    }

    #[tokio::test]
    async fn name_universe_icon_and_cache() {
        let (base, server) = serve(vec![
            json(r#"[{"placeId":12,"universeId":34,"name":"Details Name"}]"#),
            json(r#"{"Name":"  "}"#),
            json(r#"{"data":[{"targetId":34,"imageUrl":"https://tr.rbxcdn.com/icon.png"}]}"#),
        ])
        .await;
        let infos = PlaceInfos::new(Arc::new(RobloxWeb::fixture(&base)));
        let cancel = CancellationToken::new();
        let (first, second) = tokio::join!(infos.get("12", &cancel), infos.get("012", &cancel));
        let info = first.unwrap();
        assert_eq!(second.unwrap().name, info.name);
        assert_eq!(info.name, "Details Name");
        assert_eq!(info.universe_id.as_deref(), Some("34"));
        assert_eq!(
            info.icon_url.as_deref(),
            Some("https://tr.rbxcdn.com/icon.png")
        );
        assert_eq!(infos.calls.load(Ordering::SeqCst), 1);
        infos.get("12", &cancel).await.unwrap();
        assert_eq!(infos.calls.load(Ordering::SeqCst), 1);
        let requests = server.await.unwrap();
        assert!(requests
            .iter()
            .all(|request| !request.contains("ROBLOSECURITY")));
        assert!(infos.get("0", &cancel).await.is_err());
    }

    #[tokio::test]
    async fn search_lists_games_with_icons_and_caches_the_query() {
        let (base, server) = serve(vec![
            json(
                r#"{"searchResults":[
                    {"contentGroupType":"Game","contents":[{"universeId":34,"rootPlaceId":12,"name":"Blox Fruits","creatorName":"Gamer Robot","playerCount":160684}]},
                    {"contentGroupType":"User","contents":[{"name":"not a game"}]},
                    {"contentGroupType":"Game","contents":[{"universeId":56,"rootPlaceId":78,"name":"  "},{"universeId":34,"rootPlaceId":12,"name":"Blox Fruits"},{"universeId":90,"rootPlaceId":91,"name":"Doors"}]}
                ]}"#,
            ),
            json(
                r#"{"data":[{"targetId":34,"imageUrl":"https://tr.rbxcdn.com/a.png"},{"targetId":90,"imageUrl":"https://evil.test/b.png"}]}"#,
            ),
        ])
        .await;
        let infos = PlaceInfos::new(Arc::new(RobloxWeb::fixture(&base)));
        let cancel = CancellationToken::new();
        let results = infos.search(" blox fruits ", &cancel).await.unwrap();
        let requests = server.await.unwrap();
        assert!(requests[0].contains("searchQuery=blox+fruits"));
        assert!(requests[1].contains("universeIds=34,90"));
        assert_eq!(
            results
                .iter()
                .map(|game| (game.place_id.as_str(), game.name.as_str(), game.playing))
                .collect::<Vec<_>>(),
            [("12", "Blox Fruits", 160684), ("91", "Doors", 0)]
        );
        assert_eq!(results[0].creator.as_deref(), Some("Gamer Robot"));
        assert_eq!(
            results[0].icon_url.as_deref(),
            Some("https://tr.rbxcdn.com/a.png")
        );
        assert!(results[1].icon_url.is_none(), "non-CDN icons are dropped");
        // Served from the cache: the fixture server has already shut down.
        assert_eq!(infos.search("Blox Fruits", &cancel).await.unwrap(), results);
        assert!(infos.search("   ", &cancel).await.unwrap().is_empty());
    }

    /// Real Roblox search: `cargo test -p nocta-core real_game_search -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn real_game_search() {
        let infos = PlaceInfos::new(Arc::new(RobloxWeb::new().unwrap()));
        let results = infos
            .search("blox fruits", &CancellationToken::new())
            .await
            .unwrap();
        for game in &results {
            println!(
                "{} {} {} {:?}",
                game.place_id, game.name, game.playing, game.icon_url
            );
        }
        assert_eq!(results[0].place_id, "2753915549");
        assert!(
            results
                .iter()
                .filter(|game| game.icon_url.is_some())
                .count()
                >= results.len() / 2
        );
    }

    #[tokio::test]
    async fn incomplete_data_falls_back_and_unknown_places_fail() {
        let (base, server) = serve(vec![
            response("500 Internal Server Error", &[], ""),
            json(r#"{"Name":"Economy Name"}"#),
            json(r#"{"data":[{"imageUrl":"https://evil.test/x.png"}]}"#),
        ])
        .await;
        let infos = PlaceInfos::new(Arc::new(RobloxWeb::fixture(&base)));
        let info = infos.get("5", &CancellationToken::new()).await;
        server.await.unwrap();
        let info = info.unwrap();
        assert_eq!(info.name, "Economy Name");
        assert!(info.universe_id.is_none());
        assert!(info.icon_url.is_none(), "non-CDN images are rejected");

        let (base, server) = serve(vec![
            response("404 Not Found", &[], ""),
            response("404 Not Found", &[], ""),
        ])
        .await;
        let infos = PlaceInfos::new(Arc::new(RobloxWeb::fixture(&base)));
        assert!(infos.get("5", &CancellationToken::new()).await.is_err());
        server.await.unwrap();
    }
}

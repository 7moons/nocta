//! Where a Roblox server runs. Roblox's server lists do not say; the address a client would
//! connect to does. It is asked from Roblox's join service with the launching account's session
//! (nothing is launched), then the datacenter's location is looked up by that address. Only the
//! Roblox server's address goes to the location service, never anything about the account.
use super::Launcher;
use crate::{models::ServerRegion, CoreError, CoreResult};
use reqwest::{Method, Url};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Mutex, OnceLock},
};
use tokio_util::sync::CancellationToken;

/// Roblox's join service only answers its own client.
const ROBLOX_CLIENT: &str = "Roblox/WinInet";
/// Asks per server while Roblox keeps the request in its queue.
const JOIN_ATTEMPTS: usize = 5;
const QUEUE_PAUSE: std::time::Duration = std::time::Duration::from_millis(1500);

/// A server's datacenter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerLocation {
    pub region: ServerRegion,
    /// "Frankfurt, DE".
    pub place: String,
}

/// Roblox datacenters do not move; their locations are kept for the whole run.
fn cache() -> &'static Mutex<HashMap<IpAddr, ServerLocation>> {
    static CACHE: OnceLock<Mutex<HashMap<IpAddr, ServerLocation>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The public address in a join answer: status 2 means the server is ready to be joined.
pub fn join_address(value: &Value) -> Option<IpAddr> {
    if value["status"].as_u64() != Some(2) {
        return None;
    }
    let script = &value["joinScript"];
    script["UdmuxEndpoints"]
        .as_array()
        .and_then(|endpoints| endpoints.first())
        .and_then(|endpoint| endpoint["Address"].as_str())
        .or_else(|| script["MachineAddress"].as_str())
        .and_then(|address| address.parse::<IpAddr>().ok())
        .filter(|address| match address {
            IpAddr::V4(v4) => !(v4.is_private() || v4.is_loopback() || v4.is_unspecified()),
            IpAddr::V6(v6) => !(v6.is_loopback() || v6.is_unspecified()),
        })
}

/// A location answer (`continent_code`, `country_code`, `city`).
pub fn parse_location(value: &Value) -> Option<ServerLocation> {
    if value["success"].as_bool() == Some(false) {
        return None;
    }
    let region = ServerRegion::from_continent(value["continent_code"].as_str()?)?;
    let clean = |field: &str| {
        value[field]
            .as_str()
            .map(|text| {
                text.chars()
                    .filter(|c| !c.is_control())
                    .take(60)
                    .collect::<String>()
            })
            .unwrap_or_default()
    };
    let (city, country) = (clean("city"), clean("country_code"));
    let place = match (city.is_empty(), country.is_empty()) {
        (false, false) => format!("{city}, {country}"),
        (true, false) => country,
        (false, true) => city,
        (true, true) => region.name().to_owned(),
    };
    Some(ServerLocation { region, place })
}

impl Launcher {
    /// Where the server `job_id` of `place_id` runs, asked with `cookie`.
    pub async fn locate_server(
        &self,
        cookie: &str,
        place_id: &str,
        job_id: &str,
        cancel: &CancellationToken,
    ) -> CoreResult<ServerLocation> {
        let place: u64 = place_id.parse().map_err(|_| CoreError::Network)?;
        let url = Url::parse(&format!(
            "{}/v1/join-game-instance",
            self.web.endpoints.gamejoin
        ))
        .map_err(|_| CoreError::Network)?;
        let body = json!({
            "placeId": place,
            "gameId": job_id,
            "gameJoinAttemptId": uuid::Uuid::new_v4().to_string(),
            "isTeleport": false,
        });
        let mut answer = Value::Null;
        // Roblox may put the request in a queue first (status 0, 1 or 22); the same attempt is
        // asked again until the server is ready.
        for attempt in 0..JOIN_ATTEMPTS {
            if attempt > 0 {
                tokio::select! {
                    _ = cancel.cancelled() => return Err(CoreError::Cancelled),
                    _ = tokio::time::sleep(QUEUE_PAUSE) => {}
                }
            }
            let join = self
                .web
                .write_authed_as(
                    Method::POST,
                    url.clone(),
                    cookie,
                    Some(&body),
                    Some(ROBLOX_CLIENT),
                    cancel,
                )
                .await?;
            if join.status == 429 {
                return Err(CoreError::Validation(
                    crate::roblox::web::RATE_LIMITED.into(),
                ));
            }
            answer = join.json().unwrap_or(Value::Null);
            if !matches!(answer["status"].as_u64(), Some(0 | 1 | 22)) {
                break;
            }
        }
        let address = join_address(&answer).ok_or_else(|| {
            CoreError::Validation(format!(
                "Roblox did not say where this server runs (join status {}).",
                answer["status"]
            ))
        })?;
        if let Some(found) = cache().lock().unwrap().get(&address) {
            return Ok(found.clone());
        }
        let url = Url::parse(&format!(
            "{}/{address}?fields=success,continent_code,country_code,city",
            self.web.endpoints.geo
        ))
        .map_err(|_| CoreError::Network)?;
        let answer = self.web.get_raw(url, None, None, cancel).await?;
        let location = answer
            .json()
            .ok()
            .as_ref()
            .and_then(parse_location)
            .ok_or_else(|| {
                CoreError::Validation("The server's location could not be found.".into())
            })?;
        cache().lock().unwrap().insert(address, location.clone());
        Ok(location)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_public_join_address_and_its_location_are_read() {
        let ready = json!({
            "jobId": "job-1",
            "status": 2,
            "joinScript": {
                "MachineAddress": "10.0.0.5",
                "UdmuxEndpoints": [{ "Address": "128.116.32.17", "Port": 54321 }]
            }
        });
        assert_eq!(join_address(&ready), "128.116.32.17".parse().ok());
        let machine_only =
            json!({ "status": 2, "joinScript": { "MachineAddress": "128.116.1.9" } });
        assert_eq!(join_address(&machine_only), "128.116.1.9".parse().ok());
        let private_only = json!({ "status": 2, "joinScript": { "MachineAddress": "10.0.0.5" } });
        assert_eq!(join_address(&private_only), None);
        let full = json!({ "status": 6, "joinScript": null });
        assert_eq!(join_address(&full), None);

        let location = parse_location(&json!({
            "success": true, "continent_code": "EU", "country_code": "DE", "city": "Frankfurt am Main"
        }))
        .unwrap();
        assert_eq!(location.region, ServerRegion::Europe);
        assert_eq!(location.place, "Frankfurt am Main, DE");
        assert_eq!(parse_location(&json!({ "success": false })), None);
        assert_eq!(parse_location(&json!({ "continent_code": "AN" })), None);
    }
}

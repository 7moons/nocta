//! PlaceLauncher URL and `roblox-player:` URI construction (legacy `placeLauncher.ts` contract).
use super::target::{ResolvedKind, ResolvedTarget};
use crate::secret::SecretString;
use reqwest::Url;

pub const PLACE_LAUNCHER: &str = "https://assetgame.roblox.com/game/PlaceLauncher.ashx";

/// Per-launch identifiers shared by the join precheck and PlaceLauncher request.
#[derive(Clone, Debug)]
pub struct JoinIds {
    pub tracker_id: u32,
    pub attempt_id: String,
    pub event_id: String,
}
impl JoinIds {
    pub fn generate() -> Self {
        Self {
            tracker_id: rand::random::<u32>() % 1_000_000_000,
            attempt_id: uuid::Uuid::new_v4().to_string(),
            event_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

pub fn request_type(target: &ResolvedTarget) -> &'static str {
    match target.kind {
        ResolvedKind::Public => "RequestGame",
        ResolvedKind::Job { .. } => "RequestGameJob",
        ResolvedKind::Private { .. } => "RequestPrivateGame",
    }
}

/// Builds the PlaceLauncher URL. Query order: request, placeId, common flags, then target extras.
/// Private requests never receive the public/job flags.
pub fn place_launcher_url(target: &ResolvedTarget, ids: &JoinIds) -> SecretString {
    let mut url = Url::parse(PLACE_LAUNCHER).unwrap();
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("request", request_type(target))
            .append_pair("placeId", &target.place_id);
        if !matches!(target.kind, ResolvedKind::Private { .. }) {
            query
                .append_pair("browserTrackerId", &ids.tracker_id.to_string())
                .append_pair("isPlayTogetherGame", "false");
        }
        match &target.kind {
            ResolvedKind::Public => {
                query.append_pair("isUnknownPlayer", "true");
            }
            ResolvedKind::Job { job_id } => {
                query.append_pair("gameId", job_id);
            }
            ResolvedKind::Private {
                access_code,
                link_code,
            } => {
                query.append_pair("accessCode", access_code.expose());
                if let Some(link_code) = link_code {
                    query.append_pair("linkCode", link_code.expose());
                }
            }
        }
    }
    SecretString::new(String::from(url))
}

/// JavaScript `encodeURIComponent`: leaves `A-Z a-z 0-9 - _ . ! ~ * ' ( )` unescaped.
pub fn encode_uri_component(value: &str) -> String {
    let mut output = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

/// Exact legacy protocol string, without locale/channel/extra flags.
pub fn player_uri(
    target: &ResolvedTarget,
    ticket: &SecretString,
    ids: &JoinIds,
    launch_time_ms: i64,
) -> SecretString {
    let launcher = place_launcher_url(target, ids);
    SecretString::new(format!(
        "roblox-player:1+launchmode:play+gameinfo:{}+launchtime:{}+placelauncherurl:{}+browsertrackerid:{}",
        ticket.expose(),
        launch_time_ms,
        encode_uri_component(launcher.expose()),
        ids.tracker_id
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> JoinIds {
        JoinIds {
            tracker_id: 123456789,
            attempt_id: "a".into(),
            event_id: "e".into(),
        }
    }
    fn target(kind: ResolvedKind) -> ResolvedTarget {
        ResolvedTarget {
            place_id: "920587237".into(),
            kind,
            used_fallback: false,
        }
    }

    #[test]
    fn public_job_and_private_queries_match_legacy() {
        assert_eq!(
            place_launcher_url(&target(ResolvedKind::Public), &ids()).expose(),
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame&placeId=920587237&browserTrackerId=123456789&isPlayTogetherGame=false&isUnknownPlayer=true"
        );
        assert_eq!(
            place_launcher_url(
                &target(ResolvedKind::Job {
                    job_id: "abc-1".into()
                }),
                &ids()
            )
            .expose(),
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGameJob&placeId=920587237&browserTrackerId=123456789&isPlayTogetherGame=false&gameId=abc-1"
        );
        let private = place_launcher_url(
            &target(ResolvedKind::Private {
                access_code: SecretString::new("a+b%c d"),
                link_code: Some(SecretString::new("L1")),
            }),
            &ids(),
        );
        assert_eq!(
            private.expose(),
            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestPrivateGame&placeId=920587237&accessCode=a%2Bb%25c+d&linkCode=L1"
        );
        assert!(!private.expose().contains("browserTrackerId"));
        assert!(!private.expose().contains("isPlayTogetherGame"));
    }

    #[test]
    fn uri_encodes_the_whole_launcher_url_once_more() {
        assert_eq!(
            encode_uri_component("a b+c%d/é-_.!~*'()"),
            "a%20b%2Bc%25d%2F%C3%A9-_.!~*'()"
        );
        let uri = player_uri(
            &target(ResolvedKind::Private {
                access_code: SecretString::new("a+b%c"),
                link_code: None,
            }),
            &SecretString::new("TICKET"),
            &ids(),
            1700000000000,
        );
        assert_eq!(
            uri.expose(),
            "roblox-player:1+launchmode:play+gameinfo:TICKET+launchtime:1700000000000+placelauncherurl:https%3A%2F%2Fassetgame.roblox.com%2Fgame%2FPlaceLauncher.ashx%3Frequest%3DRequestPrivateGame%26placeId%3D920587237%26accessCode%3Da%252Bb%2525c+browsertrackerid:123456789"
        );
        assert!(!uri.expose().contains("robloxLocale"));
        assert!(!uri.expose().contains("channel"));
        assert!(JoinIds::generate().tracker_id < 1_000_000_000);
    }
}

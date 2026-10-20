//! Invite/link parsers matching the legacy `inviteParsing.ts` contract.
use regex::Regex;
use reqwest::Url;
use std::sync::OnceLock;

fn regex(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).unwrap())
}
fn private_invite() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    regex(
        &CELL,
        r#"Roblox\.GameLauncher\.joinPrivateGame\(\d+,\s*['"]([a-zA-Z0-9\-]+)['"]"#,
    )
}
fn private_link_code() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    regex(&CELL, r"(?i)privateServerLinkCode=([a-zA-Z0-9\-]+)")
}
fn game_instance_id() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    regex(&CELL, r"(?i)(?:gameInstanceId|jobId)=([a-zA-Z0-9\-]+)")
}
fn games_place_id() -> &'static Regex {
    static CELL: OnceLock<Regex> = OnceLock::new();
    regex(&CELL, r"(?i)/games/(\d+)")
}

/// First value of an exact (case-sensitive) query key, like `URLSearchParams.get`.
pub fn query(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
}
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

pub fn extract_place_id_from_url(url: &Url) -> Option<String> {
    non_empty(query(url, "placeId"))
        .or_else(|| non_empty(query(url, "placeid")))
        .or_else(|| non_empty(query(url, "PlaceId")))
        .or_else(|| {
            games_place_id()
                .captures(url.path())
                .map(|captures| captures[1].to_owned())
        })
}

pub fn parse_private_access_code(html: &str) -> Option<String> {
    private_invite()
        .captures(html)
        .map(|captures| captures[1].to_owned())
}

/// `decodeURIComponent(value).trim()`, keeping the trimmed input when decoding fails.
pub fn decode_link_code(value: &str) -> String {
    decode_uri_component(value)
        .map(|decoded| decoded.trim().to_owned())
        .unwrap_or_else(|| value.trim().to_owned())
}

fn decode_uri_component(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes.get(index + 1..index + 3)?;
            let hex = std::str::from_utf8(hex).ok()?;
            output.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).ok()
}

pub fn extract_place_id_from_text(raw: &str) -> Option<String> {
    games_place_id()
        .captures(raw)
        .map(|captures| captures[1].to_owned())
}

pub fn extract_private_server_link_code(raw: &str) -> Option<String> {
    if let Ok(url) = Url::parse(raw) {
        if let Some(code) = non_empty(query(&url, "privateServerLinkCode")) {
            return Some(decode_link_code(&code)).filter(|code| !code.is_empty());
        }
    }
    private_link_code()
        .captures(raw)
        .map(|captures| decode_link_code(&captures[1]))
        .filter(|code| !code.is_empty())
}

pub fn is_server_share_link(url: &Url) -> bool {
    query(url, "type").is_some_and(|kind| kind.to_lowercase() == "server")
        && non_empty(query(url, "code")).is_some()
}

pub fn is_experience_share_link(url: &Url) -> bool {
    query(url, "type").is_some_and(|kind| kind.to_lowercase() == "experienceinvite")
        && non_empty(query(url, "code")).is_some()
}

pub fn extract_job_id_from_url(url: &Url) -> Option<String> {
    non_empty(query(url, "gameInstanceId")).or_else(|| non_empty(query(url, "jobId")))
}

pub fn extract_job_id_from_text(raw: &str) -> Option<String> {
    if let Ok(url) = Url::parse(raw) {
        return extract_job_id_from_url(&url);
    }
    game_instance_id()
        .captures(raw)
        .map(|captures| captures[1].to_owned())
}

/// Collect a place ID from any text, also trying it as a URL, as the legacy browser fallback does.
pub fn place_from_any(text: &str) -> Option<String> {
    extract_place_id_from_text(text).or_else(|| {
        Url::parse(text)
            .ok()
            .and_then(|url| extract_place_id_from_url(&url))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(value: &str) -> Url {
        Url::parse(value).unwrap()
    }

    #[test]
    fn place_id_keys_follow_legacy_priority() {
        assert_eq!(
            extract_place_id_from_url(&url(
                "https://www.roblox.com/games/9?PlaceId=3&placeid=2&placeId=1"
            )),
            Some("1".into())
        );
        assert_eq!(
            extract_place_id_from_url(&url("https://www.roblox.com/x?placeid=2&PlaceId=3")),
            Some("2".into())
        );
        assert_eq!(
            extract_place_id_from_url(&url("https://www.roblox.com/x?PlaceId=3")),
            Some("3".into())
        );
        assert_eq!(
            extract_place_id_from_url(&url("https://www.roblox.com/GAMES/920587237/Name")),
            Some("920587237".into())
        );
        assert_eq!(
            extract_place_id_from_url(&url("https://www.roblox.com/home")),
            None
        );
        assert_eq!(
            extract_place_id_from_text("<a href=\"/games/42/x\">"),
            Some("42".into())
        );
    }

    #[test]
    fn link_codes_jobs_and_share_types() {
        assert_eq!(
            extract_private_server_link_code(
                "https://www.roblox.com/games/1/x?privateServerLinkCode=%2520abc-1%20"
            ),
            Some("abc-1".into())
        );
        assert_eq!(
            extract_private_server_link_code("junk PRIVATESERVERLINKCODE=Xy-9&other"),
            Some("Xy-9".into())
        );
        assert_eq!(
            extract_private_server_link_code("https://www.roblox.com/games/1"),
            None
        );
        assert_eq!(decode_link_code(" %E0%A4%A "), "%E0%A4%A");
        assert_eq!(
            extract_job_id_from_text("https://www.roblox.com/games/1?gameInstanceId=a-1&jobId=b"),
            Some("a-1".into())
        );
        assert_eq!(
            extract_job_id_from_text("https://www.roblox.com/games/1?jobId=b"),
            Some("b".into())
        );
        assert_eq!(
            extract_job_id_from_text("not a url JOBID=c-3"),
            Some("c-3".into())
        );
        assert!(is_server_share_link(&url(
            "https://www.roblox.com/share?code=abc&type=Server"
        )));
        assert!(!is_server_share_link(&url(
            "https://www.roblox.com/share?code=&type=Server"
        )));
        assert!(is_experience_share_link(&url(
            "https://www.roblox.com/share?code=abc&type=ExperienceInvite"
        )));
        assert!(!is_experience_share_link(&url(
            "https://www.roblox.com/share?code=abc&type=Server"
        )));
    }

    #[test]
    fn private_access_code_html_pattern() {
        assert_eq!(
            parse_private_access_code("x Roblox.GameLauncher.joinPrivateGame(123, 'ab-12') y"),
            Some("ab-12".into())
        );
        assert_eq!(
            parse_private_access_code("Roblox.GameLauncher.joinPrivateGame(123,\"CD34\")"),
            Some("CD34".into())
        );
        assert_eq!(
            parse_private_access_code("Roblox.GameLauncher.joinGame(123, 'x')"),
            None
        );
    }
}

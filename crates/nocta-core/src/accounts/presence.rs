use crate::models::AccountPresence;
use serde_json::Value;

/// Stored metadata only. The legacy app did not poll presence; parsing must not start network work.
pub fn parse(raw: &str) -> Option<AccountPresence> {
    if raw.len() > 8192 {
        return None;
    }
    let value: Value = serde_json::from_str(raw).ok()?;
    value.as_object()?;
    let text = |key: &str| {
        value[key]
            .as_str()
            .map(|text| {
                text.chars()
                    .filter(|c| !c.is_control())
                    .take(300)
                    .collect::<String>()
            })
            .filter(|text| !text.is_empty())
    };
    let user_presence_type = value["userPresenceType"]
        .as_u64()
        .filter(|value| *value <= 3)
        .unwrap_or(0) as u32;
    let place = value["placeId"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| value["placeId"].as_u64().map(|id| id.to_string()));
    Some(AccountPresence {
        user_presence_type,
        last_location: text("lastLocation"),
        last_online: text("lastOnline")
            .filter(|date| chrono::DateTime::parse_from_rfc3339(date).is_ok()),
        place_id: place.and_then(|place| crate::launch::target::validate_place_id(&place).ok()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_presence_is_ignored_and_fields_are_normalized() {
        for raw in ["null", "[]", "invalid", "12"] {
            assert!(parse(raw).is_none());
        }
        let presence = parse(r#"{"userPresenceType":2,"lastLocation":"A game","placeId":123,"lastOnline":"2026-10-04T12:00:00Z","other":"not returned"}"#).unwrap();
        assert_eq!(presence.user_presence_type, 2);
        assert_eq!(presence.place_id.as_deref(), Some("123"));
        assert!(presence.last_online.is_some());
        assert!(!serde_json::to_string(&presence)
            .unwrap()
            .contains("not returned"));
        let unknown =
            parse(r#"{"userPresenceType":999,"placeId":0,"lastOnline":"invalid"}"#).unwrap();
        assert_eq!(unknown.user_presence_type, 0);
        assert!(unknown.place_id.is_none() && unknown.last_online.is_none());
    }
}

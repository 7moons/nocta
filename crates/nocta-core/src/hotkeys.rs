//! System-wide shortcuts written like `Ctrl+Alt+N`. The desktop shell registers them; here they
//! are only checked, so a typo is reported when the setting is saved.
use crate::{CoreError, CoreResult};

const MODIFIERS: &[&str] = &[
    "ctrl",
    "control",
    "alt",
    "shift",
    "super",
    "win",
    "cmd",
    "commandorcontrol",
];

fn is_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    (key.len() == 1 && key.chars().all(|c| c.is_ascii_alphanumeric()))
        || key
            .strip_prefix('f')
            .and_then(|number| number.parse::<u8>().ok())
            .is_some_and(|number| (1..=24).contains(&number))
        || matches!(
            key.as_str(),
            "space"
                | "enter"
                | "tab"
                | "home"
                | "end"
                | "pageup"
                | "pagedown"
                | "insert"
                | "delete"
                | "up"
                | "down"
                | "left"
                | "right"
        )
}

fn is_function_key(key: &str) -> bool {
    key.len() > 1 && is_key(key) && key.to_ascii_lowercase().starts_with('f')
}

/// An empty string (no shortcut) or modifiers plus one key. Plain keys without a modifier are
/// refused, except F-keys, so typing never triggers a shortcut by accident.
pub fn validate(hotkey: &str) -> CoreResult<()> {
    let hotkey = hotkey.trim();
    if hotkey.is_empty() {
        return Ok(());
    }
    let invalid = || {
        CoreError::Validation(format!(
            "\"{}\" is not a shortcut. Use modifiers and a key, like Ctrl+Alt+N.",
            hotkey.chars().take(40).collect::<String>()
        ))
    };
    let parts: Vec<&str> = hotkey.split('+').map(str::trim).collect();
    let (key, modifiers) = parts.split_last().ok_or_else(invalid)?;
    if hotkey.len() > 40
        || !is_key(key)
        || modifiers
            .iter()
            .any(|part| !MODIFIERS.contains(&part.to_ascii_lowercase().as_str()))
        || (modifiers.is_empty() && !is_function_key(key))
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_need_a_modifier_and_one_key() {
        for good in [
            "",
            "Ctrl+Alt+N",
            "ctrl+shift+1",
            "F9",
            "Alt+F4",
            "Ctrl+Space",
        ] {
            assert!(validate(good).is_ok(), "{good}");
        }
        for bad in ["N", "Ctrl+", "Ctrl+Alt", "Hyper+N", "Ctrl+NN", "F30"] {
            assert!(validate(bad).is_err(), "{bad}");
        }
    }
}

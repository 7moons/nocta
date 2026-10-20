//! Two-step verification codes (TOTP, RFC 6238: HMAC-SHA1, 30-second steps, 6 digits), the kind
//! authenticator apps show for Roblox.
use crate::{CoreError, CoreResult};
use hmac::{Hmac, Mac};
use sha1::Sha1;

pub const STEP_SECS: u64 = 30;
pub const DIGITS: u32 = 6;

/// Decodes a base32 secret as shown by Roblox's authenticator setup ("Enter this key"), ignoring
/// spaces, dashes, padding and case. Also accepts `otpauth://` links.
pub fn decode_secret(raw: &str) -> CoreResult<Vec<u8>> {
    let invalid = || {
        CoreError::Validation(
            "Paste the authenticator key Roblox shows (letters A–Z and digits 2–7).".into(),
        )
    };
    let raw = raw.trim();
    let text = if let Some(query) = raw.strip_prefix("otpauth://") {
        query
            .split(['?', '&'])
            .find_map(|part| part.strip_prefix("secret="))
            .ok_or_else(invalid)?
            .to_owned()
    } else {
        raw.to_owned()
    };
    let mut bits = 0u64;
    let mut count = 0u32;
    let mut out = Vec::new();
    for c in text.chars().filter(|c| !matches!(c, ' ' | '-' | '=')) {
        let value = match c.to_ascii_uppercase() {
            c @ 'A'..='Z' => c as u64 - 'A' as u64,
            c @ '2'..='7' => c as u64 - '2' as u64 + 26,
            _ => return Err(invalid()),
        };
        bits = (bits << 5) | value;
        count += 5;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    if out.len() < 10 {
        return Err(invalid());
    }
    Ok(out)
}

pub fn code_at(secret: &[u8], unix_secs: u64, digits: u32) -> String {
    let counter = unix_secs / STEP_SECS;
    let mut mac = Hmac::<Sha1>::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(&counter.to_be_bytes());
    let hash = mac.finalize().into_bytes();
    let offset = (hash[19] & 0x0f) as usize;
    let binary = u32::from_be_bytes([
        hash[offset] & 0x7f,
        hash[offset + 1],
        hash[offset + 2],
        hash[offset + 3],
    ]);
    format!(
        "{:0width$}",
        binary % 10u32.pow(digits),
        width = digits as usize
    )
}

/// Seconds the current code stays valid.
pub fn remaining(unix_secs: u64) -> u32 {
    (STEP_SECS - unix_secs % STEP_SECS) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_rfc_6238_test_vectors() {
        // RFC 6238 appendix B, SHA-1 secret "12345678901234567890".
        let secret = decode_secret("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
        assert_eq!(secret, b"12345678901234567890");
        for (time, code) in [
            (59, "94287082"),
            (1111111109, "07081804"),
            (1111111111, "14050471"),
            (1234567890, "89005924"),
            (2000000000, "69279037"),
            (20000000000, "65353130"),
        ] {
            assert_eq!(code_at(&secret, time, 8), code, "at {time}");
        }
        assert_eq!(code_at(&secret, 59, 6), "287082");
        assert_eq!(remaining(59), 1);
        assert_eq!(remaining(60), 30);
    }

    #[test]
    fn accepts_spaced_lowercase_keys_and_links_but_not_garbage() {
        let spaced = decode_secret("gezd gnbv gy3t qojq gezd gnbv gy3t qojq").unwrap();
        assert_eq!(spaced, b"12345678901234567890");
        let link = decode_secret(
            "otpauth://totp/Roblox:me?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Roblox",
        )
        .unwrap();
        assert_eq!(link, spaced);
        assert!(decode_secret("not a key!").is_err());
        assert!(decode_secret("AAAA").is_err(), "too short");
    }
}

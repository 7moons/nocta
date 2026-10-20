//! Reads `AccountData.json` of ic3w0lf's Roblox Account Manager so its accounts can be imported.
//! The file is only read, never written.
//!
//! The manager saves the account list as JSON in one of three ways:
//! - protected with Windows DPAPI and a fixed entropy (the default),
//! - encrypted with a password: a fixed header, a 16-byte salt, a 24-byte nonce and a libsodium
//!   secretbox (XSalsa20-Poly1305) whose key is Argon2i ("moderate") of SHA-512(password),
//! - plain JSON, when the user opted out of encryption.
use crate::{
    legacy::{LegacyAccount, LegacyData, LegacyFolder},
    models::LegacySource,
    CoreError, CoreResult,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

pub const APP_NAME: &str = "Roblox Account Manager (ic3w0lf)";
pub const FILE_NAME: &str = "AccountData.json";

/// DPAPI entropy the manager uses: "ROBLOX ACCOUNT MANAGER | :) | BROUGHT TO YOU BUY ic3w0lf".
const ENTROPY: &[u8] = b"ROBLOX ACCOUNT MANAGER | :) | BROUGHT TO YOU BUY ic3w0lf";
/// Start of password-encrypted files.
const HEADER: &[u8] = b"Roblox Account Manager created by ic3w0lf22 @ github.com .......";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
/// libsodium's crypto_pwhash_argon2i "moderate" limits.
const ARGON_OPS: u32 = 6;
const ARGON_MEMORY_KIB: u32 = 128 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RamAccount {
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    security_token: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default, rename = "UserID")]
    user_id: Option<i64>,
    #[serde(default)]
    group: Option<String>,
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

/// Whether the file needs the manager's password to be read.
pub fn needs_password(bytes: &[u8]) -> bool {
    bytes.starts_with(HEADER)
}

/// The account list as JSON text.
pub fn decrypt(bytes: &[u8], password: Option<&str>) -> CoreResult<Zeroizing<Vec<u8>>> {
    if needs_password(bytes) {
        let password = password.filter(|value| !value.is_empty()).ok_or_else(|| {
            CoreError::Validation(
                "This file is protected with the account manager's password. Enter it to import."
                    .into(),
            )
        })?;
        return decrypt_with_password(bytes, password);
    }
    let text = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    if text.trim_ascii_start().starts_with(b"[") {
        return Ok(Zeroizing::new(text.to_vec()));
    }
    unprotect(bytes)
}

fn decrypt_with_password(bytes: &[u8], password: &str) -> CoreResult<Zeroizing<Vec<u8>>> {
    use crypto_secretbox::{aead::Aead, KeyInit, XSalsa20Poly1305};
    use sha2::Digest;
    let body = &bytes[HEADER.len()..];
    if body.len() < SALT_LEN + NONCE_LEN + 16 {
        return Err(CoreError::Validation(
            "This account file is damaged.".into(),
        ));
    }
    let (salt, rest) = body.split_at(SALT_LEN);
    let (nonce, cipher) = rest.split_at(NONCE_LEN);
    // The manager hashes the password before stretching it.
    let hashed = Zeroizing::new(sha2::Sha512::digest(password.as_bytes()).to_vec());
    let params = argon2::Params::new(ARGON_MEMORY_KIB, ARGON_OPS, 1, Some(32))
        .map_err(|_| CoreError::Validation("Unsupported key settings.".into()))?;
    // Old libsodium defaulted to Argon2i, newer builds to Argon2id; either opens the file.
    for algorithm in [argon2::Algorithm::Argon2i, argon2::Algorithm::Argon2id] {
        let argon = argon2::Argon2::new(algorithm, argon2::Version::V0x13, params.clone());
        let mut key = Zeroizing::new([0u8; 32]);
        argon
            .hash_password_into(&hashed, salt, key.as_mut())
            .map_err(|_| CoreError::Validation("Unsupported key settings.".into()))?;
        if let Ok(plain) = XSalsa20Poly1305::new(key.as_ref().into()).decrypt(nonce.into(), cipher)
        {
            return Ok(Zeroizing::new(plain));
        }
    }
    Err(CoreError::Validation("The password is not correct.".into()))
}

fn unprotect(bytes: &[u8]) -> CoreResult<Zeroizing<Vec<u8>>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB},
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: ENTROPY.len() as u32,
        pbData: ENTROPY.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    // SAFETY: the blobs point at live buffers for the duration of the call; the output buffer
    // is allocated by Windows, copied, wiped and freed with LocalFree as documented.
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            &entropy,
            std::ptr::null(),
            std::ptr::null(),
            0,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() {
        return Err(CoreError::Validation(
            "Windows could not unlock this file. It can only be read on the computer and Windows account that saved it.".into(),
        ));
    }
    let plain = unsafe {
        let slice = std::slice::from_raw_parts_mut(output.pbData, output.cbData as usize);
        let copy = Zeroizing::new(slice.to_vec());
        slice.fill(0);
        LocalFree(output.pbData as _);
        copy
    };
    Ok(plain)
}

/// Groups become folders; the manager's "Default" group means no folder.
pub fn parse(json: &[u8]) -> CoreResult<LegacyData> {
    let accounts: Vec<RamAccount> = serde_json::from_slice(json)
        .map_err(|_| CoreError::Validation("This is not a Roblox Account Manager file.".into()))?;
    let mut data = LegacyData::default();
    for account in accounts {
        let folder_id = account
            .group
            .as_deref()
            .map(str::trim)
            .filter(|group| !group.is_empty() && !group.eq_ignore_ascii_case("default"))
            .map(|group| {
                match data
                    .folders
                    .iter()
                    .find(|folder| folder.name.eq_ignore_ascii_case(group))
                {
                    Some(folder) => folder.id,
                    None => {
                        let id = data.folders.len() as i64 + 1;
                        data.folders.push(LegacyFolder {
                            id,
                            name: group.to_owned(),
                            color: None,
                            group: None,
                        });
                        id
                    }
                }
            });
        data.accounts.push(LegacyAccount {
            username: account.username.unwrap_or_default(),
            password: account.password.unwrap_or_default(),
            cookie: account.security_token.unwrap_or_default(),
            folder_id,
            deleted: false,
            alias: account.alias.unwrap_or_default(),
            note: account.description.unwrap_or_default(),
            user_id: account
                .user_id
                .filter(|id| *id > 0)
                .map(|id| id.to_string()),
        });
    }
    Ok(data)
}

pub async fn read(path: &Path, password: Option<&str>) -> CoreResult<LegacyData> {
    let bytes = tokio::fs::read(path).await?;
    parse(&decrypt(&bytes, password)?)
}

/// What the file holds, without its secrets. Password-protected files can only say so.
pub async fn inspect(path: &Path) -> CoreResult<LegacySource> {
    let bytes = tokio::fs::read(path).await?;
    let modified_at = std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339());
    let mut source = LegacySource {
        path: path.to_string_lossy().into_owned(),
        app: APP_NAME.into(),
        accounts: 0,
        in_trash: 0,
        folders: 0,
        modified_at,
        needs_password: needs_password(&bytes),
    };
    if !source.needs_password {
        let data = parse(&decrypt(&bytes, None)?)?;
        source.accounts = data
            .accounts
            .iter()
            .filter(|account| {
                !account.username.trim().is_empty() && !account.cookie.trim().is_empty()
            })
            .count() as u32;
        source.folders = data.folders.len() as u32;
    }
    Ok(source)
}

/// Looks for `AccountData.json` a few folders deep in the usual places people keep the
/// manager (it is a portable app with no fixed location).
pub async fn detect(home: &Path) -> Vec<LegacySource> {
    let roots: Vec<PathBuf> = ["Desktop", "Downloads", "Documents", "OneDrive\\Desktop"]
        .iter()
        .map(|folder| home.join(folder))
        .collect();
    let mut found = Vec::new();
    for root in roots {
        let mut pending = vec![(root, 0)];
        while let Some((dir, depth)) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                if kind.is_dir() && depth < 3 {
                    pending.push((path, depth + 1));
                } else if kind.is_file()
                    && path
                        .file_name()
                        .is_some_and(|name| name.eq_ignore_ascii_case(FILE_NAME))
                {
                    match inspect(&path).await {
                        Ok(source) if source.needs_password || source.accounts > 0 => {
                            found.push(source)
                        }
                        Ok(_) => {}
                        Err(error) => {
                            tracing::debug!(%error, path = %path.display(), "Skipping AccountData.json")
                        }
                    }
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[
        {"Valid":true,"SecurityToken":"_|WARNING:-DO-NOT-SHARE-THIS.--|_cookie-a","Username":"Alice_1","LastUse":"2024-01-01T00:00:00","UserID":156,"Fields":{},"LastAttemptedRefresh":"0001-01-01T00:00:00","BrowserTrackerID":"1","Group":"Farm","Alias":"Main","Description":"daily","Password":"pw-a"},
        {"Valid":true,"SecurityToken":"_|WARNING:-DO-NOT-SHARE-THIS.--|_cookie-b","Username":"Bob_2","UserID":0,"Group":"Default","Alias":"","Description":"","Password":""},
        {"Valid":false,"SecurityToken":"_|WARNING:-DO-NOT-SHARE-THIS.--|_cookie-c","Username":"Cara_3","Group":"farm"}
    ]"#;

    #[test]
    fn reads_accounts_groups_aliases_and_notes() {
        let data = parse(SAMPLE.as_bytes()).unwrap();
        assert_eq!(data.folders.len(), 1, "Farm and farm are one folder");
        assert_eq!(data.folders[0].name, "Farm");
        let alice = &data.accounts[0];
        assert_eq!(alice.username, "Alice_1");
        assert_eq!(alice.password, "pw-a");
        assert!(alice.cookie.ends_with("cookie-a"));
        assert_eq!(alice.alias, "Main");
        assert_eq!(alice.note, "daily");
        assert_eq!(alice.user_id.as_deref(), Some("156"));
        assert_eq!(alice.folder_id, Some(1));
        assert_eq!(data.accounts[1].folder_id, None, "Default means no folder");
        assert_eq!(data.accounts[1].user_id, None);
        assert_eq!(data.accounts[2].folder_id, Some(1));
    }

    #[test]
    fn plain_files_need_nothing_and_protected_ones_ask_for_the_password() {
        let with_bom = [b"\xEF\xBB\xBF".as_slice(), SAMPLE.as_bytes()].concat();
        assert_eq!(
            parse(&decrypt(&with_bom, None).unwrap())
                .unwrap()
                .accounts
                .len(),
            3
        );
        let mut locked = HEADER.to_vec();
        locked.extend([0u8; 80]);
        assert!(needs_password(&locked));
        let error = decrypt(&locked, None).unwrap_err().to_string();
        assert!(error.contains("password"), "{error}");
        let error = decrypt(&locked, Some("wrong")).unwrap_err().to_string();
        assert_eq!(error, "The password is not correct.");
    }

    /// A file encrypted by libsodium itself (PyNaCl: SHA-512 of the password, Argon2i with the
    /// "moderate" limits, secretbox), laid out like the manager writes it. Password: "hunter2".
    #[test]
    fn decrypts_a_password_protected_file_written_by_libsodium() {
        let bytes = include_bytes!("../tests/fixtures/ram-password.bin");
        let data = parse(&decrypt(bytes, Some("hunter2")).unwrap()).unwrap();
        assert_eq!(data.accounts[0].username, "Alice_1");
        assert!(decrypt(bytes, Some("hunter3")).is_err());
    }

    #[test]
    fn decrypts_a_file_protected_by_windows_dpapi() {
        // The manager calls .NET's ProtectedData.Protect, a wrapper of this Windows API.
        use windows_sys::Win32::{
            Foundation::LocalFree,
            Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB},
        };
        let input = CRYPT_INTEGER_BLOB {
            cbData: SAMPLE.len() as u32,
            pbData: SAMPLE.as_ptr() as *mut u8,
        };
        let entropy = CRYPT_INTEGER_BLOB {
            cbData: ENTROPY.len() as u32,
            pbData: ENTROPY.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let protected = unsafe {
            assert_ne!(
                CryptProtectData(
                    &input,
                    std::ptr::null(),
                    &entropy,
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    &mut output,
                ),
                0
            );
            let copy = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            LocalFree(output.pbData as _);
            copy
        };
        assert!(!needs_password(&protected));
        let data = parse(&decrypt(&protected, None).unwrap()).unwrap();
        assert_eq!(data.accounts.len(), 3);
        // Without the manager's entropy the file stays locked.
        let mut tampered = protected.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(decrypt(&tampered, None).is_err());
    }
}

use crate::models::{Account, AccountSecrets, AppConfig, Folder};
use crate::{
    vault::{open_with_key, seal_with_key},
    CoreError, CoreResult,
};
use argon2::Argon2;
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Serialize, Deserialize)]
pub(crate) struct BackupData {
    pub version: u32,
    pub config: AppConfig,
    pub folders: Vec<Folder>,
    pub accounts: Vec<BackupAccount>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct BackupAccount {
    pub metadata: Account,
    pub secrets: AccountSecrets,
}

const HEADER: &[u8] = b"NOCTA-BACKUP-1\0";

pub fn encrypt(plaintext: &[u8], password: &str) -> CoreResult<Vec<u8>> {
    if password.chars().count() < 12 {
        return Err(CoreError::Validation(
            "Backup password must be at least 12 characters.".into(),
        ));
    }
    let mut salt = [0; 16];
    OsRng.fill_bytes(&mut salt);
    let mut key = Zeroizing::new([0; 32]);
    Argon2::default()
        .hash_password_into(password.as_bytes(), &salt, key.as_mut())
        .map_err(|_| CoreError::Decryption)?;
    let ciphertext = seal_with_key(&key, plaintext, HEADER)?;
    let mut bytes = HEADER.to_vec();
    bytes.extend(salt);
    bytes.extend(ciphertext);
    Ok(bytes)
}

pub fn decrypt(bytes: &[u8], password: &str) -> CoreResult<Zeroizing<Vec<u8>>> {
    let start = HEADER.len();
    if bytes.len() < start + 16 + 28 || !bytes.starts_with(HEADER) {
        return Err(CoreError::Decryption);
    }
    let mut key = Zeroizing::new([0; 32]);
    Argon2::default()
        .hash_password_into(password.as_bytes(), &bytes[start..start + 16], key.as_mut())
        .map_err(|_| CoreError::Decryption)?;
    open_with_key(&key, &bytes[start + 16..], HEADER).map(Zeroizing::new)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_backup_roundtrip_and_wrong_password() {
        let bytes = encrypt(b"accounts-and-secrets", "a-long-test-password").unwrap();
        assert_eq!(
            decrypt(&bytes, "a-long-test-password").unwrap().as_slice(),
            b"accounts-and-secrets"
        );
        assert!(decrypt(&bytes, "incorrect-test-password").is_err());
    }
}

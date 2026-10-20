use crate::{CoreError, CoreResult};
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::{rngs::OsRng, RngCore};
use zeroize::Zeroizing;

pub struct Vault {
    key: Zeroizing<[u8; 32]>,
}

impl Vault {
    pub fn open(installation_id: &str, allow_create: bool) -> CoreResult<Self> {
        let entry = keyring::Entry::new("app.nocta.desktop", installation_id).map_err(|_| {
            CoreError::Vault("Cannot access the operating system credential store.".into())
        })?;
        let key = match entry.get_password() {
            Ok(encoded) => {
                let encoded = Zeroizing::new(encoded);
                let bytes = Zeroizing::new(
                    STANDARD
                        .decode(encoded.as_bytes())
                        .map_err(|_| CoreError::Decryption)?,
                );
                <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| CoreError::Decryption)?
            }
            Err(keyring::Error::NoEntry) => {
                if !allow_create {
                    return Err(CoreError::Vault("The encryption key for this database is missing. Restore the original OS credential or recover a password-protected backup into a fresh workspace.".into()));
                }
                let mut key = [0; 32];
                OsRng.fill_bytes(&mut key);
                entry.set_password(&STANDARD.encode(key)).map_err(|_| {
                    CoreError::Vault("Cannot save the local encryption key.".into())
                })?;
                key
            }
            Err(_) => {
                return Err(CoreError::Vault(
                    "Access was denied or the credential store is locked.".into(),
                ))
            }
        };
        Ok(Self {
            key: Zeroizing::new(key),
        })
    }

    pub fn seal(&self, plaintext: &[u8], context: &[u8]) -> CoreResult<Vec<u8>> {
        seal_with_key(&self.key, plaintext, context)
    }

    pub fn open_secret(&self, ciphertext: &[u8], context: &[u8]) -> CoreResult<Zeroizing<Vec<u8>>> {
        open_with_key(&self.key, ciphertext, context).map(Zeroizing::new)
    }

    #[cfg(test)]
    pub(crate) fn test_key() -> Self {
        Self {
            key: Zeroizing::new([7; 32]),
        }
    }

    #[cfg(test)]
    pub(crate) fn test_key_with_byte(byte: u8) -> Self {
        Self {
            key: Zeroizing::new([byte; 32]),
        }
    }
}

pub(crate) fn seal_with_key(
    key: &[u8; 32],
    plaintext: &[u8],
    context: &[u8],
) -> CoreResult<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CoreError::Decryption)?;
    let mut nonce = [0; 12];
    OsRng.fill_bytes(&mut nonce);
    let payload = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: context,
            },
        )
        .map_err(|_| CoreError::Decryption)?;
    let mut bytes = nonce.to_vec();
    bytes.extend(payload);
    Ok(bytes)
}

pub(crate) fn open_with_key(
    key: &[u8; 32],
    ciphertext: &[u8],
    context: &[u8],
) -> CoreResult<Vec<u8>> {
    if ciphertext.len() < 28 {
        return Err(CoreError::Decryption);
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CoreError::Decryption)?;
    cipher
        .decrypt(
            Nonce::from_slice(&ciphertext[..12]),
            Payload {
                msg: &ciphertext[12..],
                aad: context,
            },
        )
        .map_err(|_| CoreError::Decryption)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ciphertext_is_bound_to_record_and_rejects_tampering() {
        let vault = Vault::test_key();
        let mut bytes = vault.seal(b"secret-cookie", b"account:1").unwrap();
        assert_eq!(
            vault.open_secret(&bytes, b"account:1").unwrap().as_slice(),
            b"secret-cookie"
        );
        assert!(vault.open_secret(&bytes, b"account:2").is_err());
        bytes[15] ^= 1;
        assert!(vault.open_secret(&bytes, b"account:1").is_err());
    }
}

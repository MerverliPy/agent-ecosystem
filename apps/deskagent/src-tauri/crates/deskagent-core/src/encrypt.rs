//! Encryption at rest: AES-256-GCM with PBKDF2-HMAC-SHA256 key derivation (DEC-0009).

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

const PBKDF2_ITERATIONS: u32 = 210_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

#[derive(Debug, Serialize, Deserialize)]
pub struct EncryptedPayload {
    pub nonce: String,
    pub cipher: String,
}

/// Derive a 32-byte key from a passphrase + salt (PBKDF2-HMAC-SHA256).
pub fn derive_key(passphrase: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), salt, PBKDF2_ITERATIONS, &mut key);
    key
}

pub fn random_salt() -> Vec<u8> {
    let mut salt = vec![0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

/// A fresh 32-byte encryption key.
pub fn random_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

/// Encrypt `plain` under `key`; returns base64(nonce) + base64(ciphertext).
pub fn encrypt_string(key: &[u8; 32], plain: &str) -> EncryptedPayload {
    let cipher = Aes256Gcm::new_from_slice(key).expect("valid 32-byte key");
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(nonce, plain.as_bytes())
        .expect("aes-gcm encryption");
    EncryptedPayload {
        nonce: base64::engine::general_purpose::STANDARD.encode(nonce_bytes),
        cipher: base64::engine::general_purpose::STANDARD.encode(ct),
    }
}

/// Decrypt a payload produced by [`encrypt_string`].
pub fn decrypt_string(key: &[u8; 32], nonce_b64: &str, cipher_b64: &str) -> Result<String, aes_gcm::Error> {
    let cipher = Aes256Gcm::new_from_slice(key).expect("valid 32-byte key");
    let nonce_bytes = base64::engine::general_purpose::STANDARD
        .decode(nonce_b64)
        .map_err(|_| aes_gcm::Error)?;
    let ct = base64::engine::general_purpose::STANDARD
        .decode(cipher_b64)
        .map_err(|_| aes_gcm::Error)?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let pt = cipher.decrypt(nonce, ct.as_ref())?;
    String::from_utf8(pt).map_err(|_| aes_gcm::Error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_roundtrips() {
        let key = derive_key("correct horse battery staple", &[1u8; 16]);
        let payload = encrypt_string(&key, "user prefers dark mode");
        assert_ne!(payload.cipher, "user prefers dark mode");
        let plain = decrypt_string(&key, &payload.nonce, &payload.cipher).unwrap();
        assert_eq!(plain, "user prefers dark mode");
    }

    #[test]
    fn wrong_key_fails() {
        let k1 = derive_key("right", &[1u8; 16]);
        let k2 = derive_key("wrong", &[1u8; 16]);
        let payload = encrypt_string(&k1, "secret");
        assert!(decrypt_string(&k2, &payload.nonce, &payload.cipher).is_err());
    }

    #[test]
    fn same_plaintext_never_encrypts_identically() {
        let key = derive_key("pw", &[2u8; 16]);
        let a = encrypt_string(&key, "same");
        let b = encrypt_string(&key, "same");
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.cipher, b.cipher);
    }

    #[test]
    fn derive_key_is_deterministic_and_sensitive_to_salt() {
        assert_eq!(derive_key("pw", &[1u8; 16]), derive_key("pw", &[1u8; 16]));
        assert_ne!(derive_key("pw", &[1u8; 16]), derive_key("pw", &[2u8; 16]));
    }
}

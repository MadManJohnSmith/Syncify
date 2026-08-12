//! Cryptography module for credential encryption (CLI Standalone)
//!
//! Uses AES-256-GCM for encrypting service credentials before storage.
//! Key is stored in the OS Keychain (Windows Credential Manager / macOS Keychain / Linux Secret Service).

#![allow(dead_code)]

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::sync::OnceLock;

/// Encryption key stored in OnceLock — initialized once from OS Keychain at startup.
static ENCRYPTION_KEY: OnceLock<[u8; 32]> = OnceLock::new();

/// Keychain service/user identifiers for production use.
const KEYCHAIN_SERVICE: &str = "syncify";
const KEYCHAIN_USER: &str = "encryption-key";

pub fn init_crypto(key: [u8; 32]) -> Result<(), String> {
    ENCRYPTION_KEY
        .set(key)
        .map_err(|_| "Crypto already initialized".to_string())
}

fn get_key() -> Result<&'static [u8; 32], String> {
    ENCRYPTION_KEY
        .get()
        .ok_or_else(|| "Crypto not initialized. Call init_keychain_crypto() first.".to_string())
}

pub fn generate_random_key() -> [u8; 32] {
    use rand::rngs::OsRng as RandOsRng;
    use rand::RngCore;
    let mut key = [0u8; 32];
    RandOsRng.fill_bytes(&mut key);
    key
}

fn load_key_from_keychain() -> Result<[u8; 32], String> {
    load_key_from_keychain_with_service(KEYCHAIN_SERVICE, KEYCHAIN_USER)
}

fn store_key_in_keychain(key: &[u8; 32]) -> Result<(), String> {
    store_key_in_keychain_with_service(key, KEYCHAIN_SERVICE, KEYCHAIN_USER)
}

pub fn load_key_from_keychain_with_service(service: &str, user: &str) -> Result<[u8; 32], String> {
    let entry = keyring::Entry::new(service, user)
        .map_err(|e| format!("Keychain entry creation failed: {}", e))?;

    let encoded = entry
        .get_password()
        .map_err(|e| format!("Keychain read failed: {}", e))?;

    let decoded = BASE64
        .decode(&encoded)
        .map_err(|e| format!("Keychain Base64 decode error: {}", e))?;

    if decoded.len() != 32 {
        return Err(format!(
            "Keychain key has invalid length: {} (expected 32)",
            decoded.len()
        ));
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&decoded);
    Ok(key)
}

pub fn store_key_in_keychain_with_service(
    key: &[u8; 32],
    service: &str,
    user: &str,
) -> Result<(), String> {
    let entry = keyring::Entry::new(service, user)
        .map_err(|e| format!("Keychain entry creation failed: {}", e))?;

    let encoded = BASE64.encode(key);
    entry
        .set_password(&encoded)
        .map_err(|e| format!("Keychain write failed: {}", e))?;

    Ok(())
}

fn fallback_key_path() -> std::path::PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    path.push("com.syncify.app");
    std::fs::create_dir_all(&path).ok();
    path.push(".crypto_key");
    path
}

pub fn init_keychain_crypto() -> Result<(), String> {
    if let Ok(key) = load_key_from_keychain() {
        tracing::info!("Encryption key loaded from OS Keychain");
        return init_crypto(key);
    }
    
    let fallback_path = fallback_key_path();
    if fallback_path.exists() {
        if let Ok(encoded) = std::fs::read_to_string(&fallback_path) {
            if let Ok(decoded) = BASE64.decode(encoded.trim()) {
                if decoded.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&decoded);
                    tracing::info!("Encryption key loaded from fallback file");
                    return init_crypto(key);
                }
            }
        }
    }

    tracing::info!("No existing key found in OS Keychain or fallback, generating new key...");
    let key = generate_random_key();
    
    if let Err(e) = store_key_in_keychain(&key) {
        tracing::warn!("Failed to store key in OS Keychain: {}, using fallback file", e);
    } else {
        tracing::info!("New encryption key stored in OS Keychain");
    }
    
    let encoded = BASE64.encode(&key);
    if let Err(e) = std::fs::write(&fallback_path, encoded) {
        tracing::error!("Failed to write fallback encryption key: {}", e);
    } else {
        tracing::info!("New encryption key stored in fallback file");
    }
    
    init_crypto(key)
}

pub fn encrypt(plaintext: &str) -> Result<String, String> {
    let key = get_key()?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init error: {}", e))?;

    let mut nonce_bytes = [0u8; 12];
    use aes_gcm::aead::rand_core::RngCore;
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption error: {}", e))?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend(ciphertext);

    Ok(BASE64.encode(&combined))
}

pub fn decrypt(encrypted: &str) -> Result<String, String> {
    let key = get_key()?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init error: {}", e))?;

    let combined = BASE64
        .decode(encrypted)
        .map_err(|e| format!("Base64 decode error: {}", e))?;

    if combined.len() < 12 {
        return Err("Invalid encrypted data: too short".into());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption error: {}", e))?;

    String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode error: {}", e))
}

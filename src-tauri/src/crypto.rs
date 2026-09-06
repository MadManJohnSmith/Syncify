//! Cryptography module for credential encryption
//!
//! Uses AES-256-GCM for encrypting service credentials before storage.
//! Key is stored in the OS Keychain (Windows Credential Manager / macOS Keychain / Linux Secret Service).
//!
//! Sprint 01: Replaced deterministic SHA256-derived key with OS Keychain-backed random key.

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

// ═══════════════════════════════════════════════════════
// KEY MANAGEMENT
// ═══════════════════════════════════════════════════════

/// Initialize encryption with an explicitly provided key.
/// Returns Err if the OnceLock has already been initialized (double-init guard).
pub fn init_crypto(key: [u8; 32]) -> Result<(), String> {
    ENCRYPTION_KEY
        .set(key)
        .map_err(|_| "Crypto already initialized".to_string())
}

/// Get the active encryption key. Fails if init_crypto() was never called.
fn get_key() -> Result<&'static [u8; 32], String> {
    ENCRYPTION_KEY
        .get()
        .ok_or_else(|| "Crypto not initialized. Call init_keychain_crypto() first.".to_string())
}

/// Generate a cryptographically secure random 32-byte key using OsRng.
/// OsRng reads directly from BCryptGenRandom (Windows) / /dev/urandom (Linux/macOS)
/// without intermediate state. Correct interface for long-lived cryptographic material.
pub fn generate_random_key() -> [u8; 32] {
    use rand::rngs::OsRng as RandOsRng;
    use rand::RngCore;
    let mut key = [0u8; 32];
    RandOsRng.fill_bytes(&mut key);
    key
}

// ═══════════════════════════════════════════════════════
// OS KEYCHAIN OPERATIONS
// ═══════════════════════════════════════════════════════

/// Load the AES-256 key from the OS Keychain using default service/user identifiers.
fn load_key_from_keychain() -> Result<[u8; 32], String> {
    load_key_from_keychain_with_service(KEYCHAIN_SERVICE, KEYCHAIN_USER)
}

/// Store the AES-256 key in the OS Keychain using default service/user identifiers.
fn store_key_in_keychain(key: &[u8; 32]) -> Result<(), String> {
    store_key_in_keychain_with_service(key, KEYCHAIN_SERVICE, KEYCHAIN_USER)
}

/// Load the AES-256 key from the OS Keychain with parametrized service/user (for tests).
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

/// Store the AES-256 key in the OS Keychain with parametrized service/user (for tests).
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

/// Get the fallback key path
fn fallback_key_path() -> Option<std::path::PathBuf> {
    let mut path = dirs::data_local_dir().or_else(|| std::env::current_dir().ok())?;
    path.push("com.syncify.app");
    std::fs::create_dir_all(&path).ok();
    path.push(".crypto_key");
    Some(path)
}

/// Write fallback key to disk with strict 0600 permissions
fn write_fallback_key(path: &std::path::Path, key: &[u8; 32]) -> Result<(), String> {
    let encoded = BASE64.encode(key);
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| format!("Failed to create fallback encryption key file: {}", e))?;
        file.write_all(encoded.as_bytes())
            .map_err(|e| format!("Failed to write fallback encryption key: {}", e))?;

        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(path, perms);
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, encoded)
            .map_err(|e| format!("Failed to write fallback encryption key: {}", e))?;
    }
    tracing::info!("New encryption key stored in fallback file with 0600 permissions");
    Ok(())
}

/// Load fallback key from disk and harden permissions to 0600 if needed
fn load_fallback_key(path: &std::path::Path) -> Result<[u8; 32], String> {
    if !path.exists() {
        return Err("Fallback key file does not exist".into());
    }
    let encoded = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read fallback key file: {}", e))?;
    let decoded = BASE64
        .decode(encoded.trim())
        .map_err(|e| format!("Failed to decode fallback key: {}", e))?;
    if decoded.len() != 32 {
        return Err(format!("Invalid fallback key length: {}", decoded.len()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            let mode = metadata.permissions().mode() & 0o777;
            if mode != 0o600 {
                let perms = std::fs::Permissions::from_mode(0o600);
                let _ = std::fs::set_permissions(path, perms);
                tracing::info!("Hardened fallback encryption key file permissions to 0600");
            }
        }
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&decoded);
    Ok(key)
}

/// Internal key resolution pipeline: Keychain -> Fallback -> Generate -> (Keychain / Fallback 0600)
fn resolve_or_create_key<FLoad, FStore>(
    mut load_kc: FLoad,
    mut store_kc: FStore,
    fallback_path: Option<&std::path::Path>,
) -> Result<[u8; 32], String>
where
    FLoad: FnMut() -> Result<[u8; 32], String>,
    FStore: FnMut(&[u8; 32]) -> Result<(), String>,
{
    // 1. Try to load from keychain first
    if let Ok(key) = load_kc() {
        tracing::info!("Encryption key loaded from OS Keychain");
        if let Some(path) = fallback_path {
            if path.exists() {
                let _ = std::fs::remove_file(path);
                tracing::info!("Removed legacy fallback encryption key file");
            }
        }
        return Ok(key);
    }

    // 2. Fallback to file storage if keychain fails (e.g. dev environments or missing secret service)
    if let Some(path) = fallback_path {
        if let Ok(key) = load_fallback_key(path) {
            tracing::info!("Encryption key loaded from fallback file");
            return Ok(key);
        }
    }

    // 3. If no key found anywhere, generate a new one
    tracing::info!("No existing key found in OS Keychain or fallback, generating new key...");
    let key = generate_random_key();

    match store_kc(&key) {
        Ok(()) => {
            tracing::info!("New encryption key stored in OS Keychain");
            // Proactively clean up any existing fallback file now that Keychain is functional
            if let Some(path) = fallback_path {
                if path.exists() {
                    let _ = std::fs::remove_file(path);
                    tracing::info!("Removed legacy fallback encryption key file");
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to store key in OS Keychain: {}, using fallback file", e);
            if let Some(path) = fallback_path {
                if let Err(write_err) = write_fallback_key(path, &key) {
                    tracing::error!("{}", write_err);
                }
            }
        }
    }

    Ok(key)
}

/// Initialize crypto from the OS Keychain or fallback file.
pub fn init_keychain_crypto() -> Result<(), String> {
    let fallback_path = fallback_key_path();
    let key = resolve_or_create_key(
        load_key_from_keychain,
        store_key_in_keychain,
        fallback_path.as_deref(),
    )?;
    init_crypto(key)
}

// ═══════════════════════════════════════════════════════
// ENCRYPT / DECRYPT (public API — signatures UNCHANGED)
// ═══════════════════════════════════════════════════════

/// Encrypt a string for storage.
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    let key = get_key()?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init error: {}", e))?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; 12];
    use aes_gcm::aead::rand_core::RngCore;
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption error: {}", e))?;

    // Combine nonce + ciphertext and base64 encode
    let mut combined = nonce_bytes.to_vec();
    combined.extend(ciphertext);

    Ok(BASE64.encode(&combined))
}

/// Decrypt a stored string.
pub fn decrypt(encrypted: &str) -> Result<String, String> {
    let key = get_key()?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init error: {}", e))?;

    // Decode base64
    let combined = BASE64
        .decode(encrypted)
        .map_err(|e| format!("Base64 decode error: {}", e))?;

    if combined.len() < 12 {
        return Err("Invalid encrypted data: too short".into());
    }

    // Split nonce and ciphertext
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption error: {}", e))?;

    String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode error: {}", e))
}

// ═══════════════════════════════════════════════════════
// LEGACY MIGRATION
// ═══════════════════════════════════════════════════════

/// Decrypt using an explicitly provided key instead of the OnceLock.
/// PRIVATE — only caller is migrate_legacy_credentials.
fn decrypt_with_key(encrypted: &str, key: &[u8; 32]) -> Result<String, String> {
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

/// Derive the legacy encryption key from machine-specific data (SHA256).
///
/// DEPRECATED: Only used for migration from legacy encryption scheme.
/// Will be removed in Sprint 03.
///
/// Contains NO unwrap() or expect() — uses if let Ok / if let Some throughout.
#[deprecated(
    note = "Legacy key derivation. Only used for migration. Will be removed in Sprint 03."
)]
fn derive_stable_key() -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();

    // 1. App identifier (always the same)
    hasher.update(b"syncify-music-library-v1");

    // 2. Username (stable per user)
    if let Ok(user) = std::env::var("USERNAME").or_else(|_| std::env::var("USER")) {
        hasher.update(user.as_bytes());
    }

    // 3. Home directory path (stable per user)
    if let Some(home) = dirs::home_dir() {
        hasher.update(home.to_string_lossy().as_bytes());
    }

    // 4. Hostname (stable per machine)
    if let Ok(hostname) = hostname::get() {
        hasher.update(hostname.to_string_lossy().as_bytes());
    }

    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Migrate credentials encrypted with the legacy SHA256-derived key to the new
/// keychain-backed key. Non-blocking — designed to be called from an async task.
///
/// Schema verified against migrations/0002_normalize_schema.sql:38-48:
///   Table: accounts
///   Column: credentials_json (TEXT, nullable, stores encrypted tokens/cookies)
///
/// Returns Ok((migrated_count, failed_account_ids)).
/// Rows that fail to decrypt with the legacy key are left INTACT (not corrupted).
/// These accounts require manual re-authentication.
#[allow(deprecated)] // derive_stable_key is intentionally used here for migration
pub async fn migrate_legacy_credentials(db: &sqlx::SqlitePool) -> Result<(u32, Vec<i64>), String> {
    // Check if migration was already completed
    let already_done: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = 'credential_migration_v1_complete'")
            .fetch_optional(db)
            .await
            .map_err(|e| format!("Failed to check migration status: {}", e))?;

    if already_done.is_some() {
        tracing::debug!("Legacy credential migration already completed, skipping");
        return Ok((0, vec![]));
    }

    let legacy_key = derive_stable_key();

    // Read all accounts with non-null credentials
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, credentials_json FROM accounts WHERE credentials_json IS NOT NULL",
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Failed to read accounts for migration: {}", e))?;

    let mut migrated_count: u32 = 0;
    let mut failed_ids: Vec<i64> = Vec::new();

    for (account_id, ciphertext) in &rows {
        // Attempt decrypt with legacy key — NOT crate::crypto::decrypt()
        match decrypt_with_key(ciphertext, &legacy_key) {
            Ok(plaintext) => {
                // Re-encrypt with new keychain-backed key (via get_key() inside encrypt())
                match encrypt(&plaintext) {
                    Ok(new_ciphertext) => {
                        let update_result =
                            sqlx::query("UPDATE accounts SET credentials_json = ? WHERE id = ?")
                                .bind(&new_ciphertext)
                                .bind(account_id)
                                .execute(db)
                                .await;

                        match update_result {
                            Ok(_) => {
                                migrated_count += 1;
                                tracing::debug!(
                                    "Migrated credentials for account_id={}",
                                    account_id
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to update migrated credentials for account_id={}: {}",
                                    account_id,
                                    e
                                );
                                failed_ids.push(*account_id);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to re-encrypt credentials for account_id={}: {}",
                            account_id,
                            e
                        );
                        failed_ids.push(*account_id);
                    }
                }
            }
            Err(e) => {
                // Row not decryptable with legacy key — leave intact.
                // Could be: already migrated, different user/machine, or corrupted.
                tracing::warn!(
                    "Failed to migrate credentials for account_id={}: {}. \
                     Row left intact — account requires re-authentication.",
                    account_id,
                    e
                );
                failed_ids.push(*account_id);
            }
        }
    }

    // Determine if migration should be marked as complete.
    // Three conditions trigger completion:
    //
    // 1. Perfect migration: migrated_count > 0 && failed_ids.is_empty()
    //    All legacy credentials were successfully re-encrypted.
    //
    // 2. Fresh install / no credentials: rows.is_empty()
    //    No credentials exist to migrate — mark done so we never run again.
    //
    // 3. All rows failed = no legacy credentials remain:
    //    failed_ids.len() == rows.len() && migrated_count == 0
    //    Every row failed to decrypt with the legacy key. This means either:
    //    (a) All credentials were already migrated in a previous session, OR
    //    (b) All credentials are genuinely corrupt (unrelated to migration).
    //    In both cases, re-running migration on every boot adds no value.
    //    Rows are left intact — accounts will require re-authentication if
    //    genuinely unreadable. Mark as complete to stop infinite retries.
    //
    // NOT marked as complete: partial success (migrated_count > 0 && !failed_ids.is_empty())
    // This case means some rows migrated and some failed — the failed ones
    // might succeed on retry (e.g., transient DB lock). Re-run next boot.
    let should_mark_complete = rows.is_empty()
        || (migrated_count > 0 && failed_ids.is_empty())
        || (migrated_count == 0 && failed_ids.len() == rows.len());

    if should_mark_complete {
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) \
             VALUES ('credential_migration_v1_complete', 'true', CURRENT_TIMESTAMP)",
        )
        .execute(db)
        .await
        .map_err(|e| {
            tracing::warn!("Failed to record migration completion flag: {}", e);
            // Non-fatal: migration will re-run next boot but won't corrupt data
        });
    }

    Ok((migrated_count, failed_ids))
}

// ═══════════════════════════════════════════════════════
// PROFILE PERMISSIONS HARDENING & LOCALSTORAGE AUDIT (TASK-112)
// ═══════════════════════════════════════════════════════

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileHardeningReport {
    pub directories_hardened: usize,
    pub files_hardened: usize,
    pub audited_localstorage_items: usize,
    pub purged_localstorage_files: usize,
}

/// Configures strict process umask (0o077) on Unix platforms.
/// This guarantees that any new files and directories created by SQLite,
/// logging, key fallbacks, cookies, or Tauri webview have at most 0700/0600 permissions.
#[cfg(unix)]
pub fn set_secure_process_umask() {
    unsafe {
        libc::umask(0o077);
    }
}

#[cfg(not(unix))]
pub fn set_secure_process_umask() {
    // No-op on non-Unix platforms
}

/// Audits and purges residual external authentication sessions in `localstorage/`
/// (such as `https_accounts.spotify.com_*.localstorage*`), preventing cleartext tokens
/// or session artifacts from lingering outside encrypted storage.
pub fn audit_and_purge_webview_localstorage(profile_dir: &std::path::Path) -> Result<usize, std::io::Error> {
    let ls_dir = profile_dir.join("localstorage");
    if !ls_dir.exists() {
        return Ok(0);
    }

    let mut purged_count = 0;
    let read_dir = match std::fs::read_dir(&ls_dir) {
        Ok(rd) => rd,
        Err(e) => {
            tracing::warn!("Failed to read localstorage directory {:?}: {}", ls_dir, e);
            return Err(e);
        }
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Identify external auth localstorage files (Spotify OAuth WebView residual files)
        if file_name.contains("spotify") || file_name.contains("accounts.spotify.com") {
            tracing::info!(
                "Auditing and purging residual external auth localstorage file: {:?}",
                path
            );
            // Audit: read bytes to check for sensitive token keys (sp_dc, tokens, etc.)
            if let Ok(bytes) = std::fs::read(&path) {
                let content_lossy = String::from_utf8_lossy(&bytes);
                if content_lossy.contains("sp_dc")
                    || content_lossy.contains("access_token")
                    || content_lossy.contains("refresh_token")
                {
                    tracing::warn!(
                        "Sensitive token string found in residual auth localstorage {:?}; purging immediately",
                        path
                    );
                }
            }

            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!("Failed to remove residual localstorage file {:?}: {}", path, e);
            } else {
                purged_count += 1;
            }
        }
    }

    Ok(purged_count)
}

/// Hardens file and directory permissions across the application profile.
/// On Unix platforms, enforces 0700 for directories and 0600 for sensitive files.
/// Also audits and cleans residual WebView localstorage artifacts.
pub fn ensure_secure_profile_permissions(
    profile_dir: &std::path::Path,
) -> Result<ProfileHardeningReport, std::io::Error> {
    let mut report = ProfileHardeningReport::default();

    if !profile_dir.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(profile_dir)?;
            report.directories_hardened += 1;
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(profile_dir)?;
        }
        return Ok(report);
    }

    // Step 1: Audit and purge external auth webview residual files first
    if let Ok(purged) = audit_and_purge_webview_localstorage(profile_dir) {
        report.purged_localstorage_files = purged;
    }

    // Step 2: On Unix, enforce 0700 on root profile_dir and 0700/0600 recursively
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // Check and harden the root profile directory itself
        if let Ok(meta) = std::fs::symlink_metadata(profile_dir) {
            if meta.is_dir() {
                let mode = meta.permissions().mode() & 0o777;
                if mode != 0o700 {
                    match std::fs::set_permissions(
                        profile_dir,
                        std::fs::Permissions::from_mode(0o700),
                    ) {
                        Ok(()) => report.directories_hardened += 1,
                        Err(e) => tracing::debug!("Could not set 0700 on root profile dir {:?}: {}", profile_dir, e),
                    }
                }
            }
        }

        // Walk the profile directory
        for entry in walkdir::WalkDir::new(profile_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path == profile_dir {
                continue;
            }

            if let Ok(meta) = entry.metadata() {
                // If entry is a symlink, avoid touching target
                if meta.file_type().is_symlink() {
                    continue;
                }

                let mode = meta.permissions().mode() & 0o777;
                if meta.is_dir() {
                    if mode != 0o700 {
                        match std::fs::set_permissions(
                            path,
                            std::fs::Permissions::from_mode(0o700),
                        ) {
                            Ok(()) => report.directories_hardened += 1,
                            Err(e) => tracing::debug!("Could not set 0700 on dir {:?}: {}", path, e),
                        }
                    }
                } else if meta.is_file() {
                    if mode != 0o600 {
                        match std::fs::set_permissions(
                            path,
                            std::fs::Permissions::from_mode(0o600),
                        ) {
                            Ok(()) => report.files_hardened += 1,
                            Err(e) => tracing::debug!("Could not set 0600 on file {:?}: {}", path, e),
                        }
                    }
                }
            }
        }
    }

    Ok(report)
}

// ═══════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires OS keychain daemon (run with cargo test -- --include-ignored)"]
    fn test_keychain_roundtrip() {
        let key = generate_random_key();
        store_key_in_keychain_with_service(&key, "syncify-test", "test-key")
            .expect("Failed to store key in keychain");
        let loaded = load_key_from_keychain_with_service("syncify-test", "test-key")
            .expect("Failed to load key from keychain");
        assert_eq!(key, loaded);
        // Cleanup: remove test entry from OS keychain
        let _ = keyring::Entry::new("syncify-test", "test-key").and_then(|e| e.delete_credential());
    }

    #[test]
    fn test_key_base64_roundtrip() {
        let key = generate_random_key();
        let encoded = BASE64.encode(&key);
        let decoded = BASE64.decode(&encoded).expect("Base64 decode failed");
        assert_eq!(key.len(), 32);
        assert_eq!(&key[..], &decoded[..]);
    }

    #[test]
    fn test_credential_encrypt_decrypt() {
        // OnceLock may already be initialized from another test in this process.
        // init_crypto returns Err if already set — absorb that case.
        let key = generate_random_key();
        let _ = init_crypto(key);

        let original = r#"{"access_token":"abc123","refresh_token":"xyz789"}"#;

        let encrypted = encrypt(original).expect("encrypt() failed");
        assert_ne!(encrypted, original);

        let decrypted = decrypt(&encrypted).expect("decrypt() failed");
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_fallback_key_write_and_permissions_0600() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let fallback_file = temp_dir.path().join(".crypto_key");
        let key = generate_random_key();

        write_fallback_key(&fallback_file, &key).expect("Failed to write fallback key");
        assert!(fallback_file.exists());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(&fallback_file).expect("Failed to read metadata");
            let mode = metadata.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "Fallback key file must have strictly 0600 permissions");
        }

        let loaded = load_fallback_key(&fallback_file).expect("Failed to load fallback key");
        assert_eq!(key, loaded);
    }

    #[test]
    fn test_load_fallback_key_hardens_insecure_permissions() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let fallback_file = temp_dir.path().join(".crypto_key");
        let key = generate_random_key();

        // Write insecurely using std::fs::write
        std::fs::write(&fallback_file, BASE64.encode(&key)).expect("Failed to write key");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o644);
            std::fs::set_permissions(&fallback_file, perms).expect("Failed to set 0644");
            let mode_before = std::fs::metadata(&fallback_file).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode_before, 0o644);
        }

        let loaded = load_fallback_key(&fallback_file).expect("Failed to load fallback key");
        assert_eq!(key, loaded);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode_after = std::fs::metadata(&fallback_file).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode_after, 0o600, "load_fallback_key must harden permissions to 0600");
        }
    }

    #[test]
    fn test_resolve_key_no_unconditional_fallback_when_keychain_succeeds() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let fallback_file = temp_dir.path().join(".crypto_key");

        // Pre-create a legacy fallback file
        std::fs::write(&fallback_file, "legacy_content").expect("Failed to write legacy file");
        assert!(fallback_file.exists());

        let mut stored_key: Option<[u8; 32]> = None;

        // Simulate: load from keychain fails, but store in keychain succeeds
        let key = resolve_or_create_key(
            || Err("No key in keychain".into()),
            |k| {
                stored_key = Some(*k);
                Ok(())
            },
            Some(&fallback_file),
        )
        .expect("resolve_or_create_key failed");

        assert_eq!(stored_key, Some(key));
        // Crucial check: .crypto_key must NOT exist on disk when keychain succeeds
        assert!(
            !fallback_file.exists(),
            "Fallback file must be proactively purged and not written when keychain succeeds"
        );
    }

    #[test]
    fn test_resolve_key_keychain_load_success_removes_legacy_file() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let fallback_file = temp_dir.path().join(".crypto_key");

        // Pre-create a legacy fallback file
        std::fs::write(&fallback_file, "legacy_content").expect("Failed to write legacy file");
        assert!(fallback_file.exists());

        let kc_key = generate_random_key();

        // Simulate: load from keychain succeeds
        let key = resolve_or_create_key(
            || Ok(kc_key),
            |_| panic!("store should not be called when load succeeds"),
            Some(&fallback_file),
        )
        .expect("resolve_or_create_key failed");

        assert_eq!(key, kc_key);
        assert!(
            !fallback_file.exists(),
            "Legacy fallback file must be deleted when keychain load succeeds"
        );
    }

    #[test]
    fn test_resolve_key_keychain_failure_uses_fallback_with_0600() {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let fallback_file = temp_dir.path().join(".crypto_key");

        // Simulate: load from keychain fails, and store in keychain also fails
        let key = resolve_or_create_key(
            || Err("Keychain load error".into()),
            |_| Err("Keychain store error".into()),
            Some(&fallback_file),
        )
        .expect("resolve_or_create_key failed");

        assert!(
            fallback_file.exists(),
            "Fallback file must exist when keychain operations fail"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&fallback_file).unwrap().permissions().mode() & 0o777;
            assert_eq!(
                mode, 0o600,
                "Fallback file created on keychain failure must have 0600 permissions"
            );
        }

        let loaded = load_fallback_key(&fallback_file).expect("Failed to load fallback file");
        assert_eq!(key, loaded);
    }
}

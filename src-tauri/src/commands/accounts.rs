// Accounts Commands - included via include!() in mod.rs
// 
// Account management, service connections


// ==============================================
// ACCOUNT MANAGEMENT COMMANDS
// ==============================================

use crate::crypto;

/// Service info for frontend
#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct ServiceInfo {
    pub id: i64,
    pub name: String,
    pub supports_download: i64,
    pub max_quality: Option<String>,
}

/// Account info for frontend (credentials excluded)
#[derive(Debug, Clone, serde::Serialize)]
pub struct AccountInfo {
    pub id: i64,
    pub service_id: i64,
    pub service_name: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub is_active: bool,
    pub last_synced: Option<String>,
    pub created_at: Option<String>,
}

/// Get all supported services
#[tauri::command]
pub async fn get_services(state: State<'_, AppState>) -> Result<Vec<ServiceInfo>, String> {
    let services = sqlx::query_as::<_, ServiceInfo>(
        "SELECT id, name, supports_download, max_quality FROM services ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    Ok(services)
}

/// Get all connected accounts (without credentials)
#[tauri::command]
pub async fn get_accounts(state: State<'_, AppState>) -> Result<Vec<AccountInfo>, String> {
    let rows: Vec<(i64, i64, String, Option<String>, Option<String>, i64, Option<String>, Option<String>)> = 
        sqlx::query_as(
            r#"SELECT a.id, a.service_id, s.name, a.display_name, a.email, a.is_active, a.last_synced, a.created_at
               FROM accounts a
               JOIN services s ON s.id = a.service_id
               ORDER BY s.name, a.created_at"#
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    let accounts = rows
        .into_iter()
        .map(
            |(
                id,
                service_id,
                service_name,
                display_name,
                email,
                is_active,
                last_synced,
                created_at,
            )| {
                AccountInfo {
                    id,
                    service_id,
                    service_name,
                    display_name,
                    email,
                    is_active: is_active != 0,
                    last_synced,
                    created_at,
                }
            },
        )
        .collect();

    Ok(accounts)
}

/// Add a new account with encrypted credentials
#[tauri::command]
pub async fn add_account(
    state: State<'_, AppState>,
    service_id: i64,
    credentials_json: String,
    display_name: Option<String>,
    email: Option<String>,
) -> Result<i64, String> {
    // Encrypt credentials before storage
    let encrypted = crypto::encrypt(&credentials_json)?;

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, credentials_json, display_name, email, is_active, created_at)
           VALUES (?, ?, ?, ?, 1, CURRENT_TIMESTAMP) RETURNING id"#
    )
    .bind(service_id)
    .bind(&encrypted)
    .bind(&display_name)
    .bind(&email)
    .fetch_one(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    tracing::info!("Added account for service_id={}", service_id);

    Ok(account_id)
}

/// Remove an account
#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, account_id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM accounts WHERE id = ?")
        .bind(account_id)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Removed account id={}", account_id);

    Ok(())
}

/// Get decrypted credentials for an account (internal use)
#[tauri::command]
pub async fn get_account_credentials(
    state: State<'_, AppState>,
    account_id: i64,
) -> Result<String, String> {
    let encrypted: Option<(String,)> =
        sqlx::query_as("SELECT credentials_json FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| e.to_string())?;

    match encrypted {
        Some((creds,)) => {
            match crypto::decrypt(&creds) {
                Ok(decrypted) => Ok(decrypted),
                Err(e) if e.contains("Decryption error") || e.contains("aead::Error") => {
                    tracing::error!("Decryption error for account {}: {}. Clearing credentials.", account_id, e);
                    let _ = sqlx::query("UPDATE accounts SET credentials_json = NULL WHERE id = ?")
                        .bind(account_id)
                        .execute(&state.db)
                        .await;
                    Err("Service credentials expired. Please reconnect your account.".to_string())
                }
                Err(e) => Err(e),
            }
        },
        None => Err("Account not found".into()),
    }
}

/// Update account's last synced time
#[tauri::command]
pub async fn update_account_sync_time(
    state: State<'_, AppState>,
    account_id: i64,
) -> Result<(), String> {
    sqlx::query("UPDATE accounts SET last_synced = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(account_id)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Purge accounts with irrecoverable credentials (aeadError).
///
/// Called at startup and available as a Tauri command.
/// When switching machines, the OS Keychain key changes, making all
/// credentials encrypted on the old machine undecryptable.
/// This function detects and removes those stale accounts so the UI
/// correctly shows services as "disconnected" instead of "connected"
/// with aeadError on every import attempt.
///
/// Returns: (purged_count, vec of purged service names)
#[tauri::command]
pub async fn purge_stale_credentials(
    state: State<'_, AppState>,
) -> Result<(u32, Vec<String>), String> {
    tracing::info!("purge_stale_credentials: checking for irrecoverable credentials");

    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        r#"SELECT a.id, s.name, a.credentials_json
           FROM accounts a
           JOIN services s ON s.id = a.service_id
           WHERE a.credentials_json IS NOT NULL"#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    let mut purged_count: u32 = 0;
    let mut purged_services: Vec<String> = Vec::new();

    for (account_id, service_name, ciphertext) in &rows {
        match crypto::decrypt(ciphertext) {
            Ok(_) => {
                // Credentials are valid — this account's encryption key matches
                tracing::debug!("Account {} ({}) credentials OK", account_id, service_name);
            }
            Err(e) if e.contains("Decryption error") || e.contains("aead") => {
                // Key mismatch — credentials are from a different machine's keychain
                tracing::warn!(
                    "Purging stale account {} ({}) — credentials irrecoverable: {}",
                    account_id,
                    service_name,
                    e
                );
                let _ = sqlx::query("UPDATE accounts SET credentials_invalid = 1 WHERE id = ?")
                    .bind(account_id)
                    .execute(&state.db)
                    .await;
                purged_count += 1;
                purged_services.push(service_name.clone());
            }
            Err(e) => {
                // Other error (Base64, UTF-8, etc.) — also stale, purge
                tracing::warn!(
                    "Purging account {} ({}) — credential error: {}",
                    account_id,
                    service_name,
                    e
                );
                let _ = sqlx::query("UPDATE accounts SET credentials_invalid = 1 WHERE id = ?")
                    .bind(account_id)
                    .execute(&state.db)
                    .await;
                purged_count += 1;
                purged_services.push(service_name.clone());
            }
        }
    }

    if purged_count > 0 {
        tracing::info!(
            "Purged {} stale accounts: {:?}. Re-authentication required.",
            purged_count,
            purged_services
        );
    } else {
        tracing::info!("All account credentials are valid — no purge needed");
    }

    Ok((purged_count, purged_services))
}

/// Toggle account active status
#[tauri::command]
pub async fn toggle_account_active(
    state: State<'_, AppState>,
    account_id: i64,
    is_active: bool,
) -> Result<(), String> {
    sqlx::query("UPDATE accounts SET is_active = ? WHERE id = ?")
        .bind(if is_active { 1 } else { 0 })
        .bind(account_id)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod accounts_tests {

    use sqlx::sqlite::SqlitePoolOptions;

    /// Create an in-memory test database with schema
    async fn setup_test_db() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");

        // Create minimal schema for testing
        sqlx::query(
            r#"
            CREATE TABLE services (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                supports_download INTEGER DEFAULT 0,
                max_quality TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create services table");

        sqlx::query(
            r#"
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
                display_name TEXT,
                email TEXT,
                is_active INTEGER DEFAULT 1,
                credentials_json TEXT,
                last_synced TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(service_id, email)
            )
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create accounts table");

        // Seed services
        sqlx::query(
            r#"
            INSERT INTO services (name, supports_download, max_quality) VALUES
                ('spotify', 0, 'lossy'),
                ('qobuz', 1, 'hires'),
                ('tidal', 1, 'hires')
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to seed services");

        pool
    }

    #[tokio::test]
    async fn test_add_account_inserts_record() {
        let pool = setup_test_db().await;

        // Get spotify service id
        let (service_id,): (i64,) =
            sqlx::query_as("SELECT id FROM services WHERE name = 'spotify'")
                .fetch_one(&pool)
                .await
                .expect("Failed to get spotify id");

        // Insert account directly (simulating add_account command)
        let encrypted_creds = "encrypted_test_data";
        let result = sqlx::query(
            r#"INSERT INTO accounts (service_id, credentials_json, display_name, email, is_active, created_at)
               VALUES (?, ?, ?, ?, 1, CURRENT_TIMESTAMP)"#
        )
        .bind(service_id)
        .bind(encrypted_creds)
        .bind("Test Account")
        .bind("test@example.com")
        .execute(&pool)
        .await
        .expect("Failed to insert account");

        assert!(result.last_insert_rowid() > 0);

        // Verify account exists
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
            .fetch_one(&pool)
            .await
            .expect("Failed to count accounts");

        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn test_get_accounts_returns_account_info() {
        let pool = setup_test_db().await;

        // Get qobuz service ID dynamically
        let (qobuz_id,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'qobuz'")
            .fetch_one(&pool)
            .await
            .expect("Failed to get qobuz id");

        // Insert test account with correct service_id
        sqlx::query(
            "INSERT INTO accounts (service_id, display_name, email, is_active) VALUES (?, 'My Qobuz', 'qobuz@test.com', 1)"
        )
        .bind(qobuz_id)
        .execute(&pool)
        .await
        .expect("Failed to insert account");

        // Query accounts with join (simulating get_accounts)
        let rows: Vec<(i64, i64, String, Option<String>, Option<String>, i64, Option<String>, Option<String>)> = 
            sqlx::query_as(
                r#"SELECT a.id, a.service_id, s.name, a.display_name, a.email, a.is_active, a.last_synced, a.created_at
                   FROM accounts a
                   JOIN services s ON s.id = a.service_id
                   ORDER BY s.name, a.created_at"#
            )
            .fetch_all(&pool)
            .await
            .expect("Failed to fetch accounts");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].2, "qobuz"); // service_name
        assert_eq!(rows[0].3, Some("My Qobuz".to_string())); // display_name
        assert_eq!(rows[0].4, Some("qobuz@test.com".to_string())); // email
    }

    #[tokio::test]
    async fn test_remove_account_deletes_record() {
        let pool = setup_test_db().await;

        // Insert account
        sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active) VALUES (1, 'delete@test.com', 1)",
        )
        .execute(&pool)
        .await
        .expect("Failed to insert account");

        // Verify exists
        let count_before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
            .fetch_one(&pool)
            .await
            .expect("Failed to count accounts before delete");
        assert_eq!(count_before.0, 1);

        // Delete account (simulating remove_account)
        sqlx::query("DELETE FROM accounts WHERE id = 1")
            .execute(&pool)
            .await
            .expect("Failed to delete account");

        // Verify deleted
        let count_after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts")
            .fetch_one(&pool)
            .await
            .expect("Failed to count accounts after delete");
        assert_eq!(count_after.0, 0);
    }

    #[tokio::test]
    async fn test_toggle_account_active() {
        let pool = setup_test_db().await;

        // Insert active account
        sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active) VALUES (1, 'toggle@test.com', 1)",
        )
        .execute(&pool)
        .await
        .expect("Failed to insert account");

        // Toggle to inactive
        sqlx::query("UPDATE accounts SET is_active = 0 WHERE id = 1")
            .execute(&pool)
            .await
            .expect("Failed to toggle account");

        // Verify inactive
        let (is_active,): (i64,) = sqlx::query_as("SELECT is_active FROM accounts WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch account");

        assert_eq!(is_active, 0);

        // Toggle back to active
        sqlx::query("UPDATE accounts SET is_active = 1 WHERE id = 1")
            .execute(&pool)
            .await
            .expect("Failed to toggle account");

        let (is_active2,): (i64,) = sqlx::query_as("SELECT is_active FROM accounts WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch account after toggle back to active");

        assert_eq!(is_active2, 1);
    }

    #[tokio::test]
    async fn test_update_account_sync_time() {
        let pool = setup_test_db().await;

        // Insert account with no sync time
        sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active, last_synced) VALUES (1, 'sync@test.com', 1, NULL)"
        )
        .execute(&pool)
        .await
        .expect("Failed to insert account");

        // Update sync time
        sqlx::query("UPDATE accounts SET last_synced = CURRENT_TIMESTAMP WHERE id = 1")
            .execute(&pool)
            .await
            .expect("Failed to update sync time");

        // Verify sync time is set
        let (last_synced,): (Option<String>,) =
            sqlx::query_as("SELECT last_synced FROM accounts WHERE id = 1")
                .fetch_one(&pool)
                .await
                .expect("Failed to fetch account");

        assert!(last_synced.is_some());
    }

    #[tokio::test]
    async fn test_credentials_stored_encrypted() {
        let pool = setup_test_db().await;

        // Initialize crypto for test (OnceLock may already be set by another test — absorb)
        let key = crate::crypto::generate_random_key();
        let _ = crate::crypto::init_crypto(key);

        // Simulate encrypted credentials
        let plaintext = r#"{"access_token": "secret123"}"#;
        let encrypted = crate::crypto::encrypt(plaintext).expect("Encryption failed");

        // Insert with encrypted creds
        sqlx::query(
            "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'creds@test.com', ?, 1)"
        )
        .bind(&encrypted)
        .execute(&pool)
        .await
        .expect("Failed to insert account");

        // Fetch and verify encrypted value is different from plaintext
        let (stored_creds,): (String,) =
            sqlx::query_as("SELECT credentials_json FROM accounts WHERE id = 1")
                .fetch_one(&pool)
                .await
                .expect("Failed to fetch credentials");

        assert_ne!(stored_creds, plaintext);

        // Decrypt and verify
        let decrypted = crate::crypto::decrypt(&stored_creds).expect("Decryption failed");
        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_unique_constraint_service_email() {
        let pool = setup_test_db().await;

        // Insert first account
        sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active) VALUES (1, 'dupe@test.com', 1)",
        )
        .execute(&pool)
        .await
        .expect("Failed to insert first account");

        // Try to insert duplicate - should fail
        let result = sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active) VALUES (1, 'dupe@test.com', 1)",
        )
        .execute(&pool)
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multiple_accounts_per_service() {
        let pool = setup_test_db().await;

        // Insert multiple accounts for same service with different emails
        sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active) VALUES (1, 'user1@test.com', 1)",
        )
        .execute(&pool)
        .await
        .expect("Failed to insert first account");

        sqlx::query(
            "INSERT INTO accounts (service_id, email, is_active) VALUES (1, 'user2@test.com', 1)",
        )
        .execute(&pool)
        .await
        .expect("Failed to insert second account");

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM accounts WHERE service_id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to count accounts");

        assert_eq!(count.0, 2);
    }
}

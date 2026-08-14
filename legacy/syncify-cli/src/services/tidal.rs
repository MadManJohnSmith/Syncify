//! Tidal service - Authentication, data models, candidate scoring, and matching rules (CLI Standalone)

#![allow(dead_code)]

use sqlx::SqlitePool;
pub use syncify_tidal_downloader::{TidalAuthResolution, TidalAuthStatus, TidalGuiCredentials};


/// Refresh an expired Tidal access token using exact client_id and client_secret from credentials
pub async fn refresh_gui_token(
    client: &reqwest::Client,
    creds: &TidalGuiCredentials,
) -> Result<(String, TidalGuiCredentials), String> {
    syncify_tidal_downloader::refresh_gui_token(client, creds)
        .await
        .map_err(|e| e.to_string())
}


/// Resolve Tidal active account credentials from Syncify GUI database (`accounts` table)
pub async fn resolve_gui_credentials_from_pool(
    pool: &SqlitePool,
    http_client: &reqwest::Client,
) -> (Option<String>, TidalAuthResolution) {
    let _ = crate::crypto::init_keychain_crypto();
    let row: Option<(i64, Option<String>)> = sqlx::query_as(
        r#"
        SELECT a.id, a.credentials_json
        FROM accounts a
        JOIN services s ON s.id = a.service_id
        WHERE s.name = 'tidal' AND a.is_active = 1
        LIMIT 1
        "#
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let (account_id, encrypted_json) = match row {
        Some((id, Some(json))) if !json.trim().is_empty() => (id, json),
        _ => return (None, TidalAuthResolution::RequiresAuth),
    };

    let decrypted = match crate::crypto::decrypt(&encrypted_json) {
        Ok(d) => d,
        Err(e) => return (None, TidalAuthResolution::SourceUnavailable(format!("Failed to decrypt GUI credentials: {}", e))),
    };

    let creds: TidalGuiCredentials = match serde_json::from_str(&decrypted) {
        Ok(c) => c,
        Err(e) => return (None, TidalAuthResolution::SourceUnavailable(format!("Invalid credentials JSON structure: {}", e))),
    };

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    if !creds.is_expired(now_secs) {
        return (Some(creds.access_token.clone()), TidalAuthResolution::StoredGuiAccessToken(creds.access_token));
    }

    // Token is expired; attempt refresh if refresh_token is available
    if let Some(ref ref_tok) = creds.refresh_token {
        if !ref_tok.trim().is_empty() {
            match refresh_gui_token(http_client, &creds).await {
                Ok((new_access_token, updated_creds)) => {
                    // Re-encrypt updated credentials JSON and persist ONLY on 100% success
                    if let Ok(serialized) = serde_json::to_string(&updated_creds) {
                        if let Ok(encrypted_new) = crate::crypto::encrypt(&serialized) {
                            let _ = sqlx::query("UPDATE accounts SET credentials_json = ? WHERE id = ?")
                                .bind(&encrypted_new)
                                .bind(account_id)
                                .execute(pool)
                                .await;
                        }
                    }
                    return (Some(new_access_token.clone()), TidalAuthResolution::RefreshedGuiToken(new_access_token));
                }
                Err(e) => {
                    // DO NOT UPDATE OR OVERWRITE SQLITE DATABASE ON REFRESH FAILURE!
                    // Original credentials in DB remain intact.
                    return (None, TidalAuthResolution::SourceUnavailable(format!("GUI token refresh failed: {}; original credentials preserved in DB", e)));
                }
            }
        }
    }

    (None, TidalAuthResolution::RequiresAuth)
}
pub use syncify_core_domain::metadata::{
    artist_matches, clean_title, score_tidal_candidate, score_tidal_release, title_matches,
    TidalAlbum, TidalArtist, TidalMediaMetadata, TidalSearchResponse, TidalSearchTracks, TidalTrack,
};
pub use syncify_core_domain::quality::StreamSourceType;


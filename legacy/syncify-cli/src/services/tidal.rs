//! Tidal service - Authentication, data models, candidate scoring, and matching rules (CLI Standalone)

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Tidal Authentication Status Hierarchy
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TidalAuthStatus {
    /// Valid user token provided or stored for private library access
    UserToken(String),
    /// OAuth Client Credentials token acquired for public catalog access
    ClientCredentials(String),
    /// Authentication required but not available
    RequiresAuth,
    /// Tidal API service unavailable
    SourceUnavailable(String),
    /// General failure state
    Failed(String),
}

impl TidalAuthStatus {
    pub fn is_user_authenticated(&self) -> bool {
        matches!(self, TidalAuthStatus::UserToken(_))
    }

    pub fn can_access_public_catalog(&self) -> bool {
        matches!(self, TidalAuthStatus::UserToken(_) | TidalAuthStatus::ClientCredentials(_))
    }
}

/// Resolution state of GUI account credentials loaded from SQLite
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TidalAuthResolution {
    StoredGuiAccessToken(String),
    RefreshedGuiToken(String),
    ExplicitOverrideToken(String),
    RequiresAuth,
    SourceUnavailable(String),
}

impl std::fmt::Display for TidalAuthResolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TidalAuthResolution::StoredGuiAccessToken(_) => write!(f, "Stored GUI Access Token"),
            TidalAuthResolution::RefreshedGuiToken(_) => write!(f, "Refreshed GUI Token"),
            TidalAuthResolution::ExplicitOverrideToken(_) => write!(f, "Explicit Override Token"),
            TidalAuthResolution::RequiresAuth => write!(f, "Requires Authentication"),
            TidalAuthResolution::SourceUnavailable(reason) => write!(f, "Source Unavailable ({})", reason),
        }
    }
}

/// Decrypted structure stored in `accounts.credentials_json` by Syncify GUI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalGuiCredentials {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_expiry: Option<f64>,
    pub expires_at: Option<f64>,
    pub expires_in: Option<f64>,
    pub user_id: Option<serde_json::Value>,
    pub country_code: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
}

impl TidalGuiCredentials {
    pub fn get_client_id(&self) -> &str {
        if let Some(ref cid) = self.client_id {
            if !cid.trim().is_empty() {
                return cid.as_str();
            }
        }
        // Default Device Code OAuth client_id used by tidal_auth.py
        "fX2JxdmntZWK0ixT"
    }

    pub fn get_client_secret(&self) -> &str {
        if let Some(ref sec) = self.client_secret {
            if !sec.trim().is_empty() {
                return sec.as_str();
            }
        }
        "xeuPmY7nbpZ9IIbLAcQ93shka1VNheUAqN6IcszjTG8="
    }

    pub fn get_expiry_timestamp(&self) -> Option<f64> {
        if let Some(exp) = self.token_expiry {
            Some(exp)
        } else if let Some(exp) = self.expires_at {
            Some(exp)
        } else {
            None
        }
    }

    pub fn is_expired(&self, now_secs: f64) -> bool {
        if let Some(exp) = self.get_expiry_timestamp() {
            now_secs >= exp - 60.0
        } else {
            // If no expiration timestamp is stored, force refresh if refresh_token exists
            self.refresh_token.is_some()
        }
    }
}

/// Refresh an expired Tidal access token using exact client_id and client_secret from credentials
pub async fn refresh_gui_token(
    client: &reqwest::Client,
    creds: &TidalGuiCredentials,
) -> Result<(String, TidalGuiCredentials), String> {
    let refresh_tok = creds
        .refresh_token
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "No refresh_token available in credentials".to_string())?;

    let client_id = creds.get_client_id();
    let client_secret = creds.get_client_secret();
    let url = "https://auth.tidal.com/v1/oauth2/token";

    let params = [
        ("client_id", client_id),
        ("refresh_token", refresh_tok),
        ("grant_type", "refresh_token"),
        ("scope", "r_usr+w_usr+w_sub"),
    ];

    let resp = client
        .post(url)
        .basic_auth(client_id, Some(client_secret))
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token refresh HTTP request failed: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(format!("Token refresh HTTP {}: {}", status, text));
    }

    let json_val: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse token refresh response JSON: {}", e))?;

    let access_token = json_val["access_token"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "No access_token field in refresh response".to_string())?
        .to_string();

    let new_refresh_token = json_val["refresh_token"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .or_else(|| creds.refresh_token.clone());

    let expires_in = json_val["expires_in"].as_f64().unwrap_or(14400.0);
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    let token_expiry = now_secs + expires_in;

    let updated_creds = TidalGuiCredentials {
        access_token: access_token.clone(),
        refresh_token: new_refresh_token,
        token_expiry: Some(token_expiry),
        expires_at: Some(token_expiry),
        expires_in: Some(expires_in),
        user_id: json_val.get("user").and_then(|u| u.get("userId")).cloned().or_else(|| creds.user_id.clone()),
        country_code: json_val.get("user").and_then(|u| u.get("countryCode")).and_then(|v| v.as_str()).map(|s| s.to_string()).or_else(|| creds.country_code.clone()),
        client_id: creds.client_id.clone(),
        client_secret: creds.client_secret.clone(),
    };

    Ok((access_token, updated_creds))
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

/// Classification of Tidal Stream Sources
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StreamSourceType {
    TidalOfficial,
    TidalProxy(String),
    RequiresAuth,
    SourceUnavailable(String),
    Failed(String),
}

impl std::fmt::Display for StreamSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamSourceType::TidalOfficial => write!(f, "Tidal Official API"),
            StreamSourceType::TidalProxy(domain) => write!(f, "Tidal Proxy ({})", domain),
            StreamSourceType::RequiresAuth => write!(f, "Requires Authentication"),
            StreamSourceType::SourceUnavailable(reason) => write!(f, "Source Unavailable ({})", reason),
            StreamSourceType::Failed(reason) => write!(f, "Failed ({})", reason),
        }
    }
}

/// Tidal track data model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalTrack {
    pub id: i64,
    pub title: String,
    pub isrc: Option<String>,
    pub duration: i32,
    #[serde(rename = "audioQuality")]
    pub audio_quality: Option<String>,
    pub album: Option<TidalAlbum>,
    pub artist: Option<TidalArtist>,
    pub artists: Option<Vec<TidalArtist>>,
    #[serde(rename = "trackNumber")]
    pub track_number: Option<i32>,
    #[serde(rename = "mediaMetadata")]
    pub media_metadata: Option<TidalMediaMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalAlbum {
    pub id: Option<i64>,
    pub title: String,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    pub cover: Option<String>,
}

impl TidalAlbum {
    pub fn cover_url(&self) -> Option<String> {
        self.cover.as_ref().map(|c| {
            if c.starts_with("http") {
                c.clone()
            } else {
                format!("https://resources.tidal.com/images/{}/1280x1280.jpg", c.replace('-', "/"))
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalArtist {
    pub id: Option<i64>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalMediaMetadata {
    pub tags: Option<Vec<String>>,
}

/// Candidate scoring for smart studio origin matching
pub fn score_tidal_candidate(
    album_title: &str,
    album_artist: &str,
    performer: &str,
    track_title: &str,
    version: &str,
    expected_artist: &str,
    is_hires: bool,
) -> i32 {
    let mut score = 0i32;
    let alb_lower = album_title.to_lowercase();
    let perf_lower = performer.to_lowercase();
    let exp_lower = expected_artist.to_lowercase();
    let ver_lower = version.to_lowercase();
    let trk_lower = track_title.to_lowercase();

    if perf_lower.contains(&exp_lower) || exp_lower.contains(&perf_lower) {
        score += 30;
    }

    if is_hires {
        score += 20;
    }

    let live_keywords = ["live", "en vivo", "in concert", "bbc sessions", "bootleg", "tour"];
    let is_live_expected = live_keywords.iter().any(|k| trk_lower.contains(k));
    let is_live_album = live_keywords.iter().any(|k| alb_lower.contains(k) || ver_lower.contains(k));

    if !is_live_expected && is_live_album {
        score -= 50;
    }

    let studio_keywords = ["remaster", "remastered", "deluxe", "expanded", "studio", "original"];
    if studio_keywords.iter().any(|k| alb_lower.contains(k) || ver_lower.contains(k)) {
        score += 15;
    }

    score
}

pub fn clean_title(title: &str) -> String {
    let mut clean = title.to_string();
    if let Some(pos) = clean.find(" (Remaster") {
        clean.truncate(pos);
    }
    if let Some(pos) = clean.find(" (Deluxe") {
        clean.truncate(pos);
    }
    if let Some(pos) = clean.find(" - Remaster") {
        clean.truncate(pos);
    }
    clean.trim().to_string()
}

pub fn title_matches(expected: &str, candidate: &str) -> bool {
    let clean_exp = clean_title(expected).to_lowercase();
    let clean_cand = clean_title(candidate).to_lowercase();
    clean_exp == clean_cand || clean_cand.contains(&clean_exp) || clean_exp.contains(&clean_cand)
}

pub fn artist_matches(expected: &str, candidate: &str) -> bool {
    let exp_low = expected.to_lowercase();
    let cand_low = candidate.to_lowercase();
    exp_low == cand_low || cand_low.contains(&exp_low) || exp_low.contains(&cand_low)
}

#[derive(Debug, Deserialize)]
pub struct TidalSearchResponse {
    pub tracks: Option<TidalSearchTracks>,
}

#[derive(Debug, Deserialize)]
pub struct TidalSearchTracks {
    pub items: Vec<TidalTrack>,
}

pub fn score_tidal_release(track: &TidalTrack, expected_artist: &str) -> i32 {
    let alb_title = track.album.as_ref().map(|a| a.title.as_str()).unwrap_or("");
    let perf_name = track.artist.as_ref().map(|a| a.name.as_str()).unwrap_or("");
    let is_hires = track.audio_quality.as_deref() == Some("HI_RES_LOSSLESS") || track.audio_quality.as_deref() == Some("HI_RES");

    score_tidal_candidate(alb_title, perf_name, perf_name, &track.title, "", expected_artist, is_hires)
}

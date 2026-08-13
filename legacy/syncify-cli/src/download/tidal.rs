//! Tidal downloader - Master restoration with OAuth client credentials, proxy API cascades,
//! duration tolerance, studio origin candidate scoring, and stream resolution details.

use crate::download::http_client::{create_http_client, get_user_agent, TIDAL_LIMITER};
pub use crate::services::tidal::{
    artist_matches, clean_title, score_tidal_candidate, score_tidal_release, title_matches,
    StreamSourceType, TidalAuthResolution, TidalAuthStatus, TidalGuiCredentials, TidalTrack,
};

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityClass {
    Lossless,
    Lossy,
}

impl std::fmt::Display for QualityClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityClass::Lossless => write!(f, "Lossless"),
            QualityClass::Lossy => write!(f, "Lossy"),
        }
    }
}

/// Detailed stream resolution metrics for Tidal downloads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalStreamResolution {
    pub url: String,
    pub source: StreamSourceType,
    pub source_name: String,
    pub requested_quality: String,
    pub obtained_quality: String,
    pub quality_class_requested: QualityClass,
    pub quality_class_obtained: QualityClass,
    pub codec: String,
    pub container: String,
    pub extension: String,
    pub bit_depth: i32,
    pub sample_rate: f64,
    pub is_fallback: bool,
}

#[derive(Debug, Deserialize)]
struct TidalSearchResponse {
    tracks: Option<TidalTracksContainer>,
}

#[derive(Debug, Deserialize)]
struct TidalTracksContainer {
    items: Vec<TidalTrack>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct BTSManifest {
    #[serde(rename = "mimeType")]
    _mime_type: Option<String>,
    urls: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct DirectUrl {
    url: String,
}

pub struct TidalDownloader {
    client: Client,
    client_id: String,
    client_secret: String,
    user_token: RwLock<Option<String>>,
    cached_oauth_token: RwLock<Option<(String, Instant)>>,
}

impl TidalDownloader {
    pub fn new() -> Self {
        let client_id = BASE64
            .decode("NkJEU1JkcEs5aHFFQlRnVQ==")
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_default();

        let client_secret = BASE64
            .decode("eGV1UG1ZN25icFo5SUliTEFjUTkzc2hrYTFWTmhlVUFxTjZJY3N6alRHOD0=")
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .unwrap_or_default();

        Self {
            client: create_http_client(),
            client_id,
            client_secret,
            user_token: RwLock::new(None),
            cached_oauth_token: RwLock::new(None),
        }
    }

    pub fn with_user_token(self, token: Option<String>) -> Self {
        if let Some(tok) = token {
            let mut guard = self.user_token.write().unwrap();
            *guard = Some(tok);
        }
        self
    }

    pub fn get_proxy_apis() -> Vec<String> {
        let encoded_apis = [
            "dGlkYWwua2lub3BsdXMub25saW5l", // tidal.kinoplus.online
            "dGlkYWwtYXBpLmJpbmltdW0ub3Jn", // tidal-api.binimum.org
            "dHJpdG9uLnNxdWlkLnd0Zg==",     // triton.squid.wtf
            "dm9nZWwucXFkbC5zaXRl",         // vogel.qqdl.site
            "bWF1cy5xcWRsLnNpdGU=",         // maus.qqdl.site
            "aHVuZC5xcWRsLnNpdGU=",         // hund.qqdl.site
            "a2F0emUucXFkbC5zaXRl",         // katze.qqdl.site
            "d29sZi5xcWRsLnNpdGU=",         // wolf.qqdl.site
        ];

        encoded_apis
            .iter()
            .filter_map(|encoded| {
                BASE64.decode(encoded).ok().and_then(|bytes| {
                    String::from_utf8(bytes)
                        .ok()
                        .map(|s| format!("https://{}", s))
                })
            })
            .collect()
    }

    /// Resolve active GUI Tidal session from SQLite DB
    pub async fn resolve_gui_session(&self) -> (Option<String>, TidalAuthResolution) {
        if let Ok(db_path) = crate::crypto::resolve_syncify_db_path() {
            let conn_str = format!("sqlite:{}?mode=ro", db_path.to_string_lossy());
            if let Ok(pool) = sqlx::sqlite::SqlitePoolOptions::new().connect(&conn_str).await {
                return crate::services::tidal::resolve_gui_credentials_from_pool(&pool, &self.client).await;
            }
        }
        (None, TidalAuthResolution::RequiresAuth)
    }

    /// Check authentication status according to strict hierarchy:
    /// Explicit Override -> Active GUI Account in SQLite DB -> OAuth ClientCredentials -> RequiresAuth / SourceUnavailable
    pub async fn check_auth_status(&self, explicit_token: Option<&str>) -> TidalAuthStatus {
        if let Some(tok) = explicit_token {
            if !tok.trim().is_empty() {
                return TidalAuthStatus::UserToken(tok.to_string());
            }
        }

        {
            let guard = self.user_token.read().unwrap();
            if let Some(ref tok) = *guard {
                if !tok.trim().is_empty() {
                    return TidalAuthStatus::UserToken(tok.clone());
                }
            }
        }

        if let Ok(env_tok) = std::env::var("TIDAL_USER_TOKEN") {
            let clean = env_tok.trim().trim_matches('"').trim_matches('\'').to_string();
            if !clean.is_empty() {
                return TidalAuthStatus::UserToken(clean);
            }
        }

        match self.get_access_token().await {
            Ok(tok) => TidalAuthStatus::ClientCredentials(tok),
            Err(e) => {
                let err_msg = e.to_string();
                if err_msg.contains("401") || err_msg.contains("Unauthorized") {
                    TidalAuthStatus::RequiresAuth
                } else {
                    TidalAuthStatus::SourceUnavailable(err_msg)
                }
            }
        }
    }

    /// Get OAuth access token (cached with auto-refresh)
    pub async fn get_access_token(&self) -> Result<String> {
        // Check cache
        {
            let cache = self.cached_oauth_token.read().unwrap();
            if let Some((token, expires_at)) = cache.as_ref() {
                if expires_at.elapsed() < Duration::from_secs(55 * 60) {
                    return Ok(token.clone());
                }
            }
        }

        debug!("[Tidal] Requesting OAuth client_credentials token");

        let auth_url = BASE64
            .decode("aHR0cHM6Ly9hdXRoLnRpZGFsLmNvbS92MS9vYXV0aDIvdG9rZW4=")
            .ok()
            .and_then(|b| String::from_utf8(b).ok())
            .ok_or_else(|| anyhow!("Failed to decode auth URL"))?;

        let response = self
            .client
            .post(&auth_url)
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!(
                "client_id={}&grant_type=client_credentials",
                self.client_id
            ))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to get Tidal OAuth token: HTTP {}",
                response.status()
            ));
        }

        let token_resp: TokenResponse = response.json().await?;

        {
            let mut cache = self.cached_oauth_token.write().unwrap();
            *cache = Some((token_resp.access_token.clone(), Instant::now()));
        }

        Ok(token_resp.access_token)
    }

    /// Search for a track by ISRC with duration tolerance check
    pub async fn search_by_isrc(
        &self,
        isrc: &str,
        expected_duration_sec: i32,
    ) -> Result<TidalTrack> {
        TIDAL_LIMITER.wait("tidal").await;

        let token = match self.check_auth_status(None).await {
            TidalAuthStatus::UserToken(t) => t,
            TidalAuthStatus::ClientCredentials(t) => t,
            TidalAuthStatus::RequiresAuth => return Err(anyhow!("Tidal authentication required for search")),
            TidalAuthStatus::SourceUnavailable(msg) => return Err(anyhow!("Tidal API unavailable: {}", msg)),
            TidalAuthStatus::Failed(msg) => return Err(anyhow!("Tidal auth failed: {}", msg)),
        };

        let url = format!(
            "https://api.tidal.com/v1/search/tracks?query={}&limit=50&countryCode=US",
            urlencoding::encode(isrc)
        );

        debug!("[Tidal] Searching track by ISRC: {}", isrc);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", get_user_agent())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Tidal ISRC search failed: HTTP {}", response.status()));
        }

        let result: TidalSearchResponse = response.json().await?;
        let tracks = result
            .tracks
            .ok_or_else(|| anyhow!("No tracks section returned by Tidal search"))?;

        for track in &tracks.items {
            if track.isrc.as_deref() == Some(isrc) {
                if expected_duration_sec > 0 {
                    let duration_diff = (track.duration - expected_duration_sec).abs();
                    if duration_diff <= 10 {
                        info!("[Tidal] Found exact ISRC match '{}' (duration diff: {}s)", track.title, duration_diff);
                        return Ok(track.clone());
                    } else {
                        warn!(
                            "[Tidal] ISRC match '{}' found but duration mismatch (expected {}s, got {}s)",
                            track.title, expected_duration_sec, track.duration
                        );
                    }
                } else {
                    return Ok(track.clone());
                }
            }
        }

        Err(anyhow!("No exact ISRC match found on Tidal for: {}", isrc))
    }

    /// Search for a track by metadata (artist + title) with candidate scoring for smart studio origin
    pub async fn search_by_metadata(
        &self,
        track_name: &str,
        artist_name: &str,
        expected_duration_sec: i32,
    ) -> Result<TidalTrack> {
        self.search_by_metadata_with_studio_option(track_name, artist_name, expected_duration_sec, true).await
    }

    pub async fn search_by_metadata_with_studio_option(
        &self,
        track_name: &str,
        artist_name: &str,
        expected_duration_sec: i32,
        smart_studio_origin: bool,
    ) -> Result<TidalTrack> {
        let client_creds_token = self.get_access_token().await.ok();
        let user_tok = match self.check_auth_status(None).await {
            TidalAuthStatus::UserToken(t) => Some(t),
            _ => None,
        };

        let search_tokens = match (client_creds_token, user_tok) {
            (Some(cc), Some(ut)) => vec![cc, ut],
            (Some(cc), None) => vec![cc],
            (None, Some(ut)) => vec![ut],
            (None, None) => return Err(anyhow!("Tidal authentication required for search")),
        };

        let query = format!("{} {}", artist_name, track_name);
        let mut candidate_tracks: Vec<TidalTrack> = Vec::new();

        for token in &search_tokens {
            let official_urls = [
                format!("https://api.tidal.com/v1/search/tracks?query={}&limit=50&countryCode=US", urlencoding::encode(&query)),
                format!("https://api.tidal.com/v1/search?query={}&types=TRACKS&limit=50&countryCode=US", urlencoding::encode(&query)),
            ];

            for official_url in &official_urls {
                if let Ok(response) = self
                    .client
                    .get(official_url)
                    .timeout(Duration::from_secs(5))
                    .header("Authorization", format!("Bearer {}", token))
                    .header("User-Agent", get_user_agent())
                    .send()
                    .await
                {
                    if response.status().is_success() {
                        if let Ok(result) = response.json::<TidalSearchResponse>().await {
                            if let Some(tracks) = result.tracks {
                                if !tracks.items.is_empty() {
                                    candidate_tracks = tracks.items;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            if !candidate_tracks.is_empty() {
                break;
            }
        }

        // 2. If official search yielded no items, cascade through proxy search APIs with 2s timeout
        if candidate_tracks.is_empty() {
            let apis = Self::get_proxy_apis();
            for api in apis {
                let proxy_search_url = format!("{}/search?query={}&type=tracks", api, urlencoding::encode(&query));
                if let Ok(response) = self
                    .client
                    .get(&proxy_search_url)
                    .timeout(Duration::from_secs(2))
                    .header("User-Agent", get_user_agent())
                    .send()
                    .await
                {
                    if response.status().is_success() {
                        let text = response.text().await.unwrap_or_default();
                        if let Ok(result) = serde_json::from_str::<TidalSearchResponse>(&text) {
                            if let Some(tracks) = result.tracks {
                                if !tracks.items.is_empty() {
                                    candidate_tracks = tracks.items;
                                    break;
                                }
                            }
                        }
                        if let Ok(items) = serde_json::from_str::<Vec<TidalTrack>>(&text) {
                            if !items.is_empty() {
                                candidate_tracks = items;
                                break;
                            }
                        }
                    }
                }
            }
        }

        if candidate_tracks.is_empty() {
            return Err(anyhow!("No matching tracks found on Tidal for: {} - {}", artist_name, track_name));
        }

        let mut best_track: Option<TidalTrack> = None;
        let mut best_score: i32 = i32::MIN;

        for track in &candidate_tracks {
            if !title_matches(track_name, &track.title) {
                continue;
            }

            let track_artist = track.artist.as_ref().map(|a| a.name.as_str()).unwrap_or("");
            if !artist_matches(artist_name, track_artist) {
                continue;
            }

            if expected_duration_sec > 0 {
                let duration_diff = (track.duration - expected_duration_sec).abs();
                if duration_diff > 10 {
                    continue;
                }
            }

            if smart_studio_origin {
                let alb_title = track.album.as_ref().map(|a| a.title.as_str()).unwrap_or("");
                let is_hires = track.audio_quality.as_deref() == Some("HI_RES_LOSSLESS") || track.audio_quality.as_deref() == Some("HI_RES");
                let score = score_tidal_candidate(
                    alb_title, track_artist, track_artist, &track.title, "", artist_name, is_hires
                );
                if score > best_score {
                    best_score = score;
                    best_track = Some(track.clone());
                }
            } else {
                return Ok(track.clone());
            }
        }

        if let Some(t) = best_track {
            info!("[Tidal] Selected studio origin track: '{}' by '{}' (score: {})", t.title, artist_name, best_score);
            return Ok(t);
        }

        // If strict artist/title scoring filtered out candidates, return first candidate as fallback
        if let Some(first_track) = candidate_tracks.first() {
            info!("[Tidal] Selected top candidate track fallback: '{}'", first_track.title);
            return Ok(first_track.clone());
        }

        Err(anyhow!(
            "No matching track found on Tidal for: {} - {}",
            artist_name,
            track_name
        ))
    }
    pub async fn get_stream_resolution(
        &self,
        track_id: i64,
        quality_opt: Option<&str>,
        user_token_opt: Option<&str>,
        allow_lossy_fallback: bool,
    ) -> Result<TidalStreamResolution> {
        let requested_q = quality_opt.unwrap_or("24-192");
        let target_quality_param = match requested_q {
            "24-192" | "24-96" | "HI_RES_LOSSLESS" | "HI_RES" => "HI_RES_LOSSLESS",
            "16-44" | "LOSSLESS" => "LOSSLESS",
            "320" | "HIGH" | "LOSSY" => "HIGH",
            _ => "HI_RES_LOSSLESS",
        };

        let quality_class_requested = match requested_q {
            "320" | "HIGH" | "LOSSY" => QualityClass::Lossy,
            _ => QualityClass::Lossless,
        };

        // 1. Try Official Tidal API endpoints if user_token is present
        if let Some(user_tok) = user_token_opt {
            let official_endpoints = [
                format!("https://api.tidal.com/v1/tracks/{}/playbackinfopostpaywall?audioquality={}&playbackmode=STREAM&assetpresentation=FULL", track_id, target_quality_param),
                format!("https://api.tidal.com/v1/tracks/{}/streamUrl?soundQuality={}", track_id, target_quality_param),
                format!("https://api.tidal.com/v1/tracks/{}/url?soundQuality={}", track_id, target_quality_param),
            ];

            let mut official_error: Option<String> = None;

            for official_url in &official_endpoints {
                match self.client.get(official_url)
                    .header("Authorization", format!("Bearer {}", user_tok))
                    .header("X-Tidal-SessionId", user_tok)
                    .header("User-Agent", get_user_agent())
                    .send()
                    .await
                {
                    Ok(resp) => {
                        let status = resp.status();
                        let text = resp.text().await.unwrap_or_default();
                        info!("[Tidal] Official stream endpoint {} -> HTTP {} ({})", official_url, status, text.chars().take(150).collect::<String>());

                        if status.is_success() {
                            let mut resolved_url: Option<String> = None;
                            let mut detected_mime: Option<String> = None;
                            let mut detected_codecs: Option<String> = None;

                            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&text) {
                                if let Some(u) = json_val["url"].as_str() {
                                    resolved_url = Some(u.to_string());
                                } else if let Some(arr) = json_val["urls"].as_array() {
                                    if let Some(u) = arr.first().and_then(|v| v.as_str()) {
                                        resolved_url = Some(u.to_string());
                                    }
                                } else if let Some(b64_manifest) = json_val["manifest"].as_str() {
                                    if let Ok(decoded_bytes) = BASE64.decode(b64_manifest) {
                                        if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                                             info!("[Tidal] Decoded manifest JSON/string: {}", decoded_str);
                                             if let Ok(m_json) = serde_json::from_str::<serde_json::Value>(&decoded_str) {
                                                 if let Some(m) = m_json["mimeType"].as_str() {
                                                     detected_mime = Some(m.to_lowercase());
                                                 }
                                                 if let Some(c) = m_json["codecs"].as_str() {
                                                     detected_codecs = Some(c.to_lowercase());
                                                 }
                                                 if let Some(u) = m_json["urls"].as_array().and_then(|a| a.first()).and_then(|v| v.as_str()) {
                                                     resolved_url = Some(u.to_string());
                                                 }
                                             }

                                              if resolved_url.is_none() {
                                                  // Check if DASH XML MPD manifest
                                                  if decoded_str.contains("<MPD") || decoded_str.contains("<?xml") {
                                                      if decoded_str.contains("codecs=\"flac\"") || decoded_str.contains("codecs=\"fLaC\"") || decoded_str.contains("FLAC") {
                                                          detected_codecs = Some("flac".to_string());
                                                          detected_mime = Some("audio/flac".to_string());
                                                      }

                                                      let mut init_url_opt: Option<&str> = None;
                                                      let mut media_tmpl_opt: Option<&str> = None;
                                                      let mut total_segs: u32 = 0;

                                                      if let Some(init_idx) = decoded_str.find("initialization=\"http") {
                                                          let start = init_idx + "initialization=\"".len();
                                                          if let Some(end) = decoded_str[start..].find('"') {
                                                              init_url_opt = Some(&decoded_str[start..start + end]);
                                                          }
                                                      }

                                                      if let Some(media_idx) = decoded_str.find("media=\"http") {
                                                          let start = media_idx + "media=\"".len();
                                                          if let Some(end) = decoded_str[start..].find('"') {
                                                              media_tmpl_opt = Some(&decoded_str[start..start + end]);
                                                          }
                                                      }

                                                      let mut pos = 0;
                                                      while let Some(s_idx) = decoded_str[pos..].find("<S ") {
                                                          let abs_s = pos + s_idx;
                                                          if let Some(close_idx) = decoded_str[abs_s..].find('>') {
                                                              let tag_str = &decoded_str[abs_s..abs_s + close_idx];
                                                              let repeat_count = if let Some(r_idx) = tag_str.find("r=\"") {
                                                                  let r_start = r_idx + "r=\"".len();
                                                                  tag_str[r_start..].split('"').next().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0)
                                                              } else {
                                                                  0
                                                              };
                                                              total_segs += repeat_count + 1;
                                                              pos = abs_s + close_idx + 1;
                                                          } else {
                                                              break;
                                                          }
                                                      }

                                                      if let (Some(init_u), Some(media_u)) = (init_url_opt, media_tmpl_opt) {
                                                          if total_segs == 0 { total_segs = 1; }
                                                          resolved_url = Some(format!("DASH_MANIFEST|{}|{}|{}", init_u, media_u, total_segs));
                                                      } else if let Some(init_u) = init_url_opt {
                                                          resolved_url = Some(init_u.to_string());
                                                      }
                                                  }
                                              }

                                             if resolved_url.is_none() {
                                                 // Extract http/https link from decoded manifest text
                                                 for line in decoded_str.lines() {
                                                     let tr = line.trim();
                                                     if tr.starts_with("http://") || tr.starts_with("https://") {
                                                         resolved_url = Some(tr.to_string());
                                                         break;
                                                     }
                                                 }
                                             }
                                        }
                                    }
                                }
                            }

                            if let Some(stream_url) = resolved_url {
                                 let mime_str = detected_mime.as_deref().unwrap_or("");
                                 let codec_str = detected_codecs.as_deref().unwrap_or("");

                                 let is_flac = codec_str == "flac" || mime_str == "audio/flac";
                                 let is_mp4_aac = !is_flac && (mime_str == "audio/mp4" || codec_str.starts_with("mp4a") || stream_url.contains(".m4a"));
                                 let is_mp3 = !is_flac && (mime_str == "audio/mpeg" || codec_str == "mp3" || stream_url.contains(".mp3") || (target_quality_param == "HIGH" && !is_flac));

                                 let final_codec = if is_flac {
                                     "FLAC".to_string()
                                 } else if is_mp4_aac {
                                     "AAC".to_string()
                                 } else if is_mp3 {
                                     "MP3".to_string()
                                 } else {
                                     "FLAC".to_string()
                                 };

                                let quality_class_obtained = if final_codec == "FLAC" {
                                    QualityClass::Lossless
                                } else {
                                    QualityClass::Lossy
                                };

                                if quality_class_requested == QualityClass::Lossless && quality_class_obtained == QualityClass::Lossy && !allow_lossy_fallback {
                                    return Err(anyhow!("Quality rejection: requested_lossless_but_received_{}", final_codec.to_lowercase()));
                                }

                                let container = if final_codec == "AAC" {
                                    "M4A".to_string()
                                } else if final_codec == "MP3" {
                                    "MP3".to_string()
                                } else {
                                    "FLAC".to_string()
                                };

                                let extension = if final_codec == "AAC" {
                                    "m4a".to_string()
                                } else if final_codec == "MP3" {
                                    "mp3".to_string()
                                } else {
                                    "flac".to_string()
                                };

                                let obtained_q = if quality_class_obtained == QualityClass::Lossy { "320" } else if target_quality_param == "HI_RES_LOSSLESS" { "24-192" } else { "16-44" };
                                let is_fallback = obtained_q != requested_q;

                                info!("[Tidal] Stream URL resolved successfully via Official Tidal API endpoint (Codec: {}, Container: {})", final_codec, container);

                                return Ok(TidalStreamResolution {
                                    url: stream_url,
                                    source: StreamSourceType::TidalOfficial,
                                    source_name: "Tidal Official API".to_string(),
                                    requested_quality: requested_q.to_string(),
                                    obtained_quality: obtained_q.to_string(),
                                    quality_class_requested,
                                    quality_class_obtained,
                                    codec: final_codec,
                                    container,
                                    extension,
                                    bit_depth: if quality_class_obtained == QualityClass::Lossy { 16 } else if target_quality_param == "HI_RES_LOSSLESS" { 24 } else { 16 },
                                    sample_rate: if quality_class_obtained == QualityClass::Lossy { 44100.0 } else if target_quality_param == "HI_RES_LOSSLESS" { 96000.0 } else { 44100.0 },
                                    is_fallback,
                                });
                            }
                        } else {
                            if text.contains("11002") || text.contains("Token has invalid payload") {
                                official_error = Some("Official playback API returned HTTP 401 subStatus 11002: Token has invalid payload / Client ID audio stream incompatible".to_string());
                            } else {
                                official_error = Some(format!("Official playback API returned HTTP {}: {}", status, text));
                            }
                        }
                    }
                    Err(e) => {
                        debug!("[Tidal] Official endpoint {} error: {}", official_url, e);
                    }
                }
            }

            // Do NOT use third-party proxies when official user token authentication fails!
            if let Some(err_msg) = official_error {
                return Err(anyhow!("{}", err_msg));
            }
        }

        // 2. Cascade through Proxy APIs (WITHOUT sending user tokens to third parties!)
        let apis = Self::get_proxy_apis();
        if apis.is_empty() {
            return Err(anyhow!("No Tidal proxy APIs available in cascade list"));
        }

        debug!("[Tidal] Resolving stream URL via proxy cascade for track_id {} (requested: {})", track_id, requested_q);

        for api in &apis {
            let domain = api.replace("https://", "");
            let url = format!("{}/track/{}?quality={}", api, track_id, target_quality_param);

            let result = self
                .client
                .get(&url)
                .timeout(Duration::from_secs(2))
                .header("User-Agent", get_user_agent())
                .send()
                .await;

            match result {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        debug!("[Tidal] Proxy API {} returned HTTP status {}", api, status);
                        continue;
                    }

                    let text = resp.text().await.unwrap_or_default();
                    let trimmed = text.trim();

                    if trimmed.is_empty()
                        || trimmed.starts_with("<!DOCTYPE")
                        || trimmed.starts_with("<html")
                        || trimmed.contains("\"status\":4")
                        || trimmed.contains("\"status\":5")
                        || trimmed.contains("\"userMessage\"")
                    {
                        debug!("[Tidal] Proxy API {} returned invalid/error response body", api);
                        continue;
                    }

                    let mut resolved_url: Option<String> = None;
                    if let Ok(manifest) = serde_json::from_str::<BTSManifest>(trimmed) {
                        if !manifest.urls.is_empty() {
                            resolved_url = Some(manifest.urls[0].clone());
                        }
                    }

                    if resolved_url.is_none() {
                        if let Ok(direct) = serde_json::from_str::<DirectUrl>(trimmed) {
                            if !direct.url.trim().is_empty() {
                                resolved_url = Some(direct.url.trim().to_string());
                            }
                        }
                    }

                    if resolved_url.is_none() {
                        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                            resolved_url = Some(trimmed.to_string());
                        }
                    }

                    if let Some(stream_url) = resolved_url {
                        let is_mp3 = target_quality_param == "HIGH" || stream_url.contains(".mp3");
                        let is_m4a = stream_url.contains(".m4a") || stream_url.contains(".mp4");
                        let final_codec = if is_mp3 {
                            "MP3".to_string()
                        } else if is_m4a {
                            "AAC".to_string()
                        } else {
                            "FLAC".to_string()
                        };

                        let quality_class_obtained = if final_codec == "FLAC" {
                            QualityClass::Lossless
                        } else {
                            QualityClass::Lossy
                        };

                        if quality_class_requested == QualityClass::Lossless && quality_class_obtained == QualityClass::Lossy && !allow_lossy_fallback {
                            return Err(anyhow!("Quality rejection: requested_lossless_but_received_{}", final_codec.to_lowercase()));
                        }

                        let container = if final_codec == "AAC" {
                            "M4A".to_string()
                        } else if final_codec == "MP3" {
                            "MP3".to_string()
                        } else {
                            "FLAC".to_string()
                        };

                        let extension = if final_codec == "AAC" {
                            "m4a".to_string()
                        } else if final_codec == "MP3" {
                            "mp3".to_string()
                        } else {
                            "flac".to_string()
                        };

                        let obtained_q = if quality_class_obtained == QualityClass::Lossy { "320" } else if target_quality_param == "HI_RES_LOSSLESS" { "24-192" } else { "16-44" };
                        let is_fallback = obtained_q != requested_q;

                        info!("[Tidal] Stream URL resolved via TidalProxy ({})", domain);

                        return Ok(TidalStreamResolution {
                            url: stream_url,
                            source: StreamSourceType::TidalProxy(domain.clone()),
                            source_name: format!("Tidal Proxy ({})", domain),
                            requested_quality: requested_q.to_string(),
                            obtained_quality: obtained_q.to_string(),
                            quality_class_requested,
                            quality_class_obtained,
                            codec: final_codec,
                            container,
                            extension,
                            bit_depth: if quality_class_obtained == QualityClass::Lossy { 16 } else if target_quality_param == "HI_RES_LOSSLESS" { 24 } else { 16 },
                            sample_rate: if quality_class_obtained == QualityClass::Lossy { 44100.0 } else if target_quality_param == "HI_RES_LOSSLESS" { 96000.0 } else { 44100.0 },
                            is_fallback,
                        });
                    }
                }
                Err(e) => {
                    debug!("[Tidal] Connection error to proxy API {}: {}", api, e);
                }
            }
        }

        Err(anyhow!("Failed to obtain stream URL for Tidal track ID {} from official & proxy APIs", track_id))
    }

    /// Download stream audio payload to disk with strict chunk & format header validation
    pub async fn download_audio_payload(
        &self,
        stream_url: &str,
        output_path: &Path,
    ) -> Result<u64> {
        let temp_file_path = output_path.with_extension("tmp");
        if let Some(parent) = temp_file_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let mut downloaded: u64 = 0;

        if stream_url.starts_with("DASH_MANIFEST|") {
            let parts: Vec<&str> = stream_url.split('|').collect();
            if parts.len() < 4 {
                return Err(anyhow!("Invalid DASH manifest stream URL spec"));
            }
            let init_url = parts[1];
            let media_template = parts[2];
            let total_segments: u32 = parts[3].parse().unwrap_or(1);

            info!("[Tidal] Downloading DASH MPD stream: {} media segments", total_segments);
            let mut file = File::create(&temp_file_path).await?;

            // 1. Download init segment 0
            let mut resp = self.client.get(init_url).send().await?;
            if !resp.status().is_success() {
                let _ = tokio::fs::remove_file(&temp_file_path).await;
                return Err(anyhow!("DASH init segment download failed: HTTP {}", resp.status()));
            }
            while let Some(chunk) = resp.chunk().await? {
                file.write_all(&chunk).await?;
                downloaded += chunk.len() as u64;
            }

            // 2. Download media segments 1..total_segments
            for seg_num in 1..=total_segments {
                let seg_url = media_template.replace("$Number$", &seg_num.to_string());
                let mut seg_resp = self.client.get(&seg_url).send().await?;
                if seg_resp.status().is_success() {
                    while let Some(chunk) = seg_resp.chunk().await? {
                        file.write_all(&chunk).await?;
                        downloaded += chunk.len() as u64;
                    }
                }
            }

            file.flush().await?;
            drop(file);
        } else {
            let mut resp = self.client.get(stream_url).send().await?;
            if !resp.status().is_success() {
                return Err(anyhow!("Tidal stream download failed: HTTP {}", resp.status()));
            }

            let mut file = File::create(&temp_file_path).await?;

            while let Some(chunk) = resp.chunk().await? {
                file.write_all(&chunk).await?;
                downloaded += chunk.len() as u64;
            }

            file.flush().await?;
            drop(file);
        }

        if downloaded == 0 {
            let _ = tokio::fs::remove_file(&temp_file_path).await;
            return Err(anyhow!("Tidal downloaded file payload is zero bytes"));
        }

        // Validate audio file magic header before declaring payload valid
        let header_bytes = tokio::fs::read(&temp_file_path).await.unwrap_or_default();
        if header_bytes.len() < 4 {
            let _ = tokio::fs::remove_file(&temp_file_path).await;
            return Err(anyhow!("Downloaded file is too small to contain valid audio headers"));
        }

        let ext_str = output_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let is_flac_path = ext_str == "flac";
        let is_mp3_path = ext_str == "mp3";
        let is_m4a_path = ext_str == "m4a" || ext_str == "mp4";

        if is_flac_path && !header_bytes.starts_with(b"fLaC") && !header_bytes.starts_with(b"\x00\x00\x00") && &header_bytes[4..8] != b"ftyp" {
            let _ = tokio::fs::remove_file(&temp_file_path).await;
            return Err(anyhow!("Downloaded file fails FLAC magic header verification ('fLaC' or ISOBMFF expected)"));
        }

        if is_mp3_path && !header_bytes.starts_with(b"ID3") && !(header_bytes[0] == 0xFF && (header_bytes[1] & 0xE0) == 0xE0) {
            let _ = tokio::fs::remove_file(&temp_file_path).await;
            return Err(anyhow!("Downloaded file fails MP3 frame header verification"));
        }

        if is_m4a_path && header_bytes.len() >= 8 && &header_bytes[4..8] != b"ftyp" && !header_bytes.starts_with(b"\x00\x00\x00") {
            let _ = tokio::fs::remove_file(&temp_file_path).await;
            return Err(anyhow!("Downloaded file fails MP4/AAC magic header verification ('ftyp' expected)"));
        }

        tokio::fs::rename(&temp_file_path, output_path).await?;
        info!("[Tidal] Verified & saved audio payload: {} bytes -> {}", downloaded, output_path.display());

        Ok(downloaded)
    }

    pub async fn get_download_url(&self, track_id: i64) -> Result<String> {
        let res = self.get_stream_resolution(track_id, None, None, true).await?;
        Ok(res.url)
    }
}

impl Default for TidalDownloader {
    fn default() -> Self {
        Self::new()
    }
}


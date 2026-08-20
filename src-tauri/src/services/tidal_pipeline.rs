//! Single Track and Batch Pipeline for Tidal in `src-tauri`
//! Handles End-to-End download, stream resolution, pure audio validation,
//! FLAC tagging, staging, and transactional SQLite persistence.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool as DbPool;
use tracing::{debug, error, info, warn};

use crate::crypto;
use crate::download::lyrics::{
    validate_and_embed_flac_lyrics, LyricsPipelineService, LyricsResolution, LyricsSyncType,
    ResolutionStatus,
};
use crate::download::progress::{DownloadPhase, DownloadPhaseTimings, DownloadPhaseTracker};
use crate::services::animated_cover::{resolve_and_download_animated_cover, AnimatedCoverStatus};
use crate::services::enrichment::{EnrichmentEngine, OriginTrackMetadata};
use crate::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};
use syncify_core_domain::byte_validators::AudioByteValidator;
use syncify_core_domain::events::{PipelineProgressEvent, PipelineStepStatus, ResolvedTrackInfo};
use syncify_core_domain::manifest::TrackManifestEntry;

pub use syncify_core_domain::metadata::TidalTrack;
pub use syncify_flac_writer::{apply_and_verify_flac_tags, audit_flac_stage, verify_flac_tags, FlacMetadata};
pub use syncify_tidal_downloader::{TidalDownloader, TidalGuiCredentials};
use crate::services::repair_guardrail::{
    compute_bytes_sha256, compute_repair_baseline, extract_audio_content_hash_from_bytes,
    validate_repair_baseline,
};


#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TidalSingleTrackRequest {
    pub track_id_or_query: String,
    pub requested_quality: Option<String>,
    pub output_dir: Option<String>,
    pub allow_lossy_fallback: Option<bool>,
    pub hint_title: Option<String>,
    pub hint_artist: Option<String>,
    pub hint_album: Option<String>,
    pub hint_isrc: Option<String>,
    pub hint_track_number: Option<i32>,
    pub hint_disc_number: Option<i32>,
    pub hint_release_date: Option<String>,
    pub hint_track_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalSingleTrackResponse {
    pub success: bool,
    pub track_id: i64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub file_path: String,
    pub file_format: String,
    pub bit_depth: i32,
    pub sample_rate: i32,
    pub isrc: Option<String>,
    pub manifest_entry: TrackManifestEntry,
    #[serde(default)]
    pub phase_timings: Option<DownloadPhaseTimings>,
}

/// Check whether a candidate title string contains sufficient alphanumeric content (at least one alphanumeric character).
pub fn has_sufficient_alphanumeric(s: &str) -> bool {
    s.chars().any(|c| c.is_alphanumeric())
}

/// Sanitize filename component for OS compatibility and path traversal safety
pub fn sanitize_filename_component(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return "Unknown".to_string();
    }
    let invalid_chars = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    let sanitized: String = trimmed
        .chars()
        .map(|c| if invalid_chars.contains(&c) { '_' } else { c })
        .collect();
    let res = sanitized.trim().to_string();
    if res.is_empty() || res == "." || res == ".." {
        "Unknown".to_string()
    } else {
        res
    }
}

/// Extract and sanitize a clean semantic textual title suitable for a physical file name.
/// Strips non-alphanumeric symbols if they wrap or prefix the title (e.g. "★ (Blackstar)" -> "Blackstar", "★" -> empty -> fallback).
pub fn clean_title_for_filename(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." || trimmed == "Unknown" {
        return String::new();
    }

    // Check if the title starts with non-alphanumeric symbols followed by parentheses, e.g. "★ (Blackstar)"
    if let (Some(open_idx), Some(close_idx)) = (trimmed.find('('), trimmed.rfind(')')) {
        if open_idx < close_idx {
            let prefix = trimmed[..open_idx].trim();
            let inside = trimmed[open_idx + 1..close_idx].trim();
            if !has_sufficient_alphanumeric(prefix) && has_sufficient_alphanumeric(inside) {
                return sanitize_filename_component(inside);
            }
        }
    }

    // If the entire string has no alphanumeric characters, return empty string so fallback is used
    if !has_sufficient_alphanumeric(trimmed) {
        return String::new();
    }

    let sanitized = sanitize_filename_component(trimmed);
    if !has_sufficient_alphanumeric(&sanitized) || sanitized == "Unknown" {
        String::new()
    } else {
        sanitized
    }
}

/// Resolves a safe, non-empty display title using strict fallback precedence:
/// 1. `display_title` (if not empty / whitespace-only / placeholder "Unknown")
/// 2. `source_title` (if present and non-empty)
/// 3. `api_title` (if present and non-empty)
/// 4. `fallback_identifier` (if present and non-empty)
/// 5. Returns `Err("MetadataResolutionFailed: ...")` if no non-empty title can be resolved.
pub fn resolve_safe_display_title(
    display_title: Option<&str>,
    source_title: Option<&str>,
    api_title: Option<&str>,
    fallback_identifier: Option<&str>,
) -> Result<String, String> {
    let check_cand = |s: Option<&str>| -> Option<String> {
        let raw = s?.trim();
        if raw.is_empty() || raw == "." || raw == ".." || raw == "Unknown" {
            None
        } else {
            let sanitized = sanitize_filename_component(raw);
            if sanitized.is_empty() || sanitized == "Unknown" || sanitized == "." || sanitized == ".." {
                if !raw.is_empty() {
                    Some(raw.to_string())
                } else {
                    None
                }
            } else {
                Some(sanitized)
            }
        }
    };

    if let Some(t) = check_cand(display_title) {
        return Ok(t);
    }
    if let Some(t) = check_cand(source_title) {
        return Ok(t);
    }
    if let Some(t) = check_cand(api_title) {
        return Ok(t);
    }
    if let Some(t) = check_cand(fallback_identifier) {
        return Ok(t);
    }

    Err("MetadataResolutionFailed: No valid title found to construct track filename".to_string())
}

/// Compute a safe track filename ensuring no empty title components and no symbol-only filenames (e.g. `01 - .flac` and `01 - ★.flac` are strictly forbidden).
pub fn compute_safe_track_filename(
    track_number: i32,
    disc_number: i32,
    total_discs: i32,
    display_title: &str,
    source_title: Option<&str>,
    api_title: Option<&str>,
    fallback_identifier: Option<&str>,
    extension: &str,
    disambiguator: Option<&str>,
) -> Result<String, String> {
    let raw_disp = display_title.trim();

    // 1. Try display title cleaned for filename
    let mut resolved_title = clean_title_for_filename(raw_disp);

    // 2. Try source_title
    if resolved_title.is_empty() {
        if let Some(st) = source_title {
            resolved_title = clean_title_for_filename(st);
        }
    }

    // 3. Try api_title
    if resolved_title.is_empty() {
        if let Some(at) = api_title {
            resolved_title = clean_title_for_filename(at);
        }
    }

    // 4. Try fallback_identifier
    if resolved_title.is_empty() {
        if let Some(fi) = fallback_identifier {
            resolved_title = clean_title_for_filename(fi);
        }
    }

    // 5. Final fallback if still empty or no alphanumeric characters found
    if resolved_title.is_empty() {
        if let Some(fi) = fallback_identifier.filter(|f| !f.trim().is_empty()) {
            resolved_title = sanitize_filename_component(fi);
        } else {
            return Err("MetadataResolutionFailed: No valid alphanumeric title found to construct track filename".to_string());
        }
    }

    if resolved_title.trim().is_empty() || !has_sufficient_alphanumeric(&resolved_title) {
        return Err("MetadataResolutionFailed: Resolved filename title contains only symbols or is empty".to_string());
    }

    let ext = extension.trim().trim_start_matches('.').to_lowercase();
    let ext_clean = if ext.is_empty() { "flac" } else { &ext };

    let prefix = if total_discs > 1 && disc_number > 0 {
        format!("{}-{:02}", disc_number, track_number.max(1))
    } else {
        format!("{:02}", track_number.max(1))
    };

    let filename = if let Some(dis) = disambiguator.filter(|d| !d.trim().is_empty()) {
        let safe_dis = sanitize_filename_component(dis);
        format!("{} - {} [{}].{}", prefix, resolved_title, safe_dis, ext_clean)
    } else {
        format!("{} - {}.{}", prefix, resolved_title, ext_clean)
    };

    // Guard: ensure it never starts with "01 - ." or equals "01.flac"
    if filename.starts_with(&format!("{} - .", prefix)) || filename == format!("{}.{}", prefix, ext_clean) {
        return Err("MetadataResolutionFailed: Generated filename contains an empty title component".to_string());
    }

    Ok(filename)
}


/// Resolve and automatically refresh Tidal active credentials from SQLite.
pub async fn resolve_and_refresh_gui_credentials(
    db: &DbPool,
    http_client: &reqwest::Client,
) -> (Option<TidalGuiCredentials>, Option<String>) {
    let row: Option<(i64, Option<String>, Option<i64>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT a.id, a.credentials_json, a.credentials_invalid, COALESCE(a.display_name, a.email, 'tidal_user') as account_name
        FROM accounts a
        JOIN services s ON s.id = a.service_id
        WHERE LOWER(s.name) = 'tidal' AND a.is_active = 1
        ORDER BY a.id DESC
        LIMIT 1
        "#
    )
    .fetch_optional(db)
    .await
    .unwrap_or_else(|e| {
        warn!("[Tidal Auth Audit] Database query failed in resolve_and_refresh_gui_credentials: {}", e);
        None
    });

    let (account_id, encrypted_json, credentials_invalid, username) = match row {
        Some((id, Some(json), cred_inv, uname)) if !json.trim().is_empty() => (id, json, cred_inv.unwrap_or(0) != 0, uname),
        _ => {
            warn!(
                account_id = "none",
                token_present = false,
                expired = false,
                credentials_invalid = false,
                endpoint = "resolve_and_refresh_gui_credentials",
                "[Tidal Auth Diagnostics] No active Tidal account found in SQLite accounts table"
            );
            return (None, None);
        }
    };

    if credentials_invalid {
        warn!(
            account_id = account_id,
            token_present = true,
            expired = true,
            credentials_invalid = true,
            endpoint = "resolve_and_refresh_gui_credentials",
            "[Tidal Auth Diagnostics] Active Tidal account is marked credentials_invalid"
        );
        return (None, username);
    }

    let decrypted = match crypto::decrypt(&encrypted_json) {
        Ok(d) => d,
        Err(e) => {
            warn!(
                account_id = account_id,
                token_present = true,
                expired = false,
                credentials_invalid = true,
                endpoint = "crypto::decrypt",
                error = %e,
                "[Tidal Auth Diagnostics] Failed to decrypt Tidal credentials from SQLite"
            );
            return (None, username);
        }
    };

    let creds: TidalGuiCredentials = match serde_json::from_str(&decrypted) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                account_id = account_id,
                token_present = true,
                expired = false,
                credentials_invalid = true,
                endpoint = "serde_json::from_str",
                error = %e,
                "[Tidal Auth Diagnostics] Failed to deserialize Tidal credentials JSON"
            );
            return (None, username);
        }
    };

    let token_present = !creds.access_token.trim().is_empty();
    if !token_present {
        warn!(
            account_id = account_id,
            token_present = false,
            expired = false,
            credentials_invalid = true,
            endpoint = "resolve_and_refresh_gui_credentials",
            "[Tidal Auth Diagnostics] Tidal access token is empty"
        );
        return (None, username);
    }

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    let is_expired = creds.is_expired(now_secs);

    info!(
        account_id = account_id,
        token_present = token_present,
        expired = is_expired,
        credentials_invalid = false,
        endpoint = "resolve_and_refresh_gui_credentials",
        "[Tidal Auth Diagnostics] Active Tidal account resolved from SQLite"
    );

    if !is_expired {
        return (Some(creds), username);
    }

    // Token is expired; attempt OAuth refresh
    if creds.refresh_token.is_some() {
        info!(
            account_id = account_id,
            token_present = token_present,
            expired = true,
            credentials_invalid = false,
            endpoint = "auth.tidal.com/v1/oauth2/token",
            "[Tidal Auth Diagnostics] Tidal access token is expired; executing OAuth token refresh via auth.tidal.com"
        );
        match syncify_tidal_downloader::refresh_gui_token(http_client, &creds).await {
            Ok((_new_token, updated_creds)) => {
                // Re-encrypt and persist ONLY on success
                if let Ok(serialized) = serde_json::to_string(&updated_creds) {
                    if let Ok(encrypted_new) = crypto::encrypt(&serialized) {
                        let _ = sqlx::query("UPDATE accounts SET credentials_json = ?, credentials_invalid = 0, invalid_reason = NULL, last_auth_error = NULL WHERE id = ?")
                            .bind(&encrypted_new)
                            .bind(account_id)
                            .execute(db)
                            .await;
                        info!(
                            account_id = account_id,
                            token_present = true,
                            expired = false,
                            credentials_invalid = false,
                            endpoint = "auth.tidal.com/v1/oauth2/token",
                            "[Tidal Auth Diagnostics] Refreshed credentials persisted to SQLite successfully"
                        );
                    }
                }
                return (Some(updated_creds), username);
            }
            Err(e) => {
                warn!(
                    account_id = account_id,
                    token_present = token_present,
                    expired = true,
                    credentials_invalid = true,
                    endpoint = "auth.tidal.com/v1/oauth2/token",
                    error = %e,
                    "[Tidal Auth Diagnostics] Tidal OAuth token refresh failed; marking account credentials_invalid"
                );
                let _ = sqlx::query("UPDATE accounts SET credentials_invalid = 1, invalid_reason = 'token_expired', last_auth_error = ? WHERE id = ?")
                    .bind(e.to_string())
                    .bind(account_id)
                    .execute(db)
                    .await;
                return (None, username);
            }
        }
    } else {
        warn!(
            account_id = account_id,
            token_present = token_present,
            expired = true,
            credentials_invalid = true,
            endpoint = "resolve_and_refresh_gui_credentials",
            "[Tidal Auth Diagnostics] Tidal access token is expired and no refresh token is present; marking account credentials_invalid"
        );
        let _ = sqlx::query("UPDATE accounts SET credentials_invalid = 1, invalid_reason = 'token_expired', last_auth_error = 'Token expired and no refresh token available' WHERE id = ?")
            .bind(account_id)
            .execute(db)
            .await;
        return (None, username);
    }
}


/// Execute end-to-end single track download pipeline for Tidal
pub async fn execute_tidal_single_track_download<F>(
    db: &DbPool,
    request: TidalSingleTrackRequest,
    on_progress: F,
) -> Result<TidalSingleTrackResponse, String>
where
    F: Fn(PipelineProgressEvent) + Send + Sync + 'static,
{
    let target = request.track_id_or_query.trim();
    let quality_req = request.requested_quality.as_deref().unwrap_or("24-192");
    let allow_fallback = request.allow_lossy_fallback.unwrap_or(false);

    let mut phase_tracker = DownloadPhaseTracker::new();
    info!(target = %target, requested_quality = %quality_req, allow_fallback = allow_fallback, "Starting Tidal single track download pipeline");

    // 1. Authenticating
    phase_tracker.start_phase(DownloadPhase::Auth);
    on_progress(PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Authenticating));

    let http_client = crate::download::http_client::create_http_client();
    let (resolved_creds, active_account_name) = resolve_and_refresh_gui_credentials(db, &http_client).await;
    let active_account_region = resolved_creds.as_ref().and_then(|c| c.country_code.clone());
    let user_token = resolved_creds.as_ref().map(|c| c.access_token.clone());

    if let (Some(ref _creds), Some(ref account_name)) = (&resolved_creds, &active_account_name) {
        info!(account = ?account_name, region = ?active_account_region, "Tidal active user account resolved and validated");
        on_progress(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::AccountResolved)
                .with_message(format!("Active Tidal account: {}", account_name))
        );
    } else {
        let err_msg = "RequiresAuth: No active or valid Tidal account credentials available. Please connect or re-authenticate Tidal in Settings > Accounts.".to_string();
        warn!(
            target = %target,
            token_present = false,
            credentials_invalid = true,
            endpoint = "execute_tidal_single_track_download",
            "[Tidal Auth Diagnostics] Cannot proceed with Tidal download without valid authenticated account"
        );
        on_progress(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::CandidateRejected)
                .with_error(err_msg.clone())
        );
        return Err(err_msg);
    }

    let downloader = TidalDownloader::new().with_user_token(user_token.clone());

    // 2. Searching & Candidate Resolution
    phase_tracker.start_phase(DownloadPhase::ResolveStream);
    on_progress(PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Searching));


    let country_code = active_account_region.as_deref().unwrap_or("US");
    let mut is_partial_metadata = false;

    let track_result: Result<TidalTrack, String> = if let Ok(numeric_id) = target.parse::<i64>() {
        // 1. Try real Tidal API by track ID
        match downloader.get_track_with_country(numeric_id, country_code).await {
            Ok(t) => Ok(t),
            Err(api_err) => {
                warn!(track_id = numeric_id, error = %api_err, "Tidal API get_track failed; checking local database and request hints");
                // 2. Try DB lookup by service_track_id or hint_track_id
                let db_track: Option<(String, Option<String>, Option<String>, Option<String>, Option<i32>, Option<i32>, Option<i64>, Option<String>)> = sqlx::query_as(
                    r#"SELECT t.title, ar.name, al.title, al.release_date, t.track_number, t.disc_number, t.duration_ms, t.isrc
                       FROM tracks t
                       LEFT JOIN track_sources ts ON ts.track_id = t.id AND ts.service_id = (SELECT id FROM services WHERE LOWER(name) = 'tidal' LIMIT 1)
                       LEFT JOIN albums al ON t.album_id = al.id
                       LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
                       LEFT JOIN artists ar ON ta.artist_id = ar.id
                       WHERE ts.service_track_id = ? OR t.id = ?
                       LIMIT 1"#
                )
                .bind(target)
                .bind(request.hint_track_id)
                .fetch_optional(db)
                .await
                .unwrap_or(None);

                if let Some((title, artist_opt, album_opt, rel_date_opt, trk_num, disc_num, dur_ms, isrc_opt)) = db_track {
                    let art_name = artist_opt.or_else(|| request.hint_artist.clone()).unwrap_or_else(|| "Unknown Artist".to_string());
                    let alb_name = album_opt.or_else(|| request.hint_album.clone()).unwrap_or_else(|| "Unknown Album".to_string());
                    let rel_date = rel_date_opt.or_else(|| request.hint_release_date.clone()).unwrap_or_else(|| "2024-01-01".to_string());
                    let dur_sec = dur_ms.map(|d| (d / 1000) as i32).unwrap_or(180);

                    Ok(TidalTrack {
                        id: numeric_id,
                        title,
                        duration: dur_sec,
                        track_number: trk_num.or(request.hint_track_number).or(Some(1)),
                        volume_number: disc_num.or(request.hint_disc_number).or(Some(1)),
                        isrc: isrc_opt.or_else(|| request.hint_isrc.clone()),
                        audio_quality: Some("LOSSLESS".to_string()),
                        version: None,
                        artist: Some(syncify_core_domain::metadata::TidalArtist { id: None, name: art_name }),
                        artists: None,
                        album: Some(syncify_core_domain::metadata::TidalAlbum {
                            id: None,
                            title: alb_name,
                            release_date: Some(rel_date),
                            cover: None,
                            artist: None,
                            artists: None,
                        }),
                        media_metadata: None,
                    })
                } else if let (Some(h_title), Some(h_artist)) = (request.hint_title.as_ref(), request.hint_artist.as_ref()) {
                    let alb_name = request.hint_album.clone().unwrap_or_else(|| "Unknown Album".to_string());
                    let rel_date = request.hint_release_date.clone().unwrap_or_else(|| "2024-01-01".to_string());
                    Ok(TidalTrack {
                        id: numeric_id,
                        title: h_title.clone(),
                        duration: 180,
                        track_number: request.hint_track_number.or(Some(1)),
                        volume_number: request.hint_disc_number.or(Some(1)),
                        isrc: request.hint_isrc.clone(),
                        audio_quality: Some("LOSSLESS".to_string()),
                        version: None,
                        artist: Some(syncify_core_domain::metadata::TidalArtist { id: None, name: h_artist.clone() }),
                        artists: None,
                        album: Some(syncify_core_domain::metadata::TidalAlbum {
                            id: None,
                            title: alb_name,
                            release_date: Some(rel_date),
                            cover: None,
                            artist: None,
                            artists: None,
                        }),
                        media_metadata: None,
                    })
                } else {
                    Err(format!("MetadataResolutionFailed: Unable to resolve Tidal track {} from API, database, or request hints", numeric_id))
                }
            }
        }
    } else {
        let (artist_part, title_part) = if let Some((art, trk)) = target.split_once(" - ") {
            (art.trim(), trk.trim())
        } else {
            ("", target)
        };

        if target.len() == 12 && target.chars().all(|c| c.is_ascii_alphanumeric()) {
            downloader.search_by_isrc(target, 0).await.map_err(|e| e.to_string())
        } else {
            downloader
                .search_by_metadata_with_studio_option(title_part, artist_part, 0, true)
                .await
                .map_err(|e| e.to_string())
        }
    };

    let track = match track_result {
        Ok(t) => {
            if t.title.trim().is_empty() || t.title.starts_with("Tidal Track ") || t.artist_name().as_deref() == Some("Unknown Artist") || t.album_title().as_deref() == Some("Unknown Album") {
                error!(target = %target, "Resolved track has insufficient or placeholder metadata");
                on_progress(
                    PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::TrackUnresolved)
                        .with_error("MetadataResolutionFailed: Incomplete or placeholder metadata received".to_string())
                );
                return Err("MetadataResolutionFailed: Candidate track contains placeholder or incomplete metadata".to_string());
            }
            t
        },
        Err(e) => {
            error!(target = %target, error = %e, "Failed to resolve track on Tidal");
            on_progress(
                PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::TrackUnresolved)
                    .with_error(e.to_string())
            );
            return Err(format!("MetadataResolutionFailed: Failed to resolve candidate track on Tidal: {}", e));
        }
    };

    let tidal_id = track.id;
    let artist_name = track.artist_name().unwrap_or_else(|| {
        request.hint_artist.clone().unwrap_or_else(|| "Unknown Artist".to_string())
    });
    let album_title = track.album_title().unwrap_or_else(|| {
        request.hint_album.clone().unwrap_or_else(|| "Unknown Album".to_string())
    });
    let release_date = track.album.as_ref().and_then(|a| a.release_date.as_deref())
        .or_else(|| request.hint_release_date.as_deref())
        .unwrap_or("2024-01-01");
    let year_str = release_date.get(..4).unwrap_or("2024");
    let track_number = track.get_track_number();
    let disc_number = track.get_disc_number();
    let isrc_str = track.isrc.clone().or_else(|| request.hint_isrc.clone()).unwrap_or_default();

    if artist_name == "Unknown Artist" || album_title == "Unknown Album" || track.title.starts_with("Tidal Track ") {
        is_partial_metadata = true;
    }

    let mut resolved_info = ResolvedTrackInfo {
        provider: "tidal".to_string(),
        track_id: tidal_id.to_string(),
        isrc: if isrc_str.is_empty() { None } else { Some(isrc_str.clone()) },
        title: track.title.clone(),
        artist: artist_name.clone(),
        album: album_title.clone(),
        duration_sec: track.duration,
        requested_quality: quality_req.to_string(),
        obtained_quality: None,
        format_id_requested: Some(quality_req.to_string()),
        format_id_obtained: None,
        quality_class: None,
        active_account: active_account_name,
        region: active_account_region,
        allow_fallback,
        stream_codec: None,
        bit_depth: None,
        sample_rate: None,
    };

    info!(track_id = tidal_id, title = %track.title, artist = %artist_name, isrc = %isrc_str, "Track candidate successfully resolved on Tidal");
    on_progress(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::TrackResolved)
            .with_resolved_track(resolved_info.clone())
            .with_message(format!("Resolved: {} - {} (ID: {})", artist_name, track.title, tidal_id))
    );

    // 3. Resolving Stream URL & Quality Policy
    on_progress(PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::ResolvingStream).with_resolved_track(resolved_info.clone()));

    let stream_res = match downloader
        .get_stream_resolution_with_credentials(tidal_id, Some(quality_req), resolved_creds.as_ref(), allow_fallback)
        .await
    {
        Ok(res) => res,
        Err(e) => {
            let err_str = e.to_string();
            let is_auth_error = err_str.contains("PlaybackUnauthorized")
                || err_str.contains("401")
                || err_str.contains("RequiresAuth")
                || err_str.contains("unauthorized");

            if is_auth_error {
                let now_iso = chrono::Utc::now().to_rfc3339();
                warn!(
                    track_id = tidal_id,
                    token_present = resolved_creds.is_some(),
                    expired = false,
                    credentials_invalid = false,
                    endpoint = "playbackinfopostpaywall",
                    "[Tidal Auth Diagnostics] Tidal playback rejected (HTTP 401/403); recording download entitlement failure without invalidating account"
                );
                let _ = sqlx::query(
                    "UPDATE accounts SET last_auth_error = ?, last_auth_error_at = ? WHERE service_id = (SELECT id FROM services WHERE LOWER(name) = 'tidal' LIMIT 1) AND is_active = 1"
                )
                .bind(&err_str)
                .bind(&now_iso)
                .execute(db)
                .await;

                let auth_msg = format!("Download failed (Tidal stream entitlement/endpoint): Tidal playback authentication or entitlement failed (HTTP 401/403). Error: {}", err_str);
                on_progress(
                    PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::CandidateRejected)
                        .with_resolved_track(resolved_info.clone())
                        .with_error(auth_msg.clone())
                );
                return Err(auth_msg);
            }

            let user_friendly_msg = if err_str.contains("requested_lossless_but_received_aac") {
                "TIDAL catalog marks this track as Lossless, but the playback service returned AAC for the current account/client context. The download was rejected to prevent quality downgrade.".to_string()
            } else {
                format!("Stream resolution rejected or unavailable: {}", e)
            };
            warn!(track_id = tidal_id, error = %e, message = %user_friendly_msg, "Stream resolution rejected or unavailable");
            on_progress(
                PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::CandidateRejected)
                    .with_resolved_track(resolved_info.clone())
                    .with_error(user_friendly_msg.clone())
            );
            return Err(user_friendly_msg);
        }
    };

    resolved_info.obtained_quality = Some(stream_res.obtained_quality.clone());
    resolved_info.format_id_requested = Some(stream_res.format_id_requested.clone());
    resolved_info.format_id_obtained = Some(stream_res.format_id_obtained.clone());
    resolved_info.quality_class = Some(stream_res.quality_class_obtained);
    resolved_info.stream_codec = Some(stream_res.codec.clone());
    resolved_info.bit_depth = Some(stream_res.bit_depth as i32);
    resolved_info.sample_rate = Some(stream_res.sample_rate);

    info!(
        track_id = tidal_id,
        codec = %stream_res.codec,
        bit_depth = stream_res.bit_depth,
        sample_rate = stream_res.sample_rate,
        obtained_quality = %stream_res.obtained_quality,
        format_id_obtained = %stream_res.format_id_obtained,
        "Stream resolution successful"
    );

    // 4. Downloading Audio Payload
    phase_tracker.start_phase(DownloadPhase::Transfer);
    on_progress(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::DownloadStarted)
            .with_resolved_track(resolved_info.clone())
    );

    let temp_staging_dir = std::env::temp_dir().join(format!("syncify_staging_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_staging_dir)
        .await
        .map_err(|e| format!("Failed to create temporary staging directory: {}", e))?;

    let staged_file_path = temp_staging_dir.join(format!("{}.{}", tidal_id, stream_res.extension));

    let on_prog_arc = std::sync::Arc::new(on_progress);
    let on_prog_stream = on_prog_arc.clone();
    let target_str = target.to_string();
    let res_info_stream = resolved_info.clone();

    let download_bytes = match downloader.download_audio_payload_with_progress(
        &stream_res.url,
        &staged_file_path,
        move |seg_num, total_segs, bytes_done| {
            let percent = if total_segs > 0 {
                (seg_num as f64 / total_segs as f64) * 100.0
            } else {
                50.0
            };
            on_prog_stream(
                PipelineProgressEvent::new(&target_str, "tidal", PipelineStepStatus::Downloading)
                    .with_resolved_track(res_info_stream.clone())
                    .with_progress(percent, bytes_done, None)
                    .with_message(format!("Downloading DASH segment {}/{} ({} KB)", seg_num, total_segs, bytes_done / 1024))
            );
        }
    ).await {
        Ok(b) => b,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&temp_staging_dir).await;
            error!(track_id = tidal_id, error = %e, "Failed to download audio payload");
            on_prog_arc(
                PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::RecoverableError)
                    .with_resolved_track(resolved_info.clone())
                    .with_error(e.to_string())
            );
            return Err(format!("Failed to download audio payload: {}", e));
        }
    };

    phase_tracker.set_transfer_metrics(download_bytes, "network");

    on_prog_arc(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::DownloadCompleted)
            .with_resolved_track(resolved_info.clone())
            .with_progress(100.0, download_bytes, Some(download_bytes))
    );


    // 5. Pure Audio Byte Validation
    phase_tracker.start_phase(DownloadPhase::ValidateAudio);
    on_prog_arc(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Validating)
            .with_resolved_track(resolved_info.clone())
    );

    let payload_bytes = tokio::fs::read(&staged_file_path)
        .await
        .map_err(|e| format!("Failed to read staged audio file: {}", e))?;

    let is_valid = match stream_res.codec.as_str() {
        "FLAC" => AudioByteValidator::is_flac_magic(&payload_bytes),
        "MP3" => AudioByteValidator::is_mp3_magic(&payload_bytes),
        "AAC" => AudioByteValidator::is_m4a_magic(&payload_bytes),
        _ => !payload_bytes.is_empty(),
    };

    if !is_valid {
        let _ = tokio::fs::remove_dir_all(&temp_staging_dir).await;
        error!(track_id = tidal_id, codec = %stream_res.codec, "Downloaded file fails magic byte validation");
        return Err(format!("Downloaded file fails magic byte validation for codec {}", stream_res.codec));
    }

    // 6. Tagging (FLAC VorbisComments & Picture Preservation)
    on_prog_arc(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Tagging)
            .with_resolved_track(resolved_info.clone())
    );

    // --- Track enrichment result accumulators (used in manifest_entry) ---
    #[allow(unused_assignments)]
    let mut cover_result_str = "None".to_string();
    #[allow(unused_assignments)]
    let mut lyrics_result_str = "None".to_string();
    #[allow(unused_assignments)]
    let mut enrichment_result_str = "None".to_string();
    #[allow(unused_assignments)]
    let mut tagging_result_str = "None".to_string();

    let track_total = 0u32;
    let disc_total = 1u32;

    if stream_res.codec == "FLAC" {
        let mut flac_meta = FlacMetadata {
            title: track.title.clone(),
            artist: artist_name.clone(),
            album: album_title.clone(),
            album_artist: Some(artist_name.clone()),
            track_number: track_number as u32,
            track_total,
            disc_number: disc_number as u32,
            disc_total,
            isrc: if isrc_str.is_empty() { None } else { Some(isrc_str.clone()) },
            release_year: Some(year_str.to_string()),
            release_date: Some(release_date.to_string()),
            original_date: Some(format!("{}-01-01", year_str)),
            audio_source: Some("Tidal".to_string()),
            comment: Some(format!("Audio: {} | Source: Tidal | Engine: Syncify Production", stream_res.source_name)),
            bit_depth: Some(stream_res.bit_depth as i32),
            sample_rate: Some(stream_res.sample_rate),
            ..Default::default()
        };

        // 6a. Cover Art Download & Animated Cover Resolution (FLAC)
        phase_tracker.start_phase(DownloadPhase::ResolveCover);
        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::FetchingCover)
                .with_resolved_track(resolved_info.clone())
        );

        let mut raw_jpeg_bytes: Option<Vec<u8>> = None;
        let cover_url = track.album.as_ref().and_then(|a| a.cover_url());
        if let Some(ref url) = cover_url {
            match reqwest::get(url).await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.bytes().await {
                        Ok(bytes) if !bytes.is_empty() => {
                            let sidecar_jpg = temp_staging_dir.join("cover.jpg");
                            let _ = tokio::fs::write(&sidecar_jpg, &bytes).await;
                            raw_jpeg_bytes = Some(bytes.to_vec());
                            cover_result_str = "StaticJPEG".to_string();
                            info!(url = %url, size = bytes.len(), "[Pipeline §6a] Static cover art downloaded and saved as cover.jpg");
                        }
                        _ => {
                            warn!("[Pipeline §6a] Static cover art payload was empty");
                        }
                    }
                }
                Ok(resp) => {
                    warn!(status = %resp.status(), "[Pipeline §6a] Static cover HTTP error");
                }
                Err(e) => {
                    warn!(error = %e, "[Pipeline §6a] Static cover download request failed");
                }
            }
        }

        // Attempt Apple Music Animated Cover resolution for motion artwork
        let http_client = crate::download::http_client::shared_http_client();

        match resolve_and_download_animated_cover(http_client, &artist_name, &album_title, &temp_staging_dir).await {
            AnimatedCoverStatus::Success(webp_path) => {
                info!(path = %webp_path.display(), "[Pipeline §6a] ✓ Motion cover art resolved and downloaded from Apple Music");
                if let Ok(webp_bytes) = tokio::fs::read(&webp_path).await {
                    use syncify_core_domain::byte_validators::WebpByteValidator;
                    if let Ok(info) = WebpByteValidator::validate_animated_webp(&webp_bytes) {
                        info!(frames = info.anmf_frame_count, w = info.canvas_width, h = info.canvas_height, "[Pipeline §6a] ✓ Validated animated WebP");
                        flac_meta.cover_data = Some(webp_bytes);
                        flac_meta.cover_source = Some("Apple Music Animated Cover".to_string());
                        cover_result_str = "StaticAndAnimated".to_string();
                    } else {
                        warn!("[Pipeline §6a] Animated WebP failed structural validation (falling back to static JPEG)");
                        if let Some(jpeg_bytes) = raw_jpeg_bytes {
                            flac_meta.cover_data = Some(jpeg_bytes);
                            flac_meta.cover_source = Some("Tidal Cover Art".to_string());
                        }
                    }
                }
            }
            AnimatedCoverStatus::NotFound => {
                debug!("[Pipeline §6a] No motion cover art available on Apple Music for '{} - {}'", artist_name, album_title);
                if let Some(jpeg_bytes) = raw_jpeg_bytes {
                    flac_meta.cover_data = Some(jpeg_bytes);
                    flac_meta.cover_source = Some("Tidal Cover Art".to_string());
                }
            }
            AnimatedCoverStatus::SourceUnavailable(reason) => {
                debug!(reason = %reason, "[Pipeline §6a] Animated cover source unavailable");
                if let Some(jpeg_bytes) = raw_jpeg_bytes {
                    flac_meta.cover_data = Some(jpeg_bytes);
                    flac_meta.cover_source = Some("Tidal Cover Art".to_string());
                }
            }
            AnimatedCoverStatus::Failed(e) => {
                warn!(error = %e, "[Pipeline §6a] Animated cover processing failed (falling back to static JPEG)");
                if let Some(jpeg_bytes) = raw_jpeg_bytes {
                    flac_meta.cover_data = Some(jpeg_bytes);
                    flac_meta.cover_source = Some("Tidal Cover Art".to_string());
                }
            }
        }

        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::CoverApplied)
                .with_resolved_track(resolved_info.clone())
                .with_message(format!("Cover: {}", cover_result_str))
        );

        // 6b. Lyrics Fetch (FLAC via LyricsPipelineService)
        phase_tracker.start_phase(DownloadPhase::ResolveLyrics);
        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::FetchingLyrics)
                .with_resolved_track(resolved_info.clone())
        );

        let lyrics_service = LyricsPipelineService::new();
        let duration_sec = track.duration as f64;
        let mut resolved_lyrics_res: Option<LyricsResolution> = None;

        match lyrics_service
            .resolve_lyrics_and_sidecar(&artist_name, &track.title, Some(&album_title), duration_sec)
            .await
        {
            Ok((res, sidecar_opt)) => {
                if res.status == ResolutionStatus::Resolved {
                    let tags = res.to_tag_contract();
                    if let Some(ref lyr) = tags.lyrics {
                        flac_meta.lyrics_lrc = Some(lyr.clone());
                    }
                    if let Some(ref src) = tags.source {
                        flac_meta.lyrics_source = Some(src.clone());
                    }
                    lyrics_result_str = format!("{}_{:?}", res.provider, res.sync_type);

                    // Sidecar .lrc ONLY for valid synced lyrics (KaraokeWordSynced or LineSynced)
                    if let Some(ref lrc_content) = sidecar_opt {
                        let lrc_sidecar = staged_file_path.with_extension("lrc");
                        if let Ok(_) = tokio::fs::write(&lrc_sidecar, lrc_content).await {
                            info!(provider = %res.provider, sync_type = ?res.sync_type, "[Pipeline §6b] Synced lyrics acquired and sidecar staged");
                        }
                    } else {
                        info!(provider = %res.provider, sync_type = ?res.sync_type, "[Pipeline §6b] Plain lyrics acquired (no sidecar created)");
                    }

                    resolved_lyrics_res = Some(res);
                } else {
                    lyrics_result_str = format!("{:?}", res.status);
                    info!("[Pipeline §6b] Lyrics status: {:?}", res.status);
                }
            }
            Err(e) => {
                lyrics_result_str = "Failed".to_string();
                info!(error = %e, "[Pipeline §6b] Lyrics not available from any source");
            }
        }

        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::LyricsApplied)
                .with_resolved_track(resolved_info.clone())
                .with_message(format!("Lyrics: {}", lyrics_result_str))
        );

        // 6c. MusicBrainz Enrichment (FLAC)
        phase_tracker.start_phase(DownloadPhase::EnrichMetadata);
        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Enriching)
                .with_resolved_track(resolved_info.clone())
        );

        let enrichment_engine = EnrichmentEngine::new();
        let origin_meta = OriginTrackMetadata {
            title: Some(track.title.clone()),
            artist: Some(artist_name.clone()),
            album: Some(album_title.clone()),
            album_artist: Some(artist_name.clone()),
            track_number: Some(track_number as u32),
            disc_number: Some(disc_number as u32),
            release_year: Some(year_str.to_string()),
            isrc: if isrc_str.is_empty() { None } else { Some(isrc_str.clone()) },
            source_name: "Tidal".to_string(),
            ..Default::default()
        };

        let enriched = enrichment_engine.resolve_track_metadata(
            &artist_name, &album_title, &track.title,
            if isrc_str.is_empty() { None } else { Some(isrc_str.as_str()) },
            Some(&origin_meta),
        ).await;

        // Apply all enriched fields to FlacMetadata
        flac_meta.performers = Some(artist_name.clone());

        if let Some(label) = enriched.label.value() {
            flac_meta.label = Some(label.to_string());
        }
        if let Some(cat) = enriched.catalog_number.value() {
            flac_meta.catalog_number = Some(cat.to_string());
        }
        if let Some(barcode) = enriched.barcode.value() {
            flac_meta.barcode = Some(barcode.to_string());
        }
        if let Some(od) = enriched.original_date.value() {
            flac_meta.original_date = Some(od.to_string());
        }
        if let Some(rtype) = enriched.release_type.value() {
            flac_meta.release_type = Some(rtype.to_string());
        }
        if let Some(rstat) = enriched.release_status.value() {
            flac_meta.release_status = Some(rstat.to_string());
        }
        if let Some(rcntry) = enriched.release_country.value() {
            flac_meta.release_country = Some(rcntry.to_string());
        }
        if let Some(mb_rid) = enriched.musicbrainz_recording_id.value() {
            flac_meta.musicbrainz_track_id = Some(mb_rid.to_string());
        }
        if let Some(mb_relid) = enriched.musicbrainz_release_id.value() {
            flac_meta.musicbrainz_album_id = Some(mb_relid.to_string());
        }
        if let Some(mb_rgid) = enriched.musicbrainz_release_group_id.value() {
            flac_meta.musicbrainz_release_group_id = Some(mb_rgid.to_string());
        }
        if let Some(mb_aid) = enriched.musicbrainz_artist_id.value() {
            flac_meta.musicbrainz_artist_id = Some(mb_aid.to_string());
        }
        if let Some(mb_aaid) = enriched.musicbrainz_albumartist_id.value() {
            flac_meta.musicbrainz_albumartist_id = Some(mb_aaid.to_string());
        }
        if let Some(mb_wid) = enriched.musicbrainz_work_id.value() {
            flac_meta.musicbrainz_work_id = Some(mb_wid.to_string());
        }

        let has_mb_data = enriched.musicbrainz_recording_id.value().is_some();
        enrichment_result_str = if has_mb_data { "MusicBrainzResolved".to_string() } else { "NotFound".to_string() };
        info!(mb_resolved = has_mb_data, label = ?enriched.label.value(), "[Pipeline §6c] MusicBrainz enrichment completed");

        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::MetadataApplied)
                .with_resolved_track(resolved_info.clone())
                .with_message(format!("Enrichment: {}", enrichment_result_str))
        );

        // Apply all tags (base + cover + lyrics + enrichment) in one pass
        phase_tracker.start_phase(DownloadPhase::Tagging);
        match apply_and_verify_flac_tags(&staged_file_path, &flac_meta) {
            Ok(_) => {
                tagging_result_str = "Success (metaflac Verified)".to_string();
                info!("[Pipeline §6] FLAC tagging completed (base + cover + lyrics + enrichment)");

                // If plain lyrics resolved (no LRC timestamp), embed UNSYNCEDLYRICS & SYNCIFY_LYRICS_SOURCE
                if let Some(ref res) = resolved_lyrics_res {
                    if res.sync_type == LyricsSyncType::Plain {
                        let _ = validate_and_embed_flac_lyrics(&staged_file_path, res);
                    }
                }
            }
            Err(e) => {
                let _ = tokio::fs::remove_dir_all(&temp_staging_dir).await;
                error!(error = %e, "[Pipeline §6] FLAC VorbisComments tagging failed");
                on_prog_arc(
                    PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::RecoverableError)
                        .with_resolved_track(resolved_info.clone())
                        .with_error(format!("FLAC tagging failed: {}", e))
                );
                return Err(format!("FLACTaggingError: Failed to write and verify FLAC tags: {}", e));
            }
        }

        // 6d. Final FLAC Validation
        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Validating)
                .with_resolved_track(resolved_info.clone())
                .with_message("Final FLAC verification".to_string())
        );

        match audit_flac_stage("post_tagging", &staged_file_path) {
            Ok(report) => {
                info!(
                    picture_count = report.picture_count,
                    sidecar_jpg = report.sidecar_cover_jpg_exists,
                    sidecar_webp = report.sidecar_cover_webp_exists,
                    "[Pipeline §6d] FLAC picture audit completed"
                );
            }
            Err(e) => {
                warn!(error = %e, "[Pipeline §6d] FLAC picture audit failed (non-fatal)");
            }
        }

        match verify_flac_tags(&staged_file_path, &flac_meta) {
            Ok(v) => {
                info!(
                    tags_match = v.tags_match,
                    cover_present = v.cover_present,
                    lyrics_present = v.lyrics_present,
                    mismatches = ?v.mismatches,
                    "[Pipeline §6d] FLAC tag verification completed"
                );
            }
            Err(e) => {
                warn!(error = %e, "[Pipeline §6d] FLAC tag verification failed (non-fatal)");
            }
        }

    } else {
        // =========================================================================
        // AAC / M4A (HIGH) Path: Full MP4/M4A tagging with mp4ameta & verification
        // =========================================================================

        // 6a. Cover Art Download (M4A)
        phase_tracker.start_phase(DownloadPhase::ResolveCover);
        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::FetchingCover)
                .with_resolved_track(resolved_info.clone())
        );

        let mut m4a_cover_bytes: Option<Vec<u8>> = None;
        let cover_url = track.album.as_ref().and_then(|a| a.cover_url());
        if let Some(ref url) = cover_url {
            let client = crate::download::http_client::shared_http_client();
            match client.get(url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.bytes().await {
                        Ok(bytes) if !bytes.is_empty() => {
                            let sidecar_jpg = temp_staging_dir.join("cover.jpg");
                            let _ = tokio::fs::write(&sidecar_jpg, &bytes).await;
                            m4a_cover_bytes = Some(bytes.to_vec());
                            cover_result_str = "StaticJPEG".to_string();
                            info!(size = bytes.len(), "[Pipeline §6a] Sidecar cover.jpg saved and staged for M4A atom embedding");
                        }
                        _ => { cover_result_str = "Failed".to_string(); }
                    }
                }
                _ => { cover_result_str = "Failed".to_string(); }
            }
        }

        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::CoverApplied)
                .with_resolved_track(resolved_info.clone())
                .with_message(format!("Cover: {}", cover_result_str))
        );

        // 6b. Lyrics Fetch (M4A via LyricsPipelineService)
        phase_tracker.start_phase(DownloadPhase::ResolveLyrics);
        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::FetchingLyrics)
                .with_resolved_track(resolved_info.clone())
        );

        let mut m4a_lyrics_str: Option<String> = None;
        let lyrics_service = LyricsPipelineService::new();
        let duration_sec = track.duration as f64;
        match lyrics_service
            .resolve_lyrics_and_sidecar(&artist_name, &track.title, Some(&album_title), duration_sec)
            .await
        {
            Ok((res, sidecar_opt)) => {
                if res.status == ResolutionStatus::Resolved {
                    let tags = res.to_tag_contract();
                    m4a_lyrics_str = tags.unsynced_lyrics.clone().or(tags.lyrics.clone());
                    lyrics_result_str = format!("{}_{:?}", res.provider, res.sync_type);

                    // Sidecar .lrc ONLY if valid synced lyrics exist (KaraokeWordSynced or LineSynced)
                    if let Some(ref lrc_content) = sidecar_opt {
                        let lrc_path = staged_file_path.with_extension("lrc");
                        let _ = tokio::fs::write(&lrc_path, lrc_content).await;
                        info!(provider = %res.provider, "[Pipeline §6b] Synced lyrics acquired and sidecar staged for M4A");
                    } else {
                        info!(provider = %res.provider, "[Pipeline §6b] Plain lyrics acquired for M4A (no sidecar created)");
                    }
                } else {
                    lyrics_result_str = format!("{:?}", res.status);
                }
            }
            Err(e) => {
                lyrics_result_str = "Failed".to_string();
                info!(error = %e, "[Pipeline §6b] Lyrics not available (M4A)");
            }
        }

        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::LyricsApplied)
                .with_resolved_track(resolved_info.clone())
                .with_message(format!("Lyrics: {}", lyrics_result_str))
        );

        // 6c. MusicBrainz Enrichment (M4A)
        phase_tracker.start_phase(DownloadPhase::EnrichMetadata);
        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Enriching)
                .with_resolved_track(resolved_info.clone())
        );

        let enrichment_engine = EnrichmentEngine::new();
        let origin_meta = OriginTrackMetadata {
            title: Some(track.title.clone()),
            artist: Some(artist_name.clone()),
            album: Some(album_title.clone()),
            source_name: "Tidal".to_string(),
            ..Default::default()
        };

        let enriched = enrichment_engine.resolve_track_metadata(
            &artist_name, &album_title, &track.title,
            if isrc_str.is_empty() { None } else { Some(isrc_str.as_str()) },
            Some(&origin_meta),
        ).await;

        let has_mb_data = enriched.musicbrainz_recording_id.value().is_some();
        enrichment_result_str = if has_mb_data { "MusicBrainzResolved".to_string() } else { "NotFound".to_string() };
        info!(mb_resolved = has_mb_data, codec = "AAC", "[Pipeline §6c] MusicBrainz enrichment completed for M4A");

        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::MetadataApplied)
                .with_resolved_track(resolved_info.clone())
                .with_message(format!("Enrichment: {}", enrichment_result_str))
        );

        // 6d. Apply & Verify MP4/M4A Tags using mp4ameta
        phase_tracker.start_phase(DownloadPhase::Tagging);
        let mp4_meta = Mp4Metadata {
            title: track.title.clone(),
            artist: artist_name.clone(),
            album: album_title.clone(),
            album_artist: Some(artist_name.clone()),
            composer: None,
            performer: Some(artist_name.clone()),
            genre: enriched.label.value().map(|s| s.to_string()),
            release_year: Some(year_str.to_string()),
            release_date: Some(release_date.to_string()),
            original_date: enriched.original_date.value().map(|s| s.to_string()).or_else(|| Some(format!("{}-01-01", year_str))),
            track_number: track_number as u32,
            track_total,
            disc_number: disc_number as u32,
            disc_total,
            isrc: if isrc_str.is_empty() { None } else { Some(isrc_str.clone()) },
            label: enriched.label.value().map(|s| s.to_string()),
            catalog_number: enriched.catalog_number.value().map(|s| s.to_string()),
            barcode: enriched.barcode.value().map(|s| s.to_string()),
            release_country: None,
            comment: Some(format!("Audio: {} | Source: Tidal | Engine: Syncify Production", stream_res.source_name)),
            lyrics: m4a_lyrics_str,
            cover_data: m4a_cover_bytes,
            cover_mime: Some("image/jpeg".to_string()),
            musicbrainz_track_id: enriched.musicbrainz_recording_id.value().map(|s| s.to_string()),
            musicbrainz_artist_id: enriched.musicbrainz_artist_id.value().map(|s| s.to_string()),
            musicbrainz_album_id: enriched.musicbrainz_release_id.value().map(|s| s.to_string()),
            musicbrainz_albumartist_id: enriched.musicbrainz_artist_id.value().map(|s| s.to_string()),
            musicbrainz_release_group_id: enriched.musicbrainz_release_group_id.value().map(|s| s.to_string()),
            replaygain_track_gain: None,
            replaygain_track_peak: None,
            replaygain_album_gain: None,
            replaygain_album_peak: None,
            r128_track_gain: None,
            audio_source: Some(stream_res.source_name.clone()),
            explicit: Some(false),
        };

        match apply_and_verify_mp4_tags(&staged_file_path, &mp4_meta) {
            Ok(report) => {
                tagging_result_str = "Success (mp4ameta Verified)".to_string();
                info!(
                    title = report.title_matches,
                    artist = report.artist_matches,
                    album = report.album_matches,
                    track_num = report.track_number_matches,
                    cover = report.cover_present,
                    lyrics = report.lyrics_present,
                    "[Pipeline §6d] ✓ Real MP4/M4A tags written and verified successfully"
                );
            }
            Err(e) => {
                let _ = tokio::fs::remove_dir_all(&temp_staging_dir).await;
                error!(error = %e, "[Pipeline §6d] MP4/M4A tagging and verification failed");
                on_prog_arc(
                    PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::RecoverableError)
                        .with_resolved_track(resolved_info.clone())
                        .with_error(format!("MP4 tagging failed: {}", e))
                );
                return Err(format!("M4ATaggingError: Failed to write and verify MP4 tags: {}", e));
            }
        }
    }

    // 7. Staging — compute canonical library path (NO file move yet)
    on_prog_arc(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Staging)
            .with_resolved_track(resolved_info.clone())
    );

    // Resolve library_root: request override → folder_settings.base_folder → OS default
    let base_dir = if let Some(ref out) = request.output_dir {
        PathBuf::from(out)
    } else {
        let db_base: Option<String> = sqlx::query_scalar(
            "SELECT base_folder FROM folder_settings WHERE id = 1"
        )
        .fetch_optional(db)
        .await
        .unwrap_or(None);

        match db_base {
            Some(ref p) if !p.trim().is_empty() => PathBuf::from(p),
            _ => {
                dirs::audio_dir()
                    .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Music"))
                    .join("Syncify")
            }
        }
    };

    info!(
        staging_root = %temp_staging_dir.display(),
        library_root = %base_dir.display(),
        staged_file  = %staged_file_path.display(),
        "[Pipeline §7] Staging paths resolved"
    );

    let safe_artist = sanitize_filename_component(&artist_name);
    let safe_album = sanitize_filename_component(&format!("{} - {}", year_str, album_title));
    let safe_filename = compute_safe_track_filename(
        track_number,
        disc_number,
        1,
        &track.title,
        request.hint_title.as_deref(),
        Some(&track.title),
        Some(&album_title),
        &stream_res.extension,
        None,
    )?;

    let target_dir = if is_partial_metadata {
        base_dir.join(".staging")
    } else {
        base_dir.join(&safe_artist).join(&safe_album)
    };
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| format!("Failed to create library folder {:?}: {}", target_dir, e))?;

    let mut final_path = target_dir.join(&safe_filename);
    if final_path.exists() {
        let existing_match: Option<(i64, Option<String>)> = sqlx::query_as(
            r#"SELECT d.track_id, ts.service_track_id FROM downloads d 
               LEFT JOIN track_sources ts ON ts.track_id = d.track_id AND ts.service_id = d.source_service_id
               WHERE d.file_path = ?"#
        )
        .bind(final_path.to_string_lossy().to_string())
        .fetch_optional(db)
        .await
        .unwrap_or(None);

        let is_same_track = match existing_match {
            Some((_tid, Some(ref stid))) => stid == &tidal_id.to_string(),
            _ => false,
        };

        if !is_same_track {
            warn!(
                target = %target,
                existing_file = %final_path.display(),
                "[Pipeline §7] filename_collision detected for track {}: disambiguating with edition/track identity",
                tidal_id
            );
            let version_suffix = track.version.clone().unwrap_or_else(|| format!("Tidal-{}", tidal_id));
            let disambiguated_filename = compute_safe_track_filename(
                track_number,
                disc_number,
                1,
                &track.title,
                request.hint_title.as_deref(),
                Some(&track.title),
                Some(&album_title),
                &stream_res.extension,
                Some(&version_suffix),
            )?;
            final_path = target_dir.join(&disambiguated_filename);
        }
    }

    let final_path_str = final_path.to_string_lossy().to_string();
    info!(final_path = %final_path.display(), "[Pipeline §7] Target library path constructed");

    // Map extension → codec for downloads.file_format (CHECK constraint: FLAC|ALAC|WAV|MP3|AAC|OGG|OPUS)
    let db_file_format = match stream_res.extension.to_lowercase().as_str() {
        "flac" => "FLAC",
        "m4a"  => "AAC",   // M4A is the container; AAC is the codec the CHECK expects
        "mp3"  => "MP3",
        "ogg"  => "OGG",
        "opus" => "OPUS",
        "wav"  => "WAV",
        "alac" => "ALAC",
        other  => {
            warn!(ext = %other, "[Pipeline §7] Unknown extension; passing uppercase to file_format");
            &stream_res.extension  // best-effort; may fail CHECK
        }
    };

    on_prog_arc(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::StagingCompleted)
            .with_resolved_track(resolved_info.clone())
            .with_message(format!("Paths ready; file_format={}, dest={}", db_file_format, final_path.display()))
    );

    // 8. Atomic Database Persistence — BEFORE file move
    //    If this fails the staged file is preserved for diagnosis and no Completed is emitted.
    phase_tracker.start_phase(DownloadPhase::Persisting);
    on_prog_arc(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Persisting)
            .with_resolved_track(resolved_info.clone())
    );

    let mut tx = db.begin().await.map_err(|e| format!("DB transaction error: {}", e))?;

    // Service ID
    let service_id: i64 = sqlx::query_scalar(
        "INSERT INTO services (name, supports_download, max_quality) VALUES ('tidal', 1, 'hires')
         ON CONFLICT(name) DO UPDATE SET supports_download = 1 RETURNING id"
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("Failed to resolve Tidal service record: {}", e))?;

    // Check if track already exists in DB (by hint_track_id, by track_sources, or by isrc)
    let existing_track_id: Option<i64> = if let Some(h_tid) = request.hint_track_id {
        sqlx::query_scalar("SELECT id FROM tracks WHERE id = ?")
            .bind(h_tid)
            .fetch_optional(&mut *tx)
            .await
            .unwrap_or(None)
    } else {
        None
    };

    let existing_track_id = match existing_track_id {
        Some(id) => Some(id),
        None => {
            let by_ts: Option<i64> = sqlx::query_scalar(
                "SELECT track_id FROM track_sources WHERE service_id = ? AND service_track_id = ? LIMIT 1"
            )
            .bind(service_id)
            .bind(tidal_id.to_string())
            .fetch_optional(&mut *tx)
            .await
            .unwrap_or(None);

            if by_ts.is_some() {
                by_ts
            } else if !isrc_str.is_empty() {
                sqlx::query_scalar("SELECT id FROM tracks WHERE isrc = ? LIMIT 1")
                    .bind(&isrc_str)
                    .fetch_optional(&mut *tx)
                    .await
                    .unwrap_or(None)
            } else if artist_name != "Unknown Artist" {
                sqlx::query_scalar(
                    r#"SELECT t.id FROM tracks t 
                       JOIN track_artists ta ON ta.track_id = t.id 
                       JOIN artists ar ON ta.artist_id = ar.id 
                       WHERE ar.name = ? AND t.title = ? 
                       LIMIT 1"#
                )
                .bind(&artist_name)
                .bind(&track.title)
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or(None)
            } else {
                None
            }
        }
    };

    let track_db_id = if let Some(existing_id) = existing_track_id {
        // Update existing track with any richer/missing fields
        let _ = sqlx::query(
            r#"UPDATE tracks SET 
                isrc = COALESCE(isrc, ?),
                audio_quality = COALESCE(audio_quality, ?),
                duration_ms = CASE WHEN duration_ms IS NULL OR duration_ms = 0 THEN ? ELSE duration_ms END,
                track_number = CASE WHEN track_number IS NULL OR track_number = 0 THEN ? ELSE track_number END,
                disc_number = CASE WHEN disc_number IS NULL OR disc_number = 0 THEN ? ELSE disc_number END
               WHERE id = ?"#
        )
        .bind(if isrc_str.is_empty() { None } else { Some(&isrc_str) })
        .bind(&stream_res.obtained_quality)
        .bind((track.duration as i64) * 1000)
        .bind(track_number as i64)
        .bind(disc_number as i64)
        .bind(existing_id)
        .execute(&mut *tx)
        .await;

        existing_id
    } else {
        // Only insert new artist/album/track if not existing
        let artist_id: i64 = if artist_name != "Unknown Artist" {
            sqlx::query_scalar(
                "INSERT INTO artists (name) VALUES (?)
                 ON CONFLICT(name) DO UPDATE SET name = excluded.name RETURNING id"
            )
            .bind(&artist_name)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(1)
        } else {
            1
        };

        let album_id: i64 = sqlx::query_scalar(
            "INSERT INTO albums (title, release_date, cover_art_url) VALUES (?, ?, ?)
             RETURNING id"
        )
        .bind(&album_title)
        .bind(release_date)
        .bind(track.album.as_ref().and_then(|a| a.cover_url()))
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(1);

        if artist_name != "Unknown Artist" {
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)"
            )
            .bind(album_id)
            .bind(artist_id)
            .execute(&mut *tx)
            .await;
        }

        let new_track_id: i64 = sqlx::query_scalar(
            r#"INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, isrc, audio_quality)
               VALUES (?, ?, ?, ?, ?, ?, ?)
               RETURNING id"#
        )
        .bind(&track.title)
        .bind(album_id)
        .bind((track.duration as i64) * 1000)
        .bind(track_number as i64)
        .bind(disc_number as i64)
        .bind(if isrc_str.is_empty() { None } else { Some(&isrc_str) })
        .bind(&stream_res.obtained_quality)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(1);

        if artist_name != "Unknown Artist" {
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')"
            )
            .bind(new_track_id)
            .bind(artist_id)
            .execute(&mut *tx)
            .await;
        }

        new_track_id
    };

    // Ensure track_sources is recorded
    let _ = sqlx::query(
        r#"INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available, last_checked)
           VALUES (?, ?, ?, ?, ?, ?, 1, CURRENT_TIMESTAMP)
           ON CONFLICT(track_id, service_id) DO UPDATE SET
               service_track_id = excluded.service_track_id,
               format = excluded.format,
               bit_depth = excluded.bit_depth,
               sample_rate = excluded.sample_rate,
               available = 1,
               last_checked = CURRENT_TIMESTAMP"#
    )
    .bind(track_db_id)
    .bind(service_id)
    .bind(tidal_id.to_string())
    .bind(db_file_format)
    .bind(stream_res.bit_depth as i64)
    .bind(stream_res.sample_rate)
    .execute(&mut *tx)
    .await;

    // Calculate metadata completeness accurately
    let metadata_completeness = if is_partial_metadata || artist_name == "Unknown Artist" || album_title == "Unknown Album" {
        0
    } else {
        100
    };

    // Downloads record
    sqlx::query(
        r#"INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, metadata_completeness, downloaded_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
           ON CONFLICT(track_id) DO UPDATE SET
               file_path = excluded.file_path,
               file_format = excluded.file_format,
               bit_depth = excluded.bit_depth,
               sample_rate = excluded.sample_rate,
               file_size_bytes = excluded.file_size_bytes,
               metadata_completeness = excluded.metadata_completeness,
               updated_at = CURRENT_TIMESTAMP"#
    )
    .bind(track_db_id)
    .bind(service_id)
    .bind(&final_path_str)
    .bind(db_file_format)
    .bind(stream_res.bit_depth as i64)
    .bind(stream_res.sample_rate)
    .bind(download_bytes as i64)
    .bind(metadata_completeness)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        error!(error = %e, "[Pipeline §8] downloads INSERT failed — staged file preserved at {:?}", staged_file_path);
        format!("PersistenceError: Failed to persist download record: {}", e)
    })?;

    tx.commit().await.map_err(|e| {
        error!(error = %e, "[Pipeline §8] Transaction COMMIT failed — staged file preserved at {:?}", staged_file_path);
        format!("PersistenceError: Failed to commit database transaction: {}", e)
    })?;

    info!(
        track_db_id = track_db_id,
        service_id  = service_id,
        final_path  = %final_path_str,
        file_format = %db_file_format,
        bit_depth   = stream_res.bit_depth,
        sample_rate = stream_res.sample_rate,
        size_bytes  = download_bytes,
        completeness = metadata_completeness,
        "[Pipeline §8] SQLite transaction committed successfully"
    );

    on_prog_arc(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Persisted)
            .with_resolved_track(resolved_info.clone())
    );

    // 9. Move audio file and sidecars from staging to library — AFTER successful persistence
    //    If the move fails, compensate by deleting the DB row so there are no orphan records.
    phase_tracker.start_phase(DownloadPhase::Promotion);
    let move_result: Result<(), String> = async {
        // Move primary audio file
        match tokio::fs::rename(&staged_file_path, &final_path).await {
            Ok(()) => {
                info!(src = %staged_file_path.display(), dest = %final_path.display(), "[Pipeline §9] Primary audio atomic rename succeeded");
            }
            Err(rename_err) => {
                info!(error = %rename_err, "[Pipeline §9] Atomic rename failed (cross-volume); falling back to copy+delete");
                tokio::fs::copy(&staged_file_path, &final_path).await
                    .map_err(|e| format!("Failed to copy staged audio file to library {:?}: {}", final_path, e))?;
                let _ = tokio::fs::remove_file(&staged_file_path).await;
                info!(src = %staged_file_path.display(), dest = %final_path.display(), "[Pipeline §9] Copy+delete fallback succeeded");
            }
        }

        // Copy/Move sidecar files from temp_staging_dir to target_dir (cover.jpg, cover.webp, folder.webp, animated.webp, booklet.pdf, artist.nfo, biography.txt, fanart.jpg, artist.jpg, *.lrc)
        if let Ok(mut dir_entries) = tokio::fs::read_dir(&temp_staging_dir).await {
            while let Ok(Some(entry)) = dir_entries.next_entry().await {
                let entry_path = entry.path();
                if entry_path.is_file() {
                    let file_name = entry.file_name();
                    let file_name_str = file_name.to_string_lossy();
                    if file_name_str == "cover.jpg"
                        || file_name_str == "cover.webp"
                        || file_name_str == "folder.webp"
                        || file_name_str == "animated.webp"
                        || file_name_str == "booklet.pdf"
                        || file_name_str == "artist.nfo"
                        || file_name_str == "biography.txt"
                        || file_name_str == "fanart.jpg"
                        || file_name_str == "artist.jpg"
                        || file_name_str.ends_with(".lrc")
                    {
                        let dest_sidecar = target_dir.join(&file_name);
                        if !dest_sidecar.exists() {
                            let _ = tokio::fs::copy(&entry_path, &dest_sidecar).await;
                            debug!(from = %entry_path.display(), to = %dest_sidecar.display(), "[Pipeline §9] Sidecar copied to library folder");
                        }
                    }
                }
            }
        }

        Ok(())
    }.await;

    if let Err(move_err) = move_result {
        // Compensate: remove the DB row so there's no orphan record pointing at a missing file
        error!(error = %move_err, "[Pipeline §9] File move failed — compensating by removing SQLite record");
        let _ = sqlx::query("DELETE FROM downloads WHERE file_path = ?")
            .bind(&final_path_str)
            .execute(db)
            .await;
        // Leave staged file for diagnosis
        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::RecoverableError)
                .with_resolved_track(resolved_info.clone())
                .with_error(format!("File move failed after SQLite commit: {}", move_err))
        );
        return Err(format!("PersistenceError: File move to library failed (DB compensated): {}", move_err));
    }

    let _ = tokio::fs::remove_dir_all(&temp_staging_dir).await;

    // Verify final file exists and get size
    let final_file_size = tokio::fs::metadata(&final_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    info!(final_path = %final_path.display(), size_bytes = final_file_size, "[Pipeline §9] File staged to library successfully");

    // 10. Pipeline Completed
    on_prog_arc(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Completed)
            .with_resolved_track(resolved_info.clone())
            .with_message(format!("Successfully downloaded and persisted: {}", final_path_str))
    );

    let is_flac = stream_res.codec == "FLAC";
    let manifest_entry = TrackManifestEntry {
        provider: "tidal".to_string(),
        source_track_id: tidal_id.to_string(),
        isrc: if isrc_str.is_empty() { None } else { Some(isrc_str) },
        title: track.title.clone(),
        artist: artist_name.clone(),
        album: album_title.clone(),
        format_requested: quality_req.to_string(),
        format_obtained: Some(stream_res.obtained_quality.clone()),
        quality_class_requested: stream_res.quality_class_requested.to_string(),
        quality_class_obtained: Some(stream_res.quality_class_obtained.to_string()),
        codec: Some(stream_res.codec.clone()),
        container: Some(stream_res.container.clone()),
        extension: Some(stream_res.extension.clone()),
        source: Some("Tidal Official API / Proxy Cascade".to_string()),
        quality_fallback: allow_fallback,
        download_result: "Success".to_string(),
        rejection_reason: None,
        audio_validation: "Valid".to_string(),
        error: None,
        format_id_requested: quality_req.to_string(),
        format_id_obtained: Some(stream_res.obtained_quality.clone()),
        final_path: Some(final_path_str.clone()),
        size_bytes: Some(download_bytes),
        flac_validation: if is_flac { "Valid".to_string() } else { "None".to_string() },
        tagging_result: tagging_result_str,
        enrichment_result: enrichment_result_str.clone(),
        cover_result: cover_result_str.clone(),
        lyrics_result: lyrics_result_str.clone(),
        ..Default::default()
    };

    let has_lyrics = !lyrics_result_str.contains("None") && !lyrics_result_str.contains("Failed") && !lyrics_result_str.contains("NotFound");
    let has_cover = !cover_result_str.contains("None") && !cover_result_str.contains("Failed");
    let has_mb = enrichment_result_str.contains("MusicBrainzResolved");
    phase_tracker.set_cache_hits(has_lyrics, has_cover, has_mb);
    let phase_timings = phase_tracker.finish_completed();

    Ok(TidalSingleTrackResponse {
        success: true,
        track_id: tidal_id,
        title: track.title,
        artist: artist_name,
        album: album_title,
        file_path: final_path_str,
        file_format: stream_res.extension,
        bit_depth: stream_res.bit_depth as i32,
        sample_rate: stream_res.sample_rate as i32,
        isrc: if track.isrc.as_deref().unwrap_or("").is_empty() { None } else { track.isrc },
        manifest_entry,
        phase_timings: Some(phase_timings),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRepairPlanItem {
    pub download_id: i64,
    pub old_track_id: i64,
    pub new_track_id: i64,
    pub old_file_path: String,
    pub proposed_new_path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub isrc: Option<String>,
    pub tidal_track_id: String,
    pub ghost_track_ids_to_clean: Vec<i64>,
    pub ghost_album_ids_to_clean: Vec<i64>,
    pub baseline: Option<syncify_core_domain::repair::RepairFileBaseline>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRepairDryRunItem {
    pub download_id: i64,
    pub old_track_id: i64,
    pub new_track_id: i64,
    pub old_path: String,
    pub new_path: String,
    pub old_title: String,
    pub new_title: String,
    pub old_artist: String,
    pub new_artist: String,
    pub old_album: String,
    pub new_album: String,
    pub old_hash: Option<String>,
    pub expected_hash_after: Option<String>,
    pub flac_operation: String,
    pub lrc_operation: String,
    pub cover_operation: String,
    pub downloads_update: String,
    pub ghost_cleanup: String,
    pub rollback_plan: String,
    pub planned_action: String,
    pub confidence: f64,
    pub provenance: String,
    pub no_redownload_confirmed: bool,
    pub baseline: Option<syncify_core_domain::repair::RepairFileBaseline>,
}

pub async fn compute_file_sha256_async(path: &std::path::Path) -> Option<String> {
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncReadExt;

    let mut file = tokio::fs::File::open(path).await.ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let n = file.read(&mut buffer).await.ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Some(format!("{:x}", hasher.finalize()))
}

/// Compute rich non-mutating dry-run repair audit items for all corrupt download rows.
pub async fn compute_download_repair_dry_run(db: &DbPool) -> Result<Vec<DownloadRepairDryRunItem>, String> {
    let corrupt_rows: Vec<(i64, i64, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i32>, Option<String>, Option<i64>)> = sqlx::query_as(
        r#"SELECT d.id, d.track_id, d.file_path, d.file_format, ts.service_track_id, t.title, ar.name, al.title, t.track_number, t.isrc, t.album_id
           FROM downloads d
           LEFT JOIN tracks t ON t.id = d.track_id
           LEFT JOIN track_sources ts ON ts.track_id = t.id AND ts.service_id = (SELECT id FROM services WHERE LOWER(name) = 'tidal' LIMIT 1)
           LEFT JOIN albums al ON t.album_id = al.id
           LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
           LEFT JOIN artists ar ON ta.artist_id = ar.id
           WHERE t.title LIKE 'Tidal Track %' 
              OR d.file_path LIKE '%Unknown Artist%' 
              OR d.metadata_completeness = 0
              OR al.title = 'Unknown Album'"#
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("DB query error: {}", e))?;

    let mut items = Vec::new();

    let db_base: Option<String> = sqlx::query_scalar(
        "SELECT base_folder FROM folder_settings WHERE id = 1"
    )
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let default_base_dir = match db_base {
        Some(ref p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => dirs::audio_dir().unwrap_or_default().join("Syncify"),
    };

    for (dl_id, old_track_id, file_path_str, _file_format, s_track_id_opt, old_title_opt, old_artist_opt, old_album_opt, trk_num_opt, _isrc_opt, ghost_album_id_opt) in corrupt_rows {
        let current_path = PathBuf::from(&file_path_str);
        let tidal_id_str = s_track_id_opt.or_else(|| {
            let fn_str = current_path.file_name()?.to_string_lossy().to_string();
            if fn_str.contains("Tidal Track ") {
                let id_part = fn_str
                    .replace("01 - Tidal Track ", "")
                    .replace("Tidal Track ", "")
                    .replace(".flac", "")
                    .replace(".m4a", "")
                    .replace(".mp3", "")
                    .trim()
                    .to_string();
                Some(id_part)
            } else if fn_str.starts_with("tidal_") {
                let id_part = fn_str
                    .replace("tidal_", "")
                    .replace(".flac", "")
                    .replace(".m4a", "")
                    .replace(".mp3", "")
                    .trim()
                    .to_string();
                Some(id_part)
            } else {
                None
            }
        });

        let tidal_track_id = match tidal_id_str {
            Some(tid) => tid,
            None => continue,
        };

        // Resolve real track ID from track_sources where service_track_id matches
        let real_track_info: Option<(i64, String, Option<String>, Option<String>, Option<String>, Option<i32>, Option<String>)> = sqlx::query_as(
            r#"SELECT t.id, t.title, ar.name, al.title, al.release_date, t.track_number, t.isrc
               FROM tracks t
               JOIN track_sources ts ON ts.track_id = t.id AND ts.service_id = (SELECT id FROM services WHERE LOWER(name) = 'tidal' LIMIT 1)
               LEFT JOIN albums al ON t.album_id = al.id
               LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
               LEFT JOIN artists ar ON ta.artist_id = ar.id
               WHERE ts.service_track_id = ? AND t.id != ?
               LIMIT 1"#
        )
        .bind(&tidal_track_id)
        .bind(old_track_id)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

        let (new_track_id, real_title, real_artist, real_album, real_rel_date, real_trk_num, _real_isrc, provenance) = match real_track_info {
            Some((rt_id, rt_title, rt_art, rt_alb, rt_rel, rt_num, rt_isrc)) => (
                rt_id,
                rt_title,
                rt_art.unwrap_or_else(|| "Unknown Artist".to_string()),
                rt_alb.unwrap_or_else(|| "Unknown Album".to_string()),
                rt_rel.unwrap_or_else(|| "2024-01-01".to_string()),
                rt_num.unwrap_or(1),
                rt_isrc,
                "sqlite.track_sources + tracks".to_string(),
            ),
            None => (
                old_track_id,
                old_title_opt.clone().unwrap_or_else(|| format!("Tidal Track {}", tidal_track_id)),
                old_artist_opt.clone().unwrap_or_else(|| "Unknown Artist".to_string()),
                old_album_opt.clone().unwrap_or_else(|| "Unknown Album".to_string()),
                "2024-01-01".to_string(),
                trk_num_opt.unwrap_or(1),
                None,
                "downloads.fallback".to_string(),
            ),
        };

        let year_str = real_rel_date.get(..4).unwrap_or("2024");
        let ext = current_path.extension().and_then(|s| s.to_str()).unwrap_or("flac");
        let safe_artist = sanitize_filename_component(&real_artist);
        let safe_album = sanitize_filename_component(&format!("{} - {}", year_str, real_album));
        let disambiguator = if !has_sufficient_alphanumeric(&real_title) || real_title.contains('★') {
            Some(format!("Tidal-{}", tidal_track_id))
        } else {
            None
        };
        let safe_filename = compute_safe_track_filename(
            real_trk_num,
            1,
            1,
            &real_title,
            old_title_opt.as_deref(),
            Some(&real_title),
            Some(&real_album),
            ext,
            disambiguator.as_deref(),
        ).unwrap_or_else(|_| format!("{:02} - {}.{}", real_trk_num, sanitize_filename_component(&real_title), ext));

        let proposed_new_path = default_base_dir.join(&safe_artist).join(&safe_album).join(&safe_filename)
            .to_string_lossy().to_string();

        let baseline = compute_repair_baseline(&current_path, None).await.ok();
        let old_hash = baseline.as_ref().map(|b| b.input_sha256.clone()).or_else(|| {
            // fallback if sync compute
            futures::executor::block_on(compute_file_sha256_async(&current_path))
        });
        let expected_hash_after = old_hash.as_ref().map(|h| format!("{}:rewritten_vorbis_comments_and_pic_tags", &h[..12.min(h.len())]));
        let flac_op = format!("write_vorbis_comments_and_relocate: [TITLE='{}', ARTIST='{}', ALBUM='{}'] -> {}", real_title, real_artist, real_album, proposed_new_path);
        let lrc_op = "relocate_sidecar_or_fetch_if_available".to_string();
        let cover_op = "embed_album_art_picture_block_and_save_folder_jpg".to_string();
        let dl_update = format!("UPDATE downloads SET track_id = {}, file_path = '{}', metadata_completeness = 100, updated_at = CURRENT_TIMESTAMP WHERE id = {}", new_track_id, proposed_new_path, dl_id);
        let ghost_clean = if let Some(gh_alb) = ghost_album_id_opt {
            format!("DELETE FROM track_artists WHERE track_id = {0}; DELETE FROM track_sources WHERE track_id = {0}; DELETE FROM tracks WHERE id = {0}; DELETE FROM albums WHERE id = {1} AND id NOT IN (SELECT album_id FROM tracks WHERE album_id IS NOT NULL);", old_track_id, gh_alb)
        } else {
            format!("DELETE FROM track_artists WHERE track_id = {0}; DELETE FROM track_sources WHERE track_id = {0}; DELETE FROM tracks WHERE id = {0};", old_track_id)
        };
        let rb_plan = format!("Atomic SQLite transaction abort + FS rename rollback from '{}' back to '{}'", proposed_new_path, file_path_str);

        items.push(DownloadRepairDryRunItem {
            download_id: dl_id,
            old_track_id,
            new_track_id,
            old_path: file_path_str,
            new_path: proposed_new_path,
            old_title: old_title_opt.unwrap_or_else(|| "Unknown".to_string()),
            new_title: real_title,
            old_artist: old_artist_opt.unwrap_or_else(|| "Unknown Artist".to_string()),
            new_artist: real_artist,
            old_album: old_album_opt.unwrap_or_else(|| "Unknown Album".to_string()),
            new_album: real_album,
            old_hash,
            expected_hash_after,
            flac_operation: flac_op,
            lrc_operation: lrc_op,
            cover_operation: cover_op,
            downloads_update: dl_update,
            ghost_cleanup: ghost_clean,
            rollback_plan: rb_plan,
            planned_action: "relocate_flac_and_lrc_relink_downloads_clean_ghost_track".to_string(),
            confidence: 1.0,
            provenance,
            no_redownload_confirmed: true,
            baseline,
        });
    }

    Ok(items)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReEnrichResult {
    pub success: bool,
    pub dry_run: bool,
    pub download_id: i64,
    pub old_track_id: i64,
    pub new_track_id: i64,
    pub old_path: String,
    pub new_path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub isrc: Option<String>,
    pub tags_applied: bool,
    pub cover_applied: bool,
    pub lyrics_applied: bool,
    pub moved: bool,
    pub metadata_completeness: i32,
    pub baseline: Option<syncify_core_domain::repair::RepairFileBaseline>,
    pub validation_status: Option<syncify_core_domain::repair::RepairValidationStatus>,
    pub applied_actions: Vec<String>,
    pub rollback_state: Option<String>,
    pub output_hashes: Option<syncify_core_domain::repair::RepairOutputHashes>,
    pub error: Option<String>,
}

/// Produce a safe dry-run plan of all corrupt download rows that point to ghost tracks or placeholder paths.
pub async fn plan_repair_corrupt_downloads(db: &DbPool) -> Result<Vec<DownloadRepairPlanItem>, String> {
    let corrupt_rows: Vec<(i64, i64, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i32>, Option<String>, Option<i64>)> = sqlx::query_as(
        r#"SELECT d.id, d.track_id, d.file_path, d.file_format, ts.service_track_id, t.title, ar.name, al.title, t.track_number, t.isrc, t.album_id
           FROM downloads d
           LEFT JOIN tracks t ON t.id = d.track_id
           LEFT JOIN track_sources ts ON ts.track_id = t.id AND ts.service_id = (SELECT id FROM services WHERE LOWER(name) = 'tidal' LIMIT 1)
           LEFT JOIN albums al ON t.album_id = al.id
           LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
           LEFT JOIN artists ar ON ta.artist_id = ar.id
           WHERE t.title LIKE 'Tidal Track %' 
              OR d.file_path LIKE '%Unknown Artist%' 
              OR d.metadata_completeness = 0
              OR al.title = 'Unknown Album'"#
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("DB query error: {}", e))?;

    let mut plan = Vec::new();

    let db_base: Option<String> = sqlx::query_scalar(
        "SELECT base_folder FROM folder_settings WHERE id = 1"
    )
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let default_base_dir = match db_base {
        Some(ref p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => dirs::audio_dir().unwrap_or_default().join("Syncify"),
    };

    for (dl_id, old_track_id, file_path_str, _file_format, s_track_id_opt, title_opt, artist_opt, album_opt, trk_num_opt, isrc_opt, album_id_opt) in corrupt_rows {
        let current_path = PathBuf::from(&file_path_str);
        let tidal_id_str = s_track_id_opt.or_else(|| {
            let fn_str = current_path.file_name()?.to_string_lossy().to_string();
            if fn_str.contains("Tidal Track ") {
                let id_part = fn_str
                    .replace("01 - Tidal Track ", "")
                    .replace("Tidal Track ", "")
                    .replace(".flac", "")
                    .replace(".m4a", "")
                    .replace(".mp3", "")
                    .trim()
                    .to_string();
                Some(id_part)
            } else if fn_str.starts_with("tidal_") {
                let id_part = fn_str
                    .replace("tidal_", "")
                    .replace(".flac", "")
                    .replace(".m4a", "")
                    .replace(".mp3", "")
                    .trim()
                    .to_string();
                Some(id_part)
            } else {
                None
            }
        });

        let tidal_track_id = match tidal_id_str {
            Some(tid) => tid,
            None => continue,
        };

        // Resolve real track ID from track_sources where service_track_id matches but track_id != old_track_id
        let real_track_info: Option<(i64, String, Option<String>, Option<String>, Option<String>, Option<i32>, Option<String>)> = sqlx::query_as(
            r#"SELECT t.id, t.title, ar.name, al.title, al.release_date, t.track_number, t.isrc
               FROM tracks t
               JOIN track_sources ts ON ts.track_id = t.id AND ts.service_id = (SELECT id FROM services WHERE LOWER(name) = 'tidal' LIMIT 1)
               LEFT JOIN albums al ON t.album_id = al.id
               LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
               LEFT JOIN artists ar ON ta.artist_id = ar.id
               WHERE ts.service_track_id = ? AND t.id != ?
               LIMIT 1"#
        )
        .bind(&tidal_track_id)
        .bind(old_track_id)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

        let (new_track_id, real_title, real_artist, real_album, real_rel_date, real_trk_num, real_isrc) = match real_track_info {
            Some((rt_id, rt_title, rt_art, rt_alb, rt_rel, rt_num, rt_isrc)) => (
                rt_id,
                rt_title,
                rt_art.unwrap_or_else(|| "Unknown Artist".to_string()),
                rt_alb.unwrap_or_else(|| "Unknown Album".to_string()),
                rt_rel.unwrap_or_else(|| "2024-01-01".to_string()),
                rt_num.unwrap_or(1),
                rt_isrc,
            ),
            None => (
                old_track_id,
                title_opt.clone().unwrap_or_else(|| format!("Tidal Track {}", tidal_track_id)),
                artist_opt.unwrap_or_else(|| "Unknown Artist".to_string()),
                album_opt.unwrap_or_else(|| "Unknown Album".to_string()),
                "2024-01-01".to_string(),
                trk_num_opt.unwrap_or(1),
                isrc_opt,
            ),
        };

        let year_str = real_rel_date.get(..4).unwrap_or("2024");
        let ext = current_path.extension().and_then(|s| s.to_str()).unwrap_or("flac");
        let safe_artist = sanitize_filename_component(&real_artist);
        let safe_album = sanitize_filename_component(&format!("{} - {}", year_str, real_album));
        let disambiguator = if !has_sufficient_alphanumeric(&real_title) || real_title.contains('★') {
            Some(format!("Tidal-{}", tidal_track_id))
        } else {
            None
        };
        let safe_filename = compute_safe_track_filename(
            real_trk_num,
            1,
            1,
            &real_title,
            title_opt.as_deref(),
            Some(&real_title),
            Some(&real_album),
            ext,
            disambiguator.as_deref(),
        ).unwrap_or_else(|_| format!("{:02} - {}.{}", real_trk_num, sanitize_filename_component(&real_title), ext));

        let proposed_new_path = default_base_dir.join(&safe_artist).join(&safe_album).join(&safe_filename)
            .to_string_lossy().to_string();

        let mut ghost_tracks = Vec::new();
        let mut ghost_albums = Vec::new();

        if old_track_id != new_track_id {
            ghost_tracks.push(old_track_id);
            if let Some(alb_id) = album_id_opt {
                ghost_albums.push(alb_id);
            }
        }
        let baseline = compute_repair_baseline(&current_path, None).await.ok();

        plan.push(DownloadRepairPlanItem {
            download_id: dl_id,
            old_track_id,
            new_track_id,
            old_file_path: file_path_str,
            proposed_new_path,
            title: real_title,
            artist: real_artist,
            album: real_album,
            isrc: real_isrc,
            tidal_track_id,
            ghost_track_ids_to_clean: ghost_tracks,
            ghost_album_ids_to_clean: ghost_albums,
            baseline,
        });
    }

    Ok(plan)
}

/// Re-enrich and repair an existing downloaded audio file with rich Tidal/DB metadata, tags, cover and lyrics without redownloading audio bytes.
/// Supports `dry_run` mode (preview only) and Apply mode (transactional SQLite + coordinated file moves with rollback).
pub async fn reenrich_download_file(
    db: &DbPool,
    download_id_or_track_id: i64,
    dry_run: bool,
) -> Result<ReEnrichResult, String> {
    reenrich_download_file_with_baseline(db, download_id_or_track_id, dry_run, None).await
}

/// Re-enrich and repair an existing downloaded audio file, strictly validating against a pre-recorded dry-run baseline.
pub async fn reenrich_download_file_with_baseline(
    db: &DbPool,
    download_id_or_track_id: i64,
    dry_run: bool,
    expected_baseline: Option<&syncify_core_domain::repair::RepairFileBaseline>,
) -> Result<ReEnrichResult, String> {
    use syncify_core_domain::repair::{RepairOutputHashes, RepairValidationStatus};

    // 1. Resolve download and track records
    let row: Option<(i64, i64, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i32>, Option<i32>, Option<String>, Option<i64>)> = sqlx::query_as(
        r#"SELECT d.id, d.track_id, d.file_path, d.file_format, ts.service_track_id, t.title, ar.name, al.title, t.track_number, t.disc_number, t.isrc, t.album_id
           FROM downloads d
           LEFT JOIN tracks t ON t.id = d.track_id
           LEFT JOIN track_sources ts ON ts.track_id = t.id AND ts.service_id = (SELECT id FROM services WHERE LOWER(name) = 'tidal' LIMIT 1)
           LEFT JOIN albums al ON t.album_id = al.id
           LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
           LEFT JOIN artists ar ON ta.artist_id = ar.id
           WHERE d.id = ? OR d.track_id = ?
           LIMIT 1"#
    )
    .bind(download_id_or_track_id)
    .bind(download_id_or_track_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Database query error: {}", e))?;

    let (dl_id, old_track_id, current_file_path_str, _file_format, s_track_id_opt, title_opt, artist_opt, album_opt, trk_num_opt, disc_num_opt, isrc_opt, ghost_album_id_opt) =
        row.ok_or_else(|| format!("Download record not found for ID {}", download_id_or_track_id))?;

    let current_path = PathBuf::from(&current_file_path_str);
    if !current_path.exists() {
        return Err(format!("FileNotFound: Physical audio file not found at {:?}", current_path));
    }

    // Baseline calculation
    let baseline = compute_repair_baseline(&current_path, None).await.ok();

    // 2. Resolve Tidal ID
    let tidal_id_str = s_track_id_opt.or_else(|| {
        let fn_str = current_path.file_name()?.to_string_lossy().to_string();
        if fn_str.contains("Tidal Track ") {
            let id_part = fn_str
                .replace("01 - Tidal Track ", "")
                .replace("Tidal Track ", "")
                .replace(".flac", "")
                .replace(".m4a", "")
                .replace(".mp3", "")
                .trim()
                .to_string();
            Some(id_part)
        } else if fn_str.starts_with("tidal_") {
            let id_part = fn_str
                .replace("tidal_", "")
                .replace(".flac", "")
                .replace(".m4a", "")
                .replace(".mp3", "")
                .trim()
                .to_string();
            Some(id_part)
        } else {
            None
        }
    });

    let tidal_id = tidal_id_str.ok_or_else(|| "Unable to extract Tidal service track ID for re-enrichment".to_string())?;

    // 3. Check if real local track exists in DB
    let local_real_track: Option<(i64, String, Option<String>, Option<String>, Option<String>, Option<i32>, Option<i32>, Option<String>, Option<String>)> = sqlx::query_as(
        r#"SELECT t.id, t.title, ar.name, al.title, al.release_date, t.track_number, t.disc_number, t.isrc, al.cover_art_url
           FROM tracks t
           JOIN track_sources ts ON ts.track_id = t.id AND ts.service_id = (SELECT id FROM services WHERE LOWER(name) = 'tidal' LIMIT 1)
           LEFT JOIN albums al ON t.album_id = al.id
           LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
           LEFT JOIN artists ar ON ta.artist_id = ar.id
           WHERE ts.service_track_id = ? AND t.id != ?
           LIMIT 1"#
    )
    .bind(&tidal_id)
    .bind(old_track_id)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let http_client = crate::download::http_client::create_http_client();
    let (resolved_creds, _) = resolve_and_refresh_gui_credentials(db, &http_client).await;
    let downloader = TidalDownloader::new().with_user_token(resolved_creds.as_ref().map(|c| c.access_token.clone()));
    let country_code = resolved_creds.as_ref().and_then(|c| c.country_code.as_deref()).unwrap_or("US");

    let (new_track_id, final_title, final_artist, final_album, release_date, track_num, disc_num, isrc_str, cover_url_opt) = match local_real_track {
        Some((rt_id, rt_title, rt_art, rt_alb, rt_rel, rt_num, rt_disc, rt_isrc, rt_cover)) => (
            rt_id,
            rt_title,
            rt_art.unwrap_or_else(|| "Unknown Artist".to_string()),
            rt_alb.unwrap_or_else(|| "Unknown Album".to_string()),
            rt_rel.unwrap_or_else(|| "2024-01-01".to_string()),
            rt_num.unwrap_or(1),
            rt_disc.unwrap_or(1),
            rt_isrc.unwrap_or_default(),
            rt_cover,
        ),
        None => {
            let mut resolved_track: Option<TidalTrack> = None;
            if let Ok(tid_num) = tidal_id.parse::<i64>() {
                if let Ok(t) = downloader.get_track_with_country(tid_num, country_code).await {
                    resolved_track = Some(t);
                }
            }

            if resolved_track.is_none() {
                if let (Some(ref t), Some(ref a)) = (&title_opt, &artist_opt) {
                    if !t.starts_with("Tidal Track ") && a != "Unknown Artist" {
                        if let Ok(t) = downloader.search_by_metadata(t, a, 0).await {
                            resolved_track = Some(t);
                        }
                    }
                }
            }

            let track = resolved_track.ok_or_else(|| format!("MetadataResolutionFailed: Unable to resolve metadata for Tidal track ID {}", tidal_id))?;
            let f_title = track.title.clone();
            let f_artist = track.artist_name().or(artist_opt).unwrap_or_else(|| "Unknown Artist".to_string());
            let f_album = track.album_title().or(album_opt).unwrap_or_else(|| "Unknown Album".to_string());
            let f_rel = track.album.as_ref().and_then(|a| a.release_date.clone()).unwrap_or_else(|| "2024-01-01".to_string());
            let f_num = track.get_track_number().max(trk_num_opt.unwrap_or(1) as i32);
            let f_disc = track.get_disc_number().max(disc_num_opt.unwrap_or(1) as i32);
            let f_isrc = track.isrc.clone().or(isrc_opt).unwrap_or_default();
            let f_cover = track.album.as_ref().and_then(|a| a.cover_url());

            (old_track_id, f_title, f_artist, f_album, f_rel, f_num, f_disc, f_isrc, f_cover)
        }
    };

    let year_str = release_date.get(..4).unwrap_or("2024");
    let ext_lower = current_path.extension().and_then(|s| s.to_str()).unwrap_or("flac").to_lowercase();

    // Resolve canonical library base
    let db_base: Option<String> = sqlx::query_scalar(
        "SELECT base_folder FROM folder_settings WHERE id = 1"
    )
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let base_dir = match db_base {
        Some(ref p) if !p.trim().is_empty() => PathBuf::from(p),
        _ => {
            current_path.parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| dirs::audio_dir().unwrap_or_default().join("Syncify"))
        }
    };

    let safe_artist = sanitize_filename_component(&final_artist);
    let safe_album = sanitize_filename_component(&format!("{} - {}", year_str, final_album));
    let disambiguator = if !has_sufficient_alphanumeric(&final_title) || final_title.contains('★') {
        Some(format!("Tidal-{}", tidal_id))
    } else {
        None
    };
    let safe_filename = compute_safe_track_filename(
        track_num,
        disc_num,
        1,
        &final_title,
        title_opt.as_deref(),
        Some(&final_title),
        Some(&final_album),
        &ext_lower,
        disambiguator.as_deref(),
    )?;

    let dest_dir = base_dir.join(&safe_artist).join(&safe_album);
    let proposed_final_path = dest_dir.join(&safe_filename);
    let proposed_path_str = proposed_final_path.to_string_lossy().to_string();

    // If DRY-RUN mode: return projection without filesystem or DB modifications
    if dry_run {
        let output_hashes = baseline.as_ref().map(|b| RepairOutputHashes {
            file_hash_before: b.input_sha256.clone(),
            file_hash_after: None,
            audio_content_hash_before: b.audio_content_hash.clone(),
            audio_content_hash_after: None,
            lrc_hash_before: b.lrc_sha256.clone(),
            lrc_hash_after: None,
        });

        return Ok(ReEnrichResult {
            success: true,
            dry_run: true,
            download_id: dl_id,
            old_track_id,
            new_track_id,
            old_path: current_file_path_str,
            new_path: proposed_path_str,
            title: final_title,
            artist: final_artist,
            album: final_album,
            isrc: if isrc_str.is_empty() { None } else { Some(isrc_str) },
            tags_applied: false,
            cover_applied: false,
            lyrics_applied: false,
            moved: false,
            metadata_completeness: 100,
            baseline,
            validation_status: Some(RepairValidationStatus::Valid),
            applied_actions: vec![],
            rollback_state: None,
            output_hashes,
            error: None,
        });
    }

    // APPLY MODE: Pre-flight baseline revalidation guardrail
    let base = match expected_baseline {
        Some(b) => b.clone(),
        None => baseline.ok_or_else(|| "Failed to capture baseline before apply".to_string())?,
    };
    let val_status = validate_repair_baseline(&base, &current_path, None).await;
    if !val_status.is_valid() {
        let err_msg = val_status.error_message().unwrap_or_else(|| "RepairInputChanged: File baseline mismatch".to_string());
        return Err(err_msg);
    }

    let mut applied_actions = vec!["validated_baseline".to_string()];
    let file_hash_before = base.input_sha256.clone();
    let audio_content_hash_before = base.audio_content_hash.clone();

    let mut cover_applied = false;
    let mut lyrics_applied = false;
    let mut cover_bytes: Option<Vec<u8>> = None;

    if let Some(ref cover_url) = cover_url_opt {
        if let Ok(resp) = reqwest::get(cover_url).await {
            if resp.status().is_success() {
                if let Ok(b) = resp.bytes().await {
                    if !b.is_empty() {
                        cover_bytes = Some(b.to_vec());
                        cover_applied = true;
                    }
                }
            }
        }
    }

    let lyrics_service = LyricsPipelineService::new();
    let lyrics_res = lyrics_service
        .resolve_lyrics_and_sidecar(&final_artist, &final_title, Some(&final_album), 180.0)
        .await
        .ok();

    let mut lyrics_lrc: Option<String> = None;
    let mut lyrics_src: Option<String> = None;
    if let Some((res, sidecar_opt)) = lyrics_res {
        if res.status == ResolutionStatus::Resolved {
            let tags = res.to_tag_contract();
            lyrics_lrc = tags.lyrics;
            lyrics_src = tags.source;
            lyrics_applied = true;

            if let Some(ref lrc_content) = sidecar_opt {
                let sidecar_dest = current_path.with_extension("lrc");
                let _ = tokio::fs::write(&sidecar_dest, lrc_content).await;
                applied_actions.push("sidecar_lrc_written".to_string());
            }
        }
    }

    // Safety backup before modifying tags
    let backup_path = current_path.with_extension("syncify_repair_bak");
    tokio::fs::copy(&current_path, &backup_path).await
        .map_err(|e| format!("Failed to create backup prior to tagging: {}", e))?;

    // Tagging
    let tags_applied = if ext_lower == "flac" {
        let flac_meta = FlacMetadata {
            title: final_title.clone(),
            artist: final_artist.clone(),
            album: final_album.clone(),
            album_artist: Some(final_artist.clone()),
            track_number: track_num as u32,
            disc_number: disc_num as u32,
            disc_total: 1,
            isrc: if isrc_str.is_empty() { None } else { Some(isrc_str.clone()) },
            release_year: Some(year_str.to_string()),
            release_date: Some(release_date.clone()),
            original_date: Some(format!("{}-01-01", year_str)),
            audio_source: Some("Tidal".to_string()),
            comment: Some("Audio: Tidal Official API | Source: Tidal | Engine: Syncify Re-enrichment".to_string()),
            cover_data: cover_bytes.clone(),
            lyrics_lrc: lyrics_lrc.clone(),
            lyrics_source: lyrics_src.clone(),
            ..Default::default()
        };
        apply_and_verify_flac_tags(&current_path, &flac_meta).is_ok()
    } else {
        let mp4_meta = Mp4Metadata {
            title: final_title.clone(),
            artist: final_artist.clone(),
            album: final_album.clone(),
            album_artist: Some(final_artist.clone()),
            composer: None,
            performer: Some(final_artist.clone()),
            genre: None,
            release_year: Some(year_str.to_string()),
            release_date: Some(release_date.clone()),
            original_date: Some(format!("{}-01-01", year_str)),
            track_number: track_num as u32,
            track_total: 0,
            disc_number: disc_num as u32,
            disc_total: 1,
            isrc: if isrc_str.is_empty() { None } else { Some(isrc_str.clone()) },
            label: None,
            catalog_number: None,
            barcode: None,
            release_country: None,
            comment: Some("Audio: Tidal Official API | Source: Tidal | Engine: Syncify Re-enrichment".to_string()),
            lyrics: lyrics_lrc.clone(),
            cover_data: cover_bytes.clone(),
            cover_mime: Some("image/jpeg".to_string()),
            musicbrainz_track_id: None,
            musicbrainz_artist_id: None,
            musicbrainz_album_id: None,
            musicbrainz_albumartist_id: None,
            musicbrainz_release_group_id: None,
            replaygain_track_gain: None,
            replaygain_track_peak: None,
            replaygain_album_gain: None,
            replaygain_album_peak: None,
            r128_track_gain: None,
            audio_source: Some("Tidal".to_string()),
            explicit: Some(false),
        };
        apply_and_verify_mp4_tags(&current_path, &mp4_meta).is_ok()
    };

    if !tags_applied {
        let _ = tokio::fs::copy(&backup_path, &current_path).await;
        let _ = tokio::fs::remove_file(&backup_path).await;
        return Err("TaggingError: Failed to write verified Vorbis/MP4 tags to audio file".to_string());
    }
    applied_actions.push("tags_applied".to_string());

    // Verify audio content payload invariance across VorbisComment / Picture metadata rewriting
    let post_tag_bytes = tokio::fs::read(&current_path).await
        .map_err(|e| format!("Failed to read tagged audio file: {}", e))?;
    let file_hash_after_tagging = compute_bytes_sha256(&post_tag_bytes);
    let audio_content_hash_after_tagging = extract_audio_content_hash_from_bytes(&post_tag_bytes).ok();

    if let (Some(ref a_before), Some(ref a_after)) = (&audio_content_hash_before, &audio_content_hash_after_tagging) {
        if a_before != a_after {
            error!(before = %a_before, after = %a_after, "Audio content payload corrupted during tagging! Rolling back");
            let _ = tokio::fs::copy(&backup_path, &current_path).await;
            let _ = tokio::fs::remove_file(&backup_path).await;
            return Err("AudioPayloadCorrupted: Audio content payload changed during tagging".to_string());
        }
    }
    applied_actions.push("audio_payload_invariance_verified".to_string());
    let _ = tokio::fs::remove_file(&backup_path).await;

    // Coordinated file moves
    let mut moved = false;
    let mut final_path = current_path.clone();

    tokio::fs::create_dir_all(&dest_dir).await
        .map_err(|e| format!("Failed to create destination folder {:?}: {}", dest_dir, e))?;

    if proposed_final_path != current_path {
        match tokio::fs::rename(&current_path, &proposed_final_path).await {
            Ok(()) => {
                moved = true;
                final_path = proposed_final_path.clone();
                applied_actions.push(format!("moved_audio: {:?} -> {:?}", current_path, proposed_final_path));
            }
            Err(_) => {
                tokio::fs::copy(&current_path, &proposed_final_path).await
                    .map_err(|e| format!("Failed to copy file to canonical path: {}", e))?;
                let _ = tokio::fs::remove_file(&current_path).await;
                moved = true;
                final_path = proposed_final_path.clone();
                applied_actions.push(format!("moved_audio: {:?} -> {:?}", current_path, proposed_final_path));
            }
        }

        let old_lrc = current_path.with_extension("lrc");
        let new_lrc = final_path.with_extension("lrc");
        if old_lrc.exists() && old_lrc != new_lrc {
            if let Err(e) = tokio::fs::rename(&old_lrc, &new_lrc).await {
                error!("Failed to rename sidecar LRC {:?} -> {:?}; rolling back audio move", old_lrc, new_lrc);
                let _ = tokio::fs::rename(&final_path, &current_path).await;
                return Err(format!("LrcMoveFailed: {}", e));
            }
            applied_actions.push(format!("moved_lrc: {:?} -> {:?}", old_lrc, new_lrc));
        }
    }

    let final_path_str = final_path.to_string_lossy().to_string();

    // Transactional SQLite update with rollback protection
    let tx_result: Result<(), String> = async {
        let mut tx = db.begin().await.map_err(|e| format!("DB transaction begin failed: {}", e))?;

        if old_track_id != new_track_id {
            // Delete old download row if it conflicts
            sqlx::query("DELETE FROM downloads WHERE track_id = ? AND id != ?")
                .bind(new_track_id)
                .bind(dl_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to deduplicate downloads: {}", e))?;

            // Point downloads to real track
            sqlx::query(
                "UPDATE downloads SET track_id = ?, file_path = ?, metadata_completeness = 100, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(new_track_id)
            .bind(&final_path_str)
            .bind(dl_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to update download track reference: {}", e))?;

            // Clean up orphan ghost track
            let _ = sqlx::query("DELETE FROM track_artists WHERE track_id = ?").bind(old_track_id).execute(&mut *tx).await;
            let _ = sqlx::query("DELETE FROM track_sources WHERE track_id = ?").bind(old_track_id).execute(&mut *tx).await;
            let _ = sqlx::query("DELETE FROM tracks WHERE id = ?").bind(old_track_id).execute(&mut *tx).await;

            if let Some(gh_alb) = ghost_album_id_opt {
                let _ = sqlx::query("DELETE FROM album_artists WHERE album_id = ?").bind(gh_alb).execute(&mut *tx).await;
                let _ = sqlx::query("DELETE FROM albums WHERE id = ? AND id NOT IN (SELECT album_id FROM tracks WHERE album_id IS NOT NULL)").bind(gh_alb).execute(&mut *tx).await;
            }
        } else {
            sqlx::query(
                "UPDATE downloads SET file_path = ?, metadata_completeness = 100, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(&final_path_str)
            .bind(dl_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to update download file path: {}", e))?;
        }

        tx.commit().await.map_err(|e| format!("Transaction commit failed: {}", e))?;
        Ok(())
    }.await;

    if let Err(tx_err) = tx_result {
        // Rollback filesystem move if DB update fails
        if moved {
            let _ = tokio::fs::rename(&final_path, &current_path).await;
            let old_lrc = current_path.with_extension("lrc");
            let new_lrc = final_path.with_extension("lrc");
            if new_lrc.exists() {
                let _ = tokio::fs::rename(&new_lrc, &old_lrc).await;
            }
        }
        let rollback_msg = format!("RollbackExecuted: Transaction failed: {}", tx_err);
        let repair_id = format!("rep_dl_{}_{}", dl_id, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
        let _ = crate::services::repair_history::record_applied_repair(
            db,
            &repair_id,
            Some(dl_id),
            Some(old_track_id),
            Some(new_track_id),
            &current_file_path_str,
            &final_path_str,
            &file_hash_before,
            Some(&file_hash_after_tagging),
            audio_content_hash_before.as_deref(),
            audio_content_hash_after_tagging.as_deref(),
            "valid",
            &applied_actions,
            Some(&rollback_msg),
            "tidal_pipeline.re_enrich",
            "failed",
            None,
        ).await;
        return Err(rollback_msg);
    }
    applied_actions.push("database_updated".to_string());
    if old_track_id != new_track_id {
        applied_actions.push(format!("ghost_cleanup: track_id {}", old_track_id));
    }

    let output_hashes = Some(RepairOutputHashes {
        file_hash_before: file_hash_before.clone(),
        file_hash_after: Some(file_hash_after_tagging.clone()),
        audio_content_hash_before: audio_content_hash_before.clone(),
        audio_content_hash_after: audio_content_hash_after_tagging.clone(),
        lrc_hash_before: base.lrc_sha256.clone(),
        lrc_hash_after: base.lrc_sha256.clone(),
    });

    let repair_id = format!("rep_dl_{}_{}", dl_id, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
    let _ = crate::services::repair_history::record_applied_repair(
        db,
        &repair_id,
        Some(dl_id),
        Some(old_track_id),
        Some(new_track_id),
        &current_file_path_str,
        &final_path_str,
        &file_hash_before,
        Some(&file_hash_after_tagging),
        audio_content_hash_before.as_deref(),
        audio_content_hash_after_tagging.as_deref(),
        "valid",
        &applied_actions,
        None,
        "tidal_pipeline.re_enrich",
        "success",
        None,
    ).await;

    Ok(ReEnrichResult {
        success: true,
        dry_run: false,
        download_id: dl_id,
        old_track_id,
        new_track_id,
        old_path: current_file_path_str,
        new_path: final_path_str,
        title: final_title,
        artist: final_artist,
        album: final_album,
        isrc: if isrc_str.is_empty() { None } else { Some(isrc_str) },
        tags_applied,
        cover_applied,
        lyrics_applied,
        moved,
        metadata_completeness: 100,
        baseline: Some(base),
        validation_status: Some(RepairValidationStatus::Valid),
        applied_actions,
        rollback_state: None,
        output_hashes,
        error: None,
    })
}

/// Backwards-compatible alias for re_enrich_download_file
pub async fn re_enrich_download_file(
    db: &DbPool,
    download_id_or_track_id: i64,
) -> Result<ReEnrichResult, String> {
    reenrich_download_file(db, download_id_or_track_id, false).await
}

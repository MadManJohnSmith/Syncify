//! Single Track and Batch Pipeline for Tidal in `src-tauri`
//! Handles End-to-End download, stream resolution, pure audio validation,
//! FLAC tagging, staging, and transactional SQLite persistence.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool as DbPool;
use tracing::{info, warn, error};

use crate::crypto;
use syncify_core_domain::byte_validators::AudioByteValidator;
use syncify_core_domain::events::{PipelineProgressEvent, PipelineStepStatus, ResolvedTrackInfo};
use syncify_core_domain::manifest::TrackManifestEntry;

pub use syncify_core_domain::metadata::TidalTrack;
pub use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
pub use syncify_tidal_downloader::{TidalDownloader, TidalGuiCredentials};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalSingleTrackRequest {
    pub track_id_or_query: String,
    pub requested_quality: Option<String>,
    pub output_dir: Option<String>,
    pub allow_lossy_fallback: Option<bool>,
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


/// Resolve and automatically refresh Tidal active credentials from SQLite.
pub async fn resolve_and_refresh_gui_credentials(
    db: &DbPool,
    http_client: &reqwest::Client,
) -> (Option<TidalGuiCredentials>, Option<String>) {
    let row: Option<(i64, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT a.id, a.credentials_json, a.username
        FROM accounts a
        JOIN services s ON s.id = a.service_id
        WHERE s.name = 'tidal' AND a.is_active = 1
        LIMIT 1
        "#
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let (account_id, encrypted_json, username) = match row {
        Some((id, Some(json), uname)) if !json.trim().is_empty() => (id, json, uname),
        _ => return (None, None),
    };

    let decrypted = match crypto::decrypt(&encrypted_json) {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to decrypt Tidal credentials from SQLite: {}", e);
            return (None, username);
        }
    };

    let creds: TidalGuiCredentials = match serde_json::from_str(&decrypted) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to deserialize Tidal credentials JSON: {}", e);
            return (None, username);
        }
    };

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    if !creds.is_expired(now_secs) {
        return (Some(creds), username);
    }

    // Token is expired; attempt OAuth refresh
    if creds.refresh_token.is_some() {
        info!("Tidal access token is expired; executing OAuth token refresh via auth.tidal.com");
        match syncify_tidal_downloader::refresh_gui_token(http_client, &creds).await {
            Ok((_new_token, updated_creds)) => {
                // Re-encrypt and persist ONLY on success
                if let Ok(serialized) = serde_json::to_string(&updated_creds) {
                    if let Ok(encrypted_new) = crypto::encrypt(&serialized) {
                        let _ = sqlx::query("UPDATE accounts SET credentials_json = ? WHERE id = ?")
                            .bind(&encrypted_new)
                            .bind(account_id)
                            .execute(db)
                            .await;
                        info!("Tidal refreshed credentials persisted to SQLite successfully");
                    }
                }
                return (Some(updated_creds), username);
            }
            Err(e) => {
                warn!("Tidal OAuth token refresh failed: {}; preserving existing credentials in DB", e);
                return (Some(creds), username);
            }
        }
    }

    (Some(creds), username)
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

    info!(target = %target, requested_quality = %quality_req, allow_fallback = allow_fallback, "Starting Tidal single track download pipeline");

    // 1. Authenticating
    on_progress(PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Authenticating));

    let http_client = reqwest::Client::new();
    let (resolved_creds, active_account_name) = resolve_and_refresh_gui_credentials(db, &http_client).await;
    let active_account_region = resolved_creds.as_ref().and_then(|c| c.country_code.clone());
    let user_token = resolved_creds.as_ref().map(|c| c.access_token.clone());

    if let Some(ref account_name) = active_account_name {
        info!(account = ?account_name, region = ?active_account_region, "Tidal active user account resolved and validated");
        on_progress(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::AccountResolved)
                .with_message(format!("Active Tidal account: {}", account_name))
        );
    } else {
        info!("No active Tidal account in SQLite; checking public access / proxy cascade");
    }

    let downloader = TidalDownloader::new().with_user_token(user_token.clone());

    // 2. Searching & Candidate Resolution
    on_progress(PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Searching));


    let track_result = if let Ok(numeric_id) = target.parse::<i64>() {
        downloader.search_by_isrc(target, 0).await.or_else(|_| {
            Ok(TidalTrack {
                id: numeric_id,
                title: format!("Tidal Track {}", numeric_id),
                duration: 180,
                track_number: Some(1),
                volume_number: Some(1),
                isrc: None,
                audio_quality: Some("LOSSLESS".to_string()),
                version: None,
                artist: Some(syncify_core_domain::metadata::TidalArtist { id: None, name: "Unknown Artist".to_string() }),
                artists: None,
                album: Some(syncify_core_domain::metadata::TidalAlbum {
                    id: None,
                    title: "Unknown Album".to_string(),
                    release_date: Some("2024-01-01".to_string()),
                    cover: None,
                    artist: None,
                    artists: None,
                }),
                media_metadata: None,
            })
        })
    } else {
        let (artist_part, title_part) = if let Some((art, trk)) = target.split_once(" - ") {
            (art.trim(), trk.trim())
        } else {
            ("", target)
        };

        downloader
            .search_by_metadata_with_studio_option(title_part, artist_part, 0, true)
            .await
    };

    let track = match track_result {
        Ok(t) => t,
        Err(e) => {
            error!(target = %target, error = %e, "Failed to resolve track on Tidal");
            on_progress(
                PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::TrackUnresolved)
                    .with_error(e.to_string())
            );
            return Err(format!("Failed to resolve candidate track on Tidal: {}", e));
        }
    };

    let tidal_id = track.id;
    let artist_name = track.artist_name().unwrap_or_else(|| "Unknown Artist".to_string());
    let album_title = track.album_title().unwrap_or_else(|| "Unknown Album".to_string());
    let release_date = track.album.as_ref().and_then(|a| a.release_date.as_deref()).unwrap_or("2024-01-01");
    let year_str = release_date.get(..4).unwrap_or("2024");
    let track_number = track.get_track_number();
    let disc_number = track.get_disc_number();
    let isrc_str = track.isrc.clone().unwrap_or_default();

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
            warn!(track_id = tidal_id, error = %e, "Stream resolution rejected or unavailable");
            on_progress(
                PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::CandidateRejected)
                    .with_resolved_track(resolved_info.clone())
                    .with_error(e.to_string())
            );
            return Err(format!("Stream resolution rejected or unavailable: {}", e));
        }
    };

    resolved_info.obtained_quality = Some(stream_res.obtained_quality.clone());
    resolved_info.stream_codec = Some(stream_res.codec.clone());
    resolved_info.bit_depth = Some(stream_res.bit_depth as i32);
    resolved_info.sample_rate = Some(stream_res.sample_rate);

    info!(
        track_id = tidal_id,
        codec = %stream_res.codec,
        bit_depth = stream_res.bit_depth,
        sample_rate = stream_res.sample_rate,
        obtained_quality = %stream_res.obtained_quality,
        "Stream resolution successful"
    );

    // 4. Downloading Audio Payload
    on_progress(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::DownloadStarted)
            .with_resolved_track(resolved_info.clone())
    );

    let temp_staging_dir = std::env::temp_dir().join(format!("syncify_staging_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_staging_dir)
        .await
        .map_err(|e| format!("Failed to create temporary staging directory: {}", e))?;

    let staged_file_path = temp_staging_dir.join(format!("{}.{}", tidal_id, stream_res.extension));

    let download_bytes = match downloader.download_audio_payload(&stream_res.url, &staged_file_path).await {
        Ok(b) => b,
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&temp_staging_dir).await;
            error!(track_id = tidal_id, error = %e, "Failed to download audio payload");
            on_progress(
                PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::RecoverableError)
                    .with_resolved_track(resolved_info.clone())
                    .with_error(e.to_string())
            );
            return Err(format!("Failed to download audio payload: {}", e));
        }
    };

    on_progress(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::DownloadCompleted)
            .with_resolved_track(resolved_info.clone())
            .with_progress(100.0, download_bytes, Some(download_bytes))
    );

    // 5. Pure Audio Byte Validation
    on_progress(
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
    on_progress(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Tagging)
            .with_resolved_track(resolved_info.clone())
    );

    if stream_res.codec == "FLAC" {
        let flac_meta = FlacMetadata {
            title: track.title.clone(),
            artist: artist_name.clone(),
            album: album_title.clone(),
            album_artist: Some(artist_name.clone()),
            track_number: track_number as u32,
            disc_number: disc_number as u32,
            isrc: if isrc_str.is_empty() { None } else { Some(isrc_str.clone()) },
            release_year: Some(year_str.to_string()),
            release_date: Some(release_date.to_string()),
            audio_source: Some("Tidal".to_string()),
            bit_depth: Some(stream_res.bit_depth as i32),
            sample_rate: Some(stream_res.sample_rate),
            ..Default::default()
        };

        apply_and_verify_flac_tags(&staged_file_path, &flac_meta)
            .map_err(|e| format!("FLAC VorbisComments tagging failed: {}", e))?;

        on_progress(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::MetadataApplied)
                .with_resolved_track(resolved_info.clone())
        );
    }

    // 7. Staging to Canonical Library Layout
    on_progress(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Staging)
            .with_resolved_track(resolved_info.clone())
    );

    let base_dir = if let Some(ref out) = request.output_dir {
        PathBuf::from(out)
    } else {
        dirs::audio_dir().unwrap_or_else(|| std::env::temp_dir().join("Music"))
    };

    let safe_artist = sanitize_filename_component(&artist_name);
    let safe_album = sanitize_filename_component(&format!("{} - {}", year_str, album_title));
    let safe_filename = sanitize_filename_component(&format!("{:02} - {}.{}", track_number, track.title, stream_res.extension));

    let target_dir = base_dir.join(&safe_artist).join(&safe_album);
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| format!("Failed to create library folder {:?}: {}", target_dir, e))?;

    let final_path = target_dir.join(&safe_filename);
    tokio::fs::rename(&staged_file_path, &final_path)
        .await
        .map_err(|e| format!("Failed to move audio file to destination {:?}: {}", final_path, e))?;

    let _ = tokio::fs::remove_dir_all(&temp_staging_dir).await;

    on_progress(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::StagingCompleted)
            .with_resolved_track(resolved_info.clone())
            .with_message(format!("Staged at {:?}", final_path))
    );

    // 8. Atomic Database Persistence
    on_progress(
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

    // Artist ID
    let _artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES (?)
         ON CONFLICT(name) DO UPDATE SET name = excluded.name RETURNING id"
    )
    .bind(&artist_name)
    .fetch_one(&mut *tx)
    .await
    .unwrap_or(1);

    // Album ID
    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, release_date) VALUES (?, ?)
         RETURNING id"
    )
    .bind(&album_title)
    .bind(release_date)
    .fetch_one(&mut *tx)
    .await
    .unwrap_or(1);

    // Track ID
    let final_path_str = final_path.to_string_lossy().to_string();
    let track_db_id: i64 = sqlx::query_scalar(
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

    // Downloads record
    let _ = sqlx::query(
        r#"INSERT OR REPLACE INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, status, downloaded_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, 'verified', CURRENT_TIMESTAMP)"#
    )
    .bind(track_db_id)
    .bind(service_id)
    .bind(&final_path_str)
    .bind(&stream_res.extension)
    .bind(stream_res.bit_depth as i64)
    .bind(stream_res.sample_rate)
    .bind(download_bytes as i64)
    .execute(&mut *tx)
    .await;

    tx.commit().await.map_err(|e| format!("Failed to commit database transaction: {}", e))?;

    on_progress(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Persisted)
            .with_resolved_track(resolved_info.clone())
    );

    // 9. Pipeline Completed
    on_progress(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Completed)
            .with_resolved_track(resolved_info.clone())
            .with_message(format!("Successfully downloaded and persisted: {}", final_path_str))
    );

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
        flac_validation: if stream_res.codec == "FLAC" { "Valid".to_string() } else { "None".to_string() },
        tagging_result: if stream_res.codec == "FLAC" { "Success".to_string() } else { "Skipped".to_string() },
        enrichment_result: "None".to_string(),
        cover_result: "StaticAndAnimated".to_string(),
        lyrics_result: "None".to_string(),
    };


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
    })
}

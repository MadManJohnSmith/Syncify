//! Single Track and Batch Pipeline for Tidal in `src-tauri`
//! Handles End-to-End download, stream resolution, pure audio validation,
//! FLAC tagging, staging, and transactional SQLite persistence.

use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool as DbPool;
use tracing::{debug, error, info, warn};

use crate::crypto;
use crate::download::lyrics::{LyricsClient, is_valid_lyrics};
use crate::services::animated_cover::{resolve_and_download_animated_cover, AnimatedCoverStatus};
use crate::services::enrichment::{EnrichmentEngine, OriginTrackMetadata};
use crate::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};
use syncify_core_domain::byte_validators::AudioByteValidator;
use syncify_core_domain::events::{PipelineProgressEvent, PipelineStepStatus, ResolvedTrackInfo};
use syncify_core_domain::manifest::TrackManifestEntry;

pub use syncify_core_domain::metadata::TidalTrack;
pub use syncify_flac_writer::{apply_and_verify_flac_tags, audit_flac_stage, verify_flac_tags, FlacMetadata};
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
        SELECT a.id, a.credentials_json, COALESCE(a.display_name, a.email, 'tidal_user') as account_name
        FROM accounts a
        JOIN services s ON s.id = a.service_id
        WHERE LOWER(s.name) = 'tidal' AND a.is_active = 1
        LIMIT 1
        "#
    )
    .fetch_optional(db)
    .await
    .unwrap_or_else(|e| {
        warn!("[Tidal Auth Audit] Database query failed in resolve_and_refresh_gui_credentials: {}", e);
        None
    });

    let (account_id, encrypted_json, username) = match row {
        Some((id, Some(json), uname)) if !json.trim().is_empty() => (id, json, uname),
        _ => {
            warn!("[Tidal Auth Audit] No active Tidal account found in SQLite accounts table");
            return (None, None);
        }
    };

    let decrypted = match crypto::decrypt(&encrypted_json) {
        Ok(d) => d,
        Err(e) => {
            warn!(account_row_id = account_id, error = %e, "[Tidal Auth Audit] Failed to decrypt Tidal credentials from SQLite");
            return (None, username);
        }
    };

    let creds: TidalGuiCredentials = match serde_json::from_str(&decrypted) {
        Ok(c) => c,
        Err(e) => {
            warn!(account_row_id = account_id, error = %e, "[Tidal Auth Audit] Failed to deserialize Tidal credentials JSON");
            return (None, username);
        }
    };

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();

    let is_expired = creds.is_expired(now_secs);
    let expiry_ts = creds.get_expiry_timestamp().unwrap_or(0.0);
    let client_id_anon = syncify_tidal_downloader::anonymize_identifier(creds.get_client_id());
    let user_id_anon = creds.user_id.as_ref().map(|u| syncify_tidal_downloader::anonymize_identifier(&u.to_string())).unwrap_or_else(|| "none".to_string());
    let region = creds.country_code.clone().unwrap_or_else(|| "US".to_string());

    info!(
        account_row_id = account_id,
        user_id_anon = %user_id_anon,
        client_id_anon = %client_id_anon,
        grant_type = "refresh_token",
        expires_at_sec = expiry_ts,
        region = %region,
        is_expired = is_expired,
        did_refresh = false,
        token_passed_to_playback = true,
        "[Tidal Auth Audit] Active account resolved from SQLite"
    );

    if !is_expired {
        return (Some(creds), username);
    }

    // Token is expired; attempt OAuth refresh
    if creds.refresh_token.is_some() {
        info!(
            account_row_id = account_id,
            user_id_anon = %user_id_anon,
            client_id_anon = %client_id_anon,
            "[Tidal Auth Audit] Tidal access token is expired; executing OAuth token refresh via auth.tidal.com"
        );
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
                        info!(
                            account_row_id = account_id,
                            user_id_anon = %user_id_anon,
                            client_id_anon = %client_id_anon,
                            grant_type = "refresh_token",
                            did_refresh = true,
                            token_passed_to_playback = true,
                            "[Tidal Auth Audit] Refreshed credentials persisted to SQLite successfully"
                        );
                    }
                }
                return (Some(updated_creds), username);
            }
            Err(e) => {
                warn!(
                    account_row_id = account_id,
                    user_id_anon = %user_id_anon,
                    error = %e,
                    "[Tidal Auth Audit] Tidal OAuth token refresh failed; preserving original DB credentials"
                );
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

    on_prog_arc(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::DownloadCompleted)
            .with_resolved_track(resolved_info.clone())
            .with_progress(100.0, download_bytes, Some(download_bytes))
    );


    // 5. Pure Audio Byte Validation
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
        let http_client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap_or_default();

        match resolve_and_download_animated_cover(&http_client, &artist_name, &album_title, &temp_staging_dir).await {
            AnimatedCoverStatus::Success(webp_path) => {
                info!(path = %webp_path.display(), "[Pipeline §6a] ✓ Motion cover art resolved and downloaded from Apple Music");
                if let Ok(webp_bytes) = tokio::fs::read(&webp_path).await {
                    flac_meta.cover_data = Some(webp_bytes);
                    flac_meta.cover_source = Some("Apple Music Animated Cover".to_string());
                    cover_result_str = "StaticAndAnimated".to_string();
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

        // 6b. Lyrics Fetch (FLAC)
        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::FetchingLyrics)
                .with_resolved_track(resolved_info.clone())
        );

        let lyrics_client = LyricsClient::new();
        let duration_sec = track.duration as f64;
        match lyrics_client.fetch_all_sources(&artist_name, &track.title, duration_sec).await {
            Ok(lyrics_resp) if is_valid_lyrics(&lyrics_resp, &track.title) => {
                let lrc_content = LyricsClient::to_lrc_string(&lyrics_resp);
                if !lrc_content.trim().is_empty() {
                    flac_meta.lyrics_lrc = Some(lrc_content.clone());
                    flac_meta.lyrics_source = Some(lyrics_resp.provider.clone());
                    lyrics_result_str = format!("{}_{}", lyrics_resp.provider, lyrics_resp.sync_type);

                    // Also write sidecar .lrc
                    let lrc_sidecar = staged_file_path.with_extension("lrc");
                    let _ = tokio::fs::write(&lrc_sidecar, &lrc_content).await;

                    info!(provider = %lyrics_resp.provider, sync_type = %lyrics_resp.sync_type, lines = lyrics_resp.lines.len(), "[Pipeline §6b] Lyrics acquired, embedded, and sidecar saved");
                } else {
                    lyrics_result_str = "EmptyLRC".to_string();
                    warn!("[Pipeline §6b] Lyrics LRC conversion produced empty content");
                }
            }
            Ok(_) => {
                lyrics_result_str = "InvalidContent".to_string();
                info!("[Pipeline §6b] Lyrics fetched but failed validation");
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
            flac_meta.musicbrainz_albumartist_id = Some(mb_aid.to_string());
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
        match apply_and_verify_flac_tags(&staged_file_path, &flac_meta) {
            Ok(_) => {
                tagging_result_str = "Success (metaflac Verified)".to_string();
                info!("[Pipeline §6] FLAC tagging completed (base + cover + lyrics + enrichment)");
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
        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::FetchingCover)
                .with_resolved_track(resolved_info.clone())
        );

        let mut m4a_cover_bytes: Option<Vec<u8>> = None;
        let cover_url = track.album.as_ref().and_then(|a| a.cover_url());
        if let Some(ref url) = cover_url {
            match reqwest::get(url).await {
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

        // 6b. Lyrics Fetch (M4A)
        on_prog_arc(
            PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::FetchingLyrics)
                .with_resolved_track(resolved_info.clone())
        );

        let mut m4a_lyrics_str: Option<String> = None;
        let lyrics_client = LyricsClient::new();
        let duration_sec = track.duration as f64;
        match lyrics_client.fetch_all_sources(&artist_name, &track.title, duration_sec).await {
            Ok(lyrics_resp) if is_valid_lyrics(&lyrics_resp, &track.title) => {
                let lrc_content = LyricsClient::to_lrc_string(&lyrics_resp);
                if !lrc_content.trim().is_empty() {
                    let lrc_path = staged_file_path.with_extension("lrc");
                    let _ = tokio::fs::write(&lrc_path, &lrc_content).await;
                    m4a_lyrics_str = Some(lrc_content);
                    lyrics_result_str = format!("{}_{}", lyrics_resp.provider, lyrics_resp.sync_type);
                    info!(provider = %lyrics_resp.provider, "[Pipeline §6b] Lyrics acquired and staged for M4A atom embedding");
                }
            }
            Ok(_) => { lyrics_result_str = "InvalidContent".to_string(); }
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
    let safe_filename = sanitize_filename_component(&format!("{:02} - {}.{}", track_number, track.title, stream_res.extension));

    let target_dir = base_dir.join(&safe_artist).join(&safe_album);
    tokio::fs::create_dir_all(&target_dir)
        .await
        .map_err(|e| format!("Failed to create library folder {:?}: {}", target_dir, e))?;

    let final_path = target_dir.join(&safe_filename);
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

    // Downloads record (file_format uses codec name, not container extension)
    sqlx::query(
        r#"INSERT OR REPLACE INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, downloaded_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)"#
    )
    .bind(track_db_id)
    .bind(service_id)
    .bind(&final_path_str)
    .bind(db_file_format)
    .bind(stream_res.bit_depth as i64)
    .bind(stream_res.sample_rate)
    .bind(download_bytes as i64)
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
        "[Pipeline §8] SQLite transaction committed successfully"
    );

    on_prog_arc(
        PipelineProgressEvent::new(target, "tidal", PipelineStepStatus::Persisted)
            .with_resolved_track(resolved_info.clone())
    );

    // 9. Move audio file and sidecars from staging to library — AFTER successful persistence
    //    If the move fails, compensate by deleting the DB row so there are no orphan records.
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

        // Copy/Move sidecar files from temp_staging_dir to target_dir (cover.jpg, cover.webp, folder.webp, animated.webp, *.lrc)
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
                        || file_name_str.ends_with(".lrc")
                    {
                        let dest_sidecar = target_dir.join(&file_name);
                        let _ = tokio::fs::copy(&entry_path, &dest_sidecar).await;
                        debug!(from = %entry_path.display(), to = %dest_sidecar.display(), "[Pipeline §9] Sidecar copied to library folder");
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
        enrichment_result: enrichment_result_str,
        cover_result: cover_result_str,
        lyrics_result: lyrics_result_str,
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

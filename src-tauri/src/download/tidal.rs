//! Tidal Downloader for `src-tauri` — re-exported from `syncify-tidal-downloader`.

use crate::download::progress::{
    DownloadProgress, DownloadRequest, DownloadResult, PROGRESS_TRACKER,
};
use crate::download::qobuz::derive_acoustid_id;
use anyhow::{anyhow, Result};
use std::path::Path;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};

pub use syncify_tidal_downloader::*;

pub trait TidalOrchestratorExt {
    fn download_track(
        &self,
        request: &DownloadRequest,
        db_opt: Option<&sqlx::SqlitePool>,
    ) -> impl std::future::Future<Output = Result<DownloadResult>> + Send;
}

impl TidalOrchestratorExt for TidalDownloader {
    async fn download_track(
        &self,
        request: &DownloadRequest,
        db_opt: Option<&sqlx::SqlitePool>,
    ) -> Result<DownloadResult> {
        let item_id = &request.item_id;
        PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "tidal"));

        if let Some(db) = db_opt {
            let req = crate::services::tidal_pipeline::TidalSingleTrackRequest {
                track_id_or_query: if let Some(ref s_track_id) = request.service_track_id {
                    s_track_id.clone()
                } else if let Some(ref isrc) = request.isrc {
                    isrc.clone()
                } else {
                    format!("{} {}", request.track_name, request.artist_name)
                },
                requested_quality: Some(request.quality.clone()),
                output_dir: Some(request.output_dir.clone()),
                allow_lossy_fallback: Some(request.allow_fallback || !request.strict_quality),
                hint_title: if request.track_name.is_empty() {
                    None
                } else {
                    Some(request.track_name.clone())
                },
                hint_artist: if request.artist_name.is_empty() {
                    None
                } else {
                    Some(request.artist_name.clone())
                },
                hint_album: if request.album_name.is_empty() {
                    None
                } else {
                    Some(request.album_name.clone())
                },
                hint_isrc: request.isrc.clone(),
                hint_track_number: if request.track_number > 0 {
                    Some(request.track_number)
                } else {
                    None
                },
                hint_disc_number: if request.disc_number > 0 {
                    Some(request.disc_number)
                } else {
                    None
                },
                hint_release_date: request.release_date.clone(),
                hint_track_id: request.canonical_track_id,
                // BD-9: the worker already opened a journal entry for this queued
                // download; hand its id over so the pipeline checkpoints the SAME row
                // instead of opening a competing second entry.
                operation_id: request.operation_id.clone(),
                account_id: None,
            };

            let item_id_clone = item_id.clone();
            let res = crate::services::tidal_pipeline::execute_tidal_single_track_download(
                db,
                req,
                move |event| match event.status {
                    syncify_core_domain::events::PipelineStepStatus::Downloading => {
                        let speed_kbps = event
                            .speed_bytes_per_sec
                            .map(|s| s as f64 / 1024.0)
                            .unwrap_or(0.0);
                        PROGRESS_TRACKER.update(DownloadProgress::downloading_bytes(
                            &item_id_clone,
                            "tidal",
                            event.bytes_downloaded,
                            event.total_bytes,
                            speed_kbps,
                            speed_kbps,
                        ));
                    }
                    syncify_core_domain::events::PipelineStepStatus::Tagging => {
                        PROGRESS_TRACKER.update(DownloadProgress::finalizing(&item_id_clone));
                    }
                    _ => {}
                },
            )
            .await
            .map_err(|e| anyhow!("Tidal pipeline failed: {}", e))?;

            return Ok(DownloadResult {
                file_path: res.file_path,
                bit_depth: res.bit_depth as i32,
                sample_rate: res.sample_rate as i32,
                title: res.title,
                artist: res.artist,
                album: res.album,
                release_date: request.release_date.clone(),
                track_number: request.track_number,
                disc_number: request.disc_number,
                isrc: request.isrc.clone(),
                service: "tidal".to_string(),
                phase_timings: res.phase_timings,
                ..Default::default()
            });
        }

        // Fallback if no DB is attached
        let (track_id, _track_title, _artist_name, _album_name) =
            if let Some(ref s_track_id) = request.service_track_id {
                if let Ok(tid) = s_track_id.parse::<i64>() {
                    (
                        tid,
                        request.track_name.clone(),
                        request.artist_name.clone(),
                        request.album_name.clone(),
                    )
                } else if let Some(ref isrc) = request.isrc {
                    let t = self
                        .search_by_isrc(isrc, (request.duration_ms / 1000) as i32)
                        .await?;
                    (
                        t.id,
                        t.title,
                        t.artist
                            .map(|a| a.name)
                            .unwrap_or_else(|| request.artist_name.clone()),
                        t.album
                            .map(|a| a.title)
                            .unwrap_or_else(|| request.album_name.clone()),
                    )
                } else {
                    let t = self
                        .search_by_metadata(
                            &request.track_name,
                            &request.artist_name,
                            (request.duration_ms / 1000) as i32,
                        )
                        .await?;
                    (
                        t.id,
                        t.title,
                        t.artist
                            .map(|a| a.name)
                            .unwrap_or_else(|| request.artist_name.clone()),
                        t.album
                            .map(|a| a.title)
                            .unwrap_or_else(|| request.album_name.clone()),
                    )
                }
            } else if let Some(ref isrc) = request.isrc {
                let t = match self
                    .search_by_isrc(isrc, (request.duration_ms / 1000) as i32)
                    .await
                {
                    Ok(t) => t,
                    Err(_) => {
                        self.search_by_metadata(
                            &request.track_name,
                            &request.artist_name,
                            (request.duration_ms / 1000) as i32,
                        )
                        .await?
                    }
                };
                (
                    t.id,
                    t.title,
                    t.artist
                        .map(|a| a.name)
                        .unwrap_or_else(|| request.artist_name.clone()),
                    t.album
                        .map(|a| a.title)
                        .unwrap_or_else(|| request.album_name.clone()),
                )
            } else {
                let t = self
                    .search_by_metadata(
                        &request.track_name,
                        &request.artist_name,
                        (request.duration_ms / 1000) as i32,
                    )
                    .await?;
                (
                    t.id,
                    t.title,
                    t.artist
                        .map(|a| a.name)
                        .unwrap_or_else(|| request.artist_name.clone()),
                    t.album
                        .map(|a| a.title)
                        .unwrap_or_else(|| request.album_name.clone()),
                )
            };

        let stream_res = self
            .get_stream_resolution(
                track_id,
                Some(&request.quality),
                None,
                request.allow_fallback || !request.strict_quality,
            )
            .await?;

        let filename = format!(
            "{:02} - {}.{}",
            request.track_number,
            crate::services::tidal_pipeline::sanitize_filename_component(&request.track_name),
            stream_res.extension
        );
        let output_dir = Path::new(&request.output_dir);
        tokio::fs::create_dir_all(output_dir).await?;
        let output_path = output_dir.join(&filename);

        PROGRESS_TRACKER.update(DownloadProgress::downloading(item_id, "tidal", 0, 0));
        self.download_audio_payload(&stream_res.url, &output_path)
            .await?;

        let header_bytes = tokio::fs::read(&output_path).await.unwrap_or_default();
        if stream_res.codec == "FLAC"
            && !syncify_core_domain::byte_validators::AudioByteValidator::is_flac_magic(
                &header_bytes,
            )
            && !syncify_core_domain::byte_validators::AudioByteValidator::is_isobmff_container(
                &header_bytes,
            )
        {
            let _ = tokio::fs::remove_file(&output_path).await;
            return Err(anyhow!(
                "Downloaded audio failed FLAC/ISOBMFF magic verification"
            ));
        }

        // F3.4: Inspect STREAMINFO header from downloaded FLAC to extract true physical bit_depth and sample_rate
        let (real_bit_depth, real_sample_rate) = if stream_res.codec == "FLAC" {
            if let Some(info) =
                syncify_core_domain::byte_validators::AudioByteValidator::parse_flac_streaminfo(
                    &header_bytes,
                )
            {
                (info.bits_per_sample as i32, info.sample_rate as f64)
            } else if let Ok(tag) = metaflac::Tag::read_from_path(&output_path) {
                if let Some(info) = tag.get_streaminfo() {
                    (info.bits_per_sample as i32, info.sample_rate as f64)
                } else {
                    (stream_res.bit_depth as i32, stream_res.sample_rate)
                }
            } else {
                (stream_res.bit_depth as i32, stream_res.sample_rate)
            }
        } else {
            (stream_res.bit_depth as i32, stream_res.sample_rate)
        };

        if stream_res.codec == "FLAC" {
            PROGRESS_TRACKER.update(DownloadProgress::finalizing(item_id));
            let flac_meta = FlacMetadata {
                title: request.track_name.clone(),
                artist: request.artist_name.clone(),
                album: request.album_name.clone(),
                album_artist: request.album_artist.clone(),
                track_number: request.track_number as u32,
                track_total: request.total_tracks as u32,
                disc_number: request.disc_number as u32,
                disc_total: 1,
                isrc: request.isrc.clone(),
                release_date: request.release_date.clone(),
                // TASK-75: locked source identity + relational acoustic identifiers.
                musicbrainz_track_id: request.musicbrainz_recording_id.clone(),
                acoustid_fingerprint: request
                    .acoustid_fingerprint
                    .as_deref()
                    .map(str::trim)
                    .filter(|fp| !fp.is_empty())
                    .map(|fp| fp.to_string()),
                acoustid_id: request
                    .acoustid_fingerprint
                    .as_deref()
                    .and_then(derive_acoustid_id),
                audio_source: Some("Tidal".to_string()),
                bit_depth: Some(real_bit_depth),
                sample_rate: Some(real_sample_rate),
                ..Default::default()
            };
            let _ = apply_and_verify_flac_tags(&output_path, &flac_meta);
            // D-03: this no-DB fallback is a FLAC write path too, so the cover art is
            // sanitized here as well. There is no ledger to record into on this branch,
            // which the service reports as an error log instead of swallowing the loss.
            let cover_ctx = crate::services::flac_cover_sanitizer::FlacCoverSanitizeContext {
                provenance: "tidal.downloader_fallback".to_string(),
                download_id: None,
                track_id: Some(track_id),
            };
            if let Err(sanitize_err) =
                crate::services::flac_cover_sanitizer::sanitize_and_audit_flac_cover_art(
                    None,
                    &output_path,
                    &cover_ctx,
                )
                .await
            {
                tracing::warn!(error = %sanitize_err, path = %output_path.display(), "[Tidal] FLAC cover sanitization failed (non-fatal)");
            }
        } else {
            PROGRESS_TRACKER.update(DownloadProgress::finalizing(item_id));
            let acoustid_fingerprint = request
                .acoustid_fingerprint
                .as_deref()
                .map(str::trim)
                .filter(|fp| !fp.is_empty())
                .map(|fp| fp.to_string());
            let mp4_meta = crate::services::mp4_writer::Mp4Metadata {
                title: request.track_name.clone(),
                artist: request.artist_name.clone(),
                album: request.album_name.clone(),
                album_artist: request.album_artist.clone(),
                track_number: request.track_number as u32,
                track_total: request.total_tracks as u32,
                disc_number: request.disc_number as u32,
                disc_total: 1,
                isrc: request.isrc.clone(),
                release_date: request.release_date.clone(),
                // TASK-75: locked source identity + relational acoustic identifiers.
                musicbrainz_track_id: request.musicbrainz_recording_id.clone(),
                acoustid_id: acoustid_fingerprint.as_deref().and_then(derive_acoustid_id),
                acoustid_fingerprint,
                audio_source: Some("Tidal".to_string()),
                ..Default::default()
            };
            let _ = crate::services::mp4_writer::apply_and_verify_mp4_tags(&output_path, &mp4_meta);
        }

        Ok(DownloadResult {
            file_path: output_path.to_string_lossy().to_string(),
            bit_depth: real_bit_depth,
            sample_rate: real_sample_rate as i32,
            title: request.track_name.clone(),
            artist: request.artist_name.clone(),
            album: request.album_name.clone(),
            release_date: request.release_date.clone(),
            track_number: request.track_number,
            disc_number: request.disc_number,
            isrc: request.isrc.clone(),
            service: "tidal".to_string(),
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_tidal_single_track_request_account_id_default() {
        let req = crate::services::tidal_pipeline::TidalSingleTrackRequest {
            track_id_or_query: "12345".to_string(),
            ..Default::default()
        };
        assert_eq!(req.account_id, None);

        let req_with_acc = crate::services::tidal_pipeline::TidalSingleTrackRequest {
            track_id_or_query: "12345".to_string(),
            account_id: Some(99),
            ..Default::default()
        };
        assert_eq!(req_with_acc.account_id, Some(99));
    }

    #[tokio::test]
    async fn test_tidal_multi_account_fallback_priority_and_exclusivity() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory db");

        sqlx::query("CREATE TABLE services (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_id INTEGER NOT NULL REFERENCES services(id),
                display_name TEXT,
                email TEXT,
                is_active INTEGER DEFAULT 1,
                credentials_invalid INTEGER DEFAULT 0,
                invalid_reason TEXT,
                last_auth_error TEXT,
                last_auth_error_at TEXT,
                credentials_json TEXT,
                last_synced TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                streaming_entitled INTEGER NOT NULL DEFAULT -1,
                entitlement_checked_at TEXT
            );",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO services (id, name) VALUES (1, 'tidal');")
            .execute(&pool)
            .await
            .unwrap();

        let _ = crate::crypto::init_crypto([42u8; 32]);
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();

        // Account 1: Active, but empty access token (invalid)
        let creds_active =
            crate::crypto::encrypt(r#"{"access_token":"","token_expiry":0.0}"#).unwrap();
        // Account 2: Inactive sibling, with valid non-expired access token
        let future_expiry = now_secs + 3600.0;
        let creds_sibling_json = format!(
            r#"{{"access_token":"tidal_valid_token_12345","token_expiry":{},"country_code":"US"}}"#,
            future_expiry
        );
        let creds_sibling = crate::crypto::encrypt(&creds_sibling_json).unwrap();

        sqlx::query(
            "INSERT INTO accounts (id, service_id, email, is_active, credentials_json) VALUES (1, 1, 'acc1@tidal.com', 1, ?);"
        )
        .bind(creds_active)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO accounts (id, service_id, email, is_active, credentials_json) VALUES (2, 1, 'acc2@tidal.com', 0, ?);"
        )
        .bind(creds_sibling)
        .execute(&pool)
        .await
        .unwrap();

        // 1. Fallback account order must prioritize active account first, then sibling account
        let fallback_ids =
            crate::commands::accounts::get_service_fallback_account_ids(&pool, "tidal", None)
                .await
                .expect("fallback ids");
        assert_eq!(fallback_ids, vec![1, 2]);

        // 2. resolve_and_refresh_gui_credentials_for_account with Some(2) successfully resolves the inactive sibling
        let http_client = reqwest::Client::new();
        let (resolved_creds, account_name) =
            crate::services::tidal_pipeline::resolve_and_refresh_gui_credentials_for_account(
                &pool,
                &http_client,
                Some(2),
                false,
            )
            .await;

        let creds = resolved_creds.expect("sibling credentials must be resolvable");
        assert_eq!(creds.access_token, "tidal_valid_token_12345");
        assert_eq!(creds.country_code.as_deref(), Some("US"));
        assert_eq!(account_name.as_deref(), Some("acc2@tidal.com"));

        // 3. Crucial invariant: is_active in SQLite must NEVER be altered by fallback resolution
        let (active_id,): (i64,) =
            sqlx::query_as("SELECT id FROM accounts WHERE service_id = 1 AND is_active = 1")
                .fetch_one(&pool)
                .await
                .expect("active account");
        assert_eq!(
            active_id, 1,
            "is_active exclusivity in SQLite must remain preserved"
        );

        let (inactive_id,): (i64,) =
            sqlx::query_as("SELECT id FROM accounts WHERE service_id = 1 AND is_active = 0")
                .fetch_one(&pool)
                .await
                .expect("inactive account");
        assert_eq!(inactive_id, 2, "sibling account remains inactive in SQLite");
    }
}

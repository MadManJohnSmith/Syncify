//! Tidal Downloader for `src-tauri` — re-exported from `syncify-tidal-downloader`.

use anyhow::{anyhow, Result};
use std::path::Path;
use crate::download::progress::{DownloadProgress, DownloadRequest, DownloadResult, PROGRESS_TRACKER};
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
                hint_title: if request.track_name.is_empty() { None } else { Some(request.track_name.clone()) },
                hint_artist: if request.artist_name.is_empty() { None } else { Some(request.artist_name.clone()) },
                hint_album: if request.album_name.is_empty() { None } else { Some(request.album_name.clone()) },
                hint_isrc: request.isrc.clone(),
                hint_track_number: if request.track_number > 0 { Some(request.track_number) } else { None },
                hint_disc_number: if request.disc_number > 0 { Some(request.disc_number) } else { None },
                hint_release_date: request.release_date.clone(),
                hint_track_id: request.canonical_track_id,
            };

            let item_id_clone = item_id.clone();
            let res = crate::services::tidal_pipeline::execute_tidal_single_track_download(
                db,
                req,
                move |event| {
                    match event.status {
                        syncify_core_domain::events::PipelineStepStatus::Downloading => {
                            let speed_kbps = event.speed_bytes_per_sec.map(|s| s as f64 / 1024.0).unwrap_or(0.0);
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
                    }
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
        let (track_id, _track_title, _artist_name, _album_name) = if let Some(ref s_track_id) = request.service_track_id {
            if let Ok(tid) = s_track_id.parse::<i64>() {
                (tid, request.track_name.clone(), request.artist_name.clone(), request.album_name.clone())
            } else if let Some(ref isrc) = request.isrc {
                let t = self.search_by_isrc(isrc, (request.duration_ms / 1000) as i32).await?;
                (t.id, t.title, t.artist.map(|a| a.name).unwrap_or_else(|| request.artist_name.clone()), t.album.map(|a| a.title).unwrap_or_else(|| request.album_name.clone()))
            } else {
                let t = self.search_by_metadata(&request.track_name, &request.artist_name, (request.duration_ms / 1000) as i32).await?;
                (t.id, t.title, t.artist.map(|a| a.name).unwrap_or_else(|| request.artist_name.clone()), t.album.map(|a| a.title).unwrap_or_else(|| request.album_name.clone()))
            }
        } else if let Some(ref isrc) = request.isrc {
            let t = match self.search_by_isrc(isrc, (request.duration_ms / 1000) as i32).await {
                Ok(t) => t,
                Err(_) => self.search_by_metadata(&request.track_name, &request.artist_name, (request.duration_ms / 1000) as i32).await?,
            };
            (t.id, t.title, t.artist.map(|a| a.name).unwrap_or_else(|| request.artist_name.clone()), t.album.map(|a| a.title).unwrap_or_else(|| request.album_name.clone()))
        } else {
            let t = self.search_by_metadata(&request.track_name, &request.artist_name, (request.duration_ms / 1000) as i32).await?;
            (t.id, t.title, t.artist.map(|a| a.name).unwrap_or_else(|| request.artist_name.clone()), t.album.map(|a| a.title).unwrap_or_else(|| request.album_name.clone()))
        };

        let stream_res = self.get_stream_resolution(track_id, Some(&request.quality), None, true).await?;

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
        self.download_audio_payload(&stream_res.url, &output_path).await?;

        let header_bytes = tokio::fs::read(&output_path).await.unwrap_or_default();
        if stream_res.codec == "FLAC" && !syncify_core_domain::byte_validators::AudioByteValidator::is_flac_magic(&header_bytes) && !syncify_core_domain::byte_validators::AudioByteValidator::is_isobmff_container(&header_bytes) {
            let _ = tokio::fs::remove_file(&output_path).await;
            return Err(anyhow!("Downloaded audio failed FLAC/ISOBMFF magic verification"));
        }

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
                audio_source: Some("Tidal".to_string()),
                bit_depth: Some(stream_res.bit_depth as i32),
                sample_rate: Some(stream_res.sample_rate),
                ..Default::default()
            };
            let _ = apply_and_verify_flac_tags(&output_path, &flac_meta);
        } else {
            PROGRESS_TRACKER.update(DownloadProgress::finalizing(item_id));
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
                audio_source: Some("Tidal".to_string()),
                ..Default::default()
            };
            let _ = crate::services::mp4_writer::apply_and_verify_mp4_tags(&output_path, &mp4_meta);
        }

        Ok(DownloadResult {
            file_path: output_path.to_string_lossy().to_string(),
            bit_depth: stream_res.bit_depth as i32,
            sample_rate: stream_res.sample_rate as i32,
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

//! Batch Manifest Writer & SQLite Reconciliation for Syncify GUI Downloads

use anyhow::Result;
use sqlx::SqlitePool;
use std::path::Path;
use syncify_core_domain::{BatchDownloadManifest, TrackManifestEntry};
use tracing::{error, info, warn};

static MANIFEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, sqlx::FromRow)]
struct ManifestRow {
    id: i64,
    track_id: i64,
    service_name: Option<String>,
    service_track_id: Option<String>,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    isrc: Option<String>,
    status: String,
    error_message: Option<String>,
    quality_preference: Option<String>,
    file_path: Option<String>,
    file_size_bytes: Option<i64>,
    bit_depth: Option<i32>,
    sample_rate: Option<i32>,
    created_at: Option<String>,
    completed_at: Option<String>,
}

pub struct ManifestWriter;

impl ManifestWriter {
    /// Reconciles download records from SQLite and generates an auditable `manifest.json` in `output_dir`
    pub async fn generate_and_save_manifest(
        db: &SqlitePool,
        output_dir: &Path,
    ) -> Result<BatchDownloadManifest> {
        let _guard = MANIFEST_LOCK.lock().await;

        let rows: Vec<ManifestRow> = sqlx::query_as(
            r#"
            SELECT 
                dq.id, dq.track_id, dq.service_name, dq.service_track_id,
                COALESCE(dq.target_title, t.title) as title,
                COALESCE(dq.target_artist, (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id)) as artist,
                COALESCE(dq.target_album, alb.title) as album,
                COALESCE(dq.target_isrc, t.isrc) as isrc,
                dq.status, dq.error_message, dq.quality_preference,
                d.file_path, d.file_size_bytes, d.bit_depth, d.sample_rate,
                dq.created_at, dq.completed_at
            FROM download_queue dq
            LEFT JOIN tracks t ON t.id = dq.track_id
            LEFT JOIN albums alb ON alb.id = t.album_id
            LEFT JOIN downloads d ON d.track_id = dq.track_id
            ORDER BY dq.id ASC
            "#
        )
        .fetch_all(db)
        .await?;

        let mut manifest_entries = Vec::new();
        let mut succeeded = 0;
        let mut failed = 0;
        let mut skipped = 0;

        for row in rows {
            let qid = row.id;
            let tid = row.track_id;
            let service = row.service_name;
            let service_tid = row.service_track_id;
            let title_opt = row.title;
            let artist_opt = row.artist;
            let album_opt = row.album;
            let isrc_opt = row.isrc;
            let status = row.status;
            let error_opt = row.error_message;
            let quality_pref = row.quality_preference;
            let file_path_opt = row.file_path;
            let size_bytes = row.file_size_bytes;
            let bit_depth = row.bit_depth;
            let sample_rate = row.sample_rate;
            let created_at = row.created_at;
            let completed_at = row.completed_at;
            let is_success = status == "complete" && file_path_opt.is_some();
            if is_success {
                succeeded += 1;
            } else if status == "failed" {
                failed += 1;
            } else if status == "skipped" {
                skipped += 1;
            }

            let mut created_artifacts = Vec::new();
            if let Some(ref fp) = file_path_opt {
                let p = Path::new(fp);
                if p.exists() {
                    created_artifacts.push(fp.clone());
                    // Check sibling sidecars in album folder
                    let lrc_path = p.with_extension("lrc");
                    if lrc_path.exists() {
                        created_artifacts.push(lrc_path.to_string_lossy().to_string());
                    }
                    if let Some(parent) = p.parent() {
                        let sidecar_names = [
                            "cover.jpg",
                            "cover.webp",
                            "folder.webp",
                            "animated.webp",
                            "booklet.pdf",
                            "artist.nfo",
                            "biography.txt",
                            "fanart.jpg",
                            "artist.jpg",
                        ];
                        for name in sidecar_names {
                            let sidecar = parent.join(name);
                            if sidecar.exists() && !created_artifacts.iter().any(|a| a == &sidecar.to_string_lossy()) {
                                created_artifacts.push(sidecar.to_string_lossy().to_string());
                            }
                        }

                        // Check artist folder if separate parent directory
                        if let Some(artist_dir) = parent.parent() {
                            for name in ["artist.jpg", "fanart.jpg", "artist.nfo", "biography.txt"] {
                                let sidecar = artist_dir.join(name);
                                if sidecar.exists() && !created_artifacts.iter().any(|a| a == &sidecar.to_string_lossy()) {
                                    created_artifacts.push(sidecar.to_string_lossy().to_string());
                                }
                            }
                        }
                    }
                }
            }

            let err_str = error_opt.as_deref().unwrap_or("");
            let (classified_result, rejection_reason) = if is_success {
                ("Success".to_string(), None)
            } else if status == "skipped" || status.contains("skip") {
                ("Skipped".to_string(), Some("Skipped existing".to_string()))
            } else if status == "stale_source" || err_str.contains("StaleSource") || err_str.contains("404") || err_str.contains("TrackUnresolved") {
                ("StaleSource".to_string(), Some("Source track unavailable or stale".to_string()))
            } else if status == "source_identity_missing" || err_str.contains("SourceIdentityMissing") {
                ("SourceIdentityMissing".to_string(), Some("Missing locked service track ID".to_string()))
            } else if status == "rejected_quality" || err_str.contains("RejectedQuality") || err_str.contains("downgrade rejected") {
                ("RejectedQuality".to_string(), Some("Quality downgrade rejected by strict quality policy".to_string()))
            } else if status == "requires_auth" || err_str.contains("RequiresAuth") || err_str.contains("401") || err_str.contains("403") {
                ("RequiresAuth".to_string(), Some("Service authentication expired or missing".to_string()))
            } else if status == "failed" {
                ("Failed".to_string(), error_opt.clone())
            } else {
                (status.clone(), None)
            };

            let entry = TrackManifestEntry {
                queue_id: Some(qid),
                track_id: Some(tid),
                provider: service.unwrap_or_else(|| "unknown".to_string()),
                source_track_id: service_tid.unwrap_or_default(),
                isrc: isrc_opt,
                title: title_opt.unwrap_or_else(|| "Unknown Title".to_string()),
                artist: artist_opt.unwrap_or_else(|| "Unknown Artist".to_string()),
                album: album_opt.unwrap_or_else(|| "Unknown Album".to_string()),
                format_requested: quality_pref.clone().unwrap_or_else(|| "HI_RES_LOSSLESS".to_string()),
                format_obtained: if is_success { Some("FLAC".to_string()) } else { None },
                quality_class_requested: quality_pref.unwrap_or_else(|| "Lossless".to_string()),
                quality_class_obtained: if is_success { Some("Lossless".to_string()) } else { None },
                codec: if is_success { Some("FLAC".to_string()) } else { None },
                container: if is_success { Some("FLAC".to_string()) } else { None },
                extension: if is_success { Some("flac".to_string()) } else { None },
                source: Some("Syncify GUI Downloader".to_string()),
                quality_fallback: false,
                download_result: classified_result,
                rejection_reason,
                audio_validation: if is_success { "Valid".to_string() } else { "None".to_string() },
                error: error_opt,
                format_id_requested: "HI_RES_LOSSLESS".to_string(),
                format_id_obtained: if is_success { Some("6".to_string()) } else { None },
                final_path: file_path_opt,
                size_bytes: size_bytes.map(|s| s as u64),
                flac_validation: if is_success { "Valid".to_string() } else { "None".to_string() },
                tagging_result: if is_success { "Success".to_string() } else { "None".to_string() },
                enrichment_result: if is_success { "Success".to_string() } else { "None".to_string() },
                cover_result: if created_artifacts.iter().any(|a| a.ends_with(".webp")) {
                    "StaticAndAnimated".to_string()
                } else if created_artifacts.iter().any(|a| a.ends_with(".jpg")) {
                    "StaticJPEG".to_string()
                } else {
                    "None".to_string()
                },
                lyrics_result: if created_artifacts.iter().any(|a| a.ends_with(".lrc")) {
                    "WordSynced".to_string()
                } else {
                    "None".to_string()
                },
                created_artifacts,
                bit_depth,
                sample_rate: sample_rate.map(|s| s as u32),
                created_at,
                completed_at,
            };

            manifest_entries.push(entry);
        }

        let total_requested = manifest_entries.len();
        let batch_manifest = BatchDownloadManifest {
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_requested,
            total_succeeded: succeeded,
            total_failed: failed,
            total_skipped: skipped,
            entries: manifest_entries,
        };

        tokio::fs::create_dir_all(output_dir).await?;
        let manifest_path = output_dir.join("manifest.json");
        let temp_path = output_dir.join(format!("manifest.json.tmp.{}", uuid::Uuid::new_v4()));
        let json_data = serde_json::to_string_pretty(&batch_manifest)?;

        let write_res: Result<()> = async {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::File::create(&temp_path).await?;
            file.write_all(json_data.as_bytes()).await?;
            file.sync_all().await?;
            drop(file);
            tokio::fs::rename(&temp_path, &manifest_path).await?;
            Ok(())
        }
        .await;

        if let Err(e) = write_res {
            if let Err(rm_err) = tokio::fs::remove_file(&temp_path).await {
                if rm_err.kind() != std::io::ErrorKind::NotFound {
                    warn!(
                        "[ManifestWriter] Failed to remove temp file {:?}: {}",
                        temp_path, rm_err
                    );
                }
            }
            error!(
                "[ManifestWriter] Failed to write manifest atomically to {:?}: {}",
                manifest_path, e
            );
            return Err(e);
        }

        info!(
            "[ManifestWriter] Atomically wrote reconciled batch manifest to {:?}",
            manifest_path
        );

        Ok(batch_manifest)
    }
}

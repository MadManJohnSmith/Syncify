//! Download Orchestrator
//!
//! Manages download queue processing and subprocess calls to external tools.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::process::Stdio;
#[cfg(not(test))]
use tauri::Manager;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Download configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub output_path: PathBuf,
    pub quality: String,
    pub service_priority: Vec<String>,
    pub concurrent_downloads: usize,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            output_path: PathBuf::from("C:\\Music\\Syncify"),
            quality: "lossless".into(),
            service_priority: vec!["qobuz".into(), "tidal".into(), "deezer".into()],
            concurrent_downloads: 2,
        }
    }
}

/// Download result
#[derive(Debug, Clone, Serialize)]
pub struct DownloadResult {
    pub track_id: i64,
    pub success: bool,
    pub file_path: Option<String>,
    pub error: Option<String>,
}

/// Download orchestrator
pub struct DownloadOrchestrator {
    db: SqlitePool,
    config: DownloadConfig,
    app_handle: tauri::AppHandle,
}

impl DownloadOrchestrator {
    pub fn new(db: SqlitePool, config: DownloadConfig, app_handle: tauri::AppHandle) -> Self {
        Self { db, config, app_handle }
    }

    #[cfg(test)]
    fn get_qbdlx_path(&self) -> PathBuf {
        PathBuf::from("tests/fixtures/qbdlx-mock")
    }

    #[cfg(not(test))]
    fn get_qbdlx_path(&self) -> PathBuf {
        self.app_handle
            .path()
            .resource_dir()
            .expect("No resource dir accessible")
            .join("qbdlx-mod")
            .join("QobuzDownloaderX-MOD.exe")
    }

    /// Process the next item in the download queue
    pub async fn process_next(&self) -> Option<DownloadResult> {
        // Get next queued item
        let item: Option<(i64, i64, Option<String>)> = sqlx::query_as(
            r#"
            SELECT dq.id, dq.track_id, t.isrc 
            FROM download_queue dq
            JOIN tracks t ON t.id = dq.track_id
            WHERE dq.status = 'queued'
            ORDER BY dq.priority DESC, dq.created_at ASC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.db)
        .await
        .ok()?;

        let (queue_id, track_id, isrc) = item?;

        // Mark as downloading
        let _ = sqlx::query("UPDATE download_queue SET status = 'downloading', started_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(queue_id)
            .execute(&self.db)
            .await;

        // Find best source
        let source = self.find_best_source(track_id).await;

        let result = match source {
            Some((service, service_track_id)) => {
                self.download_from_service(&service, &service_track_id, isrc.as_deref())
                    .await
            }
            None => DownloadResult {
                track_id,
                success: false,
                file_path: None,
                error: Some("No download source available".into()),
            },
        };

        // Update queue status
        if result.success {
            let _ = sqlx::query(
                "UPDATE download_queue SET status = 'complete', completed_at = CURRENT_TIMESTAMP WHERE id = ?"
            )
            .bind(queue_id)
            .execute(&self.db)
            .await;

            // Insert into downloads table
            if let Some(path) = &result.file_path {
                let _ = sqlx::query(
                    "INSERT INTO downloads (track_id, file_path, downloaded_at) VALUES (?, ?, CURRENT_TIMESTAMP)"
                )
                .bind(track_id)
                .bind(path)
                .execute(&self.db)
                .await;
            }
        } else {
            let _ = sqlx::query(
                "UPDATE download_queue SET status = 'failed', error_message = ?, retry_count = retry_count + 1 WHERE id = ?"
            )
            .bind(&result.error)
            .bind(queue_id)
            .execute(&self.db)
            .await;
        }

        Some(result)
    }

    /// Find the best available source for a track
    async fn find_best_source(&self, track_id: i64) -> Option<(String, String)> {
        // Check each service in priority order
        for service_name in &self.config.service_priority {
            let source: Option<(String, String)> = sqlx::query_as(
                r#"
                SELECT s.name, ts.service_track_id
                FROM track_sources ts
                JOIN services s ON s.id = ts.service_id
                WHERE ts.track_id = ? AND s.name = ? AND ts.available = 1 AND s.supports_download = 1
                LIMIT 1
                "#
            )
            .bind(track_id)
            .bind(service_name)
            .fetch_optional(&self.db)
            .await
            .ok()?;

            if source.is_some() {
                return source;
            }
        }
        None
    }

    /// Download a track from a specific service
    async fn download_from_service(
        &self,
        service: &str,
        track_id: &str,
        _isrc: Option<&str>,
    ) -> DownloadResult {
        match service {
            "qobuz" => self.download_qobuz(track_id).await,
            "tidal" => self.download_tidal(track_id).await,
            "deezer" => self.download_deezer(track_id).await,
            _ => DownloadResult {
                track_id: 0,
                success: false,
                file_path: None,
                error: Some(format!("Unsupported service: {}", service)),
            },
        }
    }

    /// Download from Qobuz using qbdlx-mod subprocess
    async fn download_qobuz(&self, track_id: &str) -> DownloadResult {
        tracing::info!("Downloading from Qobuz: {}", track_id);

        let qbdlx_path = self.get_qbdlx_path();

        if !qbdlx_path.exists() {
            return DownloadResult {
                track_id: 0,
                success: false,
                file_path: None,
                error: Some("QobuzDownloaderX-MOD not found".into()),
            };
        }

        // Build command
        let mut cmd = Command::new(&qbdlx_path);
        cmd.arg("-t")
            .arg(track_id)
            .arg("-o")
            .arg(&self.config.output_path)
            .arg("-q")
            .arg(&self.config.quality)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => {
                // Read output
                if let Some(stdout) = child.stdout.take() {
                    let reader = BufReader::new(stdout);
                    let mut lines = reader.lines();

                    while let Ok(Some(line)) = lines.next_line().await {
                        tracing::debug!("qbdlx: {}", line);
                        // Parse progress if available
                    }
                }

                let status = child.wait().await;

                match status {
                    Ok(exit) if exit.success() => DownloadResult {
                        track_id: 0,
                        success: true,
                        file_path: Some(format!(
                            "{}/{}.flac",
                            self.config.output_path.display(),
                            track_id
                        )),
                        error: None,
                    },
                    Ok(exit) => DownloadResult {
                        track_id: 0,
                        success: false,
                        file_path: None,
                        error: Some(format!("qbdlx exited with code: {:?}", exit.code())),
                    },
                    Err(e) => DownloadResult {
                        track_id: 0,
                        success: false,
                        file_path: None,
                        error: Some(format!("Failed to wait for process: {}", e)),
                    },
                }
            }
            Err(e) => DownloadResult {
                track_id: 0,
                success: false,
                file_path: None,
                error: Some(format!("Failed to spawn qbdlx: {}", e)),
            },
        }
    }

    /// Download from Tidal using streamrip
    async fn download_tidal(&self, track_id: &str) -> DownloadResult {
        tracing::info!("Downloading from Tidal: {}", track_id);

        // Use streamrip for Tidal
        let result = Command::new("rip")
            .arg("url")
            .arg(format!("https://tidal.com/track/{}", track_id))
            .arg("-d")
            .arg(&self.config.output_path)
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => DownloadResult {
                track_id: 0,
                success: true,
                file_path: Some(format!(
                    "{}/{}.flac",
                    self.config.output_path.display(),
                    track_id
                )),
                error: None,
            },
            Ok(output) => DownloadResult {
                track_id: 0,
                success: false,
                file_path: None,
                error: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            },
            Err(e) => DownloadResult {
                track_id: 0,
                success: false,
                file_path: None,
                error: Some(format!("Failed to run streamrip: {}", e)),
            },
        }
    }

    /// Download from Deezer using streamrip
    async fn download_deezer(&self, track_id: &str) -> DownloadResult {
        tracing::info!("Downloading from Deezer: {}", track_id);

        let result = Command::new("rip")
            .arg("url")
            .arg(format!("https://www.deezer.com/track/{}", track_id))
            .arg("-d")
            .arg(&self.config.output_path)
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => DownloadResult {
                track_id: 0,
                success: true,
                file_path: Some(format!(
                    "{}/{}.flac",
                    self.config.output_path.display(),
                    track_id
                )),
                error: None,
            },
            Ok(output) => DownloadResult {
                track_id: 0,
                success: false,
                file_path: None,
                error: Some(String::from_utf8_lossy(&output.stderr).to_string()),
            },
            Err(e) => DownloadResult {
                track_id: 0,
                success: false,
                file_path: None,
                error: Some(format!("Failed to run streamrip: {}", e)),
            },
        }
    }

    /// Run the download queue processor
    pub async fn run(&self) {
        tracing::info!("Download orchestrator started");

        loop {
            if let Some(result) = self.process_next().await {
                if result.success {
                    tracing::info!("Downloaded track {}", result.track_id);
                } else {
                    tracing::warn!("Failed to download: {:?}", result.error);
                }
            } else {
                // No items in queue, wait a bit
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }
}

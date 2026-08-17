// Amazon Music downloader - via DoubleDouble service

use crate::download::http_client::{create_http_client, get_user_agent, AMAZON_LIMITER};
use crate::download::progress::{
    DownloadProgress, DownloadRequest, DownloadResult, PROGRESS_TRACKER,
};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

/// DoubleDouble submit response
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde
struct SubmitResponse {
    success: bool,
    id: Option<String>,
}

/// DoubleDouble status response
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde
struct StatusResponse {
    status: String,
    #[serde(rename = "friendlyStatus")]
    friendly_status: Option<String>,
    url: Option<String>,
    current: Option<CurrentTrack>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields used by serde
struct CurrentTrack {
    name: Option<String>,
    artist: Option<String>,
}

/// Amazon downloader using DoubleDouble service
pub struct AmazonDownloader {
    client: Client,
}

impl AmazonDownloader {
    pub fn new() -> Self {
        Self {
            client: create_http_client(),
        }
    }

    /// Get DoubleDouble API base URL
    fn get_api_base() -> String {
        "https://doubledouble.top/api/v2".to_string()
    }

    /// Submit a track for download
    async fn submit(&self, amazon_url: &str) -> Result<String> {
        AMAZON_LIMITER.wait("amazon").await;

        let api_base = Self::get_api_base();
        let url = format!("{}/submit", api_base);

        debug!("[Amazon] Submitting URL: {}", amazon_url);

        // Submit as form data
        let response = self
            .client
            .post(&url)
            .header("User-Agent", get_user_agent())
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("url={}", urlencoding::encode(amazon_url)))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "DoubleDouble submit failed: HTTP {}",
                response.status()
            ));
        }

        let result: SubmitResponse = response.json().await?;

        if !result.success {
            return Err(anyhow!("DoubleDouble submit failed: success=false"));
        }

        result.id.ok_or_else(|| anyhow!("No job ID in response"))
    }

    /// Poll for download status
    async fn poll_status(&self, job_id: &str, max_attempts: u32) -> Result<String> {
        let api_base = Self::get_api_base();
        let url = format!("{}/status/{}", api_base, job_id);

        for attempt in 0..max_attempts {
            AMAZON_LIMITER.wait("amazon").await;

            debug!(
                "[Amazon] Polling status (attempt {}/{})",
                attempt + 1,
                max_attempts
            );

            let response = self
                .client
                .get(&url)
                .header("User-Agent", get_user_agent())
                .send()
                .await?;

            if !response.status().is_success() {
                warn!("[Amazon] Status poll failed: HTTP {}", response.status());
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }

            let status: StatusResponse = response.json().await?;

            match status.status.as_str() {
                "complete" | "done" => {
                    if let Some(download_url) = status.url {
                        info!("[Amazon] Job complete, got download URL");
                        return Ok(download_url);
                    }
                    return Err(anyhow!("Job complete but no URL"));
                }
                "failed" | "error" => {
                    let msg = status
                        .friendly_status
                        .unwrap_or_else(|| "Unknown error".to_string());
                    return Err(anyhow!("Download failed: {}", msg));
                }
                "processing" | "pending" | "queued" => {
                    debug!("[Amazon] Status: {}", status.status);
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
                _ => {
                    debug!("[Amazon] Unknown status: {}", status.status);
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }

        Err(anyhow!("Timeout waiting for DoubleDouble"))
    }

    /// Download a file with progress tracking
    pub async fn download_file(
        &self,
        download_url: &str,
        output_path: &Path,
        item_id: &str,
    ) -> Result<()> {
        debug!("[Amazon] Downloading to {:?}", output_path);

        let response = self
            .client
            .get(download_url)
            .header("User-Agent", get_user_agent())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Download failed: HTTP {}", response.status()));
        }

        let total_size = response.content_length().unwrap_or(0);
        PROGRESS_TRACKER.update(DownloadProgress::downloading(
            item_id, "amazon", 0, total_size,
        ));

        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = File::create(output_path).await?;

        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if downloaded % (64 * 1024) < chunk.len() as u64 {
                PROGRESS_TRACKER.update(DownloadProgress::downloading(
                    item_id, "amazon", downloaded, total_size,
                ));
            }
        }

        file.flush().await?;
        info!("[Amazon] Download complete: {} bytes", downloaded);
        Ok(())
    }

    /// Download a track given an Amazon Music URL
    pub async fn download_from_url(
        &self,
        amazon_url: &str,
        output_path: &Path,
        item_id: &str,
    ) -> Result<()> {
        PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "Amazon Music"));

        // Submit job
        let job_id = self.submit(amazon_url).await?;

        // Poll for completion
        let download_url = self.poll_status(&job_id, 60).await?;

        // Download
        self.download_file(&download_url, output_path, item_id)
            .await
    }

    /// Full download flow (requires Amazon URL from SongLink)
    pub async fn download_track(
        &self,
        request: &DownloadRequest,
        amazon_url: &str,
    ) -> Result<DownloadResult> {
        let item_id = &request.item_id;

        let filename = format!(
            "{} - {}.flac",
            sanitize_filename(&request.artist_name),
            sanitize_filename(&request.track_name)
        );
        let output_path = Path::new(&request.output_dir).join(&filename);

        self.download_from_url(amazon_url, &output_path, item_id)
            .await?;

        // Amazon provides up to 24-bit/48kHz
        Ok(DownloadResult {
            file_path: output_path.to_string_lossy().to_string(),
            bit_depth: 24,
            sample_rate: 48000,
            title: request.track_name.clone(),
            artist: request.artist_name.clone(),
            album: request.album_name.clone(),
            release_date: request.release_date.clone(),
            track_number: request.track_number,
            disc_number: request.disc_number,
            isrc: request.isrc.clone(),
            service: "amazon".to_string(),
            ..Default::default()
        })
    }
}

impl Default for AmazonDownloader {
    fn default() -> Self {
        Self::new()
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if "/:*?\"<>|\\".contains(c) { '_' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

//! Amazon Music downloader via the DoubleDouble service.
//!
//! The public status payload observed in 2026-11 does not include audio
//! quality. The service may still declare it under `format`, `bitDepth` and
//! `sampleRate`; when it does, that is evidence. When it does not, the
//! delivered file is the only source of truth. If neither the payload nor the
//! file can declare format/bit depth/sample rate, the delivery fails: the
//! adapter never assumes 24-bit/48 kHz.

use crate::download::audio_inspector::{inspect_physical_audio_file, PhysicalAudioMetadata};
use crate::download::http_client::{create_http_client, get_user_agent, AMAZON_LIMITER};
use crate::download::progress::{
    DownloadProgress, DownloadRequest, DownloadResult, PROGRESS_TRACKER,
};
use anyhow::{anyhow, bail, Result};
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// DoubleDouble submit response
#[derive(Debug, Deserialize)]
struct SubmitResponse {
    success: bool,
    id: Option<String>,
}

/// Quality declared by the service for this delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmazonDeclaredQuality {
    pub format: String,
    pub bit_depth: i32,
    pub sample_rate: i32,
}

impl std::fmt::Display for AmazonDeclaredQuality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}-bit / {:.1}kHz",
            self.format,
            self.bit_depth,
            self.sample_rate as f64 / 1000.0
        )
    }
}

/// Normalized status payload from DoubleDouble.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmazonStatus {
    pub status: String,
    pub friendly_status: Option<String>,
    pub message: Option<String>,
    pub url: Option<String>,
    pub declared_quality: Option<AmazonDeclaredQuality>,
}

/// A completed DoubleDouble job and the quality it declared, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmazonCompletion {
    pub url: String,
    pub declared_quality: Option<AmazonDeclaredQuality>,
}

impl AmazonStatus {
    /// Extracts the download URL from a terminal `complete`/`done` status.
    pub fn into_completion(self) -> Result<AmazonCompletion> {
        match self.status.as_str() {
            "complete" | "done" => {
                let url = self
                    .url
                    .ok_or_else(|| anyhow!("DoubleDouble completed without a download URL"))?;
                Ok(AmazonCompletion {
                    url,
                    declared_quality: self.declared_quality,
                })
            }
            other => bail!("DoubleDouble status '{other}' is not a completed download"),
        }
    }
}

/// Quality fields that the service may attach at the top level or under
/// `current`. All three are required together: a partial declaration is an
/// invalid payload, not a quality to guess.
#[derive(Debug, Default, Deserialize)]
struct QualityFields {
    #[serde(rename = "bitDepth", alias = "bit_depth", default)]
    bit_depth: Option<i32>,
    #[serde(rename = "sampleRate", alias = "sample_rate", default)]
    sample_rate: Option<i32>,
    #[serde(default)]
    format: Option<String>,
}

/// DoubleDouble status response
#[derive(Debug, Deserialize)]
struct StatusResponse {
    status: String,
    #[serde(rename = "friendlyStatus")]
    friendly_status: Option<String>,
    message: Option<String>,
    url: Option<String>,
    #[serde(flatten)]
    quality: QualityFields,
    current: Option<CurrentTrack>,
}

#[derive(Debug, Deserialize)]
struct CurrentTrack {
    #[allow(dead_code)]
    // Campo del contrato de datos (serde/sqlx FromRow): lo puebla la deserialización de la respuesta, no el código Rust.
    name: Option<String>,
    #[allow(dead_code)]
    // Campo del contrato de datos (serde/sqlx FromRow): lo puebla la deserialización de la respuesta, no el código Rust.
    artist: Option<String>,
    #[serde(flatten)]
    quality: QualityFields,
}

impl StatusResponse {
    fn declared_quality(&self) -> Result<Option<AmazonDeclaredQuality>> {
        let mut format = None;
        let mut bit_depth = None;
        let mut sample_rate = None;

        // The same three fields can arrive top-level or inside `current`.
        // Merge them without inventing: every field must come from the payload.
        for source in [
            Some(&self.quality),
            self.current.as_ref().map(|c| &c.quality),
        ]
        .into_iter()
        .flatten()
        {
            format = format.or_else(|| source.format.clone());
            bit_depth = bit_depth.or(source.bit_depth);
            sample_rate = sample_rate.or(source.sample_rate);
        }

        if format.is_none() && bit_depth.is_none() && sample_rate.is_none() {
            return Ok(None);
        }

        let (format, bit_depth, sample_rate) = match (format, bit_depth, sample_rate) {
            (Some(format), Some(bit_depth), Some(sample_rate)) => (format, bit_depth, sample_rate),
            (format, bit_depth, sample_rate) => {
                let missing = [
                    format.is_none().then_some("format"),
                    bit_depth.is_none().then_some("bitDepth"),
                    sample_rate.is_none().then_some("sampleRate"),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ");
                bail!(
                    "DoubleDouble status payload declares incomplete audio quality; missing {missing}"
                );
            }
        };

        validate_quality(format, bit_depth, sample_rate)
            .map(Some)
            .map_err(|err| anyhow!("DoubleDouble status payload declares invalid quality: {err}"))
    }
}

/// Parses a DoubleDouble status payload and normalizes its quality declaration.
pub fn parse_status_payload(payload: &str) -> Result<AmazonStatus> {
    let raw: StatusResponse = serde_json::from_str(payload)
        .map_err(|err| anyhow!("DoubleDouble status payload is invalid: {err}"))?;
    let declared_quality = raw.declared_quality()?;
    Ok(AmazonStatus {
        status: raw.status,
        friendly_status: raw.friendly_status,
        message: raw.message,
        url: raw.url,
        declared_quality,
    })
}

fn validate_quality(
    format: String,
    bit_depth: i32,
    sample_rate: i32,
) -> Result<AmazonDeclaredQuality> {
    let format = format.trim().to_ascii_uppercase();
    if !matches!(
        format.as_str(),
        "FLAC" | "ALAC" | "WAV" | "AIFF" | "MP3" | "AAC" | "M4A" | "OGG" | "OPUS"
    ) {
        bail!("unsupported audio format '{format}'");
    }
    if !matches!(bit_depth, 16 | 24 | 32) {
        bail!("invalid bit depth {bit_depth}");
    }
    if !(8_000..=768_000).contains(&sample_rate) {
        bail!("invalid sample rate {sample_rate}");
    }
    Ok(AmazonDeclaredQuality {
        format,
        bit_depth,
        sample_rate,
    })
}

/// Resolves the quality effectively delivered by this Amazon download.
///
/// The delivered file wins over the payload: it is the physical evidence the
/// worker persists in the row and the ledger. A payload declaration is used
/// only when the file cannot be inspected. With neither source there is no
/// quality to declare, and the delivery fails instead of assuming 24/48.
pub fn effective_quality(
    declared: Option<&AmazonDeclaredQuality>,
    physical: Option<&PhysicalAudioMetadata>,
) -> Result<AmazonDeclaredQuality> {
    if let Some(physical) = physical {
        let quality = validate_quality(
            physical.format.clone(),
            physical.bit_depth,
            physical.sample_rate,
        )
        .map_err(|err| anyhow!("delivered file does not expose a valid quality: {err}"))?;
        if let Some(declared) = declared {
            if declared != &quality {
                warn!(
                    declared = %declared,
                    effective = %quality,
                    "[Amazon] Declared quality differs from the delivered file; physical evidence wins"
                );
            }
        }
        return Ok(quality);
    }

    if let Some(declared) = declared {
        warn!(
            quality = %declared,
            "[Amazon] Delivered file could not be inspected; using the quality declared by the service"
        );
        return Ok(declared.clone());
    }

    bail!(
        "Amazon delivery quality is not declared: the DoubleDouble status payload has no \
         format/bitDepth/sampleRate and the delivered file could not be inspected"
    );
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
    async fn poll_status(
        &self,
        job_id: &str,
        max_attempts: u32,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<AmazonCompletion> {
        let api_base = Self::get_api_base();
        let url = format!("{}/status/{}", api_base, job_id);

        for attempt in 0..max_attempts {
            if let Some(token) = cancel_token {
                if token.is_cancelled() {
                    return Err(anyhow!("Download cancelled by user"));
                }
            }
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

            let body = response.text().await?;
            let status = parse_status_payload(&body)?;

            match status.status.as_str() {
                "complete" | "done" => {
                    let completion = status.into_completion()?;
                    info!("[Amazon] Job complete, got download URL");
                    return Ok(completion);
                }
                "failed" | "error" => {
                    let msg = status
                        .message
                        .or(status.friendly_status)
                        .unwrap_or_else(|| "Unknown error".to_string());
                    return Err(anyhow!("DoubleDouble download failed: {msg}"));
                }
                "processing" | "pending" | "queued" | "downloading" | "uploading"
                | "initialising" => {
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
        cancel_token: Option<&CancellationToken>,
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
            if let Some(token) = cancel_token {
                if token.is_cancelled() {
                    let _ = tokio::fs::remove_file(output_path).await;
                    PROGRESS_TRACKER.update(DownloadProgress::cancelled(item_id));
                    return Err(anyhow!("Download cancelled by user"));
                }
            }
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
        cancel_token: Option<&CancellationToken>,
    ) -> Result<AmazonCompletion> {
        PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "Amazon Music"));

        // Submit job
        let job_id = self.submit(amazon_url).await?;

        // Poll for completion
        let completion = self.poll_status(&job_id, 60, cancel_token).await?;

        // Download
        self.download_file(&completion.url, output_path, item_id, cancel_token)
            .await?;
        Ok(completion)
    }

    /// Full download flow (requires Amazon URL from SongLink)
    pub async fn download_track(
        &self,
        request: &DownloadRequest,
        amazon_url: &str,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<DownloadResult> {
        let item_id = &request.item_id;

        let filename = format!(
            "{} - {}.flac",
            sanitize_filename(&request.artist_name),
            sanitize_filename(&request.track_name)
        );
        let output_path = Path::new(&request.output_dir).join(&filename);

        let completion = self
            .download_from_url(amazon_url, &output_path, item_id, cancel_token)
            .await?;

        let physical = inspect_physical_audio_file(&output_path);
        let effective =
            match effective_quality(completion.declared_quality.as_ref(), physical.as_ref()) {
                Ok(quality) => quality,
                Err(err) => {
                    let _ = tokio::fs::remove_file(&output_path).await;
                    return Err(err);
                }
            };
        info!("[Amazon] Effective delivery quality: {effective}");

        Ok(DownloadResult {
            file_path: output_path.to_string_lossy().to_string(),
            bit_depth: effective.bit_depth,
            sample_rate: effective.sample_rate,
            physical_audio: physical,
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

// Qobuz downloader - credential-free downloads via proxy APIs

use crate::download::http_client::{create_http_client, get_user_agent, QOBUZ_LIMITER};
use crate::download::progress::{
    DownloadProgress, DownloadRequest, DownloadResult, PROGRESS_TRACKER,
};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

/// Qobuz track information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzTrack {
    pub id: i64,
    pub title: String,
    pub isrc: Option<String>,
    pub duration: i32,
    #[serde(rename = "maximum_bit_depth")]
    pub max_bit_depth: Option<i32>,
    #[serde(rename = "maximum_sampling_rate")]
    pub max_sample_rate: Option<f64>,
    pub track_number: Option<i32>,
    pub performer: Option<QobuzPerformer>,
    pub album: Option<QobuzAlbum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzPerformer {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzAlbum {
    pub title: String,
    pub release_date_original: Option<String>,
    pub image: Option<QobuzImage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzImage {
    pub large: Option<String>,
}

/// Qobuz search response
#[derive(Debug, Deserialize)]
struct QobuzSearchResponse {
    tracks: Option<QobuzTracksContainer>,
}

#[derive(Debug, Deserialize)]
struct QobuzTracksContainer {
    items: Vec<QobuzTrack>,
}

/// Download URL response from proxy API
#[derive(Debug, Deserialize)]
struct StreamResponse {
    url: Option<String>,
    error: Option<String>,
}

/// Qobuz downloader using proxy APIs
pub struct QobuzDownloader {
    client: Client,
    app_id: String,
}

impl QobuzDownloader {
    pub fn new() -> Self {
        Self {
            client: create_http_client(),
            app_id: "798273057".to_string(),
        }
    }

    /// Get available proxy APIs (decoded from base64)
    fn get_proxy_apis() -> Vec<String> {
        let encoded_apis = [
            "ZGFiLnllZXQuc3UvYXBpL3N0cmVhbT90cmFja0lkPQ==", // dab.yeet.su
            "ZGFibXVzaWMueHl6L2FwaS9zdHJlYW0/dHJhY2tJZD0=", // dabmusic.xyz
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

    /// Get Qobuz API base URL (decoded)
    fn get_api_base() -> String {
        let encoded = "aHR0cHM6Ly93d3cucW9idXouY29tL2FwaS5qc29uLzAuMi90cmFjay9zZWFyY2g/cXVlcnk9";
        BASE64
            .decode(encoded)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .unwrap_or_default()
    }

    /// Search for a track by ISRC
    pub async fn search_by_isrc(
        &self,
        isrc: &str,
        expected_duration_sec: i32,
    ) -> Result<QobuzTrack> {
        QOBUZ_LIMITER.wait("qobuz").await;

        let api_base = Self::get_api_base();
        let url = format!(
            "{}{}&limit=50&app_id={}",
            api_base,
            urlencoding::encode(isrc),
            self.app_id
        );

        debug!("[Qobuz] Searching by ISRC: {}", isrc);

        let response = self
            .client
            .get(&url)
            .header("User-Agent", get_user_agent())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Qobuz search failed: HTTP {}", response.status()));
        }

        let result: QobuzSearchResponse = response.json().await?;

        let tracks = result
            .tracks
            .ok_or_else(|| anyhow!("No tracks in response"))?;

        // Find exact ISRC match with duration verification
        for track in &tracks.items {
            if track.isrc.as_deref() == Some(isrc) {
                // Verify duration (allow 10 second tolerance)
                if expected_duration_sec > 0 {
                    let duration_diff = (track.duration - expected_duration_sec).abs();
                    if duration_diff <= 10 {
                        info!(
                            "[Qobuz] Found ISRC match: '{}' (duration verified)",
                            track.title
                        );
                        return Ok(track.clone());
                    } else {
                        warn!(
                            "[Qobuz] ISRC match but duration mismatch: expected {}s, got {}s",
                            expected_duration_sec, track.duration
                        );
                    }
                } else {
                    return Ok(track.clone());
                }
            }
        }

        Err(anyhow!("No exact ISRC match found for: {}", isrc))
    }

    /// Search for a track by metadata (artist + track name)
    pub async fn search_by_metadata(
        &self,
        track_name: &str,
        artist_name: &str,
        expected_duration_sec: i32,
    ) -> Result<QobuzTrack> {
        QOBUZ_LIMITER.wait("qobuz").await;

        let api_base = Self::get_api_base();
        let query = format!("{} {}", artist_name, track_name);
        let url = format!(
            "{}{}&limit=50&app_id={}",
            api_base,
            urlencoding::encode(&query),
            self.app_id
        );

        debug!(
            "[Qobuz] Searching by metadata: {} - {}",
            artist_name, track_name
        );

        let response = self
            .client
            .get(&url)
            .header("User-Agent", get_user_agent())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Qobuz search failed: HTTP {}", response.status()));
        }

        let result: QobuzSearchResponse = response.json().await?;
        let tracks = result
            .tracks
            .ok_or_else(|| anyhow!("No tracks in response"))?;

        // Find best match by title and duration
        for track in &tracks.items {
            // Check title similarity
            if !title_matches(track_name, &track.title) {
                continue;
            }

            // Check artist if available
            if let Some(performer) = &track.performer {
                if !artist_matches(artist_name, &performer.name) {
                    continue;
                }
            }

            // Verify duration if provided
            if expected_duration_sec > 0 {
                let duration_diff = (track.duration - expected_duration_sec).abs();
                if duration_diff > 10 {
                    continue;
                }
            }

            info!(
                "[Qobuz] Found metadata match: '{}' by '{}'",
                track.title,
                track
                    .performer
                    .as_ref()
                    .map(|p| p.name.as_str())
                    .unwrap_or("Unknown")
            );
            return Ok(track.clone());
        }

        Err(anyhow!(
            "No matching track found for: {} - {}",
            artist_name,
            track_name
        ))
    }

    /// Get download URL from proxy APIs (parallel requests, first success wins)
    pub async fn get_download_url(&self, track_id: i64, quality: &str) -> Result<String> {
        let apis = Self::get_proxy_apis();
        if apis.is_empty() {
            return Err(anyhow!("No Qobuz proxy APIs available"));
        }

        debug!(
            "[Qobuz] Getting download URL for track {} (quality: {})",
            track_id, quality
        );

        // Try all APIs in parallel
        let mut handles = Vec::new();
        for api in apis {
            let client = self.client.clone();
            let url = format!("{}{}&quality={}", api, track_id, quality);

            handles.push(tokio::spawn(async move {
                let result = client
                    .get(&url)
                    .timeout(Duration::from_secs(15))
                    .send()
                    .await;

                match result {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(stream_resp) = resp.json::<StreamResponse>().await {
                            if let Some(download_url) = stream_resp.url {
                                return Ok(download_url);
                            }
                            if let Some(error) = stream_resp.error {
                                if error.contains("401") || error.to_lowercase().contains("unauthorized") {
                                    return Err(anyhow!("RequiresAuth: Qobuz API returned HTTP 401 Unauthorized ({})", error));
                                }
                                return Err(anyhow!("{}", error));
                            }
                        }
                        Err(anyhow!("Invalid response from proxy"))
                    }
                    Ok(resp) if resp.status().as_u16() == 401 => {
                        Err(anyhow!("RequiresAuth: Qobuz API returned HTTP 401 Unauthorized"))
                    }
                    Ok(resp) => Err(anyhow!("HTTP {}", resp.status())),
                    Err(e) => Err(anyhow!("{}", e)),
                }
            }));
        }

        // Return first successful result or detect auth failure
        let mut had_auth_error = false;
        for handle in handles {
            match handle.await {
                Ok(Ok(url)) => {
                    info!("[Qobuz] Got download URL");
                    return Ok(url);
                }
                Ok(Err(e)) => {
                    if e.to_string().contains("RequiresAuth") {
                        had_auth_error = true;
                    }
                }
                _ => {}
            }
        }

        if had_auth_error {
            Err(anyhow!("RequiresAuth: Qobuz account authentication required (HTTP 401)"))
        } else {
            Err(anyhow!("All Qobuz proxy APIs failed"))
        }
    }


    /// Download a file with progress tracking
    pub async fn download_file(
        &self,
        download_url: &str,
        output_path: &Path,
        item_id: &str,
    ) -> Result<()> {
        debug!("[Qobuz] Downloading to {:?}", output_path);

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
            item_id, "qobuz", 0, total_size,
        ));

        // Create output file
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut file = File::create(output_path).await?;

        // Download with progress
        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            // Update progress every 64KB
            if downloaded % (64 * 1024) < chunk.len() as u64 {
                PROGRESS_TRACKER.update(DownloadProgress::downloading(
                    item_id, "qobuz", downloaded, total_size,
                ));
            }
        }

        file.flush().await?;
        info!("[Qobuz] Download complete: {} bytes", downloaded);
        Ok(())
    }

    /// Full download flow: search → get URL → download
    pub async fn download_track(&self, request: &DownloadRequest) -> Result<DownloadResult> {
        let item_id = &request.item_id;
        let duration_sec = (request.duration_ms / 1000) as i32;

        PROGRESS_TRACKER.update(DownloadProgress::searching(item_id, "Qobuz"));

        // Try to find track
        let track = if let Some(isrc) = &request.isrc {
            match self.search_by_isrc(isrc, duration_sec).await {
                Ok(t) => t,
                Err(_) => {
                    // Fallback to metadata search
                    self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec)
                        .await?
                }
            }
        } else {
            self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec)
                .await?
        };

        // Map quality
        let quality = match request.quality.as_str() {
            "LOSSLESS" => "6",         // 16-bit FLAC
            "HI_RES" => "7",           // 24-bit 96kHz
            "HI_RES_LOSSLESS" => "27", // 24-bit 192kHz
            _ => "27",                 // Default to highest
        };

        // Get download URL
        let download_url = self.get_download_url(track.id, quality).await?;

        // Build output filename
        let filename = format!(
            "{} - {}.flac",
            sanitize_filename(&request.artist_name),
            sanitize_filename(&request.track_name)
        );
        let output_path = Path::new(&request.output_dir).join(&filename);

        // Download file
        self.download_file(&download_url, &output_path, item_id)
            .await?;

        // Return result
        Ok(DownloadResult {
            file_path: output_path.to_string_lossy().to_string(),
            bit_depth: track.max_bit_depth.unwrap_or(16),
            sample_rate: (track.max_sample_rate.unwrap_or(44.1) * 1000.0) as i32,
            title: track.title,
            artist: track.performer.map(|p| p.name).unwrap_or_default(),
            album: track
                .album
                .as_ref()
                .map(|a| a.title.clone())
                .unwrap_or_default(),
            release_date: track.album.and_then(|a| a.release_date_original),
            track_number: track.track_number.unwrap_or(request.track_number),
            disc_number: request.disc_number,
            isrc: track.isrc,
            service: "qobuz".to_string(),
        })
    }
}

impl Default for QobuzDownloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if two titles match (fuzzy)
fn title_matches(expected: &str, found: &str) -> bool {
    let expected_clean = clean_title(expected);
    let found_clean = clean_title(found);

    // Exact match after cleaning
    if expected_clean == found_clean {
        return true;
    }

    // Check if one contains the other (for different versions)
    if found_clean.contains(&expected_clean) || expected_clean.contains(&found_clean) {
        return true;
    }

    false
}

/// Check if two artist names match (fuzzy)
fn artist_matches(expected: &str, found: &str) -> bool {
    let expected_lower = expected.to_lowercase();
    let found_lower = found.to_lowercase();

    // Exact match
    if expected_lower == found_lower {
        return true;
    }

    // Check if any artist in a list matches
    let expected_parts: Vec<&str> = expected_lower
        .split(&[',', ';', '&', '/', '|'][..])
        .collect();
    let found_parts: Vec<&str> = found_lower.split(&[',', ';', '&', '/', '|'][..]).collect();

    for ep in &expected_parts {
        for fp in &found_parts {
            if ep.trim() == fp.trim() {
                return true;
            }
        }
    }

    false
}

/// Clean title for comparison
fn clean_title(title: &str) -> String {
    let mut clean = title.to_lowercase();

    // Remove common suffixes
    let suffixes = [
        "(remaster",
        "(remastered",
        "(deluxe",
        "(expanded",
        "(live",
        "(acoustic",
        "(remix",
        "(radio edit",
        "- remaster",
        "- remastered",
        "- deluxe",
    ];

    for suffix in suffixes {
        if let Some(pos) = clean.find(suffix) {
            clean = clean[..pos].to_string();
        }
    }

    clean.trim().to_string()
}

/// Sanitize filename for filesystem
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_matches() {
        assert!(title_matches("Bohemian Rhapsody", "Bohemian Rhapsody"));
        assert!(title_matches(
            "Bohemian Rhapsody",
            "Bohemian Rhapsody (Remastered)"
        ));
        assert!(title_matches("Yesterday", "Yesterday - Remastered 2009"));
    }

    #[test]
    fn test_artist_matches() {
        assert!(artist_matches("The Beatles", "The Beatles"));
        assert!(artist_matches("Queen", "Queen, David Bowie"));
        assert!(artist_matches(
            "Freddie Mercury & David Bowie",
            "David Bowie"
        ));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("Hello/World"), "Hello_World");
        assert_eq!(sanitize_filename("What?"), "What_");
    }
}

//! Qobuz Downloader and Download Request DTOs (CLI Standalone)

use crate::download::http_client::{create_http_client, QOBUZ_LIMITER};
use crate::metadata::tag_writer::FlacMetadata;
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub item_id: String,
    pub isrc: Option<String>,
    pub spotify_id: Option<String>,
    pub track_name: String,
    pub artist_name: String,
    pub album_name: String,
    pub album_artist: Option<String>,
    pub duration_ms: i64,
    pub track_number: u32,
    pub disc_number: u32,
    pub total_tracks: u32,
    pub total_discs: u32,
    pub release_date: Option<String>,
    pub cover_url: Option<String>,
    pub output_dir: String,
    pub quality: String,
    pub qobuz_token: Option<String>,
    pub embed_lyrics: bool,
    pub embed_artwork: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResult {
    pub file_path: String,
    pub bit_depth: i32,
    pub sample_rate: i32,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub release_date: Option<String>,
    pub track_number: u32,
    pub disc_number: u32,
    pub isrc: Option<String>,
    pub service: String,
}

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
}

#[derive(Debug, Deserialize)]
struct QobuzSearchResponse {
    tracks: Option<QobuzTracksContainer>,
}

#[derive(Debug, Deserialize)]
struct QobuzTracksContainer {
    items: Vec<QobuzTrack>,
}

#[derive(Debug, Deserialize)]
struct StreamResponse {
    url: Option<String>,
    error: Option<String>,
}

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

    fn get_proxy_apis() -> Vec<String> {
        let encoded_apis = [
            "aHR0cHM6Ly9xb2J1ei5raW5vcGx1cy5vbmxpbmUvdHJhY2svZ2V0P2lkPQ==",
            "aHR0cHM6Ly9xb2J1ei1hcGkuYmluaW11bS5vcmcvdHJhY2svZ2V0P2lkPQ==",
        ];
        encoded_apis
            .iter()
            .filter_map(|encoded| {
                BASE64.decode(encoded).ok().and_then(|bytes| String::from_utf8(bytes).ok())
            })
            .collect()
    }

    pub async fn search_by_isrc(&self, isrc: &str, expected_duration_sec: i32) -> Result<QobuzTrack> {
        QOBUZ_LIMITER.wait("qobuz").await;
        let url = format!(
            "https://www.qobuz.com/api.json/0.2/track/search?query={}&limit=50&app_id={}",
            urlencoding::encode(isrc),
            self.app_id
        );

        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!("Qobuz search failed: HTTP {}", response.status()));
        }

        let result: QobuzSearchResponse = response.json().await?;
        let tracks = result.tracks.ok_or_else(|| anyhow!("No tracks in response"))?;

        for track in &tracks.items {
            if track.isrc.as_deref() == Some(isrc) {
                if expected_duration_sec > 0 {
                    let diff = (track.duration - expected_duration_sec).abs();
                    if diff <= 10 {
                        return Ok(track.clone());
                    }
                } else {
                    return Ok(track.clone());
                }
            }
        }
        Err(anyhow!("No exact ISRC match found for: {}", isrc))
    }

    pub async fn search_by_metadata(
        &self,
        track_name: &str,
        artist_name: &str,
        expected_duration_sec: i32,
    ) -> Result<QobuzTrack> {
        QOBUZ_LIMITER.wait("qobuz").await;
        let query = format!("{} {}", artist_name, track_name);
        let url = format!(
            "https://www.qobuz.com/api.json/0.2/track/search?query={}&limit=50&app_id={}",
            urlencoding::encode(&query),
            self.app_id
        );

        let response = self.client.get(&url).send().await?;
        if !response.status().is_success() {
            return Err(anyhow!("Qobuz search failed: HTTP {}", response.status()));
        }

        let result: QobuzSearchResponse = response.json().await?;
        let tracks = result.tracks.ok_or_else(|| anyhow!("No tracks in response"))?;

        for track in &tracks.items {
            let perf = track.performer.as_ref().map(|p| p.name.as_str()).unwrap_or("");
            if track.title.to_lowercase().contains(&track_name.to_lowercase())
                && perf.to_lowercase().contains(&artist_name.to_lowercase())
            {
                if expected_duration_sec > 0 {
                    let diff = (track.duration - expected_duration_sec).abs();
                    if diff > 10 {
                        continue;
                    }
                }
                return Ok(track.clone());
            }
        }

        Err(anyhow!("No matching track found for: {} - {}", artist_name, track_name))
    }

    pub async fn get_download_url(&self, track_id: i64, quality: &str) -> Result<String> {
        let apis = Self::get_proxy_apis();
        for api in apis {
            let url = format!("{}{}&quality={}", api, track_id, quality);
            let result = self.client.get(&url).timeout(Duration::from_secs(15)).send().await;
            if let Ok(resp) = result {
                if resp.status().is_success() {
                    if let Ok(stream_resp) = resp.json::<StreamResponse>().await {
                        if let Some(download_url) = stream_resp.url {
                            return Ok(download_url);
                        }
                    }
                }
            }
        }
        Err(anyhow!("All Qobuz proxy APIs failed"))
    }

    pub async fn download_track(&self, request: &DownloadRequest) -> Result<DownloadResult> {
        let duration_sec = (request.duration_ms / 1000) as i32;
        let track = if let Some(isrc) = &request.isrc {
            match self.search_by_isrc(isrc, duration_sec).await {
                Ok(t) => t,
                Err(_) => self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec).await?,
            }
        } else {
            self.search_by_metadata(&request.track_name, &request.artist_name, duration_sec).await?
        };

        let quality = match request.quality.as_str() {
            "LOSSLESS" => "6",
            "HI_RES" => "7",
            _ => "27",
        };

        let download_url = self.get_download_url(track.id, quality).await?;
        let filename = format!("{} - {}.flac", request.artist_name, request.track_name);
        let output_path = Path::new(&request.output_dir).join(&filename);

        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let resp = self.client.get(&download_url).send().await?;
        let bytes = resp.bytes().await?;
        let mut file = File::create(&output_path).await?;
        file.write_all(&bytes).await?;

        Ok(DownloadResult {
            file_path: output_path.to_string_lossy().to_string(),
            bit_depth: track.max_bit_depth.unwrap_or(16),
            sample_rate: (track.max_sample_rate.unwrap_or(44.1) * 1000.0) as i32,
            title: track.title,
            artist: track.performer.map(|p| p.name).unwrap_or_default(),
            album: track.album.as_ref().map(|a| a.title.clone()).unwrap_or_default(),
            release_date: track.album.and_then(|a| a.release_date_original),
            track_number: track.track_number.unwrap_or(request.track_number as i32) as u32,
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

pub fn build_flac_metadata(res: &DownloadResult, req: &DownloadRequest) -> FlacMetadata {
    FlacMetadata {
        title: res.title.clone(),
        artist: res.artist.clone(),
        album: res.album.clone(),
        album_artist: req.album_artist.clone(),
        track_number: res.track_number,
        track_total: req.total_tracks,
        disc_number: res.disc_number,
        disc_total: req.total_discs,
        isrc: res.isrc.clone().or_else(|| req.isrc.clone()),
        release_date: res.release_date.clone().or_else(|| req.release_date.clone()),
        bit_depth: Some(res.bit_depth),
        sample_rate: Some(res.sample_rate as f64),
        ..Default::default()
    }
}

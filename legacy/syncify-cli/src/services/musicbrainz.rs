//! MusicBrainz API client for metadata enrichment (CLI Standalone)

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

const MUSICBRAINZ_API_BASE: &str = "https://musicbrainz.org/ws/2";
const USER_AGENT: &str = "Syncify/1.0.0 (https://github.com/syncify/syncify)";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzRecording {
    pub id: String,
    pub title: String,
    pub artist_credit: Option<Vec<ArtistCredit>>,
    pub releases: Option<Vec<Release>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistCredit {
    pub name: String,
    pub artist: Artist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub id: String,
    pub title: String,
    #[serde(rename = "release-group")]
    pub release_group: Option<ReleaseGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseGroup {
    pub id: String,
    #[serde(rename = "primary-type")]
    pub primary_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzGenre {
    pub name: String,
    pub count: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzRecordingDetail {
    pub id: String,
    pub title: String,
    pub genres: Option<Vec<MusicBrainzGenre>>,
    pub isrcs: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RecordingQueryResponse {
    recordings: Option<Vec<MusicBrainzRecording>>,
}

pub struct MusicBrainzClient {
    client: Client,
    last_request: std::sync::Mutex<std::time::Instant>,
}

impl MusicBrainzClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent(USER_AGENT)
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            last_request: std::sync::Mutex::new(std::time::Instant::now() - Duration::from_secs(2)),
        }
    }

    async fn rate_limit(&self) {
        let elapsed = {
            let last = self.last_request.lock().unwrap();
            last.elapsed()
        };

        if elapsed < Duration::from_millis(1100) {
            sleep(Duration::from_millis(1100) - elapsed).await;
        }

        *self.last_request.lock().unwrap() = std::time::Instant::now();
    }

    pub async fn lookup_by_isrc(&self, isrc: &str) -> Result<Option<MusicBrainzRecording>, String> {
        if isrc.is_empty() {
            return Ok(None);
        }

        self.rate_limit().await;

        let url = format!(
            "{}/recording?query=isrc:{}&fmt=json",
            MUSICBRAINZ_API_BASE, isrc
        );

        tracing::debug!("MusicBrainz lookup: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("MusicBrainz request failed: {}", e))?;

        if response.status() == 404 {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::debug!("MusicBrainz error response: {}", body);
            return Err(format!("MusicBrainz returned {}", status));
        }

        let data: RecordingQueryResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse MusicBrainz response: {}", e))?;

        Ok(data
            .recordings
            .and_then(|r: Vec<MusicBrainzRecording>| r.into_iter().next()))
    }

    pub async fn search_recordings(
        &self,
        title: &str,
        artist: &str,
        album: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MusicBrainzRecording>, String> {
        self.rate_limit().await;

        let mut query = format!(
            "recording:{} AND artist:{}",
            escape_lucene(title),
            escape_lucene(artist)
        );

        if let Some(a) = album {
            if !a.is_empty() {
                query.push_str(&format!(" AND release:{}", escape_lucene(a)));
            }
        }

        let url = format!(
            "{}/recording?query={}&fmt=json&limit={}",
            MUSICBRAINZ_API_BASE,
            urlencoding::encode(&query),
            limit
        );

        tracing::debug!("MusicBrainz search: {}", query);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("MusicBrainz request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::debug!("MusicBrainz error: {}", body);
            return Err(format!("MusicBrainz returned {}", status));
        }

        let data: RecordingQueryResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse MusicBrainz response: {}", e))?;

        Ok(data.recordings.unwrap_or_default())
    }

    pub async fn search_releases(
        &self,
        title: &str,
        artist: &str,
        limit: usize,
    ) -> Result<Vec<Release>, String> {
        let recs = self.search_recordings(title, artist, None, limit).await?;
        let releases = recs
            .into_iter()
            .filter_map(|r| r.releases)
            .flatten()
            .collect();
        Ok(releases)
    }

    pub async fn get_recording_details(
        &self,
        mbid: &str,
    ) -> Result<MusicBrainzRecordingDetail, String> {
        if mbid.is_empty() {
            return Err("Empty MBID".to_string());
        }

        self.rate_limit().await;

        let url = format!(
            "{}/recording/{}?inc=genres+isrcs&fmt=json",
            MUSICBRAINZ_API_BASE, mbid
        );

        tracing::debug!("MusicBrainz detail lookup: {}", url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("MusicBrainz request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::debug!("MusicBrainz error: {}", body);
            return Err(format!("MusicBrainz returned {}", status));
        }

        let data: MusicBrainzRecordingDetail = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse MusicBrainz response: {}", e))?;

        Ok(data)
    }
}

impl Default for MusicBrainzClient {
    fn default() -> Self {
        Self::new()
    }
}

fn escape_lucene(input: &str) -> String {
    let special_chars = [
        '+', '-', '&', '|', '!', '(', ')', '{', '}', '[', ']', '^', '"', '~', '*', '?', ':', '\\',
        '/',
    ];
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        if special_chars.contains(&c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

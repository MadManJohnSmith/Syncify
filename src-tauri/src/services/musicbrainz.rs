//! MusicBrainz API client for metadata enrichment
//!
//! Looks up recordings by ISRC to get MusicBrainz IDs and additional metadata.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

const MUSICBRAINZ_API_BASE: &str = "https://musicbrainz.org/ws/2";
const USER_AGENT: &str = "Syncify/1.0.0 (https://github.com/syncify/syncify)";

/// MusicBrainz recording info
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

/// MusicBrainz API client with rate limiting
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

    /// Enforce MusicBrainz rate limit (1 request per second)
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

    /// Look up a recording by ISRC
    pub async fn lookup_by_isrc(&self, isrc: &str) -> Result<Option<MusicBrainzRecording>, String> {
        if isrc.is_empty() {
            return Ok(None);
        }

        self.rate_limit().await;

        // Use recording query with ISRC instead of /isrc/ endpoint
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
            tracing::debug!("ISRC {} not found in MusicBrainz", isrc);
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

        // Return the first recording if available
        Ok(data
            .recordings
            .and_then(|r: Vec<MusicBrainzRecording>| r.into_iter().next()))
    }

    /// Batch lookup recordings by multiple ISRCs in one request
    /// Returns a map of ISRC -> MusicBrainzRecording
    pub async fn batch_lookup_by_isrc(
        &self,
        isrcs: &[String],
    ) -> Result<std::collections::HashMap<String, MusicBrainzRecording>, String> {
        use std::collections::HashMap;

        if isrcs.is_empty() {
            return Ok(HashMap::new());
        }

        self.rate_limit().await;

        // Build OR query: isrc:(ISRC1 OR ISRC2 OR ISRC3 ...)
        let isrc_query = isrcs
            .iter()
            .map(|i| i.as_str())
            .collect::<Vec<_>>()
            .join(" OR ");

        let url = format!(
            "{}/recording?query=isrc:({})&fmt=json&limit=100",
            MUSICBRAINZ_API_BASE,
            urlencoding::encode(&isrc_query)
        );

        tracing::debug!("MusicBrainz batch lookup: {} ISRCs", isrcs.len());

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

        // Build map of ISRC -> Recording
        // Note: MusicBrainz doesn't return ISRC in the response, so we need to match by querying
        // For now, we'll return all recordings and let caller match them
        let mut result = HashMap::new();
        if let Some(recordings) = data.recordings {
            for recording in recordings {
                // Store by recording ID for now - caller will need to match ISRCs
                result.insert(recording.id.clone(), recording);
            }
        }

        Ok(result)
    }

    /// Search for recordings by title and artist
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

    /// Get detailed recording info including genres and ISRCs
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

    /// Batch enrich tracks with MusicBrainz IDs
    pub async fn enrich_tracks(
        &self,
        db: &sqlx::SqlitePool,
        limit: i64,
    ) -> Result<EnrichmentResult, String> {
        // Find tracks with ISRC but no MusicBrainz ID
        let tracks: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, isrc FROM tracks WHERE isrc IS NOT NULL AND isrc != '' AND musicbrainz_id IS NULL LIMIT ?"
        )
        .bind(limit)
        .fetch_all(db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        let total = tracks.len();
        let mut enriched = 0;
        let mut failed = 0;

        for (track_id, isrc) in tracks {
            match self.lookup_by_isrc(&isrc).await {
                Ok(Some(recording)) => {
                    // Update track with MusicBrainz ID
                    let result = sqlx::query("UPDATE tracks SET musicbrainz_id = ? WHERE id = ?")
                        .bind(&recording.id)
                        .bind(track_id)
                        .execute(db)
                        .await;

                    if result.is_ok() {
                        enriched += 1;
                        tracing::info!("Enriched track {} with MB ID {}", track_id, recording.id);
                    } else {
                        failed += 1;
                    }
                }
                Ok(None) => {
                    // No match found - mark as checked to avoid re-checking
                    let _ =
                        sqlx::query("UPDATE tracks SET musicbrainz_id = 'NOT_FOUND' WHERE id = ?")
                            .bind(track_id)
                            .execute(db)
                            .await;
                }
                Err(e) => {
                    tracing::warn!("Failed to look up ISRC {}: {}", isrc, e);
                    failed += 1;
                }
            }
        }

        Ok(EnrichmentResult {
            total,
            enriched,
            failed,
        })
    }
}

impl Default for MusicBrainzClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EnrichmentResult {
    pub total: usize,
    pub enriched: usize,
    pub failed: usize,
}

/// Escape special characters for Lucene query syntax
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

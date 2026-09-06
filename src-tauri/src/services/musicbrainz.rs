//! MusicBrainz API client for metadata enrichment
//!
//! Looks up recordings by ISRC to get MusicBrainz IDs and additional metadata.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use syncify_metadata_domain::FieldValidator;
use tokio::time::sleep;

const MUSICBRAINZ_API_BASE: &str = "https://musicbrainz.org/ws/2";
const USER_AGENT: &str = "Syncify/1.0.0 (https://github.com/syncify/syncify)";

/// MusicBrainz recording info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzRecording {
    pub id: String,
    pub title: String,
    #[serde(rename = "artist-credit", alias = "artist_credit", default)]
    pub artist_credit: Option<Vec<ArtistCredit>>,
    pub releases: Option<Vec<Release>>,
    pub genres: Option<Vec<MusicBrainzGenre>>,
    pub tags: Option<Vec<MusicBrainzGenre>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArtistCredit {
    #[serde(default)]
    pub name: String,
    pub artist: Artist,
    #[serde(default)]
    pub joinphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Artist {
    pub id: String,
    pub name: String,
    #[serde(rename = "sort-name", alias = "sort_name", default)]
    pub sort_name: Option<String>,
    #[serde(default)]
    pub disambiguation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRepresentation {
    pub language: Option<String>,
    pub script: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub id: String,
    pub title: String,
    pub status: Option<String>,
    pub country: Option<String>,
    pub date: Option<String>,
    pub barcode: Option<String>,
    #[serde(rename = "release-group")]
    pub release_group: Option<ReleaseGroup>,
    #[serde(rename = "label-info")]
    pub label_info: Option<Vec<LabelInfo>>,
    #[serde(rename = "text-representation")]
    pub text_representation: Option<TextRepresentation>,
    #[serde(rename = "artist-credit", alias = "artist_credit", default)]
    pub artist_credit: Option<Vec<ArtistCredit>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelInfo {
    #[serde(rename = "catalog-number")]
    pub catalog_number: Option<String>,
    pub label: Option<Label>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseGroup {
    pub id: String,
    pub title: Option<String>,
    #[serde(rename = "primary-type")]
    pub primary_type: Option<String>,
    #[serde(rename = "secondary-types", default)]
    pub secondary_types: Option<Vec<String>>,
    #[serde(rename = "artist-credit", alias = "artist_credit", default)]
    pub artist_credit: Option<Vec<ArtistCredit>>,
    pub genres: Option<Vec<MusicBrainzGenre>>,
    pub tags: Option<Vec<MusicBrainzGenre>>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzArtistItem {
    pub id: String,
    pub name: String,
    pub score: Option<i32>,
    #[serde(rename = "type")]
    pub artist_type: Option<String>,
    pub country: Option<String>,
    pub disambiguation: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ArtistQueryResponse {
    pub artists: Option<Vec<MusicBrainzArtistItem>>,
}

#[derive(Debug, Deserialize)]
struct ReleaseQueryResponse {
    pub releases: Option<Vec<Release>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzArtistDetail {
    pub id: String,
    pub name: String,
    pub relations: Option<Vec<MusicBrainzRelation>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzRelation {
    #[serde(rename = "type")]
    pub relation_type: Option<String>,
    pub url: Option<MusicBrainzUrlResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzUrlResource {
    pub resource: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzReleaseWithMedia {
    pub id: String,
    pub title: String,
    pub date: Option<String>,
    pub barcode: Option<String>,
    pub media: Option<Vec<MusicBrainzMedium>>,
    #[serde(rename = "artist-credit", alias = "artist_credit", default)]
    pub artist_credit: Option<Vec<ArtistCredit>>,
    #[serde(rename = "label-info")]
    pub label_info: Option<Vec<LabelInfo>>,
    #[serde(rename = "release-group", alias = "release_group", default)]
    pub release_group: Option<ReleaseGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzMedium {
    pub position: Option<u32>,
    pub format: Option<String>,
    #[serde(rename = "track-count")]
    pub track_count: Option<u32>,
    pub tracks: Option<Vec<MusicBrainzReleaseTrack>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzReleaseTrack {
    pub id: String,
    pub position: Option<u32>,
    pub number: Option<String>,
    pub title: String,
    pub length: Option<i64>,
    pub recording: Option<MusicBrainzRecordingSummary>,
    #[serde(rename = "artist-credit", alias = "artist_credit", default)]
    pub artist_credit: Option<Vec<ArtistCredit>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicBrainzRecordingSummary {
    pub id: String,
    pub title: String,
    pub length: Option<i64>,
    #[serde(rename = "first-release-date")]
    pub first_release_date: Option<String>,
    #[serde(rename = "artist-credit", alias = "artist_credit", default)]
    pub artist_credit: Option<Vec<ArtistCredit>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GhostArtistReport {
    pub duplicates_merged: usize,
    pub musicbrainz_resolved: usize,
    pub external_ids_linked: usize,
    pub total_processed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StubAlbumHydrationReport {
    pub duplicate_stubs_merged: usize,
    pub albums_hydrated: usize,
    pub tracks_inserted: usize,
    pub total_processed: usize,
}

use std::sync::RwLock;
use std::collections::HashMap;

/// In-memory cache for MusicBrainz lookups
static MB_QUERY_CACHE: RwLock<Option<HashMap<String, Option<MusicBrainzRecording>>>> = RwLock::new(None);

/// Clear MusicBrainz in-memory cache
#[allow(dead_code)]
pub fn clear_musicbrainz_cache() {
    if let Ok(mut guard) = MB_QUERY_CACHE.write() {
        *guard = Some(HashMap::new());
    }
}

/// Set a cached entry in the MusicBrainz query cache
#[allow(dead_code)]
pub fn set_cached_musicbrainz_recording(key: &str, recording: Option<MusicBrainzRecording>) {
    if let Ok(mut guard) = MB_QUERY_CACHE.write() {
        let cache = guard.get_or_insert_with(HashMap::new);
        cache.insert(key.to_string(), recording);
    }
}

/// MusicBrainz API client with rate limiting and in-memory response caching
pub struct MusicBrainzClient {
    client: Client,
    last_request: std::sync::Mutex<std::time::Instant>,
}

impl MusicBrainzClient {
    pub fn new() -> Self {
        Self {
            client: crate::download::http_client::create_http_client(),
            last_request: std::sync::Mutex::new(std::time::Instant::now() - Duration::from_secs(2)),
        }
    }

    /// Enforce MusicBrainz rate limit (1 request per second)
    async fn rate_limit(&self) {
        let elapsed = {
            let last = self.last_request.lock().unwrap_or_else(|e| e.into_inner());
            last.elapsed()
        };

        if elapsed < Duration::from_millis(1100) {
            sleep(Duration::from_millis(1100) - elapsed).await;
        }

        *self.last_request.lock().unwrap_or_else(|e| e.into_inner()) = std::time::Instant::now();
    }

    /// Look up a recording by ISRC with in-memory caching
    pub async fn lookup_by_isrc(&self, isrc: &str) -> Result<Option<MusicBrainzRecording>, String> {
        let trimmed_isrc = isrc.trim();
        if trimmed_isrc.is_empty() {
            return Ok(None);
        }

        let cache_key = format!("isrc:{}", trimmed_isrc);

        // Check cache
        let cached_opt: Option<Option<MusicBrainzRecording>> = if let Ok(guard) = MB_QUERY_CACHE.read() {
            guard.as_ref().and_then(|c: &HashMap<String, Option<MusicBrainzRecording>>| c.get(&cache_key).cloned())
        } else {
            None
        };

        if let Some(cached) = cached_opt {
            tracing::debug!("[MusicBrainz] Reusing cached lookup for ISRC {}", trimmed_isrc);
            return Ok(cached);
        }

        self.rate_limit().await;

        // Use recording query with ISRC instead of /isrc/ endpoint
        let url = format!(
            "{}/recording?query=isrc:{}&fmt=json",
            MUSICBRAINZ_API_BASE, trimmed_isrc
        );

        tracing::debug!("MusicBrainz lookup: {}", url);

        let response = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| format!("MusicBrainz request failed: {}", e))?;

        if response.status() == 404 {
            tracing::debug!("ISRC {} not found in MusicBrainz", trimmed_isrc);
            if let Ok(mut guard) = MB_QUERY_CACHE.write() {
                let cache = guard.get_or_insert_with(HashMap::new);
                cache.insert(cache_key, None);
            }
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
        let result = data
            .recordings
            .and_then(|r: Vec<MusicBrainzRecording>| r.into_iter().next());

        // Cache result
        if let Ok(mut guard) = MB_QUERY_CACHE.write() {
            let cache = guard.get_or_insert_with(HashMap::new);
            cache.insert(cache_key, result.clone());
        }

        Ok(result)
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

    /// Search for recordings by title and artist with in-memory caching
    pub async fn search_recordings(
        &self,
        title: &str,
        artist: &str,
        album: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MusicBrainzRecording>, String> {
        let cache_key = format!(
            "search:{}:::{}:::{}",
            artist.trim().to_lowercase(),
            title.trim().to_lowercase(),
            album.unwrap_or("").trim().to_lowercase()
        );

        let cached_search: Option<Option<MusicBrainzRecording>> = if let Ok(guard) = MB_QUERY_CACHE.read() {
            guard.as_ref().and_then(|c: &HashMap<String, Option<MusicBrainzRecording>>| c.get(&cache_key).cloned())
        } else {
            None
        };

        if let Some(Some(rec)) = cached_search {
            tracing::debug!("[MusicBrainz] Reusing cached search for {} - {}", artist, title);
            return Ok(vec![rec]);
        }

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
            .header("User-Agent", USER_AGENT)
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

        let list = data.recordings.unwrap_or_default();

        if let Ok(mut guard) = MB_QUERY_CACHE.write() {
            let cache = guard.get_or_insert_with(HashMap::new);
            cache.insert(cache_key, list.first().cloned());
        }

        Ok(list)
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
    #[allow(dead_code)]
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
                    if !FieldValidator::is_valid_musicbrainz_id(&recording.id) {
                        tracing::warn!("Rejecting invalid or synthetic MBID for track {}: {}", track_id, recording.id);
                        failed += 1;
                        continue;
                    }

                    // Update track with MusicBrainz ID, genre, and release year if available
                    let mb_genre = recording.genres.as_ref()
                        .and_then(|g| g.first().map(|g| g.name.as_str()))
                        .or_else(|| recording.tags.as_ref().and_then(|t| t.first().map(|t| t.name.as_str())))
                        .and_then(crate::services::enrichment::clean_primary_genre);

                    let mb_date = recording.releases.as_ref()
                        .and_then(|rels| rels.first().and_then(|r| r.date.as_deref()));
                    let mb_year = mb_date.and_then(|d| d.get(..4).and_then(|y| y.parse::<i32>().ok()));

                    let result = sqlx::query(
                        r#"
                        UPDATE tracks SET
                            musicbrainz_id = ?,
                            genre = COALESCE(genre, ?),
                            release_year = COALESCE(release_year, ?)
                        WHERE id = ?
                        "#
                    )
                    .bind(&recording.id)
                    .bind(mb_genre.as_deref())
                    .bind(mb_year)
                    .bind(track_id)
                    .execute(db)
                    .await;

                    if let Some(d) = mb_date {
                        let _ = sqlx::query(
                            r#"
                            UPDATE albums
                            SET release_date = COALESCE(release_date, ?)
                            WHERE id = (SELECT album_id FROM tracks WHERE id = ?)
                              AND (release_date IS NULL OR release_date = '')
                            "#
                        )
                        .bind(d)
                        .bind(track_id)
                        .execute(db)
                        .await;
                    }

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

    /// Search for an artist by name
    pub async fn search_artist(&self, name: &str) -> Result<Option<MusicBrainzArtistItem>, String> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        self.rate_limit().await;

        let query = format!("artist:\"{}\"", escape_lucene(trimmed));
        let url = format!(
            "{}/artist?query={}&fmt=json&limit=5",
            MUSICBRAINZ_API_BASE,
            urlencoding::encode(&query)
        );

        let response = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| format!("MusicBrainz artist search failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("MusicBrainz returned {}", response.status()));
        }

        let data: ArtistQueryResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse MusicBrainz response: {}", e))?;

        let first = data.artists.and_then(|list| list.into_iter().next());
        Ok(first)
    }

    /// Get streaming/external IDs (Spotify, Tidal) from artist URL relationships
    pub async fn get_artist_external_ids(&self, mbid: &str) -> Result<(Option<String>, Option<String>), String> {
        if mbid.is_empty() {
            return Ok((None, None));
        }

        self.rate_limit().await;

        let url = format!("{}/artist/{}?inc=url-rels&fmt=json", MUSICBRAINZ_API_BASE, mbid);

        let response = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| format!("MusicBrainz artist rels failed: {}", e))?;

        if !response.status().is_success() {
            return Ok((None, None));
        }

        let data: MusicBrainzArtistDetail = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse MusicBrainz rels response: {}", e))?;

        let mut spotify_id = None;
        let mut tidal_id = None;

        if let Some(rels) = data.relations {
            for rel in rels {
                if let Some(url_obj) = rel.url {
                    let r = url_obj.resource;
                    if r.contains("open.spotify.com/artist/") {
                        if let Some(id) = r.split("/artist/").nth(1) {
                            let clean_id = id.split('?').next().unwrap_or(id);
                            spotify_id = Some(clean_id.to_string());
                        }
                    } else if r.contains("tidal.com/artist/") {
                        if let Some(id) = r.split("/artist/").nth(1) {
                            let clean_id = id.split('?').next().unwrap_or(id);
                            tidal_id = Some(clean_id.to_string());
                        }
                    }
                }
            }
        }

        Ok((spotify_id, tidal_id))
    }

    /// Search for release by barcode / UPC
    pub async fn search_release_by_barcode(&self, barcode: &str) -> Result<Option<Release>, String> {
        let trimmed = barcode.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        self.rate_limit().await;

        let url = format!(
            "{}/release?query=barcode:{}&fmt=json&limit=5",
            MUSICBRAINZ_API_BASE,
            urlencoding::encode(trimmed)
        );

        let response = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| format!("MusicBrainz release barcode failed: {}", e))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let data: ReleaseQueryResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse MusicBrainz response: {}", e))?;

        let first = data.releases.and_then(|list| list.into_iter().next());
        Ok(first)
    }

    /// Search for release by title and artist
    pub async fn search_release_by_title_and_artist(&self, title: &str, artist: &str) -> Result<Option<Release>, String> {
        let trimmed_t = title.trim();
        let trimmed_a = artist.trim();
        if trimmed_t.is_empty() {
            return Ok(None);
        }

        self.rate_limit().await;

        let query = if !trimmed_a.is_empty() {
            format!("release:\"{}\" AND artist:\"{}\"", escape_lucene(trimmed_t), escape_lucene(trimmed_a))
        } else {
            format!("release:\"{}\"", escape_lucene(trimmed_t))
        };

        let url = format!(
            "{}/release?query={}&fmt=json&limit=5",
            MUSICBRAINZ_API_BASE,
            urlencoding::encode(&query)
        );

        let response = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| format!("MusicBrainz release search failed: {}", e))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let data: ReleaseQueryResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse MusicBrainz response: {}", e))?;

        let first = data.releases.and_then(|list| list.into_iter().next());
        Ok(first)
    }

    /// Fetch full release details including media and tracklist
    pub async fn get_release_with_tracks(&self, release_mbid: &str) -> Result<Option<MusicBrainzReleaseWithMedia>, String> {
        if release_mbid.is_empty() {
            return Ok(None);
        }

        self.rate_limit().await;

        let url = format!(
            "{}/release/{}?inc=recordings+artists+labels&fmt=json",
            MUSICBRAINZ_API_BASE, release_mbid
        );

        let response = self
            .client
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .send()
            .await
            .map_err(|e| format!("MusicBrainz release detail failed: {}", e))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let data: MusicBrainzReleaseWithMedia = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse MusicBrainz release with tracks: {}", e))?;

        Ok(Some(data))
    }

    /// Resolve ghost favorite artists: merge casing duplicates into populated library artists,
    /// and resolve standalone favorite artists against MusicBrainz linking MBIDs and external IDs.
    pub async fn resolve_ghost_artists(&self, db: &sqlx::SqlitePool) -> Result<GhostArtistReport, String> {
        let mut report = GhostArtistReport::default();

        let ghost_artists: Vec<(i64, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT g.id, g.name, g.favorite_at
            FROM artists g
            WHERE g.is_favorite = 1
              AND g.id NOT IN (SELECT DISTINCT artist_id FROM track_artists)
              AND g.id NOT IN (SELECT DISTINCT artist_id FROM album_artists)
            ORDER BY g.id
            "#
        )
        .fetch_all(db)
        .await
        .map_err(|e| format!("DB query failed: {}", e))?;

        report.total_processed = ghost_artists.len();

        for (ghost_id, ghost_name, ghost_fav_at) in ghost_artists {
            let existing_match: Option<(i64, String)> = sqlx::query_as(
                r#"
                SELECT a.id, a.name
                FROM artists a
                WHERE LOWER(a.name) = LOWER(?) AND a.id != ?
                  AND (a.id IN (SELECT artist_id FROM track_artists) OR a.id IN (SELECT artist_id FROM album_artists))
                LIMIT 1
                "#
            )
            .bind(&ghost_name)
            .bind(ghost_id)
            .fetch_optional(db)
            .await
            .unwrap_or(None);

            if let Some((target_id, _target_name)) = existing_match {
                let _ = sqlx::query(
                    "UPDATE artists SET is_favorite = 1, favorite_at = COALESCE(favorite_at, ?) WHERE id = ?"
                )
                .bind(ghost_fav_at)
                .bind(target_id)
                .execute(db)
                .await;

                let _ = sqlx::query("UPDATE OR IGNORE track_credits SET artist_id = ? WHERE artist_id = ?")
                    .bind(target_id)
                    .bind(ghost_id)
                    .execute(db)
                    .await;
                let _ = sqlx::query("DELETE FROM track_credits WHERE artist_id = ?")
                    .bind(ghost_id)
                    .execute(db)
                    .await;

                let _ = sqlx::query("DELETE FROM artists WHERE id = ?")
                    .bind(ghost_id)
                    .execute(db)
                    .await;

                report.duplicates_merged += 1;
            } else {
                match self.search_artist(&ghost_name).await {
                    Ok(Some(mb_artist)) => {
                        let mbid = mb_artist.id;
                        if !FieldValidator::is_valid_musicbrainz_artist_id(&mbid, Some(&ghost_name)) {
                            tracing::warn!("Rejecting invalid or synthetic MBID from API for artist {}: {}", ghost_name, mbid);
                            continue;
                        }

                        let (spotify_id, tidal_id) = self.get_artist_external_ids(&mbid).await.unwrap_or((None, None));
                        let has_ext = spotify_id.is_some() || tidal_id.is_some();

                        let _ = sqlx::query(
                            r#"
                            UPDATE artists
                            SET musicbrainz_id = ?,
                                spotify_id = COALESCE(spotify_id, ?),
                                tidal_id = COALESCE(tidal_id, ?)
                            WHERE id = ?
                            "#
                        )
                        .bind(&mbid)
                        .bind(spotify_id)
                        .bind(tidal_id)
                        .bind(ghost_id)
                        .execute(db)
                        .await;

                        report.musicbrainz_resolved += 1;
                        if has_ext {
                            report.external_ids_linked += 1;
                        }
                    }
                    Ok(None) => {
                        let _ = sqlx::query("UPDATE artists SET musicbrainz_id = 'NOT_FOUND' WHERE id = ?")
                            .bind(ghost_id)
                            .execute(db)
                            .await;
                    }
                    Err(e) => {
                        tracing::warn!("Failed MusicBrainz search for {}: {}", ghost_name, e);
                    }
                }
            }
        }

        Ok(report)
    }

    /// Hydrate stub favorite albums: merge duplicate stubs with populated counterparts,
    /// and fetch full tracklists for unpopulated stubs via MusicBrainz.
    pub async fn hydrate_stub_albums(&self, db: &sqlx::SqlitePool) -> Result<StubAlbumHydrationReport, String> {
        let mut report = StubAlbumHydrationReport::default();

        let stub_albums: Vec<(i64, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT a.id, a.title, a.release_date, a.cover_art_url, a.spotify_id, a.qobuz_id, a.tidal_id, a.upc
            FROM albums a
            WHERE a.is_favorite = 1
              AND a.id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)
            ORDER BY a.id
            "#
        )
        .fetch_all(db)
        .await
        .map_err(|e| format!("DB query failed: {}", e))?;

        report.total_processed = stub_albums.len();

        for (stub_id, stub_title, rel_date, cover_url, spot_id, qob_id, tid_id, upc) in stub_albums {
            let clean_upc = upc.as_deref().map(|u| u.trim_start_matches('0')).filter(|u| !u.is_empty());

            let target_populated: Option<(i64,)> = if let Some(u) = clean_upc {
                sqlx::query_as(
                    r#"
                    SELECT a.id FROM albums a
                    WHERE a.id != ?
                      AND LTRIM(a.upc, '0') = ?
                      AND a.id IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)
                    LIMIT 1
                    "#
                )
                .bind(stub_id)
                .bind(u)
                .fetch_optional(db)
                .await
                .unwrap_or(None)
            } else {
                sqlx::query_as(
                    r#"
                    SELECT a.id FROM albums a
                    WHERE a.id != ?
                      AND LOWER(a.title) = LOWER(?)
                      AND a.id IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)
                    LIMIT 1
                    "#
                )
                .bind(stub_id)
                .bind(&stub_title)
                .fetch_optional(db)
                .await
                .unwrap_or(None)
            };

            if let Some((target_id,)) = target_populated {
                let _ = sqlx::query(
                    r#"
                    UPDATE albums
                    SET is_favorite = 1,
                        favorite_at = COALESCE(favorite_at, CURRENT_TIMESTAMP),
                        spotify_id = COALESCE(spotify_id, ?),
                        qobuz_id = COALESCE(qobuz_id, ?),
                        tidal_id = COALESCE(tidal_id, ?),
                        upc = COALESCE(upc, ?),
                        cover_art_url = COALESCE(cover_art_url, ?)
                    WHERE id = ?
                    "#
                )
                .bind(spot_id)
                .bind(qob_id)
                .bind(tid_id)
                .bind(upc)
                .bind(cover_url)
                .bind(target_id)
                .execute(db)
                .await;

                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO album_artists (album_id, artist_id) SELECT ?, artist_id FROM album_artists WHERE album_id = ?"
                )
                .bind(target_id)
                .bind(stub_id)
                .execute(db)
                .await;

                let _ = sqlx::query("DELETE FROM album_artists WHERE album_id = ?")
                    .bind(stub_id)
                    .execute(db)
                    .await;
                let _ = sqlx::query("DELETE FROM albums WHERE id = ?")
                    .bind(stub_id)
                    .execute(db)
                    .await;

                report.duplicate_stubs_merged += 1;
            } else {
                let artist_name_row: Option<(String,)> = sqlx::query_as(
                    r#"
                    SELECT ar.name FROM album_artists aa
                    JOIN artists ar ON ar.id = aa.artist_id
                    WHERE aa.album_id = ?
                    LIMIT 1
                    "#
                )
                .bind(stub_id)
                .fetch_optional(db)
                .await
                .unwrap_or(None);

                let artist_name = artist_name_row.map(|r| r.0).unwrap_or_default();

                let mb_release = if let Some(u) = clean_upc {
                    self.search_release_by_barcode(u).await.unwrap_or(None)
                } else {
                    self.search_release_by_title_and_artist(&stub_title, &artist_name).await.unwrap_or(None)
                };

                if let Some(rel) = mb_release {
                    if let Ok(Some(full_rel)) = self.get_release_with_tracks(&rel.id).await {
                        let mut inserted_for_album = 0;
                        if let Some(media_list) = full_rel.media {
                            for medium in media_list {
                                let disc_num = medium.position.unwrap_or(1) as i32;
                                if let Some(tracks) = medium.tracks {
                                    for t in tracks {
                                        let track_num = t.position.unwrap_or(1) as i32;
                                        let duration = t.length;
                                        let rec_id = t.recording.as_ref()
                                            .map(|r| r.id.clone())
                                            .filter(|id| FieldValidator::is_valid_musicbrainz_id(id));

                                        let track_artist_name = t.artist_credit
                                            .as_ref()
                                            .and_then(|ac| ac.first().map(|a| {
                                                if !a.name.is_empty() {
                                                    a.name.clone()
                                                } else if !a.artist.name.is_empty() {
                                                    a.artist.name.clone()
                                                } else {
                                                    String::new()
                                                }
                                            }))
                                            .filter(|name| !name.is_empty())
                                            .unwrap_or_else(|| if artist_name.is_empty() { "Various Artists".to_string() } else { artist_name.clone() });

                                        let mut artist_id: Option<i64> = sqlx::query_scalar(
                                            "SELECT id FROM artists WHERE LOWER(name) = LOWER(?)"
                                        )
                                        .bind(&track_artist_name)
                                        .fetch_optional(db)
                                        .await
                                        .unwrap_or(None);

                                        let track_artist_mbid = t.artist_credit
                                            .as_ref()
                                            .and_then(|ac| ac.first())
                                            .map(|a| a.artist.id.as_str())
                                            .filter(|id| FieldValidator::is_valid_musicbrainz_artist_id(id, Some(&track_artist_name)));

                                        if artist_id.is_none() {
                                            artist_id = sqlx::query_scalar(
                                                "INSERT INTO artists (name, musicbrainz_id) VALUES (?, ?) RETURNING id"
                                            )
                                            .bind(&track_artist_name)
                                            .bind(track_artist_mbid)
                                            .fetch_optional(db)
                                            .await
                                            .unwrap_or(None);
                                        } else if let (Some(aid), Some(mbid)) = (artist_id, track_artist_mbid) {
                                            let _ = sqlx::query(
                                                "UPDATE artists SET musicbrainz_id = ? WHERE id = ? AND musicbrainz_id IS NULL"
                                            )
                                            .bind(mbid)
                                            .bind(aid)
                                            .execute(db)
                                            .await;
                                        }

                                        let mb_track_genre = rel.release_group.as_ref()
                                            .and_then(|rg| rg.genres.as_ref())
                                            .and_then(|g| g.first().map(|g| g.name.as_str()))
                                            .or_else(|| {
                                                rel.release_group.as_ref()
                                                    .and_then(|rg| rg.tags.as_ref())
                                                    .and_then(|t| t.first().map(|t| t.name.as_str()))
                                            })
                                            .and_then(crate::services::enrichment::clean_primary_genre);

                                        let effective_rel_date = rel_date.as_deref().or(rel.date.as_deref());
                                        let effective_track_year = effective_rel_date.and_then(|d| d.get(..4).and_then(|y| y.parse::<i32>().ok()));

                                        let track_id_res: Option<i64> = sqlx::query_scalar(
                                            r#"
                                            INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, musicbrainz_id, enrichment_status, release_year, genre)
                                            VALUES (?, ?, ?, ?, ?, ?, 'enriched', ?, ?)
                                            RETURNING id
                                            "#
                                        )
                                        .bind(&t.title)
                                        .bind(stub_id)
                                        .bind(duration)
                                        .bind(track_num)
                                        .bind(disc_num)
                                        .bind(rec_id)
                                        .bind(effective_track_year)
                                        .bind(mb_track_genre)
                                        .fetch_optional(db)
                                        .await
                                        .unwrap_or(None);

                                        if let (Some(tid), Some(aid)) = (track_id_res, artist_id) {
                                            let _ = sqlx::query(
                                                "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')"
                                            )
                                            .bind(tid)
                                            .bind(aid)
                                            .execute(db)
                                            .await;
                                            inserted_for_album += 1;
                                            report.tracks_inserted += 1;
                                        }
                                    }
                                }
                            }
                        }

                        if inserted_for_album > 0 {
                            let rel_mbid = if FieldValidator::is_valid_musicbrainz_id(&rel.id) {
                                Some(rel.id.as_str())
                            } else {
                                None
                            };

                            let effective_rel_date = rel_date.as_deref().or(rel.date.as_deref());
                            let _ = sqlx::query("UPDATE albums SET musicbrainz_id = COALESCE(?, musicbrainz_id), release_date = COALESCE(release_date, ?), total_tracks = ? WHERE id = ?")
                                .bind(rel_mbid)
                                .bind(effective_rel_date)
                                .bind(inserted_for_album as i32)
                                .bind(stub_id)
                                .execute(db)
                                .await;
                            report.albums_hydrated += 1;
                        }
                    }
                }
            }
        }

        Ok(report)
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

/// Extract MusicBrainz track/recording ID physically recorded in FLAC Vorbis comments (TASK-84).
pub fn extract_musicbrainz_track_id_from_flac(flac_path: &Path) -> Option<String> {
    if !flac_path.exists() || !flac_path.is_file() {
        return None;
    }
    let tag = metaflac::Tag::read_from_path(flac_path).ok()?;
    let vc = tag.vorbis_comments()?;
    vc.get("MUSICBRAINZ_TRACKID")
        .and_then(|v| v.first())
        .or_else(|| vc.get("MUSICBRAINZ_RELEASETRACKID").and_then(|v| v.first()))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Helper that reads physical Vorbis comments from a FLAC file and updates `tracks.musicbrainz_id` in SQLite (TASK-84).
pub async fn sync_flac_musicbrainz_id_to_track(
    db: &sqlx::SqlitePool,
    track_id: i64,
    flac_path: &Path,
) -> Result<Option<String>, String> {
    if let Some(mbid) = extract_musicbrainz_track_id_from_flac(flac_path) {
        sqlx::query("UPDATE tracks SET musicbrainz_id = ? WHERE id = ?")
            .bind(&mbid)
            .bind(track_id)
            .execute(db)
            .await
            .map_err(|e| format!("Failed to update track {}: {}", track_id, e))?;
        tracing::info!(track_id = track_id, mbid = %mbid, "[MusicBrainz] ✓ Synced physical Vorbis MBID to tracks table");
        Ok(Some(mbid))
    } else {
        Ok(None)
    }
}

/// Report detailing the results of MusicBrainz tag reconciliation across physical FLAC files (TASK-84).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct MusicBrainzTagReconciliationReport {
    pub scanned_files: usize,
    pub mbid_found_in_tags: usize,
    pub db_updated: usize,
    pub already_synchronized: usize,
    pub errors: Vec<String>,
}

/// Reconciles physical FLAC Vorbis comments with `tracks.musicbrainz_id` in SQLite (TASK-84).
/// If `path_override` is specified:
/// - A single file will be reconciled against the corresponding track in the database.
/// - A directory will be scanned recursively for `.flac` files.
/// If `path_override` is None, all FLAC files registered in the `downloads` ledger are checked.
pub async fn reconcile_musicbrainz_from_physical_flacs(
    db: &sqlx::SqlitePool,
    path_override: Option<&Path>,
) -> Result<MusicBrainzTagReconciliationReport, String> {
    let mut report = MusicBrainzTagReconciliationReport::default();

    if let Some(path) = path_override {
        if path.is_file() {
            report.scanned_files += 1;
            if let Some(mbid) = extract_musicbrainz_track_id_from_flac(path) {
                report.mbid_found_in_tags += 1;
                let path_str = path.to_string_lossy().to_string();

                // 1. Try resolving track by downloads.file_path
                let mut tid_opt: Option<i64> = sqlx::query_scalar(
                    "SELECT track_id FROM downloads WHERE file_path = ? LIMIT 1"
                )
                .bind(&path_str)
                .fetch_optional(db)
                .await
                .unwrap_or(None);

                // 2. Fallback: match by ISRC from Vorbis comments
                if tid_opt.is_none() {
                    if let Ok(tag) = metaflac::Tag::read_from_path(path) {
                        if let Some(vc) = tag.vorbis_comments() {
                            if let Some(isrc) = vc.get("ISRC").and_then(|v| v.first()).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                                tid_opt = sqlx::query_scalar("SELECT id FROM tracks WHERE isrc = ? LIMIT 1")
                                    .bind(isrc)
                                    .fetch_optional(db)
                                    .await
                                    .unwrap_or(None);
                            }
                        }
                    }
                }

                if let Some(tid) = tid_opt {
                    let current_mbid: Option<String> = sqlx::query_scalar(
                        "SELECT musicbrainz_id FROM tracks WHERE id = ?"
                    )
                    .bind(tid)
                    .fetch_optional(db)
                    .await
                    .unwrap_or(None)
                    .flatten();

                    if current_mbid.as_deref() != Some(&mbid) {
                        if let Err(e) = sqlx::query("UPDATE tracks SET musicbrainz_id = ? WHERE id = ?")
                            .bind(&mbid)
                            .bind(tid)
                            .execute(db)
                            .await
                        {
                            report.errors.push(format!("Failed to update track {}: {}", tid, e));
                        } else {
                            report.db_updated += 1;
                        }
                    } else {
                        report.already_synchronized += 1;
                    }
                }
            }
            return Ok(report);
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                let p = entry.path();
                if p.is_file() && p.extension().map(|e| e.to_string_lossy().to_lowercase() == "flac").unwrap_or(false) {
                    report.scanned_files += 1;
                    if let Some(mbid) = extract_musicbrainz_track_id_from_flac(p) {
                        report.mbid_found_in_tags += 1;
                        let path_str = p.to_string_lossy().to_string();

                        let mut tid_opt: Option<i64> = sqlx::query_scalar(
                            "SELECT track_id FROM downloads WHERE file_path = ? LIMIT 1"
                        )
                        .bind(&path_str)
                        .fetch_optional(db)
                        .await
                        .unwrap_or(None);

                        if tid_opt.is_none() {
                            if let Ok(tag) = metaflac::Tag::read_from_path(p) {
                                if let Some(vc) = tag.vorbis_comments() {
                                    if let Some(isrc) = vc.get("ISRC").and_then(|v| v.first()).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                                        tid_opt = sqlx::query_scalar("SELECT id FROM tracks WHERE isrc = ? LIMIT 1")
                                            .bind(isrc)
                                            .fetch_optional(db)
                                            .await
                                            .unwrap_or(None);
                                    }
                                }
                            }
                        }

                        if let Some(tid) = tid_opt {
                            let current_mbid: Option<String> = sqlx::query_scalar(
                                "SELECT musicbrainz_id FROM tracks WHERE id = ?"
                            )
                            .bind(tid)
                            .fetch_optional(db)
                            .await
                            .unwrap_or(None)
                            .flatten();

                            if current_mbid.as_deref() != Some(&mbid) {
                                if let Err(e) = sqlx::query("UPDATE tracks SET musicbrainz_id = ? WHERE id = ?")
                                    .bind(&mbid)
                                    .bind(tid)
                                    .execute(db)
                                    .await
                                {
                                    report.errors.push(format!("Failed to update track {}: {}", tid, e));
                                } else {
                                    report.db_updated += 1;
                                }
                            } else {
                                report.already_synchronized += 1;
                            }
                        }
                    }
                }
            }
            return Ok(report);
        }
    }

    // Default: query all recorded FLAC files from downloads ledger
    let rows: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT d.track_id, d.file_path, t.musicbrainz_id
        FROM downloads d
        JOIN tracks t ON t.id = d.track_id
        WHERE d.file_path LIKE '%.flac'
        "#
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Failed to query downloads for FLAC reconciliation: {}", e))?;

    for (tid, file_path_str, current_mbid) in rows {
        let p = Path::new(&file_path_str);
        if !p.exists() {
            continue;
        }
        report.scanned_files += 1;
        if let Some(mbid) = extract_musicbrainz_track_id_from_flac(p) {
            report.mbid_found_in_tags += 1;
            if current_mbid.as_deref() != Some(&mbid) {
                if let Err(e) = sqlx::query("UPDATE tracks SET musicbrainz_id = ? WHERE id = ?")
                    .bind(&mbid)
                    .bind(tid)
                    .execute(db)
                    .await
                {
                    report.errors.push(format!("Failed to update track {}: {}", tid, e));
                } else {
                    report.db_updated += 1;
                }
            } else {
                report.already_synchronized += 1;
            }
        }
    }

    Ok(report)
}

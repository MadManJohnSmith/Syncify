//! Discogs API v2.0 Client
//!
//! Handles Discogs release search, genre/style retrieval, and community stats
//! with rate limiting (60 req/min) and zero unwrap safety.

#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

const DISCOGS_API_BASE: &str = "https://api.discogs.com";

lazy_static::lazy_static! {
    static ref DISCOGS_LIMITER: crate::services::rate_limiter::RateLimiter = crate::services::rate_limiter::RateLimiter::new();
}

/// Discogs search item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscogsSearchResult {
    pub id: i64,
    pub title: Option<String>,
    pub year: Option<String>,
    pub country: Option<String>,
    pub genre: Option<Vec<String>>,
    pub style: Option<Vec<String>>,
    pub label: Option<Vec<String>>,
    pub master_id: Option<i64>,
    pub resource_url: Option<String>,
    pub community: Option<DiscogsCommunityStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscogsCommunityStats {
    pub have: Option<i64>,
    pub want: Option<i64>,
}

/// Discogs release details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscogsReleaseDetails {
    pub id: i64,
    pub title: Option<String>,
    pub genres: Vec<String>,
    pub styles: Vec<String>,
    pub country: Option<String>,
    pub year: Option<i64>,
    pub labels: Vec<String>,
    pub community_have: i64,
}

#[derive(Debug, Deserialize)]
struct DiscogsSearchResponse {
    results: Option<Vec<DiscogsSearchResult>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscogsRawRelease {
    id: i64,
    title: Option<String>,
    genres: Option<Vec<String>>,
    styles: Option<Vec<String>>,
    country: Option<String>,
    year: Option<i64>,
    labels: Option<Vec<DiscogsRawLabel>>,
    community: Option<DiscogsCommunityStats>,
}

#[derive(Debug, Clone, Deserialize)]
struct DiscogsRawLabel {
    name: Option<String>,
}

/// Discogs Client
pub struct DiscogsClient {
    client: Client,
    token: Option<String>,
}

impl DiscogsClient {
    /// Create a new Discogs client using DISCOGS_TOKEN env var if present
    pub fn new() -> Self {
        let token = std::env::var("DISCOGS_TOKEN").ok().filter(|t| !t.trim().is_empty());
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Syncify/1.0 +https://github.com/MadManJohnSmith/Syncify")
            .build()
            .unwrap_or_else(|_| Client::new());

        Self { client, token }
    }

    /// Create with explicit token
    pub fn with_token(token: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Syncify/1.0 +https://github.com/MadManJohnSmith/Syncify")
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            token: Some(token),
        }
    }

    /// Search Discogs for a release matching artist and album
    pub async fn search_release(
        &self,
        artist: &str,
        album: &str,
    ) -> Result<Option<DiscogsSearchResult>, String> {
        self.search_release_with_format(artist, album, None).await
    }

    /// Search Discogs for a release matching artist, album, and optional format (e.g. "Single", "EP")
    pub async fn search_release_with_format(
        &self,
        artist: &str,
        album: &str,
        format_type: Option<&str>,
    ) -> Result<Option<DiscogsSearchResult>, String> {
        DISCOGS_LIMITER.acquire("discogs").await;

        let query_str = format!("{} {}", artist, album);
        let mut url = match &self.token {
            Some(token) => format!(
                "{}/database/search?q={}&type=release&token={}",
                DISCOGS_API_BASE,
                urlencoding::encode(&query_str),
                token
            ),
            None => format!(
                "{}/database/search?q={}&type=release",
                DISCOGS_API_BASE,
                urlencoding::encode(&query_str)
            ),
        };

        if let Some(fmt) = format_type {
            url.push_str(&format!("&format={}", urlencoding::encode(fmt)));
        }

        info!("[Discogs] Querying API (format: {:?}): {} - {}", format_type, artist, album);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Discogs search request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            warn!("[Discogs] Search HTTP error: {}", status);
            return Ok(None);
        }

        let search_res: DiscogsSearchResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Discogs search response: {}", e))?;

        if let Some(results) = search_res.results {
            if let Some(first) = results.into_iter().next() {
                info!("[Discogs] Found release match ID {} ('{}')", first.id, first.title.as_deref().unwrap_or(""));
                return Ok(Some(first));
            }
        }

        info!("[Discogs] Search returned 0 matching releases for '{} - {}'", artist, album);
        Ok(None)
    }

    /// Get detailed release information by Discogs release ID
    pub async fn get_release(&self, release_id: i64) -> Result<DiscogsReleaseDetails, String> {
        let token = match &self.token {
            Some(t) => t,
            None => return Err("DISCOGS_TOKEN environment variable not set".to_string()),
        };

        DISCOGS_LIMITER.acquire("discogs").await;

        let url = format!("{}/releases/{}?token={}", DISCOGS_API_BASE, release_id, token);

        debug!("[Discogs] Fetching release details for ID {}", release_id);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Discogs release request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Discogs API error: HTTP {}", response.status()));
        }

        let raw: DiscogsRawRelease = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Discogs release JSON: {}", e))?;

        let labels = raw
            .labels
            .unwrap_or_default()
            .into_iter()
            .filter_map(|l| l.name)
            .collect();

        let community_have = raw.community.and_then(|c| c.have).unwrap_or(0);

        Ok(DiscogsReleaseDetails {
            id: raw.id,
            title: raw.title,
            genres: raw.genres.unwrap_or_default(),
            styles: raw.styles.unwrap_or_default(),
            country: raw.country,
            year: raw.year,
            labels,
            community_have,
        })
    }
}

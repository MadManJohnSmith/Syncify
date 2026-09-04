//! Last.fm API client for genre/tag enrichment
//!
//! Uses Last.fm's track.getTopTags API to fetch genre tags for tracks.

use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::time::sleep;

const LASTFM_API_BASE: &str = "https://ws.audioscrobbler.com/2.0";

/// Last.fm tag response
#[derive(Debug, Clone, Deserialize)]
pub struct LastFmTagResponse {
    pub toptags: Option<TopTags>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TopTags {
    pub tag: Option<Vec<LastFmTag>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LastFmTag {
    pub name: String,
    pub count: i32,
}

/// Last.fm API client with rate limiting
pub struct LastFmClient {
    client: Client,
    api_key: String,
    last_request: AtomicU64,
}

impl LastFmClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("Syncify/1.0.0 (https://github.com/syncify/syncify)")
                .build()
                .unwrap_or_else(|_| Client::new()),
            api_key,
            last_request: AtomicU64::new(0),
        }
    }

    /// Load API key from environment
    #[allow(dead_code)]
    pub fn from_env() -> Result<Self, String> {
        let api_key =
            std::env::var("LASTFM_API_KEY").map_err(|_| "LASTFM_API_KEY not set in environment")?;
        Ok(Self::new(api_key))
    }

    /// Enforce rate limit (5 requests per second max)
    async fn rate_limit(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let last = self.last_request.load(Ordering::SeqCst);
        let min_interval_ms = 200; // 5 req/sec

        if now < last + min_interval_ms {
            let wait = last + min_interval_ms - now;
            sleep(Duration::from_millis(wait)).await;
        }

        self.last_request.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            Ordering::SeqCst,
        );
    }

    /// Get top tags for a track
    pub async fn get_track_tags(
        &self,
        artist: &str,
        track: &str,
    ) -> Result<Vec<LastFmTag>, String> {
        if artist.is_empty() || track.is_empty() {
            return Ok(vec![]);
        }

        self.rate_limit().await;

        let url = format!(
            "{}/?method=track.gettoptags&artist={}&track={}&api_key={}&format=json",
            LASTFM_API_BASE,
            urlencoding::encode(artist),
            urlencoding::encode(track),
            self.api_key
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Last.fm request failed: {}", e))?;

        if !response.status().is_success() {
            return Ok(vec![]); // Return empty on error, don't fail
        }

        let data: LastFmTagResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse Last.fm response: {}", e))?;

        Ok(data.toptags.and_then(|t| t.tag).unwrap_or_default())
    }

    /// Extract genre from tags (first tag above threshold count)
    pub fn extract_genre(tags: &[LastFmTag]) -> Option<String> {
        // Common genre mappings from Last.fm tags
        let genre_map: HashMap<&str, &str> = [
            ("rock", "Rock"),
            ("pop", "Pop"),
            ("electronic", "Electronic"),
            ("hip-hop", "Hip-Hop"),
            ("hip hop", "Hip-Hop"),
            ("rap", "Hip-Hop"),
            ("jazz", "Jazz"),
            ("classical", "Classical"),
            ("metal", "Metal"),
            ("alternative", "Alternative"),
            ("indie", "Indie"),
            ("r&b", "R&B"),
            ("rnb", "R&B"),
            ("soul", "Soul"),
            ("country", "Country"),
            ("folk", "Folk"),
            ("blues", "Blues"),
            ("punk", "Punk"),
            ("reggae", "Reggae"),
            ("latin", "Latin"),
            ("dance", "Dance"),
            ("house", "House"),
            ("techno", "Techno"),
            ("ambient", "Ambient"),
            ("k-pop", "K-Pop"),
            ("kpop", "K-Pop"),
            ("j-pop", "J-Pop"),
            ("jpop", "J-Pop"),
        ]
        .into_iter()
        .collect();

        // Find first tag that matches a known genre
        for tag in tags.iter().take(10) {
            let tag_lower = tag.name.to_lowercase();
            if let Some(genre) = genre_map.get(tag_lower.as_str()) {
                return Some(genre.to_string());
            }

            // Check for partial matches
            for (pattern, genre) in &genre_map {
                if tag_lower.contains(pattern) {
                    return Some(genre.to_string());
                }
            }
        }

        // Fallback: use first tag if count is high enough
        tags.first()
            .filter(|t| t.count >= 50)
            .map(|t| t.name.clone())
    }

    /// Extract subgenre from tags (second distinct genre tag)
    pub fn extract_subgenre(tags: &[LastFmTag], primary_genre: Option<&str>) -> Option<String> {
        let primary_lower = primary_genre.map(|g| g.to_lowercase());

        tags.iter()
            .take(5)
            .filter(|t| t.count >= 20)
            .map(|t| t.name.clone())
            .find(|name| {
                let name_lower = name.to_lowercase();
                primary_lower
                    .as_ref()
                    .map(|p| !name_lower.contains(p))
                    .unwrap_or(true)
            })
    }
}

impl Default for LastFmClient {
    fn default() -> Self {
        Self::new(String::new())
    }
}

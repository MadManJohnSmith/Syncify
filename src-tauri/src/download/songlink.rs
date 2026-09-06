// SongLink/Odesli - cross-platform track matching

use crate::download::http_client::{create_http_client, SONGLINK_LIMITER};
use crate::download::progress::DownloadRequest;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Track availability across platforms
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct TrackAvailability {
    pub spotify_id: Option<String>,
    pub tidal: bool,
    pub qobuz: bool,
    pub amazon: bool,
    pub deezer: bool,
    pub tidal_id: Option<String>,
    pub qobuz_id: Option<String>,
    pub amazon_url: Option<String>,
    pub deezer_id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artist_name: Option<String>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
}

/// Type alias for TrackAvailability to match domain terminology
pub type SongLinkAvailability = TrackAvailability;

/// SongLink API response
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SongLinkResponse {
    #[serde(rename = "entityUniqueId")]
    pub entity_unique_id: Option<String>,
    #[serde(rename = "linksByPlatform")]
    pub links_by_platform: Option<HashMap<String, PlatformLink>>,
    #[serde(rename = "entitiesByUniqueId")]
    pub entities_by_unique_id: Option<HashMap<String, EntityInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlatformLink {
    pub url: Option<String>,
    #[serde(rename = "entityUniqueId")]
    pub entity_unique_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntityInfo {
    pub id: Option<String>,
    #[serde(rename = "apiProvider")]
    pub api_provider: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "artistName")]
    pub artist_name: Option<String>,
    #[serde(rename = "thumbnailUrl")]
    pub thumbnail_url: Option<String>,
}

impl TrackAvailability {
    /// Build TrackAvailability from parsed SongLinkResponse
    pub fn from_response(result: &SongLinkResponse, origin_spotify_id: Option<String>) -> Self {
        let mut availability = TrackAvailability {
            spotify_id: origin_spotify_id,
            ..Default::default()
        };

        if let Some(links) = &result.links_by_platform {
            let entities = result.entities_by_unique_id.as_ref();

            // Tidal
            if let Some(tidal) = links.get("tidal") {
                availability.tidal = true;
                availability.tidal_id = extract_platform_id(tidal, entities, "tidal");
            }

            // Qobuz
            if let Some(qobuz) = links.get("qobuz") {
                availability.qobuz = true;
                availability.qobuz_id = extract_platform_id(qobuz, entities, "qobuz");
            }

            // Amazon Music
            if let Some(amazon) = links.get("amazonMusic").or_else(|| links.get("amazon")) {
                availability.amazon = true;
                availability.amazon_url = amazon.url.clone();
            }

            // Deezer
            if let Some(deezer) = links.get("deezer") {
                availability.deezer = true;
                availability.deezer_id = extract_platform_id(deezer, entities, "deezer");
            }

            // Spotify (populate if not already set by caller)
            if let Some(spotify) = links.get("spotify") {
                if availability.spotify_id.is_none() {
                    availability.spotify_id = extract_platform_id(spotify, entities, "spotify");
                }
            }
        }

        if let Some(entities) = &result.entities_by_unique_id {
            let primary = result
                .entity_unique_id
                .as_ref()
                .and_then(|id| entities.get(id));
            if let Some(p) = primary {
                availability.title = p.title.clone();
                availability.artist_name = p.artist_name.clone();
                availability.thumbnail_url = p.thumbnail_url.clone();
            } else {
                for (_id, entity) in entities {
                    if entity.title.is_some() {
                        availability.title = entity.title.clone();
                        availability.artist_name = entity.artist_name.clone();
                        availability.thumbnail_url = entity.thumbnail_url.clone();
                        break;
                    }
                }
            }
        }

        availability
    }

    /// Parse SongLink JSON response or serialized TrackAvailability
    #[allow(dead_code)]
    pub fn parse_from_json(json_str: &str) -> Result<Self> {
        // 1. Try parsing directly as TrackAvailability (roundtrip check)
        if let Ok(avail) = serde_json::from_str::<TrackAvailability>(json_str) {
            if avail.tidal_id.is_some() || avail.qobuz_id.is_some() || avail.amazon_url.is_some() {
                return Ok(avail);
            }
        }

        // 2. Parse as full SongLink API response
        let response: SongLinkResponse = serde_json::from_str(json_str)
            .map_err(|e| anyhow!("Failed to parse SongLink response JSON: {}", e))?;
        Ok(Self::from_response(&response, None))
    }
}

/// SongLink client for cross-platform matching
pub struct SongLinkClient {
    client: Client,
    base_url: String,
}

#[allow(dead_code)]
impl SongLinkClient {
    pub fn new() -> Self {
        Self {
            client: create_http_client(),
            base_url: "https://api.song.link".to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.base_url = base_url;
        self
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Check track availability across platforms via Spotify ID
    pub async fn check_availability(
        &self,
        spotify_id: &str,
        _isrc: Option<&str>,
    ) -> Result<TrackAvailability> {
        SONGLINK_LIMITER.wait("songlink").await;

        let url = format!(
            "{}/v1-alpha.1/links?url=https://open.spotify.com/track/{}",
            self.base_url, spotify_id
        );

        debug!(
            "[SongLink] Checking availability for Spotify ID: {}",
            spotify_id
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "SongLink request failed: HTTP {}",
                response.status()
            ));
        }

        let result: SongLinkResponse = response.json().await?;
        let availability = TrackAvailability::from_response(&result, Some(spotify_id.to_string()));

        debug!(
            "[SongLink] Availability: Tidal={:?}, Qobuz={:?}, Amazon={}, Deezer={:?}",
            availability.tidal_id, availability.qobuz_id, availability.amazon, availability.deezer_id
        );

        Ok(availability)
    }

    /// Check availability from track URL across any platform
    pub async fn check_from_url(&self, track_url: &str) -> Result<TrackAvailability> {
        SONGLINK_LIMITER.wait("songlink").await;

        let encoded_url = urlencoding::encode(track_url);
        let url = format!(
            "{}/v1-alpha.1/links?url={}",
            self.base_url, encoded_url
        );

        debug!(
            "[SongLink] Checking availability from URL: {}",
            track_url
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "SongLink request failed: HTTP {}",
                response.status()
            ));
        }

        let result: SongLinkResponse = response.json().await?;
        Ok(TrackAvailability::from_response(&result, None))
    }

    /// Check availability from Deezer ID
    pub async fn check_from_deezer(&self, deezer_id: &str) -> Result<TrackAvailability> {
        SONGLINK_LIMITER.wait("songlink").await;

        let url = format!(
            "{}/v1-alpha.1/links?url=https://www.deezer.com/track/{}",
            self.base_url, deezer_id
        );

        debug!(
            "[SongLink] Checking availability from Deezer ID: {}",
            deezer_id
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "SongLink request failed: HTTP {}",
                response.status()
            ));
        }

        let result: SongLinkResponse = response.json().await?;
        let mut availability = TrackAvailability::from_response(&result, None);
        if availability.deezer_id.is_none() {
            availability.deezer = true;
            availability.deezer_id = Some(deezer_id.to_string());
        }

        Ok(availability)
    }

    /// Query availability using full DownloadRequest information
    pub async fn query_songlink(&self, request: &DownloadRequest) -> Result<TrackAvailability> {
        if let Some(spotify_id) = &request.spotify_id {
            let id = spotify_id.trim();
            if !id.is_empty() {
                return self.check_availability(id, request.isrc.as_deref()).await;
            }
        }

        let svc = request.service_name.as_deref().unwrap_or("").to_lowercase();
        if let Some(ref track_id) = request.service_track_id {
            let tid = track_id.trim();
            if !tid.is_empty() {
                if tid.starts_with("http://") || tid.starts_with("https://") {
                    return self.check_from_url(tid).await;
                }
                match svc.as_str() {
                    "spotify" => return self.check_availability(tid, request.isrc.as_deref()).await,
                    "deezer" => return self.check_from_deezer(tid).await,
                    "apple_music" | "apple" | "applemusic" => {
                        let url = format!("https://music.apple.com/us/song/{}", tid);
                        return self.check_from_url(&url).await;
                    }
                    _ => {
                        return self.check_availability(tid, request.isrc.as_deref()).await;
                    }
                }
            }
        }

        Err(anyhow!("No valid identifier or URL found in request to query SongLink"))
    }

    /// Get Qobuz track ID from Spotify ID
    pub async fn get_qobuz_id(&self, spotify_id: &str) -> Result<Option<String>> {
        let availability = self.check_availability(spotify_id, None).await?;
        Ok(availability.qobuz_id)
    }

    /// Get Tidal track ID from Spotify ID
    pub async fn get_tidal_id(&self, spotify_id: &str) -> Result<Option<String>> {
        let availability = self.check_availability(spotify_id, None).await?;
        Ok(availability.tidal_id)
    }

    /// Get Amazon Music URL from Spotify ID
    pub async fn get_amazon_url(&self, spotify_id: &str) -> Result<Option<String>> {
        let availability = self.check_availability(spotify_id, None).await?;
        Ok(availability.amazon_url)
    }
}

impl Default for SongLinkClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract platform track ID from link, entity info, or URL
pub fn extract_platform_id(
    link: &PlatformLink,
    entities: Option<&HashMap<String, EntityInfo>>,
    platform: &str,
) -> Option<String> {
    // 1. Check entitiesByUniqueId using entity_unique_id
    if let (Some(entity_id), Some(entities_map)) = (&link.entity_unique_id, entities) {
        if let Some(entity_info) = entities_map.get(entity_id) {
            if let Some(ref id) = entity_info.id {
                let trimmed = id.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    // 2. Check entity_unique_id (split by "::" or direct)
    if let Some(entity_id) = &link.entity_unique_id {
        if let Some(id) = extract_id_from_entity(entity_id) {
            return Some(id);
        }
    }

    // 3. Fallback: Parse from URL
    if let Some(ref url) = link.url {
        if let Some(id) = extract_id_from_url(url, platform) {
            return Some(id);
        }
    }

    None
}

/// Extract numeric/string ID from SongLink entity unique ID
pub fn extract_id_from_entity(entity_id: &str) -> Option<String> {
    let trimmed = entity_id.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Format: "PLATFORM_SONG::ID"
    if let Some(pos) = trimmed.rfind("::") {
        let after = trimmed[pos + 2..].trim();
        if !after.is_empty() {
            return Some(after.to_string());
        }
    }
    // If no "::", return trimmed if non-empty
    Some(trimmed.to_string())
}

/// Extract platform track ID from platform-specific URL
pub fn extract_id_from_url(url: &str, platform: &str) -> Option<String> {
    let clean_url = url.trim();
    if clean_url.is_empty() {
        return None;
    }

    match platform.to_lowercase().as_str() {
        "tidal" => {
            // e.g. https://tidal.com/browse/track/123456 or https://listen.tidal.com/track/123456
            if let Some(pos) = clean_url.find("/track/") {
                let rest = &clean_url[pos + 7..];
                let id: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !id.is_empty() {
                    return Some(id);
                }
            }
        }
        "qobuz" => {
            // e.g. https://open.qobuz.com/track/7891011 or http://play.qobuz.com/track/7891011
            if let Some(pos) = clean_url.find("/track/") {
                let rest = &clean_url[pos + 7..];
                let id: String = rest.chars().take_while(|c| c.is_alphanumeric()).collect();
                if !id.is_empty() {
                    return Some(id);
                }
            }
        }
        "deezer" => {
            // e.g. https://www.deezer.com/track/3135556
            if let Some(pos) = clean_url.find("/track/") {
                let rest = &clean_url[pos + 7..];
                let id: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !id.is_empty() {
                    return Some(id);
                }
            }
        }
        "spotify" => {
            // e.g. https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT
            if let Some(pos) = clean_url.find("/track/") {
                let rest = &clean_url[pos + 7..];
                let id: String = rest.chars().take_while(|c| c.is_alphanumeric()).collect();
                if !id.is_empty() {
                    return Some(id);
                }
            }
        }
        _ => {}
    }
    None
}

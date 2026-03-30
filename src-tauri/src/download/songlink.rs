// SongLink/Odesli - cross-platform track matching

use crate::download::http_client::{create_http_client, SONGLINK_LIMITER};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Track availability across platforms
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
}

/// SongLink API response
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SongLinkResponse {
    #[serde(rename = "entityUniqueId")]
    entity_unique_id: Option<String>,
    #[serde(rename = "linksByPlatform")]
    links_by_platform: Option<HashMap<String, PlatformLink>>,
    #[serde(rename = "entitiesByUniqueId")]
    entities_by_unique_id: Option<HashMap<String, EntityInfo>>,
}

#[derive(Debug, Deserialize)]
struct PlatformLink {
    url: Option<String>,
    #[serde(rename = "entityUniqueId")]
    entity_unique_id: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct EntityInfo {
    id: Option<String>,
    #[serde(rename = "apiProvider")]
    api_provider: Option<String>,
}

/// SongLink client for cross-platform matching
pub struct SongLinkClient {
    client: Client,
}

#[allow(dead_code)]
impl SongLinkClient {
    pub fn new() -> Self {
        Self {
            client: create_http_client(),
        }
    }

    /// Check track availability across platforms
    pub async fn check_availability(
        &self,
        spotify_id: &str,
        _isrc: Option<&str>,
    ) -> Result<TrackAvailability> {
        SONGLINK_LIMITER.wait("songlink").await;

        let url = format!(
            "https://api.song.link/v1-alpha.1/links?url=https://open.spotify.com/track/{}",
            spotify_id
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
        let mut availability = TrackAvailability {
            spotify_id: Some(spotify_id.to_string()),
            ..Default::default()
        };

        if let Some(links) = &result.links_by_platform {
            // Tidal
            if let Some(tidal) = links.get("tidal") {
                availability.tidal = true;
                if let Some(entity_id) = &tidal.entity_unique_id {
                    availability.tidal_id = extract_id_from_entity(entity_id);
                }
            }

            // Qobuz
            if let Some(qobuz) = links.get("qobuz") {
                availability.qobuz = true;
                if let Some(entity_id) = &qobuz.entity_unique_id {
                    availability.qobuz_id = extract_id_from_entity(entity_id);
                }
            }

            // Amazon Music
            if let Some(amazon) = links.get("amazonMusic") {
                availability.amazon = true;
                availability.amazon_url = amazon.url.clone();
            }

            // Deezer
            if let Some(deezer) = links.get("deezer") {
                availability.deezer = true;
                if let Some(entity_id) = &deezer.entity_unique_id {
                    availability.deezer_id = extract_id_from_entity(entity_id);
                }
            }
        }

        debug!(
            "[SongLink] Availability: Tidal={}, Qobuz={}, Amazon={}, Deezer={}",
            availability.tidal, availability.qobuz, availability.amazon, availability.deezer
        );

        Ok(availability)
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

    /// Check availability from Deezer ID
    pub async fn check_from_deezer(&self, deezer_id: &str) -> Result<TrackAvailability> {
        SONGLINK_LIMITER.wait("songlink").await;

        let url = format!(
            "https://api.song.link/v1-alpha.1/links?url=https://www.deezer.com/track/{}",
            deezer_id
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
        let mut availability = TrackAvailability {
            deezer: true,
            deezer_id: Some(deezer_id.to_string()),
            ..Default::default()
        };

        if let Some(links) = &result.links_by_platform {
            if let Some(tidal) = links.get("tidal") {
                availability.tidal = true;
                if let Some(entity_id) = &tidal.entity_unique_id {
                    availability.tidal_id = extract_id_from_entity(entity_id);
                }
            }
            if let Some(qobuz) = links.get("qobuz") {
                availability.qobuz = true;
                if let Some(entity_id) = &qobuz.entity_unique_id {
                    availability.qobuz_id = extract_id_from_entity(entity_id);
                }
            }
            if let Some(amazon) = links.get("amazonMusic") {
                availability.amazon = true;
                availability.amazon_url = amazon.url.clone();
            }
            if let Some(spotify) = links.get("spotify") {
                if let Some(entity_id) = &spotify.entity_unique_id {
                    availability.spotify_id = extract_id_from_entity(entity_id);
                }
            }
        }

        Ok(availability)
    }
}

impl Default for SongLinkClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract numeric/string ID from SongLink entity unique ID
fn extract_id_from_entity(entity_id: &str) -> Option<String> {
    // Format: "PLATFORM_SONG::ID" or similar
    entity_id.split("::").last().map(|s| s.to_string())
}

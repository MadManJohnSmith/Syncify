// Qobuz Favorites Client for Syncify
// Fetches favorite albums, tracks, and artists directly with standard Web Player authentication

use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use crate::services::qobuz::{QOBUZ_API_BASE, QOBUZ_APP_ID};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteItem {
    pub id: String,
    pub title: String,
    pub artist_name: String,
    pub item_type: String,
    pub hires: bool,
}

pub struct QobuzFavoritesClient {
    client: Client,
}

impl QobuzFavoritesClient {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .connect_timeout(Duration::from_secs(5))
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .build()
                .unwrap_or_default(),
        }
    }

    /// Fetch all favorite items of a given type (albums, tracks, artists)
    pub async fn fetch_favorites(
        &self,
        token: &str,
        fav_type: &str,
    ) -> Result<Vec<FavoriteItem>> {
        self.fetch_favorites_with_limit(token, fav_type, None).await
    }

    /// Fetch favorite items with an optional maximum limit
    pub async fn fetch_favorites_with_limit(
        &self,
        token: &str,
        fav_type: &str,
        max_limit: Option<usize>,
    ) -> Result<Vec<FavoriteItem>> {
        let mut all_items = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();
        let mut offset = 0;
        let limit = 500;

        loop {
            println!("   [Qobuz API] Fetching favorite {} (offset: {}, limit: {})...", fav_type, offset, limit);

            let url = format!(
                "{}/favorite/getUserFavorites?type={}&limit={}&offset={}",
                QOBUZ_API_BASE, fav_type, limit, offset
            );

            let res = match self
                .client
                .get(&url)
                .header("X-App-Id", QOBUZ_APP_ID)
                .header("X-User-Auth-Token", token)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("⚠️ [Qobuz API] Request error on offset {}: {}", offset, e);
                    if all_items.is_empty() {
                        return Err(anyhow!("Network/Timeout error fetching favorites: {}", e));
                    }
                    break;
                }
            };

            let status = res.status();
            if !status.is_success() {
                eprintln!("⚠️ [Qobuz API] HTTP {} returned for favorite/getUserFavorites?type={}", status, fav_type);
                if all_items.is_empty() {
                    return Err(anyhow!("Qobuz API error: HTTP {}", status));
                }
                break;
            }

            let json: Value = match res.json().await {
                Ok(j) => j,
                Err(e) => {
                    eprintln!("⚠️ [Qobuz API] JSON parse error: {}", e);
                    break;
                }
            };

            let items = json[fav_type]["items"].as_array()
                .or_else(|| json["items"].as_array())
                .or_else(|| json[fav_type].as_array());

            let items_arr = match items {
                Some(arr) if !arr.is_empty() => arr,
                _ => {
                    println!("   [Qobuz API] Reached end of favorite {} (total collected: {}).", fav_type, all_items.len());
                    break;
                }
            };

            let page_count = items_arr.len();
            for item in items_arr {
                let id = item["id"].as_str().map(|s| s.to_string())
                    .or_else(|| item["id"].as_i64().map(|n| n.to_string()))
                    .unwrap_or_default();

                let title = item["title"].as_str()
                    .or_else(|| item["name"].as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let artist_name = item["artist"]["name"].as_str()
                    .or_else(|| item["performer"]["name"].as_str())
                    .or_else(|| item["composer"]["name"].as_str())
                    .or_else(|| item["name"].as_str())
                    .unwrap_or("Unknown Artist")
                    .to_string();

                let hires = item["hires"].as_bool().unwrap_or(false)
                    || item["maximum_bit_depth"].as_i64().unwrap_or(16) > 16;

                if !id.is_empty() && seen_ids.insert(id.clone()) {
                    all_items.push(FavoriteItem {
                        id,
                        title,
                        artist_name,
                        item_type: fav_type.to_string(),
                        hires,
                    });

                    if let Some(max) = max_limit {
                        if all_items.len() >= max {
                            println!("   [Qobuz API] Reached requested limit of {} favorite {}.", max, fav_type);
                            return Ok(all_items);
                        }
                    }
                }
            }

            if page_count < limit {
                break;
            }

            offset += limit;
        }

        Ok(all_items)
    }
}

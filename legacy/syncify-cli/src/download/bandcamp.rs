// Bandcamp & Internet Archive Audio Vault Connector for Syncify
// Rescues independent, self-released, and unreleased albums

use anyhow::{anyhow, Result};
use reqwest::Client;
use tracing::info;

/// Bandcamp / Internet Archive track search result
#[derive(Debug, Clone)]
pub struct BandcampTrackResult {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub stream_url: String,
    pub format: String,
}

/// Search Internet Archive Audio Vault for self-released / rare albums
pub async fn search_internet_archive(
    client: &Client,
    artist: &str,
    album_or_track: &str,
) -> Result<Vec<BandcampTrackResult>> {
    let query = format!("title:({}) AND mediatype:(audio)", artist);
    let url = format!(
        "https://archive.org/advancedsearch.php?q={}&fl[]=identifier,title,creator,year&output=json",
        urlencoding::encode(&query)
    );

    info!("[InternetArchive] Searching Audio Vault for '{} - {}'...", artist, album_or_track);
    let req = client.get(&url).timeout(std::time::Duration::from_millis(3000));
    let res = req.send().await?;

    if !res.status().is_success() {
        return Err(anyhow!("Internet Archive HTTP {}", res.status()));
    }

    let json: serde_json::Value = res.json().await?;
    let docs = json["response"]["docs"].as_array().ok_or_else(|| anyhow!("No docs"))?;
    let mut results = Vec::new();

    for doc in docs {
        let title = doc["title"].as_str().unwrap_or("");
        let creator = doc["creator"].as_str().unwrap_or(artist);
        let id = doc["identifier"].as_str().unwrap_or("");

        if !id.is_empty() {
            results.push(BandcampTrackResult {
                title: title.to_string(),
                artist: creator.to_string(),
                album: album_or_track.to_string(),
                stream_url: format!("https://archive.org/details/{}", id),
                format: "FLAC/MP3".to_string(),
            });
        }
    }

    Ok(results)
}

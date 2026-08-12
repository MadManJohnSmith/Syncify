pub mod artist_info;
pub mod bandcamp;
pub mod favorites;
pub mod http_client;
pub mod layout;
pub mod lyrics;
pub mod playlist_resolver;
pub mod rescue;
pub mod songlink;
pub mod soulseek;
pub mod staging;

pub use artist_info::*;
pub use layout::LibraryLayout;
pub use lyrics::{LyricsClient, LyricsResponse};
pub use playlist_resolver::PlaylistResolver;

#[derive(Debug, Clone)]
pub struct TidalTrackStub {
    pub id: String,
}

pub struct TidalDownloader;
impl TidalDownloader {
    pub fn new() -> Self {
        Self
    }
    pub async fn search_by_isrc(&self, _isrc: &str, _idx: u32) -> anyhow::Result<TidalTrackStub> {
        Err(anyhow::anyhow!("Tidal not implemented"))
    }
    pub async fn search_by_metadata(&self, _title: &str, _artist: &str, _idx: u32) -> anyhow::Result<TidalTrackStub> {
        Err(anyhow::anyhow!("Tidal not implemented"))
    }
    pub async fn get_download_url(&self, _id: String) -> anyhow::Result<String> {
        Err(anyhow::anyhow!("Tidal not implemented"))
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct QobuzFavoriteItem {
    pub id: String,
    pub title: String,
    pub artist_name: String,
}

pub struct QobuzFavoritesClient;
impl QobuzFavoritesClient {
    pub fn new() -> Self {
        Self
    }
    pub async fn fetch_favorites(&self, _token: &str, _fav_type: &str) -> anyhow::Result<Vec<QobuzFavoriteItem>> {
        Ok(vec![])
    }
}

#[derive(Debug, Clone)]
pub struct MissingTrackInfo {
    pub title: String,
    pub track_number: u32,
    pub total_tracks: u32,
    pub disc_number: u32,
    pub total_discs: u32,
    pub isrc: Option<String>,
    pub duration_sec: f64,
}

pub async fn fetch_expected_release_tracklist(_client: &reqwest::Client, _artist: &str, _album: &str) -> anyhow::Result<Vec<MissingTrackInfo>> {
    Ok(vec![])
}

pub async fn rescue_missing_track(
    _client: &reqwest::Client,
    _artist: &str,
    _album: &str,
    _year: i32,
    _info: &MissingTrackInfo,
    _target_path: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    Err(anyhow::anyhow!("Rescue missing track pending"))
}

pub async fn download_animated_cover(
    _client: &reqwest::Client,
    _artist: &str,
    _album: &str,
    _output_dir: &std::path::Path,
) -> Option<std::path::PathBuf> {
    None
}

pub async fn download_goodies_booklet(
    _client: &reqwest::Client,
    _album_id: &str,
    _output_dir: &std::path::Path,
) -> anyhow::Result<Option<std::path::PathBuf>> {
    Ok(None)
}

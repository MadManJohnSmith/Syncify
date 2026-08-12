//! Qobuz service - Authentication and API integration (CLI Standalone)

#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const QOBUZ_APP_ID: &str = "798273057";
pub const QOBUZ_APP_SECRET: &str = "abb21364945c0583309667d13ca3d93a";
pub const QOBUZ_API_BASE: &str = "https://www.qobuz.com/api.json/0.2";

pub fn resolve_qobuz_app_id() -> String {
    std::env::var("QOBUZ_APP_ID").unwrap_or_else(|_| QOBUZ_APP_ID.to_string())
}

pub fn resolve_qobuz_app_secret() -> String {
    std::env::var("QOBUZ_APP_SECRET").unwrap_or_else(|_| QOBUZ_APP_SECRET.to_string())
}

/// Qobuz credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QobuzCredentials {
    pub user_auth_token: String,
    pub user_id: Option<String>,
}

/// Qobuz track from API
#[derive(Debug, Clone, Deserialize)]
pub struct QobuzTrack {
    pub id: i64,
    pub title: Option<String>,
    pub duration: i64,
    pub isrc: Option<String>,
    pub copyright: Option<String>,
    pub performers: Option<String>,
    pub composer: Option<QobuzArtist>,
    pub work: Option<String>,
    pub track_number: Option<i32>,
    pub media_number: Option<i32>,
    pub maximum_bit_depth: Option<i32>,
    pub maximum_sampling_rate: Option<f64>,
    pub performer: Option<QobuzArtist>,
    pub album: Option<QobuzAlbum>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzArtist {
    pub id: Option<i64>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzLabel {
    pub id: Option<i64>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzAlbum {
    pub id: String,
    pub title: Option<String>,
    pub released_at: Option<i64>,
    pub image: Option<QobuzImage>,
    pub label: Option<QobuzLabel>,
    pub upc: Option<String>,
    #[serde(default)]
    pub artist: Option<QobuzArtist>,
    #[serde(default)]
    pub tracks: Option<QobuzTracksContainer>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzImage {
    pub small: Option<String>,
    pub large: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzFavoritesResponse {
    pub tracks: QobuzTracksContainer,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzTracksContainer {
    pub items: Vec<QobuzTrack>,
    pub total: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzAlbumsResponse {
    pub albums: QobuzAlbumsContainer,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QobuzAlbumsContainer {
    pub items: Vec<QobuzAlbum>,
    pub total: i32,
}

pub fn score_qobuz_candidate(
    album_title: &str,
    album_artist: &str,
    performer: &str,
    _track_title: &str,
    version: &str,
    expected_artist: &str,
    is_hires: bool,
) -> i32 {
    let mut score = 0i32;
    let alb_lower = album_title.to_lowercase();
    let perf_lower = performer.to_lowercase();
    let exp_lower = expected_artist.to_lowercase();
    let ver_lower = version.to_lowercase();

    if !alb_lower.contains("live") && !alb_lower.contains("best of") && !alb_lower.contains("greatest hits") {
        score += 30;
    }
    if perf_lower.contains(&exp_lower) || album_artist.to_lowercase().contains(&exp_lower) {
        score += 40;
    }
    if !ver_lower.contains("remix") && !ver_lower.contains("live") {
        score += 20;
    }
    if is_hires {
        score += 10;
    }
    score
}

pub fn score_qobuz_release(
    album_title: &str,
    album_artist: &str,
    performer: &str,
    expected_artist: &str,
    is_hires: bool,
) -> i32 {
    score_qobuz_candidate(album_title, album_artist, performer, "", "", expected_artist, is_hires)
}

pub struct QobuzClient {
    client: Client,
    app_id: String,
    app_secret: String,
    user_auth_token: Option<String>,
}

impl QobuzClient {
    pub fn new(app_id: String, app_secret: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| Client::new()),
            app_id,
            app_secret,
            user_auth_token: None,
        }
    }

    pub fn with_auth_token(mut self, token: String) -> Self {
        self.user_auth_token = Some(token);
        self
    }
}

impl Default for QobuzClient {
    fn default() -> Self {
        Self::new(QOBUZ_APP_ID.to_string(), QOBUZ_APP_SECRET.to_string())
    }
}

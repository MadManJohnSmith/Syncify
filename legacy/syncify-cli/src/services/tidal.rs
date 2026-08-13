//! Tidal service - Authentication, data models, candidate scoring, and matching rules (CLI Standalone)

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Tidal Authentication Status Hierarchy
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TidalAuthStatus {
    /// Valid user token provided or stored for private library access
    UserToken(String),
    /// OAuth Client Credentials token acquired for public catalog access
    ClientCredentials(String),
    /// Authentication required but not available
    RequiresAuth,
    /// Tidal API service unavailable
    SourceUnavailable(String),
    /// General failure state
    Failed(String),
}

impl TidalAuthStatus {
    pub fn is_user_authenticated(&self) -> bool {
        matches!(self, TidalAuthStatus::UserToken(_))
    }

    pub fn can_access_public_catalog(&self) -> bool {
        matches!(self, TidalAuthStatus::UserToken(_) | TidalAuthStatus::ClientCredentials(_))
    }
}

/// Classification of Tidal Stream Sources
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StreamSourceType {
    TidalOfficial,
    TidalProxy(String),
    RequiresAuth,
    SourceUnavailable(String),
    Failed(String),
}

impl std::fmt::Display for StreamSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamSourceType::TidalOfficial => write!(f, "Tidal Official API"),
            StreamSourceType::TidalProxy(domain) => write!(f, "Tidal Proxy ({})", domain),
            StreamSourceType::RequiresAuth => write!(f, "Requires Authentication"),
            StreamSourceType::SourceUnavailable(reason) => write!(f, "Source Unavailable ({})", reason),
            StreamSourceType::Failed(reason) => write!(f, "Failed ({})", reason),
        }
    }
}

/// Tidal track data model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalTrack {
    pub id: i64,
    pub title: String,
    pub isrc: Option<String>,
    pub duration: i32,
    #[serde(rename = "audioQuality")]
    pub audio_quality: Option<String>,
    pub album: Option<TidalAlbum>,
    pub artist: Option<TidalArtist>,
    pub artists: Option<Vec<TidalArtist>>,
    #[serde(rename = "trackNumber")]
    pub track_number: Option<i32>,
    #[serde(rename = "mediaMetadata")]
    pub media_metadata: Option<TidalMediaMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalAlbum {
    pub id: Option<i64>,
    pub title: String,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    pub cover: Option<String>,
}

impl TidalAlbum {
    pub fn cover_url(&self) -> Option<String> {
        self.cover.as_ref().map(|c| {
            if c.starts_with("http") {
                c.clone()
            } else {
                format!("https://resources.tidal.com/images/{}/1280x1280.jpg", c.replace('-', "/"))
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalArtist {
    pub id: Option<i64>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TidalMediaMetadata {
    pub tags: Option<Vec<String>>,
}

/// Candidate scoring for smart studio origin matching
pub fn score_tidal_candidate(
    album_title: &str,
    album_artist: &str,
    performer: &str,
    track_title: &str,
    version: &str,
    expected_artist: &str,
    is_hires: bool,
) -> i32 {
    let mut score = 0i32;
    let alb_lower = album_title.to_lowercase();
    let perf_lower = performer.to_lowercase();
    let exp_lower = expected_artist.to_lowercase();
    let ver_lower = version.to_lowercase();
    let trk_lower = track_title.to_lowercase();

    // Deduct heavy points for compilations, live, acoustic, or remix when studio origin is preferred
    if !alb_lower.contains("live") && !alb_lower.contains("best of") && !alb_lower.contains("greatest hits") && !alb_lower.contains("compilation") {
        score += 30;
    } else {
        score -= 20;
    }

    if perf_lower.contains(&exp_lower) || album_artist.to_lowercase().contains(&exp_lower) {
        score += 40;
    }

    if !ver_lower.contains("remix") && !ver_lower.contains("live") && !ver_lower.contains("acoustic") && !trk_lower.contains("live") && !trk_lower.contains("remix") {
        score += 20;
    } else {
        score -= 25;
    }

    if is_hires {
        score += 10;
    }

    score
}

pub fn score_tidal_release(
    album_title: &str,
    album_artist: &str,
    performer: &str,
    expected_artist: &str,
    is_hires: bool,
) -> i32 {
    score_tidal_candidate(album_title, album_artist, performer, "", "", expected_artist, is_hires)
}

pub fn title_matches(expected: &str, found: &str) -> bool {
    let expected_clean = clean_title(expected);
    let found_clean = clean_title(found);
    expected_clean == found_clean
        || found_clean.contains(&expected_clean)
        || expected_clean.contains(&found_clean)
}

pub fn artist_matches(expected: &str, found: &str) -> bool {
    let expected_lower = expected.to_lowercase();
    let found_lower = found.to_lowercase();
    if expected_lower == found_lower {
        return true;
    }

    let expected_parts: Vec<&str> = expected_lower
        .split(&[',', ';', '&', '/', '|'][..])
        .collect();
    let found_parts: Vec<&str> = found_lower.split(&[',', ';', '&', '/', '|'][..]).collect();

    expected_parts
        .iter()
        .any(|ep| found_parts.iter().any(|fp| ep.trim() == fp.trim()))
}

pub fn clean_title(title: &str) -> String {
    let mut clean = title.to_lowercase();
    for suffix in [
        "(remaster",
        "(remastered",
        "(deluxe",
        "(live",
        "(remix",
        "- remaster",
        "- live",
    ] {
        if let Some(pos) = clean.find(suffix) {
            clean = clean[..pos].to_string();
        }
    }
    clean.trim().to_string()
}

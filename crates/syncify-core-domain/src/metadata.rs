//! Pure metadata models, extractors, candidate scoring, and matching rules.

use serde::{Deserialize, Serialize};

/// Tidal track data model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TidalTrack {
    pub id: i64,
    pub title: String,
    pub version: Option<String>,
    pub isrc: Option<String>,
    pub duration: i32,
    #[serde(rename = "audioQuality")]
    pub audio_quality: Option<String>,
    pub album: Option<TidalAlbum>,
    pub artist: Option<TidalArtist>,
    pub artists: Option<Vec<TidalArtist>>,
    #[serde(rename = "trackNumber")]
    pub track_number: Option<i32>,
    #[serde(rename = "volumeNumber")]
    pub volume_number: Option<i32>,
    #[serde(rename = "mediaMetadata")]
    pub media_metadata: Option<TidalMediaMetadata>,
}

impl TidalTrack {
    /// Return track title cleanly formatted.
    pub fn clean_title(&self) -> String {
        self.title.trim().to_string()
    }

    /// Return track artist name if present.
    pub fn artist_name(&self) -> Option<String> {
        self.artist
            .as_ref()
            .map(|a| a.name.clone())
            .or_else(|| self.artists.as_ref().and_then(|arr| arr.first()).map(|a| a.name.clone()))
    }

    /// Return album title ONLY if album is present; NEVER fall back to track title!
    pub fn album_title(&self) -> Option<String> {
        self.album.as_ref().map(|a| a.title.trim().to_string())
    }

    /// Return album artist name if available, or track artist.
    pub fn album_artist_name(&self) -> Option<String> {
        self.album
            .as_ref()
            .and_then(|a| a.artist.as_ref().map(|art| art.name.clone()))
            .or_else(|| {
                self.album
                    .as_ref()
                    .and_then(|a| a.artists.as_ref().and_then(|arr| arr.first()).map(|art| art.name.clone()))
            })
            .or_else(|| self.artist_name())
    }

    /// Return release ID (album ID) if present.
    pub fn release_id(&self) -> Option<i64> {
        self.album.as_ref().and_then(|a| a.id)
    }

    /// Return track number (default 1).
    pub fn get_track_number(&self) -> i32 {
        self.track_number.unwrap_or(1)
    }

    /// Return disc/volume number (default 1).
    pub fn get_disc_number(&self) -> i32 {
        self.volume_number.unwrap_or(1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TidalAlbum {
    pub id: Option<i64>,
    pub title: String,
    #[serde(rename = "releaseDate")]
    pub release_date: Option<String>,
    pub cover: Option<String>,
    pub artist: Option<TidalArtist>,
    pub artists: Option<Vec<TidalArtist>>,
}

impl TidalAlbum {
    pub fn cover_url(&self) -> Option<String> {
        self.cover.as_ref().map(|c| {
            if c.starts_with("http") {
                c.clone()
            } else {
                format!(
                    "https://resources.tidal.com/images/{}/1280x1280.jpg",
                    c.replace('-', "/")
                )
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TidalArtist {
    pub id: Option<i64>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TidalMediaMetadata {
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct TidalSearchResponse {
    pub tracks: Option<TidalSearchTracks>,
}

#[derive(Debug, Deserialize)]
pub struct TidalSearchTracks {
    pub items: Vec<TidalTrack>,
}

/// Candidate scoring for smart studio origin matching.
pub fn score_tidal_candidate(
    album_title: &str,
    _album_artist: &str,
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

    if perf_lower.contains(&exp_lower) || exp_lower.contains(&perf_lower) {
        score += 30;
    }

    if is_hires {
        score += 20;
    }

    let live_keywords = ["live", "en vivo", "in concert", "bbc sessions", "bootleg", "tour"];
    let is_live_expected = live_keywords.iter().any(|k| trk_lower.contains(k));
    let is_live_album = live_keywords.iter().any(|k| alb_lower.contains(k) || ver_lower.contains(k));

    if !is_live_expected && is_live_album {
        score -= 50;
    }

    let studio_keywords = ["remaster", "remastered", "deluxe", "expanded", "studio", "original"];
    if studio_keywords.iter().any(|k| alb_lower.contains(k) || ver_lower.contains(k)) {
        score += 15;
    }

    score
}

pub fn clean_title(title: &str) -> String {
    let mut clean = title.to_string();
    for suffix in &[" (Remaster", " (Deluxe", " - Remaster", " - Live", " (Live"] {
        if let Some(pos) = clean.find(suffix) {
            clean.truncate(pos);
        }
    }
    clean.trim().to_lowercase()
}

pub fn title_matches(expected: &str, candidate: &str) -> bool {
    let clean_exp = clean_title(expected).to_lowercase();
    let clean_cand = clean_title(candidate).to_lowercase();
    clean_exp == clean_cand || clean_cand.contains(&clean_exp) || clean_exp.contains(&clean_cand)
}

pub fn artist_matches(expected: &str, candidate: &str) -> bool {
    let exp_low = expected.to_lowercase();
    let cand_low = candidate.to_lowercase();
    exp_low == cand_low || cand_low.contains(&exp_low) || exp_low.contains(&cand_low)
}

pub fn score_tidal_release(track: &TidalTrack, expected_artist: &str) -> i32 {
    let alb_title = track.album.as_ref().map(|a| a.title.as_str()).unwrap_or("");
    let perf_name = track.artist.as_ref().map(|a| a.name.as_str()).unwrap_or("");
    let is_hires = track.audio_quality.as_deref() == Some("HI_RES_LOSSLESS")
        || track.audio_quality.as_deref() == Some("HI_RES");

    score_tidal_candidate(alb_title, perf_name, perf_name, &track.title, "", expected_artist, is_hires)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tidal_track_album_title_does_not_fallback_to_title() {
        let track_without_album = TidalTrack {
            id: 12345,
            title: "Standalone Single".to_string(),
            version: None,
            isrc: Some("US1234567890".to_string()),
            duration: 210,
            audio_quality: Some("HI_RES_LOSSLESS".to_string()),
            album: None,
            artist: Some(TidalArtist {
                id: Some(1),
                name: "Artist".to_string(),
            }),
            artists: None,
            track_number: Some(1),
            volume_number: Some(1),
            media_metadata: None,
        };

        // Must strictly return None, NOT "Standalone Single"!
        assert_eq!(track_without_album.album_title(), None);
        assert_eq!(track_without_album.clean_title(), "Standalone Single");
        assert_eq!(track_without_album.artist_name(), Some("Artist".to_string()));
    }

    #[test]
    fn test_clean_title_remaster_strip() {
        assert_eq!(clean_title("Heroes - Remastered"), "heroes");
        assert_eq!(clean_title("Heroes (Remastered 2017)"), "heroes");
        assert_eq!(clean_title("Apologize"), "apologize");
    }

    #[test]
    fn test_scoring_studio_vs_live() {
        let studio_score = score_tidal_candidate(
            "Heroes (Deluxe Edition)",
            "David Bowie",
            "David Bowie",
            "Heroes",
            "",
            "David Bowie",
            true,
        );

        let live_score = score_tidal_candidate(
            "Live in Berlin 1978",
            "David Bowie",
            "David Bowie",
            "Heroes",
            "",
            "David Bowie",
            true,
        );

        assert!(studio_score > live_score, "Studio candidate should score higher than live album when studio expected");
    }
}

//! Pure metadata models, extractors, candidate scoring, and matching rules.

use serde::{Deserialize, Serialize};

/// Canonical status for track identity resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdentityResolutionStatus {
    Canonical,
    Partial,
    Ambiguous,
    Deferred,
    Unavailable,
    InvalidProviderPayload,
    Conflict,
    RepairRequired,
}

impl std::fmt::Display for IdentityResolutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdentityResolutionStatus::Canonical => write!(f, "Canonical"),
            IdentityResolutionStatus::Partial => write!(f, "Partial"),
            IdentityResolutionStatus::Ambiguous => write!(f, "Ambiguous"),
            IdentityResolutionStatus::Deferred => write!(f, "Deferred"),
            IdentityResolutionStatus::Unavailable => write!(f, "Unavailable"),
            IdentityResolutionStatus::InvalidProviderPayload => write!(f, "InvalidProviderPayload"),
            IdentityResolutionStatus::Conflict => write!(f, "Conflict"),
            IdentityResolutionStatus::RepairRequired => write!(f, "RepairRequired"),
        }
    }
}

/// Provider-specific track identity payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ProviderTrackIdentity {
    pub service_id: i64,
    pub service_name: String,
    pub service_track_id: String,
    pub isrc: Option<String>,
    pub provider_album_id: Option<String>,
    pub provider_artist_id: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub explicit: Option<bool>,
}

impl ProviderTrackIdentity {
    /// Validate if ISRC is syntactically valid; if not, returns None.
    pub fn sanitized_isrc(&self) -> Option<String> {
        self.isrc.as_deref().and_then(|c| {
            let trimmed = c.trim();
            if is_valid_isrc(trimmed) {
                Some(trimmed.to_uppercase())
            } else {
                None
            }
        })
    }

    /// Check if minimum metadata exists without relying on placeholders.
    pub fn has_minimum_metadata(&self) -> bool {
        let has_title = self.title.as_deref().map(|t| !is_placeholder_title(t)).unwrap_or(false);
        let has_artist = self.artist.as_deref().map(|a| !is_placeholder_artist(a)).unwrap_or(false);
        has_title && has_artist
    }
}

/// Strict international standard ISRC validator (12 alfanumeric characters: 2 letter country, 3 registrant, 2 year, 5 designation).
/// Never treat numeric provider IDs as ISRC.
pub fn is_valid_isrc(candidate: &str) -> bool {
    let trimmed = candidate.trim();
    if trimmed.len() != 12 {
        return false;
    }
    let chars: Vec<char> = trimmed.chars().collect();
    // First 2: Country code (letters A-Z)
    if !chars[0].is_ascii_alphabetic() || !chars[1].is_ascii_alphabetic() {
        return false;
    }
    // Next 3: Registrant code (alphanumeric A-Z, 0-9)
    if !chars[2].is_ascii_alphanumeric() || !chars[3].is_ascii_alphanumeric() || !chars[4].is_ascii_alphanumeric() {
        return false;
    }
    // Next 2: Reference year (digits 0-9)
    if !chars[5].is_ascii_digit() || !chars[6].is_ascii_digit() {
        return false;
    }
    // Last 5: Designation code (digits 0-9)
    chars[7..12].iter().all(|c| c.is_ascii_digit())
}

/// Granular classification of metadata strings to separate legitimate catalog items from placeholders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataClassification {
    LegitimateCatalogName,
    ProviderPlaceholder,
    PartialMetadata,
    AmbiguousMetadata,
}

/// Check if a title is an uninformative placeholder.
pub fn is_placeholder_title(title: &str) -> bool {
    matches!(classify_title(Some(title)), MetadataClassification::ProviderPlaceholder | MetadataClassification::PartialMetadata)
}

/// Classify a track title into legitimate catalog name, placeholder, or partial metadata.
pub fn classify_title(title: Option<&str>) -> MetadataClassification {
    match title {
        None => MetadataClassification::PartialMetadata,
        Some(t) => {
            let trimmed = t.trim();
            if trimmed.is_empty() {
                return MetadataClassification::PartialMetadata;
            }
            let lower = trimmed.to_lowercase();
            if lower == "unknown" || lower == "unknown track" || lower == "n/a" || lower == "null" || lower == "none" || lower == "???" || lower == "??" {
                return MetadataClassification::ProviderPlaceholder;
            }
            if lower.starts_with("tidal track ")
                || lower.starts_with("qobuz track ")
                || lower.starts_with("spotify track ")
            {
                return MetadataClassification::ProviderPlaceholder;
            }
            if let Some(rest) = lower.strip_prefix("track ") {
                let rest_trimmed = rest.trim();
                if !rest_trimmed.is_empty() && rest_trimmed.chars().all(|c| c.is_ascii_digit()) {
                    return MetadataClassification::ProviderPlaceholder;
                }
            }
            MetadataClassification::LegitimateCatalogName
        }
    }
}

/// Check if an artist name is an uninformative placeholder.
pub fn is_placeholder_artist(artist: &str) -> bool {
    matches!(classify_artist(Some(artist)), MetadataClassification::ProviderPlaceholder | MetadataClassification::PartialMetadata)
}

/// Classify an artist name into legitimate catalog name, placeholder, or partial metadata.
pub fn classify_artist(artist: Option<&str>) -> MetadataClassification {
    match artist {
        None => MetadataClassification::PartialMetadata,
        Some(a) => {
            let trimmed = a.trim();
            if trimmed.is_empty() {
                return MetadataClassification::PartialMetadata;
            }
            let lower = trimmed.to_lowercase();
            if lower == "unknown artist" || lower == "unknown" || lower == "various artists" || lower == "n/a" || lower == "null" || lower == "none" || lower == "???" || lower == "??" {
                return MetadataClassification::ProviderPlaceholder;
            }
            MetadataClassification::LegitimateCatalogName
        }
    }
}

/// Check if an album title is an uninformative placeholder.
pub fn is_placeholder_album(album: &str) -> bool {
    matches!(classify_album(Some(album)), MetadataClassification::ProviderPlaceholder | MetadataClassification::PartialMetadata)
}

/// Classify an album title into legitimate catalog name, placeholder, or partial metadata.
pub fn classify_album(album: Option<&str>) -> MetadataClassification {
    match album {
        None => MetadataClassification::PartialMetadata,
        Some(alb) => {
            let trimmed = alb.trim();
            if trimmed.is_empty() {
                return MetadataClassification::PartialMetadata;
            }
            let lower = trimmed.to_lowercase();
            if lower == "unknown album" || lower == "unknown" || lower == "n/a" || lower == "null" || lower == "none" || lower == "???" || lower == "??" {
                return MetadataClassification::ProviderPlaceholder;
            }
            MetadataClassification::LegitimateCatalogName
        }
    }
}

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
    pub bpm: Option<f64>,
    pub copyright: Option<String>,
    pub explicit: Option<bool>,
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
    #[serde(rename = "numberOfTracks")]
    pub number_of_tracks: Option<u32>,
    #[serde(rename = "numberOfVolumes")]
    pub number_of_volumes: Option<u32>,
    pub copyright: Option<String>,
    pub upc: Option<String>,
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

    #[test]
    fn test_is_valid_isrc_and_numeric_rejection() {
        // Valid ISRCs
        assert!(is_valid_isrc("USRC17607839"));
        assert!(is_valid_isrc("GBAYE0601477"));
        assert!(is_valid_isrc("FR6V81200045"));

        // Invalid: Numeric Provider IDs (e.g. Tidal track ID treated as ISRC)
        assert!(!is_valid_isrc("134683067"));
        assert!(!is_valid_isrc("280721704"));
        assert!(!is_valid_isrc("123456789012")); // 12 digits but no country code letters

        // Invalid lengths / chars
        assert!(!is_valid_isrc(""));
        assert!(!is_valid_isrc("US-RC1-76-07839")); // dashes
        assert!(!is_valid_isrc("SHORT"));
    }

    #[test]
    fn test_placeholder_detection() {
        assert!(is_placeholder_title("Tidal Track 134683067"));
        assert!(is_placeholder_title("Unknown Track"));
        assert!(is_placeholder_title(""));
        assert!(!is_placeholder_title("Bohemian Rhapsody"));

        assert!(is_placeholder_artist("Unknown Artist"));
        assert!(is_placeholder_artist("Unknown"));
        assert!(is_placeholder_artist(""));
        assert!(!is_placeholder_artist("Queen"));

        assert!(is_placeholder_album("Unknown Album"));
        assert!(is_placeholder_album(""));
        assert!(!is_placeholder_album("A Night at the Opera"));
    }

    #[test]
    fn test_provider_track_identity_sanitization() {
        let ident = ProviderTrackIdentity {
            service_id: 3,
            service_name: "tidal".to_string(),
            service_track_id: "134683067".to_string(),
            isrc: Some("134683067".to_string()), // invalid numeric ISRC
            provider_album_id: Some("134683060".to_string()),
            provider_artist_id: Some("3567".to_string()),
            title: Some("Tidal Track 134683067".to_string()),
            artist: Some("Unknown Artist".to_string()),
            album: None,
            duration_ms: Some(210000),
            track_number: Some(1),
            disc_number: Some(1),
            explicit: Some(false),
        };

        assert_eq!(ident.sanitized_isrc(), None);
        assert!(!ident.has_minimum_metadata());
    }
}

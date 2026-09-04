//! Pure metadata models, extractors, candidate scoring, and matching rules.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use regex::Regex;

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
            let clean = c.trim().replace('-', "");
            if is_valid_isrc(&clean) {
                Some(clean.to_uppercase())
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
    let unescaped = decode_html_entities(title);
    let unmojibake = clean_mojibake(&unescaped);
    let mut clean = unmojibake;
    for suffix in &[" (Remaster", " (Deluxe", " - Remaster", " - Live", " (Live", " (remaster", " (deluxe", " - remaster", " - live", " (live"] {
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

static FEAT_KEYWORD_PATTERN: &str = r"(?:\bfeaturing\b|\bfeat\b\.?|\bft\b\.?)";

static BRACKET_FEAT_REGEX: OnceLock<Regex> = OnceLock::new();
static BARE_FEAT_REGEX: OnceLock<Regex> = OnceLock::new();
static AS_FEATURED_REGEX: OnceLock<Regex> = OnceLock::new();
static SPLIT_SEPARATORS_REGEX: OnceLock<Regex> = OnceLock::new();

/// Extracts featured collaborating artists from a track title.
///
/// Detects patterns such as:
/// - `(feat. Artista)` or `[feat. Artista]` or `{feat. Artista}`
/// - `(ft. Artista)` or `[ft. Artista]`
/// - `(featuring Artista)` or `[featuring Artista]`
/// - `feat. Artista` at the end or before a trailing dash separator
///
/// Supports multiple artists separated by `,`, `&`, or `and` (e.g. `(feat. A, B & C)`).
/// Excludes false positives like `BIRDS OF A FEATHER` or `as featured in ...`.
pub fn extract_featured_artists(title: &str) -> Vec<String> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let as_featured = AS_FEATURED_REGEX.get_or_init(|| {
        Regex::new(r"(?i)\bas\s+featured\s+in\b").expect("Valid regex")
    });
    if as_featured.is_match(trimmed) {
        return Vec::new();
    }

    let bracket_re = BRACKET_FEAT_REGEX.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)[\(\[\{{](?:[^\)\]\}}]*?\b)?{}\s*([^\)\]\}}]+)[\)\]\}}]",
            FEAT_KEYWORD_PATTERN
        ))
        .expect("Valid regex")
    });

    let bare_re = BARE_FEAT_REGEX.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)(?:^|[\s_]){}\s*([^\-]+?)(?:\s+-\s+.*|$)",
            FEAT_KEYWORD_PATTERN
        ))
        .expect("Valid regex")
    });

    let raw_capture = if let Some(caps) = bracket_re.captures(trimmed) {
        caps.get(1).map(|m| m.as_str())
    } else if let Some(caps) = bare_re.captures(trimmed) {
        caps.get(1).map(|m| m.as_str())
    } else {
        None
    };

    let raw_text = match raw_capture {
        Some(text) => text.trim(),
        None => return Vec::new(),
    };

    if raw_text.is_empty() {
        return Vec::new();
    }

    // Protect known multi-word artist names containing internal commas like "Tyler, The Creator"
    let protected = raw_text
        .replace("Tyler, The Creator", "Tyler__COMMA_SPACE__The Creator")
        .replace("Tyler, the Creator", "Tyler__COMMA_SPACE__The Creator")
        .replace("Tyler,THE CREATOR", "Tyler__COMMA_SPACE__The Creator");

    let split_re = SPLIT_SEPARATORS_REGEX.get_or_init(|| {
        Regex::new(r"(?i)\s*(?:,\s*(?:and\s+)?|\s+and\s+|\s*&\s*)\s*").expect("Valid regex")
    });

    let mut result = Vec::new();
    for token in split_re.split(&protected) {
        let restored = token.replace("__COMMA_SPACE__", ", ");
        let mut cleaned = restored.trim();

        // Strip leading "with " or "+ " if present
        if cleaned.to_lowercase().starts_with("with ") {
            cleaned = cleaned[5..].trim();
        } else if cleaned.starts_with('+') {
            cleaned = cleaned[1..].trim();
        }

        // Strip surrounding quotes or brackets
        let cleaned = cleaned.trim_matches(|c| c == '\'' || c == '"' || c == '“' || c == '”');
        let cleaned = cleaned.trim();

        if !cleaned.is_empty() && !result.iter().any(|existing: &String| existing.eq_ignore_ascii_case(cleaned)) {
            result.push(cleaned.to_string());
        }
    }

    result
}

/// Decode common HTML entities (both named and numeric).
pub fn decode_html_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }

    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '&' {
            let mut entity = String::with_capacity(10);
            let mut found_semi = false;
            while let Some(&next_c) = chars.peek() {
                if next_c == ';' {
                    chars.next();
                    found_semi = true;
                    break;
                }
                if next_c == '&' || next_c == ' ' || entity.len() > 10 {
                    break;
                }
                entity.push(chars.next().unwrap());
            }

            if found_semi {
                let lower = entity.to_lowercase();
                if let Some(decoded) = match lower.as_str() {
                    "amp" => Some('&'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "nbsp" => Some(' '),
                    "ndash" => Some('–'),
                    "mdash" => Some('—'),
                    "hellip" => Some('…'),
                    "lsquo" => Some('‘'),
                    "rsquo" => Some('’'),
                    "ldquo" => Some('“'),
                    "rdquo" => Some('”'),
                    "copy" => Some('©'),
                    "reg" => Some('®'),
                    "trade" => Some('™'),
                    "aacute" => Some('á'),
                    "eacute" => Some('é'),
                    "iacute" => Some('í'),
                    "oacute" => Some('ó'),
                    "uacute" => Some('ú'),
                    "ntilde" => Some('ñ'),
                    "uuml" => Some('ü'),
                    "ouml" => Some('ö'),
                    "auml" => Some('ä'),
                    "ccedil" => Some('ç'),
                    _ => {
                        if let Some(dec_str) = entity.strip_prefix('#') {
                            if let Some(hex_str) = dec_str.strip_prefix('x').or_else(|| dec_str.strip_prefix('X')) {
                                u32::from_str_radix(hex_str, 16).ok().and_then(char::from_u32)
                            } else {
                                dec_str.parse::<u32>().ok().and_then(char::from_u32)
                            }
                        } else {
                            None
                        }
                    }
                } {
                    result.push(decoded);
                } else {
                    result.push('&');
                    result.push_str(&entity);
                    result.push(';');
                }
            } else {
                result.push('&');
                result.push_str(&entity);
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Clean mojibake caused by UTF-8 bytes misinterpreted as ISO-8859-1 or Windows-1252.
pub fn clean_mojibake(s: &str) -> String {
    if !s.contains('Ã') && !s.contains('Â') && !s.contains("â") && !s.contains('â') {
        return s.to_string();
    }

    // Direct fix for typical sequences
    let mut fixed = s.to_string();
    let direct_replacements = [
        ("Â¿", "¿"),
        ("Â¡", "¡"),
        ("Ã¡", "á"),
        ("Ã©", "é"),
        ("Ã­", "í"),
        ("Ã³", "ó"),
        ("Ãº", "ú"),
        ("Ã±", "ñ"),
        ("Ã", "Á"),
        ("Ã", "É"),
        ("Ã", "Í"),
        ("Ã", "Ó"),
        ("Ã", "Ú"),
        ("Ã", "Ñ"),
        ("Ã¼", "ü"),
        ("Ã¶", "ö"),
        ("Ã¤", "ä"),
        ("Ã", "Ü"),
        ("Ã", "Ö"),
        ("Ã", "Ä"),
        ("â", "–"),
        ("â", "—"),
        ("â", "’"),
        ("â", "‘"),
        ("â", "“"),
        ("â", "”"),
        ("â¦", "…"),
    ];

    for (from, to) in direct_replacements {
        if fixed.contains(from) {
            fixed = fixed.replace(from, to);
        }
    }

    // General fallback: try decoding as UTF-8 if chars fit in 0..=255
    if (fixed.contains('Ã') || fixed.contains('Â')) && fixed.chars().all(|c| (c as u32) <= 255) {
        let bytes: Vec<u8> = fixed.chars().map(|c| c as u8).collect();
        if let Ok(utf8_decoded) = std::str::from_utf8(&bytes) {
            if !utf8_decoded.contains('\u{FFFD}') {
                return utf8_decoded.to_string();
            }
        }
    }

    fixed
}

/// Strict sanitization for artist names:
/// - Strips accidental Qobuz `"Role\r - Name"` or `"Role\n - Name"` prefix if present
/// - Decodes HTML entities (`&amp;` -> `&`, etc.)
/// - Cleans mojibake
/// - Strips all internal newlines/carriage returns
/// - Strictly trims leading and trailing whitespace
pub fn sanitize_artist_name(raw: &str) -> String {
    let unescaped = decode_html_entities(raw);
    let unmojibake = clean_mojibake(&unescaped);

    // If it has role prefix e.g. "Piano\r - Glenn Gould" or "Piano\n - Glenn Gould"
    let candidate = if let Some(dash_pos) = unmojibake.find("\r - ")
        .or_else(|| unmojibake.find("\n - "))
        .or_else(|| unmojibake.find("\r\n - "))
    {
        if let Some(after_dash) = unmojibake[dash_pos..].split_once("- ") {
            after_dash.1
        } else {
            &unmojibake
        }
    } else {
        &unmojibake
    };

    // Remove any leftover \r, \n, tabs, and trim
    candidate
        .replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Strict sanitization for track titles:
/// - Decodes HTML entities
/// - Cleans mojibake
/// - Trims leading and trailing whitespace
pub fn sanitize_track_title(raw: &str) -> String {
    let unescaped = decode_html_entities(raw);
    let unmojibake = clean_mojibake(&unescaped);
    unmojibake.trim().to_string()
}

static CREDIT_ROLE_REGEX: OnceLock<Regex> = OnceLock::new();

/// Extracts role and clean artist name from a credit entry segment.
///
/// Specifically parses Qobuz formats like:
/// - `"Piano\r - Glenn Gould"` -> `("Glenn Gould", "Piano")`
/// - `"Piano\n - Glenn Gould"` -> `("Glenn Gould", "Piano")`
/// - `"Composer\r\n - Johann Sebastian Bach"` -> `("Johann Sebastian Bach", "Composer")`
///
/// If no role format is found, returns `(sanitize_artist_name(raw), default_role)`.
pub fn parse_credit_role_and_name(raw: &str, default_role: &str) -> (String, String) {
    let re = CREDIT_ROLE_REGEX.get_or_init(|| {
        Regex::new(r"(?s)^([^\r\n]+?)[\r\n]+[\t ]*[-–—][\t ]*(.+)$").expect("Valid regex")
    });

    if let Some(caps) = re.captures(raw.trim()) {
        let raw_role = caps.get(1).map(|m| m.as_str()).unwrap_or(default_role);
        let raw_name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let clean_role = decode_html_entities(raw_role)
            .replace(['\r', '\n'], " ")
            .trim()
            .to_string();
        let clean_name = sanitize_artist_name(raw_name);
        (clean_name, if clean_role.is_empty() { default_role.to_string() } else { clean_role })
    } else {
        (sanitize_artist_name(raw), default_role.trim().to_string())
    }
}

/// Splits a raw credit string into individual entries, extracting their roles and clean names.
///
/// Handles multi-entry lists delimited by commas, semicolons, slashes, or entry-separating newlines.
pub fn parse_credits_string(raw: &str, default_role: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();

    // Protect "Tyler, The Creator" etc from naive comma split
    let protected = raw
        .replace("Tyler, The Creator", "Tyler__COMMA_SPACE__The Creator")
        .replace("Tyler, the Creator", "Tyler__COMMA_SPACE__The Creator")
        .replace("Tyler,THE CREATOR", "Tyler__COMMA_SPACE__The Creator");

    // Split on commas, semicolons, slashes, or newlines that are NOT followed by "- "
    let mut current = String::new();
    let chars: Vec<char> = protected.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];
        if c == ',' || c == ';' || c == '/' {
            if !current.trim().is_empty() {
                entries.push(current.trim().to_string());
                current.clear();
            }
            i += 1;
        } else if c == '\n' || c == '\r' {
            // Check if what follows is optional whitespace then '-'
            let mut j = i + 1;
            while j < len && (chars[j] == '\r' || chars[j] == '\n' || chars[j] == ' ' || chars[j] == '\t') {
                j += 1;
            }
            if j < len && (chars[j] == '-' || chars[j] == '–' || chars[j] == '—') {
                // This newline is part of "\r - ", keep it inside current entry
                current.push(c);
                i += 1;
            } else {
                // This newline separates distinct credit entries
                if !current.trim().is_empty() {
                    entries.push(current.trim().to_string());
                    current.clear();
                }
                i += 1;
            }
        } else {
            current.push(c);
            i += 1;
        }
    }

    if !current.trim().is_empty() {
        entries.push(current.trim().to_string());
    }

    let mut result = Vec::new();
    for entry in entries {
        let restored = entry.replace("__COMMA_SPACE__", ", ");
        let (name, role) = parse_credit_role_and_name(&restored, default_role);
        if !name.is_empty() && name != "???" && name != "null" && name != "None" {
            result.push((name, role));
        }
    }

    result
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
            bpm: None,
            copyright: None,
            explicit: None,
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

    #[test]
    fn test_extract_featured_artists_patterns() {
        // Parentheses
        assert_eq!(extract_featured_artists("23 (feat. Sasha Dobson)"), vec!["Sasha Dobson"]);
        assert_eq!(extract_featured_artists("After The Storm (Ft. Tyler, The Creator)"), vec!["Tyler, The Creator"]);
        assert_eq!(extract_featured_artists("DARE (featuring Shaun Ryder and Rosie Wilson)"), vec!["Shaun Ryder", "Rosie Wilson"]);
        
        // Square brackets
        assert_eq!(extract_featured_artists("Ain't No Love [feat. Melanie Williams]"), vec!["Melanie Williams"]);
        assert_eq!(extract_featured_artists("Cobra (Rock Remix) [feat. Spiritbox]"), vec!["Spiritbox"]);
        
        // Multiple artists with comma, & and 'and'
        assert_eq!(extract_featured_artists("4 Minutes (feat. Justin Timberlake & Timbaland)"), vec!["Justin Timberlake", "Timbaland"]);
        assert_eq!(extract_featured_artists("Audio (feat. Sia, Diplo, and Labrinth)"), vec!["Sia", "Diplo", "Labrinth"]);
        assert_eq!(
            extract_featured_artists("Downtown (feat. Melle Mel, Grandmaster Caz, Kool Moe Dee & Eric Nally)"), 
            vec!["Melle Mel", "Grandmaster Caz", "Kool Moe Dee", "Eric Nally"]
        );

        // Bare feat. at end or before dash
        assert_eq!(extract_featured_artists("Burn My Shadow feat. Ian Astbury"), vec!["Ian Astbury"]);
        assert_eq!(extract_featured_artists("Fly By Day feat. JU!iE"), vec!["JU!iE"]);
        assert_eq!(extract_featured_artists("202 feat. 泉まくら - New Mix"), vec!["泉まくら"]);
        assert_eq!(extract_featured_artists("GIRL feat.呂布"), vec!["呂布"]);

        // Complex with/feat patterns
        assert_eq!(extract_featured_artists("Feel The Fiyaaaah (with A$AP Rocky & feat. Takeoff)"), vec!["Takeoff"]);
        assert_eq!(extract_featured_artists("Too Many Nights (feat. Don Toliver & with Future)"), vec!["Don Toliver", "Future"]);

        // Exclusions / False positives
        assert!(extract_featured_artists("BIRDS OF A FEATHER").is_empty());
        assert!(extract_featured_artists("Feather").is_empty());
        assert!(extract_featured_artists("Bloodfeather").is_empty());
        assert!(extract_featured_artists("Funny Feathers").is_empty());
        assert!(extract_featured_artists("Light as a Feather").is_empty());
        assert!(extract_featured_artists("Sexy Rouge (as featured in \"Sky Rojo\") (Remix) (Original TV Series Soundtrack)").is_empty());
        assert!(extract_featured_artists("").is_empty());
    }

    #[test]
    fn test_credit_extraction_and_sanitization() {
        // 1. "Piano\r - Glenn Gould" splits into artist "Glenn Gould" and role "Piano"
        let (artist, role) = parse_credit_role_and_name("Piano\r - Glenn Gould", "performer");
        assert_eq!(artist, "Glenn Gould");
        assert_eq!(role, "Piano");

        // Variants with \n, \r\n, spaces
        let (artist_n, role_n) = parse_credit_role_and_name("Piano\n - Glenn Gould", "performer");
        assert_eq!(artist_n, "Glenn Gould");
        assert_eq!(role_n, "Piano");

        let (artist_rn, role_rn) = parse_credit_role_and_name("Composer\r\n - Johann Sebastian Bach", "composer");
        assert_eq!(artist_rn, "Johann Sebastian Bach");
        assert_eq!(role_rn, "Composer");

        // 2. "SNEAKER KIDS &amp; Eli Noir" normalizes to "SNEAKER KIDS & Eli Noir"
        let artist_amp = sanitize_artist_name("SNEAKER KIDS &amp; Eli Noir");
        assert_eq!(artist_amp, "SNEAKER KIDS & Eli Noir");

        // 3. Artists with spaces " Oasis " trim to "Oasis"
        let artist_spaces = sanitize_artist_name(" Oasis ");
        assert_eq!(artist_spaces, "Oasis");

        // Additional validations: mojibake cleaning & HTML decoding in clean_title
        assert_eq!(clean_mojibake("Â¿Y TÃº QuÃ© Has Hecho?"), "¿Y Tú Qué Has Hecho?");
        assert_eq!(clean_title("Tom &amp; Jerry (Remastered)"), "tom & jerry");

        // Multi-entry credits parsing
        let parsed = parse_credits_string("Piano\r - Glenn Gould, Violin\r - Yehudi Menuhin", "performer");
        assert_eq!(parsed, vec![
            ("Glenn Gould".to_string(), "Piano".to_string()),
            ("Yehudi Menuhin".to_string(), "Violin".to_string()),
        ]);

        let plain = parse_credits_string("David Bowie, Robert Fripp", "performer");
        assert_eq!(plain, vec![
            ("David Bowie".to_string(), "performer".to_string()),
            ("Robert Fripp".to_string(), "performer".to_string()),
        ]);
    }
}


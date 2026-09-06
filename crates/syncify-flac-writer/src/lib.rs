//! FLAC Metadata DTO & Tag Writer — source-agnostic VorbisComment tagging
//! with non-destructive METADATA_BLOCK_PICTURE and animated WebP preservation.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info};

pub use syncify_core_domain::cover_rules::{CoverPreservationPolicy, CoverType, CoverUpdateDecision};
pub use syncify_core_domain::byte_validators::{ImageByteValidator, ImageDimensions, WebpByteValidator};
pub use syncify_core_domain::metadata::{clean_title_and_extract_featured, extract_featured_artists};

/// Maximum recommended size for embedded FLAC PICTURE metadata block (800 KB / 819,200 bytes).
pub const MAX_EMBEDDED_PICTURE_BYTES: usize = 800 * 1024;

/// Hard ceiling for FLAC PICTURE metadata block (1 MB / 1,048,576 bytes).
pub const HARD_CEILING_PICTURE_BYTES: usize = 1024 * 1024;

/// Pure metadata DTO for FLAC tagging.
/// Constructed by service-specific builders or enrichment engines,
/// applied by `apply_flac_tags`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlacMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub composer: Option<String>,
    pub performers: Option<String>,
    pub work: Option<String>,
    pub genre: Option<String>,
    pub style: Option<String>,
    pub mood: Option<String>,
    pub release_type: Option<String>,
    pub release_status: Option<String>,
    pub release_country: Option<String>,
    pub release_region: Option<String>,
    pub language: Option<String>,
    pub copyright: Option<String>,
    pub label: Option<String>,
    pub barcode: Option<String>,
    pub catalog_number: Option<String>,
    pub original_date: Option<String>,
    pub track_number: u32,
    pub track_total: u32,
    pub disc_number: u32,
    pub disc_total: u32,
    pub total_discs: Option<u32>,
    pub disc_track_total: Option<u32>,
    pub disc_subtitle: Option<String>,
    pub isrc: Option<String>,
    pub release_year: Option<String>,
    pub release_date: Option<String>,
    pub explicit: Option<bool>,
    pub bpm: Option<u32>,
    pub initial_key: Option<String>,
    pub energy: Option<f64>,
    pub danceability: Option<f64>,
    pub loudness: Option<f64>,
    pub replaygain_track_gain: Option<String>,
    pub replaygain_track_peak: Option<String>,
    pub replaygain_album_gain: Option<String>,
    pub replaygain_album_peak: Option<String>,
    pub r128_track_gain: Option<String>,
    pub comment: Option<String>,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<f64>,
    pub musicbrainz_track_id: Option<String>,
    pub musicbrainz_artist_id: Option<String>,
    pub musicbrainz_album_id: Option<String>,
    pub musicbrainz_albumartist_id: Option<String>,
    pub musicbrainz_release_group_id: Option<String>,
    pub musicbrainz_work_id: Option<String>,
    pub lyrics_lrc: Option<String>,
    pub cover_data: Option<Vec<u8>>,
    pub lyrics_source: Option<String>,
    pub cover_source: Option<String>,
    pub audio_source: Option<String>,
    pub compilation: Option<bool>,
    pub grouping: Option<String>,
    pub tags: Option<String>,
    pub artist_tags: Option<Vec<String>>,
    /// Discrete artist list for independent multi-value VorbisComment `ARTIST` blocks (TASK-67).
    /// Used by Symfonium to index multiple artists per track.
    pub artists: Option<Vec<String>>,
    pub media_type: Option<String>,
}

impl FlacMetadata {
    /// Return the effective total tracks for the specific disc.
    /// In multidisc releases, TRACKTOTAL must reflect the local disc track count,
    /// preferring `disc_track_total` if set, otherwise falling back to `track_total`.
    pub fn effective_track_total(&self) -> u32 {
        self.disc_track_total.filter(|&t| t > 0).unwrap_or(self.track_total)
    }

    /// Return the effective disc total for the release.
    /// Prefers `total_discs` if set, otherwise falling back to `disc_total`.
    pub fn effective_disc_total(&self) -> u32 {
        self.total_discs.filter(|&d| d > 0).unwrap_or(self.disc_total)
    }

    /// Checks if this track belongs to a compilation release.
    ///
    /// Returns true if:
    /// - `compilation` is explicitly `Some(true)`
    /// - OR `album_artist` is "Various Artists" or "Various" (case-insensitive)
    /// - OR `release_type` or `media_type` indicates "compilation" or "soundtrack" (unless `compilation == Some(false)`)
    pub fn is_compilation(&self) -> bool {
        if let Some(comp) = self.compilation {
            if comp {
                return true;
            }
            if let Some(ref aa) = self.album_artist {
                if syncify_core_domain::metadata::is_various_artists_variant(aa) {
                    return true;
                }
            }
            return false;
        }

        if let Some(ref aa) = self.album_artist {
            if syncify_core_domain::metadata::is_various_artists_variant(aa) {
                return true;
            }
        }

        if let Some(ref rt) = self.release_type {
            let t = rt.trim();
            if t.eq_ignore_ascii_case("compilation") || t.eq_ignore_ascii_case("soundtrack") {
                return true;
            }
        }

        if let Some(ref mt) = self.media_type {
            let t = mt.trim();
            if t.eq_ignore_ascii_case("compilation") || t.eq_ignore_ascii_case("soundtrack") {
                return true;
            }
        }

        false
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TagVerification {
    pub file_exists: bool,
    pub flac_valid: bool,
    pub tags_match: bool,
    pub cover_present: bool,
    pub cover_size_bytes: Option<usize>,
    pub cover_mime: Option<String>,
    pub cover_width: Option<u32>,
    pub cover_height: Option<u32>,
    pub lyrics_present: bool,
    pub synced_lyrics_present: bool,
    pub unsynced_lyrics_present: bool,
    pub bpm_present: bool,
    pub duration_sec: Option<f64>,
    pub mismatches: Vec<(String, String, String)>,
}

/// Strip LRC timestamps [mm:ss.xx] or <mm:ss.xx> for clean UNSYNCEDLYRICS plain text
pub fn strip_lrc_timestamps(lrc: &str) -> String {
    syncify_lyrics_domain::strip_lrc_timestamps(lrc)
}

/// Validate tag value to reject empty and placeholder strings ("Unknown", "N/A", "null", "none", "???")
pub fn is_valid_tag_val(val: &str) -> bool {
    let t = val.trim();
    !t.is_empty()
        && !t.eq_ignore_ascii_case("unknown")
        && !t.eq_ignore_ascii_case("unknown artist")
        && !t.eq_ignore_ascii_case("unknown album")
        && !t.eq_ignore_ascii_case("unknown track")
        && !t.eq_ignore_ascii_case("n/a")
        && !t.eq_ignore_ascii_case("null")
        && !t.eq_ignore_ascii_case("none")
        && t != "???"
}

/// Resolves individual artists for discrete VorbisComment `ARTIST` blocks (TASK-67).
///
/// Deduplicates entries case-insensitively and filters out placeholders / empty strings.
pub fn resolve_flac_artists(
    artist: &str,
    artists: Option<&[String]>,
    featured_from_title: &[String],
) -> Vec<String> {
    let mut resolved: Vec<String> = Vec::new();

    fn add_artist(resolved: &mut Vec<String>, name: &str) {
        let trimmed = name.trim();
        if is_valid_tag_val(trimmed)
            && !resolved.iter().any(|existing| existing.eq_ignore_ascii_case(trimmed))
        {
            resolved.push(trimmed.to_string());
        }
    }

    if let Some(list) = artists {
        for a in list {
            add_artist(&mut resolved, a);
        }
    }

    if resolved.is_empty() && is_valid_tag_val(artist) {
        if artist.contains(';') {
            for part in artist.split(';') {
                let (cleaned, feats) = clean_title_and_extract_featured(part);
                if !cleaned.is_empty() {
                    add_artist(&mut resolved, &cleaned);
                }
                for f in feats {
                    add_artist(&mut resolved, &f);
                }
            }
        } else {
            let (cleaned, feats) = clean_title_and_extract_featured(artist);
            if !cleaned.is_empty() {
                add_artist(&mut resolved, &cleaned);
            }
            for f in feats {
                add_artist(&mut resolved, &f);
            }
        }
    }

    for feat in featured_from_title {
        add_artist(&mut resolved, feat);
    }

    resolved
}

/// Detects whether a collection of tracks forming an album represents a compilation.
///
/// An album is considered a compilation if:
/// 1. Any track has `compilation == Some(true)`, or
/// 2. Any track has `album_artist` equal to "Various Artists" or "Various" (case-insensitive), or
/// 3. Any track has `release_type` or `media_type` indicating "compilation" or "soundtrack", or
/// 4. The tracks possess multiple divergent primary artists (more than 1 distinct non-empty artist name).
pub fn detect_album_is_compilation(tracks: &[FlacMetadata]) -> bool {
    if tracks.is_empty() {
        return false;
    }

    let mut distinct_artists = std::collections::HashSet::new();

    for t in tracks {
        if t.is_compilation() {
            return true;
        }
        let clean_artist = t.artist.trim();
        if is_valid_tag_val(clean_artist) {
            distinct_artists.insert(clean_artist.to_lowercase());
        }
    }

    distinct_artists.len() > 1
}

/// Unifies metadata for a collection of tracks belonging to the same album.
///
/// If the album is detected as a compilation (or has multiple divergent artists):
/// - Sets `compilation = Some(true)` on all tracks.
/// - Guarantees that `album_artist` is set to `"Various Artists"` (or the specified `compilation_artist` override,
///   or preserves any existing shared compilation artist).
///
/// If the album is a mono-artist release:
/// - Preserves the individual artist as `album_artist` (if `album_artist` is unset, defaults to the common artist).
/// - Ensures `compilation` is not set to `true`.
pub fn unify_album_compilation_metadata(
    tracks: &mut [FlacMetadata],
    compilation_artist: Option<&str>,
) {
    if tracks.is_empty() {
        return;
    }

    let is_comp = detect_album_is_compilation(tracks);

    if is_comp {
        let effective_comp_artist = compilation_artist
            .filter(|s| is_valid_tag_val(s))
            .map(|s| syncify_core_domain::metadata::normalize_compilation_artist(s))
            .or_else(|| {
                // If any track already had a non-divergent album artist other than track artist
                tracks.iter()
                    .filter_map(|t| t.album_artist.as_deref())
                    .find(|aa| is_valid_tag_val(aa) && !aa.eq_ignore_ascii_case("unknown"))
                    .map(|s| syncify_core_domain::metadata::normalize_compilation_artist(s))
            })
            .unwrap_or_else(|| syncify_core_domain::metadata::CANONICAL_VARIOUS_ARTISTS.to_string());

        for t in tracks.iter_mut() {
            t.compilation = Some(true);
            if t.album_artist.as_deref().map(|aa| !is_valid_tag_val(aa)).unwrap_or(true)
                || t.album_artist.as_deref() == Some(&t.artist)
            {
                t.album_artist = Some(effective_comp_artist.clone());
            }
        }
    } else {
        // Mono-artist: find the common artist if album_artist is missing
        let common_artist = tracks.iter()
            .find(|t| is_valid_tag_val(&t.artist))
            .map(|t| t.artist.clone());

        for t in tracks.iter_mut() {
            if t.album_artist.is_none() && common_artist.is_some() {
                t.album_artist = common_artist.clone();
            }
        }
    }
}

/// Apply FLAC tags directly into the FLAC file using metaflac for complete Symfonium compatibility.
///
/// Uses VorbisComments (XiphComment) for FLAC files following exact Symfonium tag naming rules.
/// Preserves unrelated tags, frame boundaries, and padding cleanly.
pub fn apply_flac_tags(file_path: &Path, metadata: &FlacMetadata) -> std::result::Result<(), String> {
    use metaflac::block::PictureType;

    if !file_path.exists() {
        return Err(format!("File does not exist: {:?}", file_path));
    }

    let file_meta = std::fs::metadata(file_path)
        .map_err(|e| format!("Failed to read metadata for {:?}: {}", file_path, e))?;
    if file_meta.len() == 0 {
        return Err(format!("File is empty (0 bytes): {:?}", file_path));
    }

    let mut tag = metaflac::Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to open audio file for tagging: {}", e))?;

    let comments = tag.vorbis_comments_mut();

    let (cleaned_title, feat_from_title) = clean_title_and_extract_featured(&metadata.title);
    let title_to_write = if !cleaned_title.is_empty() {
        &cleaned_title
    } else {
        &metadata.title
    };

    if is_valid_tag_val(title_to_write) {
        comments.set_title(vec![title_to_write.clone()]);
    }

    let artists_to_write = resolve_flac_artists(
        &metadata.artist,
        metadata.artists.as_deref(),
        &feat_from_title,
    );
    if !artists_to_write.is_empty() {
        comments.set_artist(artists_to_write);
    } else if is_valid_tag_val(&metadata.artist) {
        comments.set_artist(vec![metadata.artist.clone()]);
    }
    if is_valid_tag_val(&metadata.album) {
        comments.set_album(vec![metadata.album.clone()]);
    }

    let is_comp = metadata.is_compilation();

    if let Some(ref album_artist) = metadata.album_artist {
        if is_valid_tag_val(album_artist) {
            comments.set("ALBUMARTIST", vec![album_artist.clone()]);
        } else if is_comp {
            comments.set("ALBUMARTIST", vec!["Various Artists".to_string()]);
        }
    } else if is_comp {
        comments.set("ALBUMARTIST", vec!["Various Artists".to_string()]);
    }

    if let Some(ref composer) = metadata.composer {
        if is_valid_tag_val(composer) {
            comments.set("COMPOSER", vec![composer.clone()]);
        }
    }

    if let Some(ref performers) = metadata.performers {
        if is_valid_tag_val(performers) {
            comments.set("PERFORMER", vec![performers.clone()]);
        }
    }

    if let Some(ref work) = metadata.work {
        if is_valid_tag_val(work) {
            comments.set("WORK", vec![work.clone()]);
        }
    }

    if let Some(ref genre) = metadata.genre {
        if is_valid_tag_val(genre) {
            let genres = syncify_metadata_domain::fuse_genres(&[genre.as_str()]);
            if !genres.is_empty() {
                comments.set("GENRE", genres);
            } else {
                let fallback: Vec<String> = genre
                    .split(|c| c == ';' || c == '/')
                    .map(|s| s.trim())
                    .filter(|s| is_valid_tag_val(s))
                    .map(|s| s.to_string())
                    .collect();
                if !fallback.is_empty() {
                    comments.set("GENRE", fallback);
                } else {
                    comments.set("GENRE", vec![genre.trim().to_string()]);
                }
            }
        }
    }

    if let Some(ref style) = metadata.style {
        if is_valid_tag_val(style) {
            // Facets split on ';' only: slash-joined compounds ("Glam Rock / Berlin Trilogy")
            // must survive as a single value instead of being torn into secondary blocks.
            let styles = syncify_metadata_domain::fuse_genres_semicolon_only(&[style.as_str()]);
            if !styles.is_empty() {
                comments.set("STYLE", styles.clone());
                comments.set("ALBUMSTYLE", styles.clone());
                comments.set("TRACKSTYLE", styles);
            } else {
                comments.set("STYLE", vec![style.clone()]);
                comments.set("ALBUMSTYLE", vec![style.clone()]);
                comments.set("TRACKSTYLE", vec![style.clone()]);
            }
        }
    }

    if let Some(ref mood) = metadata.mood {
        if is_valid_tag_val(mood) {
            let moods = syncify_metadata_domain::fuse_genres_semicolon_only(&[mood.as_str()]);
            if !moods.is_empty() {
                comments.set("MOOD", moods.clone());
                comments.set("ALBUMMOOD", moods.clone());
                comments.set("TRACKMOOD", moods);
            } else {
                comments.set("MOOD", vec![mood.clone()]);
                comments.set("ALBUMMOOD", vec![mood.clone()]);
                comments.set("TRACKMOOD", vec![mood.clone()]);
            }
        }
    }

    if let Some(ref tags) = metadata.tags {
        if is_valid_tag_val(tags) {
            let split_tags = syncify_metadata_domain::fuse_genres(&[tags.as_str()]);
            if !split_tags.is_empty() {
                comments.set("TAGS", split_tags.clone());
                comments.set("ALBUMTAGS", split_tags);
            } else {
                comments.set("TAGS", vec![tags.clone()]);
                comments.set("ALBUMTAGS", vec![tags.clone()]);
            }
        }
    }

    if let Some(ref artist_tags) = metadata.artist_tags {
        let valid_tags: Vec<String> = artist_tags
            .iter()
            .flat_map(|t| syncify_metadata_domain::fuse_genres(&[t.as_str()]))
            .filter(|t| is_valid_tag_val(t))
            .collect();
        if !valid_tags.is_empty() {
            comments.set("ARTISTS_TAGS", valid_tags);
        }
    }

    if let Some(ref media_type) = metadata.media_type {
        if is_valid_tag_val(media_type) {
            comments.set("MEDIA", vec![media_type.clone()]);
            comments.set("MUSICTYPE", vec![media_type.clone()]);
        }
    }

    if let Some(ref release_type) = metadata.release_type {
        if is_valid_tag_val(release_type) {
            comments.set("RELEASETYPE", vec![release_type.clone()]);
        }
    }

    if let Some(ref release_status) = metadata.release_status {
        if is_valid_tag_val(release_status) {
            comments.set("RELEASESTATUS", vec![release_status.clone()]);
        }
    }

    if let Some(ref release_country) = metadata.release_country {
        if is_valid_tag_val(release_country) {
            // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
            // COUNTRY & RELEASECOUNTRY carry the canonical English name whenever the value
            // resolves to a sovereign country ("US"/"United States" -> "United States").
            // Regions keep RELEASEREGION semantics and unrecognized values are written
            // verbatim (never invented). Write & verify share ONE domain helper
            // (wire_country_value / wire_region_value) so divergence is impossible.
            let wire_country = syncify_metadata_domain::wire_country_value(release_country);
            comments.set("RELEASECOUNTRY", vec![wire_country.clone()]);
            comments.set("COUNTRY", vec![wire_country]);
            if let Some(region_val) = syncify_metadata_domain::wire_region_value(release_country) {
                comments.set("RELEASEREGION", vec![region_val]);
            }
        }
    }

    if let Some(ref release_region) = metadata.release_region {
        if is_valid_tag_val(release_region) {
            comments.set("RELEASEREGION", vec![release_region.clone()]);
        }
    }

    if let Some(ref language) = metadata.language {
        if is_valid_tag_val(language) {
            // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
            // LANGUAGE carries the English display name ("eng" -> "English"); the SAME
            // wire_language_value helper backs verification below.
            let norm_lang = syncify_metadata_domain::wire_language_value(language);
            comments.set("LANGUAGE", vec![norm_lang]);
        }
    }

    if is_comp {
        comments.set("COMPILATION", vec!["1".to_string()]);
    }

    if let Some(ref grp) = metadata.grouping {
        if is_valid_tag_val(grp) {
            comments.set("GROUPING", vec![grp.clone()]);
        }
    }

    if let Some(ref copyright) = metadata.copyright {
        if is_valid_tag_val(copyright) {
            comments.set("COPYRIGHT", vec![copyright.clone()]);
        }
    }

    if let Some(ref label) = metadata.label {
        if is_valid_tag_val(label) {
            comments.set("LABEL", vec![label.clone()]);
            comments.set("RECORDLABEL", vec![label.clone()]);
            comments.set("ORGANIZATION", vec![label.clone()]);
        }
    }

    if let Some(ref barcode) = metadata.barcode {
        if is_valid_tag_val(barcode) {
            comments.set("BARCODE", vec![barcode.clone()]);
            comments.set("UPC", vec![barcode.clone()]);
        }
    }

    if let Some(ref cn) = metadata.catalog_number {
        if is_valid_tag_val(cn) {
            comments.set("CATALOGNUMBER", vec![cn.clone()]);
        }
    }

    if let Some(ref od) = metadata.original_date {
        if is_valid_tag_val(od) {
            comments.set("ORIGINALDATE", vec![od.clone()]);
        }
    }

    if metadata.track_number > 0 {
        comments.set_track(metadata.track_number);
    }

    let effective_track_total = metadata.effective_track_total();
    if effective_track_total > 0 {
        comments.set("TRACKTOTAL", vec![effective_track_total.to_string()]);
    }

    if metadata.disc_number > 0 {
        comments.set("DISCNUMBER", vec![metadata.disc_number.to_string()]);
    }

    let effective_disc_total = metadata.effective_disc_total();
    if effective_disc_total > 0 {
        let disc_total_str = effective_disc_total.to_string();
        comments.set("DISCTOTAL", vec![disc_total_str.clone()]);
        comments.set("TOTALDISCS", vec![disc_total_str]);
    }

    if let Some(ref disc_sub) = metadata.disc_subtitle {
        if !disc_sub.trim().is_empty() {
            comments.set("DISCSUBTITLE", vec![disc_sub.clone()]);
        }
    }

    if let Some(ref isrc) = metadata.isrc {
        if !isrc.trim().is_empty() {
            comments.set("ISRC", vec![isrc.clone()]);
        }
    }

    if let Some(ref year) = metadata.release_year {
        if !year.trim().is_empty() {
            comments.set("YEAR", vec![year.clone()]);
        }
    }

    if let Some(ref date) = metadata.release_date {
        if !date.trim().is_empty() {
            comments.set("RELEASEDATE", vec![date.clone()]);
        }
    }

    if metadata.explicit == Some(true) {
        comments.set("EXPLICIT", vec!["1"]);
    }

    if let Some(bpm) = metadata.bpm {
        if bpm > 0 {
            let bpm_str = bpm.to_string();
            comments.set("BPM", vec![bpm_str.clone()]);
            comments.set("TEMPO", vec![bpm_str]);
        }
    }

    if let Some(ref key) = metadata.initial_key {
        if !key.trim().is_empty() {
            comments.set("KEY", vec![key.clone()]);
            comments.set("INITIALKEY", vec![key.clone()]);
        }
    }

    if let Some(ref rg_gain) = metadata.replaygain_track_gain {
        if !rg_gain.trim().is_empty() {
            comments.set("REPLAYGAIN_TRACK_GAIN", vec![rg_gain.clone()]);
        }
    }

    if let Some(ref rg_peak) = metadata.replaygain_track_peak {
        if !rg_peak.trim().is_empty() {
            comments.set("REPLAYGAIN_TRACK_PEAK", vec![rg_peak.clone()]);
        }
    }

    if let Some(ref rg_again) = metadata.replaygain_album_gain {
        if !rg_again.trim().is_empty() {
            comments.set("REPLAYGAIN_ALBUM_GAIN", vec![rg_again.clone()]);
        }
    }

    if let Some(ref rg_apeak) = metadata.replaygain_album_peak {
        if !rg_apeak.trim().is_empty() {
            comments.set("REPLAYGAIN_ALBUM_PEAK", vec![rg_apeak.clone()]);
        }
    }

    if let Some(ref r128) = metadata.r128_track_gain {
        if !r128.trim().is_empty() {
            comments.set("R128_TRACK_GAIN", vec![r128.clone()]);
        }
    }

    if let Some(energy) = metadata.energy {
        comments.set("ENERGY", vec![format!("{:.2}", energy)]);
    }

    if let Some(danceability) = metadata.danceability {
        comments.set("DANCEABILITY", vec![format!("{:.2}", danceability)]);
    }

    if let Some(loudness) = metadata.loudness {
        comments.set("LOUDNESS", vec![format!("{:.1}", loudness)]);
    }

    if let Some(ref comment) = metadata.comment {
        if !comment.trim().is_empty() {
            comments.set("COMMENT", vec![comment.clone()]);
        }
    }

    if let Some(ref l_src) = metadata.lyrics_source {
        comments.set("SYNCIFY_LYRICS_SOURCE", vec![l_src.clone()]);
    }

    if let Some(ref c_src) = metadata.cover_source {
        comments.set("SYNCIFY_COVER_SOURCE", vec![c_src.clone()]);
    }

    if let Some(ref a_src) = metadata.audio_source {
        comments.set("SYNCIFY_AUDIO_SOURCE", vec![a_src.clone()]);
    }

    if let Some(depth) = metadata.bit_depth {
        comments.set("BITDEPTH", vec![depth.to_string()]);
    }

    if let Some(rate) = metadata.sample_rate {
        comments.set("SAMPLINGRATE", vec![rate.to_string()]);
    }

    if let Some(ref lyrics) = metadata.lyrics_lrc {
        if !lyrics.trim().is_empty() {
            comments.set("LYRICS", vec![lyrics.clone()]);
            let clean_plain = strip_lrc_timestamps(lyrics);
            if !clean_plain.is_empty() {
                comments.set("UNSYNCEDLYRICS", vec![clean_plain]);
            } else {
                comments.set("UNSYNCEDLYRICS", vec![lyrics.clone()]);
            }
        }
    }

    if let Some(ref mbid) = metadata.musicbrainz_track_id {
        if !mbid.trim().is_empty() {
            comments.set("MUSICBRAINZ_TRACKID", vec![mbid.clone()]);
            comments.set("MUSICBRAINZ_RELEASETRACKID", vec![mbid.clone()]);
        }
    }

    if let Some(ref mbid) = metadata.musicbrainz_artist_id {
        let t = mbid.trim();
        if !t.is_empty() {
            comments.set("MUSICBRAINZ_ARTISTID", vec![t.to_string()]);
        }
    }

    if let Some(ref mbid) = metadata.musicbrainz_album_id {
        if !mbid.trim().is_empty() {
            comments.set("MUSICBRAINZ_ALBUMID", vec![mbid.clone()]);
        }
    }

    if let Some(ref mbid) = metadata.musicbrainz_albumartist_id {
        if !mbid.trim().is_empty() {
            comments.set("MUSICBRAINZ_ALBUMARTISTID", vec![mbid.clone()]);
        }
    }

    if let Some(ref mbid) = metadata.musicbrainz_release_group_id {
        if !mbid.trim().is_empty() {
            comments.set("MUSICBRAINZ_RELEASEGROUPID", vec![mbid.clone()]);
        }
    }

    if let Some(ref mbid) = metadata.musicbrainz_work_id {
        if !mbid.trim().is_empty() {
            comments.set("MUSICBRAINZ_WORKID", vec![mbid.clone()]);
        }
    }

    // Embed cover art adhering to Symfonium invariant & robust mobile memory limits:
    // 1. Validate dimensions (width > 0 && height > 0) to avoid 0x0 crash/empty art.
    // 2. Bound buffer size to <= MAX_EMBEDDED_PICTURE_BYTES (800 KB) to prevent OOM.
    // 3. For WebP sources (or oversized/animated), extract/convert static frame to JPEG.
    // 4. Preserve existing valid animated WebP (dims > 0 && size <= 1 MB) per Symfonium invariant;
    //    force overwrite/repair if existing picture is corrupt (0x0) or oversized (> 1 MB).
    if let Some(ref cover_bytes) = metadata.cover_data {
        if !cover_bytes.is_empty() {
            let prepared_pic = prepare_flac_picture(cover_bytes)?;

            let existing_pic = tag.pictures().find(|p| p.picture_type == PictureType::CoverFront);

            // Determine actual physical dimensions of existing picture block (TASK-131)
            let (existing_w, existing_h) = existing_pic.map(|p| {
                if p.width > 0 && p.height > 0 {
                    (p.width, p.height)
                } else {
                    extract_image_dimensions(&p.data)
                }
            }).unwrap_or((0, 0));

            let existing_is_corrupt_or_oversized = existing_pic.map(|p| {
                (existing_w == 0 || existing_h == 0) || p.data.len() > HARD_CEILING_PICTURE_BYTES
            }).unwrap_or(false);

            let existing_front_type = existing_pic
                .map(|p| WebpByteValidator::detect_cover_type(&p.data))
                .unwrap_or(CoverType::None);

            let incoming_type = WebpByteValidator::detect_cover_type(&prepared_pic.data);

            let decision = if existing_is_corrupt_or_oversized {
                CoverUpdateDecision::Overwrite
            } else {
                CoverPreservationPolicy::evaluate(existing_front_type, incoming_type)
            };

            if decision == CoverUpdateDecision::Overwrite {
                tag.remove_picture_type(PictureType::CoverFront);
                tag.push_block(metaflac::Block::Picture(prepared_pic));
            } else {
                // Symfonium invariant: preserving existing valid animated WebP CoverFront.
                // If it had legacy 0x0 block dimensions, assign its real physical dimensions (TASK-131).
                if let Some(mut existing) = existing_pic.cloned() {
                    if (existing.width == 0 || existing.height == 0) && existing_w > 0 && existing_h > 0 {
                        existing.width = existing_w;
                        existing.height = existing_h;
                        tag.remove_picture_type(PictureType::CoverFront);
                        tag.push_block(metaflac::Block::Picture(existing));
                        debug!(
                            "Preserved and healed legacy 0x0 animated WebP CoverFront with real dimensions {}x{} in {:?}",
                            existing_w, existing_h, file_path
                        );
                    } else {
                        debug!("Preserving existing valid animated image/webp CoverFront block against static JPEG/PNG incoming payload in {:?}", file_path);
                    }
                }
            }
        }
    }

    // Ensure all PICTURE blocks in the FLAC container declare real physical dimensions (TASK-131).
    // If any PICTURE block has 0x0 dimensions (e.g. legacy tagger or untouched block), extract and assign real dimensions.
    let pictures: Vec<_> = tag.pictures().cloned().collect();
    let mut picture_dims_healed = false;
    let mut updated_pictures = Vec::new();
    for mut pic in pictures {
        if pic.width == 0 || pic.height == 0 {
            let (w, h) = extract_image_dimensions(&pic.data);
            if w > 0 && h > 0 {
                pic.width = w;
                pic.height = h;
                picture_dims_healed = true;
            }
        }
        updated_pictures.push(pic);
    }
    if picture_dims_healed {
        tag.remove_blocks(metaflac::BlockType::Picture);
        for pic in updated_pictures {
            tag.push_block(metaflac::Block::Picture(pic));
        }
    }

    tag.write_to_path(file_path)
        .map_err(|e| format!("Failed to save FLAC tags: {}", e))?;

    info!("Symfonium-compatible VorbisComments tags written to {:?}", file_path);
    Ok(())
}

/// Write FLAC metadata and embed/preserve cover art with accurate dimensions (TASK-131).
///
/// Ensures all VorbisComments are applied according to Symfonium standards,
/// embedded PICTURE blocks contain real physical dimensions (width > 0, height > 0),
/// and existing animated WebP CoverFront blocks are preserved per the Symfonium invariant.
pub fn write_flac_metadata(file_path: &Path, metadata: &FlacMetadata) -> std::result::Result<(), String> {
    apply_flac_tags(file_path, metadata)
}

/// Convert or extract a static frame to JPEG format using ffmpeg.
///
/// Ensures the resulting image has valid dimensions and fits within the target constraint.
pub fn convert_or_extract_to_jpeg(bytes: &[u8], max_dim: Option<u32>, quality: u32) -> Result<Vec<u8>, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let scale_filter = if let Some(dim) = max_dim {
        format!("scale='min({},iw)':-1", dim)
    } else {
        "scale='min(1200,iw)':-1".to_string()
    };

    let q_str = quality.to_string();

    let mut child = Command::new("ffmpeg")
        .args([
            "-y",
            "-i", "pipe:0",
            "-vframes", "1",
            "-vf", &scale_filter,
            "-q:v", &q_str,
            "-f", "image2",
            "-c:v", "mjpeg",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(bytes);
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait on ffmpeg: {}", e))?;

    if !output.status.success() || output.stdout.is_empty() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg image conversion failed: {}", err_msg.lines().next().unwrap_or("unknown error")));
    }

    Ok(output.stdout)
}

/// Validate, sanitize, and construct a FLAC `metaflac::block::Picture` block.
///
/// Adheres strictly to the following contracts:
/// 1. Verifies physical dimensions: `width > 0 && height > 0`. Rejects images with dimensions 0x0.
/// 2. If the payload is WebP (animated or static without dimensions, or oversized > 800 KB):
///    extracts the first static frame to JPEG.
/// 3. Bounds the picture buffer size to `<= MAX_EMBEDDED_PICTURE_BYTES` (800 KB).
/// 4. Populates all fields of `metaflac::block::Picture` (`width`, `height`, `depth`, `mime_type`, `data`).
pub fn prepare_flac_picture(cover_bytes: &[u8]) -> Result<metaflac::block::Picture, String> {
    if cover_bytes.is_empty() {
        return Err("Picture payload is empty".to_string());
    }

    let initial_dims = ImageByteValidator::parse_dimensions(cover_bytes);
    let incoming_type = WebpByteValidator::detect_cover_type(cover_bytes);

    // Reject 0x0 dimensions immediately if parsed from headers
    if let Some(ref dims) = initial_dims {
        if dims.width == 0 || dims.height == 0 {
            // Attempt conversion/repair via ffmpeg; if decoding fails or still 0x0, reject!
            match convert_or_extract_to_jpeg(cover_bytes, Some(1000), 85) {
                Ok(converted) => {
                    let conv_dims = ImageByteValidator::parse_dimensions(&converted)
                        .ok_or_else(|| "Converted image has unrecognized header".to_string())?;
                    if conv_dims.width == 0 || conv_dims.height == 0 {
                        return Err("Cover image rejected: dimensions are 0x0".to_string());
                    }
                    return build_picture_block(converted, conv_dims);
                }
                Err(_) => {
                    return Err("Cover image rejected: dimensions are 0x0".to_string());
                }
            }
        }
    }

    let is_animated_webp = incoming_type.is_animated()
        || WebpByteValidator::validate_animated_webp(cover_bytes).map(|w| w.is_animated).unwrap_or(false);

    let needs_conversion = incoming_type.is_webp()
        || is_animated_webp
        || cover_bytes.len() > MAX_EMBEDDED_PICTURE_BYTES
        || initial_dims.is_none();

    let (final_bytes, final_dims) = if needs_conversion {
        let mut jpeg_bytes = convert_or_extract_to_jpeg(cover_bytes, Some(1200), 85)?;

        if jpeg_bytes.len() > MAX_EMBEDDED_PICTURE_BYTES {
            if let Ok(recomp) = convert_or_extract_to_jpeg(&jpeg_bytes, Some(1000), 75) {
                jpeg_bytes = recomp;
            }
        }
        if jpeg_bytes.len() > MAX_EMBEDDED_PICTURE_BYTES {
            if let Ok(recomp) = convert_or_extract_to_jpeg(&jpeg_bytes, Some(800), 65) {
                jpeg_bytes = recomp;
            }
        }
        if jpeg_bytes.len() > MAX_EMBEDDED_PICTURE_BYTES {
            return Err(format!(
                "Picture buffer size ({} bytes) exceeds maximum limit ({} bytes) after compression",
                jpeg_bytes.len(),
                MAX_EMBEDDED_PICTURE_BYTES
            ));
        }

        let dims = ImageByteValidator::parse_dimensions(&jpeg_bytes)
            .ok_or_else(|| "Failed to parse dimensions of converted JPEG cover".to_string())?;
        if dims.width == 0 || dims.height == 0 {
            return Err("Converted cover has invalid dimensions (0x0)".to_string());
        }
        (jpeg_bytes, dims)
    } else {
        let dims = initial_dims.unwrap();
        if dims.width == 0 || dims.height == 0 {
            return Err("Cover image rejected: dimensions are 0x0".to_string());
        }
        (cover_bytes.to_vec(), dims)
    };

    build_picture_block(final_bytes, final_dims)
}

fn build_picture_block(data: Vec<u8>, mut dims: ImageDimensions) -> Result<metaflac::block::Picture, String> {
    if data.len() > MAX_EMBEDDED_PICTURE_BYTES {
        return Err(format!(
            "Picture buffer size ({} bytes) exceeds maximum limit ({} bytes)",
            data.len(),
            MAX_EMBEDDED_PICTURE_BYTES
        ));
    }

    let (ext_w, ext_h) = extract_image_dimensions(&data);
    if ext_w > 0 && ext_h > 0 {
        dims.width = ext_w;
        dims.height = ext_h;
    }

    if dims.width == 0 || dims.height == 0 {
        return Err("Picture dimensions must be > 0".to_string());
    }

    let mut pic = metaflac::block::Picture::new();
    pic.picture_type = metaflac::block::PictureType::CoverFront;
    pic.mime_type = dims.mime_type.to_string();
    pic.description = "Front Cover".to_string();
    pic.width = dims.width;
    pic.height = dims.height;
    pic.depth = dims.depth;
    pic.num_colors = 0;
    pic.data = data;
    Ok(pic)
}

/// Extract physical image dimensions (width, height) from raw image bytes (TASK-131).
///
/// Supports JPEG (SOF0/SOF1/SOF2 markers), WebP (VP8X canvas, VP8 keyframe, VP8L),
/// and PNG (IHDR chunk).
///
/// Returns `(width, height)` in pixels.
/// Defensively falls back to `(0, 0)` if the format is unrecognized, truncated, or invalid.
pub fn extract_image_dimensions(data: &[u8]) -> (u32, u32) {
    if data.is_empty() {
        return (0, 0);
    }

    // 1. PNG: 8-byte magic "\x89PNG\r\n\x1a\n" followed by IHDR chunk
    if data.starts_with(b"\x89PNG\r\n\x1a\n") && data.len() >= 24 {
        if &data[12..16] == b"IHDR" {
            let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
            let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
            return (width, height);
        }
    }

    // 2. WebP: RIFF container with WEBP fourcc
    if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        let mut offset = 12;
        while offset + 8 <= data.len() {
            let chunk_fourcc = &data[offset..offset + 4];
            let chunk_len = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;
            let payload_start = offset + 8;

            if chunk_fourcc == b"VP8X" {
                // VP8X canvas dimensions:
                // payload bytes 4..7: Canvas width - 1 (24 bits LE)
                // payload bytes 7..10: Canvas height - 1 (24 bits LE)
                if payload_start + 10 <= data.len() {
                    let w_raw = data[payload_start + 4] as u32
                        | ((data[payload_start + 5] as u32) << 8)
                        | ((data[payload_start + 6] as u32) << 16);
                    let h_raw = data[payload_start + 7] as u32
                        | ((data[payload_start + 8] as u32) << 8)
                        | ((data[payload_start + 9] as u32) << 16);
                    return (1 + w_raw, 1 + h_raw);
                }
            } else if chunk_fourcc == b"VP8 " {
                // Lossy VP8 keyframe:
                // payload byte 0: frame tag (bit 0 must be 0 for keyframe)
                // payload bytes 3..6: start code 0x9D 0x01 0x2A
                // payload bytes 6..8: 14-bit width LE
                // payload bytes 8..10: 14-bit height LE
                if payload_start + 10 <= data.len() {
                    if (data[payload_start] & 0x01) == 0
                        && &data[payload_start + 3..payload_start + 6] == [0x9D, 0x01, 0x2A]
                    {
                        let width = (data[payload_start + 6] as u32
                            | ((data[payload_start + 7] as u32) << 8))
                            & 0x3FFF;
                        let height = (data[payload_start + 8] as u32
                            | ((data[payload_start + 9] as u32) << 8))
                            & 0x3FFF;
                        return (width, height);
                    }
                }
            } else if chunk_fourcc == b"VP8L" {
                // Lossless VP8L:
                // payload byte 0: 0x2F signature
                // payload bytes 1..5: 14 bits width-1, 14 bits height-1
                if payload_start + 5 <= data.len() && data[payload_start] == 0x2F {
                    let b1 = data[payload_start + 1] as u32;
                    let b2 = data[payload_start + 2] as u32;
                    let b3 = data[payload_start + 3] as u32;
                    let b4 = data[payload_start + 4] as u32;
                    let width = 1 + ((b1 | (b2 << 8)) & 0x3FFF);
                    let height = 1 + (((b2 >> 6) | (b3 << 2) | (b4 << 10)) & 0x3FFF);
                    return (width, height);
                }
            }

            // RIFF chunks are 2-byte aligned
            let padded_len = chunk_len + (chunk_len & 1);
            offset = match payload_start.checked_add(padded_len) {
                Some(next) => next,
                None => break,
            };
        }
    }

    // 3. JPEG: Starts with SOI marker 0xFF 0xD8
    if data.starts_with(&[0xFF, 0xD8]) {
        let mut offset = 2;
        while offset < data.len() {
            if data[offset] != 0xFF {
                offset += 1;
                continue;
            }
            while offset < data.len() && data[offset] == 0xFF {
                offset += 1;
            }
            if offset >= data.len() {
                break;
            }
            let marker = data[offset];
            offset += 1;

            // Standalone markers without payload
            if (0xD0..=0xD7).contains(&marker) || marker == 0xD8 || marker == 0x01 {
                continue;
            }
            // EOI (0xD9) or SOS (0xDA) terminates header scan
            if marker == 0xD9 || marker == 0xDA {
                break;
            }
            if offset + 2 > data.len() {
                break;
            }
            let segment_len = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
            if segment_len < 2 {
                break;
            }
            // Start Of Frame markers: SOF0..SOF3, SOF5..SOF7, SOF9..SOF11, SOF13..SOF15
            let is_sof = matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
            if is_sof && segment_len >= 7 && offset + 7 <= data.len() {
                let height = u16::from_be_bytes([data[offset + 3], data[offset + 4]]) as u32;
                let width = u16::from_be_bytes([data[offset + 5], data[offset + 6]]) as u32;
                return (width, height);
            }

            offset = match offset.checked_add(segment_len) {
                Some(next) => next,
                None => break,
            };
        }
    }

    (0, 0)
}

/// Construct a new FLAC `metaflac::block::Picture` populated with real extracted dimensions (TASK-131).
pub fn create_flac_picture(
    data: Vec<u8>,
    picture_type: metaflac::block::PictureType,
    mime_type: Option<&str>,
    description: Option<&str>,
) -> metaflac::block::Picture {
    let (width, height) = extract_image_dimensions(&data);
    let mime = mime_type
        .map(|s| s.to_string())
        .or_else(|| ImageByteValidator::parse_dimensions(&data).map(|d| d.mime_type.to_string()))
        .unwrap_or_else(|| "image/jpeg".to_string());

    let mut pic = metaflac::block::Picture::new();
    pic.picture_type = picture_type;
    pic.mime_type = mime;
    pic.description = description.unwrap_or("Front Cover").to_string();
    pic.width = width;
    pic.height = height;
    pic.depth = 24;
    pic.num_colors = 0;
    pic.data = data;
    pic
}

/// Extension trait for `metaflac::Tag` providing dimension-aware picture methods (TASK-131).
pub trait FlacTagExt {
    /// Add a picture block to the FLAC tag with automatically extracted physical dimensions.
    fn add_picture_with_dimensions(
        &mut self,
        mime_type: &str,
        picture_type: metaflac::block::PictureType,
        data: Vec<u8>,
    );
}

impl FlacTagExt for metaflac::Tag {
    fn add_picture_with_dimensions(
        &mut self,
        mime_type: &str,
        picture_type: metaflac::block::PictureType,
        data: Vec<u8>,
    ) {
        let pic = create_flac_picture(data, picture_type, Some(mime_type), None);
        self.push_block(metaflac::Block::Picture(pic));
    }
}

/// Inspect a FLAC file and sanitize any embedded PICTURE blocks that violate
/// the compatibility contract (dimensions 0x0, size > 800 KB, or oversized/corrupt WebP).
///
/// Returns `Ok(true)` if repairs were applied, `Ok(false)` if already compliant.
pub fn sanitize_flac_pictures(file_path: &Path) -> Result<bool, String> {
    let mut tag = metaflac::Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to read FLAC file: {}", e))?;

    let pictures: Vec<_> = tag.pictures().cloned().collect();
    if pictures.is_empty() {
        return Ok(false);
    }

    let mut modified = false;
    let mut sanitized_blocks = Vec::new();

    for pic in pictures {
        let is_corrupt_dims = pic.width == 0 || pic.height == 0;
        let is_oversized = pic.data.len() > MAX_EMBEDDED_PICTURE_BYTES;
        let is_webp = pic.mime_type.to_lowercase().contains("webp");

        if is_corrupt_dims || is_oversized || is_webp {
            match prepare_flac_picture(&pic.data) {
                Ok(mut clean_pic) => {
                    clean_pic.picture_type = pic.picture_type;
                    sanitized_blocks.push(clean_pic);
                    modified = true;
                }
                Err(e) => {
                    tracing::warn!("Removing unrepairable picture block from {:?}: {}", file_path, e);
                    modified = true;
                }
            }
        } else {
            sanitized_blocks.push(pic);
        }
    }

    if modified {
        tag.remove_blocks(metaflac::BlockType::Picture);
        for p in sanitized_blocks {
            tag.push_block(metaflac::Block::Picture(p));
        }
        tag.write_to_path(file_path)
            .map_err(|e| format!("Failed to write sanitized FLAC tags: {}", e))?;
    }

    Ok(modified)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PictureBlockSummary {
    pub picture_type: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub data_len: usize,
    pub data_md5: String,
    pub has_vp8x: bool,
    pub has_anim: bool,
    pub anmf_frames: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlacPictureAuditReport {
    pub stage: String,
    pub file_path: String,
    pub picture_count: usize,
    pub pictures: Vec<PictureBlockSummary>,
    pub sidecar_cover_webp_exists: bool,
    pub sidecar_folder_webp_exists: bool,
    pub sidecar_animated_webp_exists: bool,
    pub sidecar_cover_jpg_exists: bool,
}

/// Audit internal METADATA_BLOCK_PICTURE blocks and sidecars at any pipeline stage.
pub fn audit_flac_stage(stage_name: &str, file_path: &Path) -> Result<FlacPictureAuditReport, String> {
    let tag = metaflac::Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to read FLAC at stage '{}': {}", stage_name, e))?;

    let pictures: Vec<_> = tag.pictures().collect();
    let mut pic_summaries = Vec::new();

    for pic in &pictures {
        let md5_hex = format!("{:x}", md5::compute(&pic.data));

        let (has_vp8x, has_anim, anmf_frames) = match WebpByteValidator::validate_animated_webp(&pic.data) {
            Ok(info) => (info.is_extended, info.is_animated, info.anmf_frame_count),
            Err(_) => {
                let _is_webp = pic.data.starts_with(b"RIFF") && pic.data.len() > 12 && &pic.data[8..12] == b"WEBP";
                (false, false, 0)
            }
        };

        pic_summaries.push(PictureBlockSummary {
            picture_type: format!("{:?}", pic.picture_type),
            mime_type: pic.mime_type.clone(),
            width: pic.width,
            height: pic.height,
            data_len: pic.data.len(),
            data_md5: md5_hex,
            has_vp8x,
            has_anim,
            anmf_frames,
        });
    }

    let parent = file_path.parent();
    let sidecar_cover_webp = parent.map(|p| p.join("cover.webp").exists()).unwrap_or(false);
    let sidecar_folder_webp = parent.map(|p| p.join("folder.webp").exists()).unwrap_or(false);
    let sidecar_animated_webp = parent.map(|p| p.join("animated.webp").exists()).unwrap_or(false);
    let sidecar_cover_jpg = parent.map(|p| p.join("cover.jpg").exists()).unwrap_or(false);

    let report = FlacPictureAuditReport {
        stage: stage_name.to_string(),
        file_path: file_path.to_string_lossy().to_string(),
        picture_count: pictures.len(),
        pictures: pic_summaries,
        sidecar_cover_webp_exists: sidecar_cover_webp,
        sidecar_folder_webp_exists: sidecar_folder_webp,
        sidecar_animated_webp_exists: sidecar_animated_webp,
        sidecar_cover_jpg_exists: sidecar_cover_jpg,
    };

    info!(
        "[Audit::Stage: {}] FLAC: {:?} | Pictures: {} | CoverWebP: {} | FolderWebP: {} | AnimatedWebP: {} | CoverJpg: {}",
        report.stage, file_path.file_name().unwrap_or_default(), report.picture_count,
        report.sidecar_cover_webp_exists, report.sidecar_folder_webp_exists, report.sidecar_animated_webp_exists, report.sidecar_cover_jpg_exists
    );
    for (i, p) in report.pictures.iter().enumerate() {
        info!(
            "  -> Picture #{}: Type={}, MIME={}, Size={}B, MD5={}, VP8X={}, ANIM={}, ANMF_frames={}",
            i + 1, p.picture_type, p.mime_type, p.data_len, p.data_md5,
            p.has_vp8x, p.has_anim, p.anmf_frames
        );
    }

    Ok(report)
}

/// Re-read FLAC file, verify structure, compare persisted tags against expected metadata, and return TagVerification.
pub fn verify_flac_tags(file_path: &Path, expected: &FlacMetadata) -> Result<TagVerification, String> {
    let mut verification = TagVerification {
        file_exists: file_path.exists(),
        flac_valid: false,
        tags_match: false,
        cover_present: false,
        cover_size_bytes: None,
        cover_mime: None,
        cover_width: None,
        cover_height: None,
        lyrics_present: false,
        synced_lyrics_present: false,
        unsynced_lyrics_present: false,
        bpm_present: false,
        duration_sec: None,
        mismatches: Vec::new(),
    };

    if !verification.file_exists {
        return Err(format!("File does not exist: {:?}", file_path));
    }

    let tag = metaflac::Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to parse FLAC file for verification: {}", e))?;

    verification.flac_valid = true;

    // Check STREAMINFO for duration
    let streaminfo = tag.get_streaminfo();
    if let Some(info) = streaminfo {
        if info.sample_rate > 0 {
            verification.duration_sec = Some(info.total_samples as f64 / info.sample_rate as f64);
        }
    }

    // Check Cover Art
    for pic in tag.pictures() {
        verification.cover_present = true;
        verification.cover_size_bytes = Some(pic.data.len());
        verification.cover_mime = Some(pic.mime_type.clone());
        verification.cover_width = Some(pic.width);
        verification.cover_height = Some(pic.height);
        break;
    }

    // Check VorbisComments
    if let Some(comments) = tag.vorbis_comments() {
        let read_val = |key: &str| -> Option<String> {
            comments.get(key).and_then(|v| v.first().cloned())
        };

        if let Some(lrc) = comments.get("LYRICS") {
            verification.synced_lyrics_present = !lrc.is_empty();
            verification.lyrics_present = true;
        }
        if let Some(un) = comments.get("UNSYNCEDLYRICS") {
            verification.unsynced_lyrics_present = !un.is_empty();
            verification.lyrics_present = true;
        }
        if comments.get("BPM").is_some() || comments.get("TEMPO").is_some() {
            verification.bpm_present = true;
        }

        let mut mismatches = Vec::new();

        fn check_field(
            mismatches: &mut Vec<(String, String, String)>,
            key: &str,
            expected_val: Option<&str>,
            actual_val: Option<String>,
        ) {
            if let Some(exp) = expected_val {
                if !exp.trim().is_empty() {
                    let actual = actual_val.unwrap_or_default();
                    if actual != exp {
                        mismatches.push((key.to_string(), exp.to_string(), actual));
                    }
                }
            }
        }

        let (cleaned_expected_title, feat_from_exp_title) = clean_title_and_extract_featured(&expected.title);
        let exp_title = if !cleaned_expected_title.is_empty() {
            &cleaned_expected_title
        } else {
            &expected.title
        };
        let actual_title = read_val("TITLE").unwrap_or_default();
        if is_valid_tag_val(&expected.title) && actual_title != expected.title && actual_title != *exp_title {
            mismatches.push(("TITLE".to_string(), exp_title.to_string(), actual_title));
        }

        let actual_artists = comments.get("ARTIST").cloned().unwrap_or_default();
        if let Some(ref exp_artists) = expected.artists {
            let valid_exp: Vec<String> = exp_artists
                .iter()
                .filter(|a| is_valid_tag_val(a))
                .cloned()
                .collect();
            if actual_artists != valid_exp {
                mismatches.push((
                    "ARTIST".to_string(),
                    valid_exp.join("; "),
                    actual_artists.join("; "),
                ));
            }
        } else {
            let exp_resolved = resolve_flac_artists(&expected.artist, None, &feat_from_exp_title);
            let first_actual = actual_artists.first().map(|s| s.as_str()).unwrap_or("");
            if is_valid_tag_val(&expected.artist) {
                if !actual_artists.contains(&expected.artist)
                    && first_actual != expected.artist
                    && (exp_resolved.is_empty() || actual_artists != exp_resolved)
                {
                    mismatches.push(("ARTIST".to_string(), expected.artist.clone(), first_actual.to_string()));
                }
            }
        }
        check_field(&mut mismatches, "ALBUM", Some(&expected.album), read_val("ALBUM"));
        let expected_album_artist = if let Some(ref aa) = expected.album_artist {
            if is_valid_tag_val(aa) {
                Some(aa.as_str())
            } else if expected.is_compilation() {
                Some("Various Artists")
            } else {
                None
            }
        } else if expected.is_compilation() {
            Some("Various Artists")
        } else {
            None
        };
        check_field(&mut mismatches, "ALBUMARTIST", expected_album_artist, read_val("ALBUMARTIST"));
        check_field(&mut mismatches, "COMPOSER", expected.composer.as_deref(), read_val("COMPOSER"));
        check_field(&mut mismatches, "PERFORMER", expected.performers.as_deref(), read_val("PERFORMER"));
        if expected.track_number > 0 {
            check_field(&mut mismatches, "TRACKNUMBER", Some(&expected.track_number.to_string()), read_val("TRACKNUMBER"));
        }
        let effective_track_total = expected.effective_track_total();
        if effective_track_total > 0 {
            check_field(&mut mismatches, "TRACKTOTAL", Some(&effective_track_total.to_string()), read_val("TRACKTOTAL"));
        }
        if expected.disc_number > 0 {
            check_field(&mut mismatches, "DISCNUMBER", Some(&expected.disc_number.to_string()), read_val("DISCNUMBER"));
        }
        let effective_disc_total = expected.effective_disc_total();
        if effective_disc_total > 0 {
            check_field(&mut mismatches, "DISCTOTAL", Some(&effective_disc_total.to_string()), read_val("DISCTOTAL"));
        }
        fn check_multi_field(
            mismatches: &mut Vec<(String, String, String)>,
            key: &str,
            expected_val: Option<&str>,
            actual_vals: Vec<String>,
        ) {
            if let Some(exp) = expected_val {
                if !exp.trim().is_empty() {
                    let exp_list = syncify_metadata_domain::fuse_genres(&[exp]);
                    let matches = if actual_vals.len() > 1 && exp_list.len() > 1 {
                        actual_vals == exp_list
                    } else {
                        actual_vals.first().map(|s| s.as_str()) == Some(exp)
                            || actual_vals.join("; ") == exp
                            || actual_vals.join(";") == exp
                            || actual_vals == exp_list
                    };
                    if !matches {
                        mismatches.push((key.to_string(), exp.to_string(), actual_vals.join("; ")));
                    }
                }
            }
        }

        check_multi_field(
            &mut mismatches,
            "GENRE",
            expected.genre.as_deref(),
            comments.get("GENRE").cloned().unwrap_or_default(),
        );
        check_multi_field(
            &mut mismatches,
            "STYLE",
            expected.style.as_deref(),
            comments.get("STYLE").cloned().unwrap_or_default(),
        );
        check_multi_field(
            &mut mismatches,
            "MOOD",
            expected.mood.as_deref(),
            comments.get("MOOD").cloned().unwrap_or_default(),
        );
        check_multi_field(
            &mut mismatches,
            "TAGS",
            expected.tags.as_deref(),
            comments.get("TAGS").cloned().unwrap_or_default(),
        );
        if let Some(ref artist_tags) = expected.artist_tags {
            let actual_artist_tags = comments.get("ARTISTS_TAGS").cloned().unwrap_or_default();
            let exp_flat: Vec<String> = artist_tags
                .iter()
                .flat_map(|t| syncify_metadata_domain::fuse_genres(&[t.as_str()]))
                .collect();
            if !exp_flat.is_empty() && actual_artist_tags != exp_flat {
                mismatches.push((
                    "ARTISTS_TAGS".to_string(),
                    exp_flat.join("; "),
                    actual_artist_tags.join("; "),
                ));
            }
        }
        check_field(&mut mismatches, "MEDIA", expected.media_type.as_deref(), read_val("MEDIA"));
        check_field(&mut mismatches, "MUSICTYPE", expected.media_type.as_deref(), read_val("MUSICTYPE"));
        check_field(&mut mismatches, "GROUPING", expected.grouping.as_deref(), read_val("GROUPING"));
        if expected.is_compilation() {
            check_field(&mut mismatches, "COMPILATION", Some("1"), read_val("COMPILATION"));
        } else if expected.compilation == Some(false) {
            if let Some(actual) = read_val("COMPILATION") {
                mismatches.push(("COMPILATION".to_string(), "None".to_string(), actual));
            }
        }
        check_field(&mut mismatches, "RELEASETYPE", expected.release_type.as_deref(), read_val("RELEASETYPE"));
        check_field(&mut mismatches, "RELEASESTATUS", expected.release_status.as_deref(), read_val("RELEASESTATUS"));
        // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
        // The verifier computes the expected wire value with the SAME shared domain
        // helper apply_flac_tags uses (wire_country_value), so it validates against the
        // exact value that was written — canonical English names for sovereign countries.
        let norm_country = expected
            .release_country
            .as_deref()
            .map(syncify_metadata_domain::wire_country_value);
        check_field(&mut mismatches, "RELEASECOUNTRY", norm_country.as_deref().or(expected.release_country.as_deref()), read_val("RELEASECOUNTRY"));
        check_field(&mut mismatches, "COUNTRY", norm_country.as_deref().or(expected.release_country.as_deref()), read_val("COUNTRY"));
        check_field(&mut mismatches, "RELEASEREGION", expected.release_region.as_deref(), read_val("RELEASEREGION"));
        let norm_lang = expected
            .language
            .as_deref()
            .map(syncify_metadata_domain::wire_language_value);
        check_field(&mut mismatches, "LANGUAGE", norm_lang.as_deref().or(expected.language.as_deref()), read_val("LANGUAGE"));
        check_field(&mut mismatches, "LABEL", expected.label.as_deref(), read_val("LABEL"));
        check_field(&mut mismatches, "BARCODE", expected.barcode.as_deref(), read_val("BARCODE"));
        check_field(&mut mismatches, "CATALOGNUMBER", expected.catalog_number.as_deref(), read_val("CATALOGNUMBER"));
        check_field(&mut mismatches, "ORIGINALDATE", expected.original_date.as_deref(), read_val("ORIGINALDATE"));
        check_field(&mut mismatches, "ISRC", expected.isrc.as_deref(), read_val("ISRC"));
        check_field(&mut mismatches, "YEAR", expected.release_year.as_deref(), read_val("YEAR"));
        check_field(&mut mismatches, "RELEASEDATE", expected.release_date.as_deref(), read_val("RELEASEDATE"));
        if let Some(bpm) = expected.bpm {
            check_field(&mut mismatches, "BPM", Some(&bpm.to_string()), read_val("BPM"));
        }
        check_field(&mut mismatches, "INITIALKEY", expected.initial_key.as_deref(), read_val("INITIALKEY"));
        check_field(&mut mismatches, "REPLAYGAIN_TRACK_GAIN", expected.replaygain_track_gain.as_deref(), read_val("REPLAYGAIN_TRACK_GAIN"));
        check_field(&mut mismatches, "REPLAYGAIN_ALBUM_GAIN", expected.replaygain_album_gain.as_deref(), read_val("REPLAYGAIN_ALBUM_GAIN"));
        check_field(&mut mismatches, "MUSICBRAINZ_TRACKID", expected.musicbrainz_track_id.as_deref(), read_val("MUSICBRAINZ_TRACKID"));
        check_field(&mut mismatches, "MUSICBRAINZ_ARTISTID", expected.musicbrainz_artist_id.as_deref(), read_val("MUSICBRAINZ_ARTISTID"));
        check_field(&mut mismatches, "MUSICBRAINZ_ALBUMID", expected.musicbrainz_album_id.as_deref(), read_val("MUSICBRAINZ_ALBUMID"));
        check_field(&mut mismatches, "MUSICBRAINZ_ALBUMARTISTID", expected.musicbrainz_albumartist_id.as_deref(), read_val("MUSICBRAINZ_ALBUMARTISTID"));

        verification.mismatches = mismatches;
    }

    verification.tags_match = verification.mismatches.is_empty();
    Ok(verification)
}

/// Helper that applies FLAC tags and performs instant re-read validation.
pub fn apply_and_verify_flac_tags(file_path: &Path, metadata: &FlacMetadata) -> std::result::Result<TagVerification, String> {
    apply_flac_tags(file_path, metadata)?;
    verify_flac_tags(file_path, metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct TestFlacFile {
        path: PathBuf,
    }

    impl Drop for TestFlacFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    fn create_test_flac_file() -> TestFlacFile {
        let path = std::env::temp_dir().join(format!("test_flac_writer_{}.flac", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let mut flac_bytes = Vec::new();
        flac_bytes.extend_from_slice(b"fLaC");
        flac_bytes.extend_from_slice(&[
            0x80, 0x00, 0x00, 0x22, // Last metadata block (STREAMINFO), length 34
            0x10, 0x00, 0x10, 0x00, // min/max block size
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // min/max frame size
            0x0A, 0xC4, 0x42, 0xF0, // 44.1kHz, 2 channels, 16 bits, 0 samples
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]);
        std::fs::write(&path, &flac_bytes).expect("Failed to write initial FLAC bytes");
        TestFlacFile { path }
    }

    #[test]
    fn test_explicit_tag_omitted_when_false_or_none() {
        let temp_file = create_test_flac_file();
        let path = &temp_file.path;

        let meta_false = FlacMetadata {
            title: "Clean Track".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            explicit: Some(false),
            ..Default::default()
        };
        apply_flac_tags(path, &meta_false).expect("apply_flac_tags failed");

        let read_tag = metaflac::Tag::read_from_path(path).expect("Failed to read FLAC tag");
        let comments = read_tag.vorbis_comments().expect("No vorbis comments");
        assert!(comments.get("EXPLICIT").is_none(), "EXPLICIT tag should be omitted when explicit == false");

        let meta_none = FlacMetadata {
            title: "Clean Track 2".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            explicit: None,
            ..Default::default()
        };
        apply_flac_tags(path, &meta_none).expect("apply_flac_tags failed");

        let read_tag = metaflac::Tag::read_from_path(path).expect("Failed to read FLAC tag");
        let comments = read_tag.vorbis_comments().expect("No vorbis comments");
        assert!(comments.get("EXPLICIT").is_none(), "EXPLICIT tag should be omitted when explicit == None");
    }

    #[test]
    fn test_explicit_tag_written_when_true() {
        let temp_file = create_test_flac_file();
        let path = &temp_file.path;

        let meta_true = FlacMetadata {
            title: "Explicit Track".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            explicit: Some(true),
            ..Default::default()
        };
        apply_flac_tags(path, &meta_true).expect("apply_flac_tags failed");

        let read_tag = metaflac::Tag::read_from_path(path).expect("Failed to read FLAC tag");
        let comments = read_tag.vorbis_comments().expect("No vorbis comments");
        let explicit_comments = comments.get("EXPLICIT").expect("EXPLICIT tag should be written when explicit == true");
        assert_eq!(explicit_comments, &vec!["1".to_string()]);
    }

    #[test]
    fn test_work_tag_whitespace_omitted() {
        let temp_file = create_test_flac_file();
        let path = &temp_file.path;

        let meta_empty_work = FlacMetadata {
            title: "Non-classical Track".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            work: Some("   ".to_string()),
            ..Default::default()
        };
        apply_flac_tags(path, &meta_empty_work).expect("apply_flac_tags failed");

        let read_tag = metaflac::Tag::read_from_path(path).expect("Failed to read FLAC tag");
        let comments = read_tag.vorbis_comments().expect("No vorbis comments");
        assert!(comments.get("WORK").is_none(), "WORK tag should be omitted when string is empty or whitespace");
    }

    #[test]
    fn test_catalog_number_and_original_date_written() {
        let temp_file = create_test_flac_file();
        let path = &temp_file.path;

        let meta = FlacMetadata {
            title: "Track".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            catalog_number: Some("CAT-12345".to_string()),
            original_date: Some("1973-03-01".to_string()),
            ..Default::default()
        };
        apply_flac_tags(path, &meta).expect("apply_flac_tags failed");

        let read_tag = metaflac::Tag::read_from_path(path).expect("Failed to read FLAC tag");
        let comments = read_tag.vorbis_comments().expect("No vorbis comments");
        assert_eq!(comments.get("CATALOGNUMBER"), Some(&vec!["CAT-12345".to_string()]));
        assert_eq!(comments.get("ORIGINALDATE"), Some(&vec!["1973-03-01".to_string()]));
    }

    #[test]
    fn test_apply_and_verify_flac_tags_full_roundtrip() {
        let temp_file = create_test_flac_file();
        let path = &temp_file.path;

        let meta = FlacMetadata {
            title: "Verified Track".to_string(),
            artist: "Verified Artist".to_string(),
            album: "Verified Album".to_string(),
            genre: Some("Rock".to_string()),
            style: Some("Hard Rock".to_string()),
            mood: Some("energetic".to_string()),
            bpm: Some(120),
            lyrics_lrc: Some("[00:10.00] Line 1\n[00:20.00] Line 2".to_string()),
            ..Default::default()
        };

        let ver = apply_and_verify_flac_tags(path, &meta).expect("apply_and_verify_flac_tags failed");

        assert!(ver.file_exists);
        assert!(ver.flac_valid);
        assert!(ver.tags_match);
        assert!(ver.bpm_present);
        assert!(ver.lyrics_present);
        assert!(ver.synced_lyrics_present);
        assert!(ver.unsynced_lyrics_present);
        assert!(ver.mismatches.is_empty());
    }

    #[test]
    fn test_multidisc_flac_tags_disctotal_totaldiscs_tracktotal() {
        let temp_file = create_test_flac_file();
        let path = &temp_file.path;

        let meta = FlacMetadata {
            title: "Disc 2 Track 3".to_string(),
            artist: "Multidisc Artist".to_string(),
            album: "Complete Anthology (Box Set)".to_string(),
            track_number: 3,
            track_total: 41,               // Total tracks in box set
            disc_track_total: Some(14),    // Total tracks on Disc 2 specifically
            disc_number: 2,
            total_discs: Some(3),          // 3-CD box set
            ..Default::default()
        };

        let ver = apply_and_verify_flac_tags(path, &meta).expect("apply_and_verify_flac_tags failed");
        assert!(ver.tags_match, "Tags must match: {:?}", ver.mismatches);

        let read_tag = metaflac::Tag::read_from_path(path).expect("Failed to read FLAC tag");
        let comments = read_tag.vorbis_comments().expect("No vorbis comments");

        assert_eq!(comments.get("DISCNUMBER"), Some(&vec!["2".to_string()]));
        assert_eq!(comments.get("DISCTOTAL"), Some(&vec!["3".to_string()]));
        assert_eq!(comments.get("TOTALDISCS"), Some(&vec!["3".to_string()]));
        assert_eq!(comments.get("TRACKNUMBER"), Some(&vec!["3".to_string()]));
        // TRACKTOTAL must reflect local disc total (14), NOT box set total (41)
        assert_eq!(comments.get("TRACKTOTAL"), Some(&vec!["14".to_string()]));
    }

    #[test]
    fn test_compilation_various_artists_emitted() {
        let temp_file = create_test_flac_file();
        let path = &temp_file.path;

        let meta = FlacMetadata {
            title: "Track from Compilation".to_string(),
            artist: "Soloist Artist".to_string(),
            album: "Top Hits 2024".to_string(),
            compilation: Some(true),
            album_artist: None, // Unset: should automatically emit "Various Artists"
            ..Default::default()
        };

        let ver = apply_and_verify_flac_tags(path, &meta).expect("apply_and_verify_flac_tags failed");
        assert!(ver.tags_match, "Tags must match: {:?}", ver.mismatches);

        let read_tag = metaflac::Tag::read_from_path(path).expect("Failed to read FLAC tag");
        let comments = read_tag.vorbis_comments().expect("No vorbis comments");

        assert_eq!(comments.get("ALBUMARTIST"), Some(&vec!["Various Artists".to_string()]));
        assert_eq!(comments.get("COMPILATION"), Some(&vec!["1".to_string()]));
        assert_eq!(comments.get("ARTIST"), Some(&vec!["Soloist Artist".to_string()]));
    }

    #[test]
    fn test_compilation_with_compiler_artist_preserved() {
        let temp_file = create_test_flac_file();
        let path = &temp_file.path;

        let meta = FlacMetadata {
            title: "Misirlou".to_string(),
            artist: "Dick Dale".to_string(),
            album: "Pulp Fiction Soundtrack".to_string(),
            compilation: Some(true),
            album_artist: Some("Various Artists".to_string()),
            ..Default::default()
        };

        let ver = apply_and_verify_flac_tags(path, &meta).expect("apply_and_verify_flac_tags failed");
        assert!(ver.tags_match, "Tags must match: {:?}", ver.mismatches);

        let read_tag = metaflac::Tag::read_from_path(path).expect("Failed to read FLAC tag");
        let comments = read_tag.vorbis_comments().expect("No vorbis comments");

        assert_eq!(comments.get("ALBUMARTIST"), Some(&vec!["Various Artists".to_string()]));
        assert_eq!(comments.get("COMPILATION"), Some(&vec!["1".to_string()]));
        assert_eq!(comments.get("ARTIST"), Some(&vec!["Dick Dale".to_string()]));
    }

    #[test]
    fn test_mono_artist_album_artist_preserved_no_compilation() {
        let temp_file = create_test_flac_file();
        let path = &temp_file.path;

        let meta = FlacMetadata {
            title: "Time".to_string(),
            artist: "Pink Floyd".to_string(),
            album: "The Dark Side of the Moon".to_string(),
            album_artist: Some("Pink Floyd".to_string()),
            compilation: None,
            ..Default::default()
        };

        let ver = apply_and_verify_flac_tags(path, &meta).expect("apply_and_verify_flac_tags failed");
        assert!(ver.tags_match, "Tags must match: {:?}", ver.mismatches);

        let read_tag = metaflac::Tag::read_from_path(path).expect("Failed to read FLAC tag");
        let comments = read_tag.vorbis_comments().expect("No vorbis comments");

        assert_eq!(comments.get("ALBUMARTIST"), Some(&vec!["Pink Floyd".to_string()]));
        assert!(comments.get("COMPILATION").is_none(), "COMPILATION tag must NOT be present on mono-artist album");
        assert_eq!(comments.get("ARTIST"), Some(&vec!["Pink Floyd".to_string()]));
    }

    #[test]
    fn test_unify_album_compilation_metadata_multi_artist() {
        let mut tracks = vec![
            FlacMetadata {
                title: "Track 1".to_string(),
                artist: "Artist A".to_string(),
                album: "Unified Hits".to_string(),
                album_artist: None,
                compilation: None,
                ..Default::default()
            },
            FlacMetadata {
                title: "Track 2".to_string(),
                artist: "Artist B".to_string(),
                album: "Unified Hits".to_string(),
                album_artist: None,
                compilation: None,
                ..Default::default()
            },
        ];

        assert!(detect_album_is_compilation(&tracks));
        unify_album_compilation_metadata(&mut tracks, None);

        for t in &tracks {
            assert_eq!(t.compilation, Some(true));
            assert_eq!(t.album_artist, Some("Various Artists".to_string()));
        }
    }

    #[test]
    fn test_unify_album_compilation_metadata_mono_artist() {
        let mut tracks = vec![
            FlacMetadata {
                title: "Track 1".to_string(),
                artist: "Solo Artist".to_string(),
                album: "Solo Album".to_string(),
                album_artist: None,
                compilation: None,
                ..Default::default()
            },
            FlacMetadata {
                title: "Track 2".to_string(),
                artist: "Solo Artist".to_string(),
                album: "Solo Album".to_string(),
                album_artist: None,
                compilation: None,
                ..Default::default()
            },
        ];

        assert!(!detect_album_is_compilation(&tracks));
        unify_album_compilation_metadata(&mut tracks, None);

        for t in &tracks {
            assert_ne!(t.compilation, Some(true));
            assert_eq!(t.album_artist, Some("Solo Artist".to_string()));
        }
    }
}

//! FLAC Metadata DTO & Tag Writer — source-agnostic VorbisComment tagging.
//!
//! Extracted from `download/qobuz.rs` in Sprint 113 to decouple tag writing
//! from the Qobuz download module. All services (Qobuz, Tidal, MusicBrainz,
//! Last.fm, Discogs) construct `FlacMetadata` and call `apply_flac_tags`.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

/// Pure metadata DTO for FLAC tagging — no I/O dependencies.
/// Constructed by service-specific builders (e.g. `build_flac_metadata` in qobuz.rs),
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
    pub r128_track_gain: Option<String>,
    pub comment: Option<String>,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<f64>,
    pub musicbrainz_track_id: Option<String>,
    pub musicbrainz_artist_id: Option<String>,
    pub musicbrainz_album_id: Option<String>,
    pub musicbrainz_release_group_id: Option<String>,
    pub musicbrainz_work_id: Option<String>,
    pub lyrics_lrc: Option<String>,
    pub cover_data: Option<Vec<u8>>,
    pub lyrics_source: Option<String>,
    pub cover_source: Option<String>,
    pub audio_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TagVerification {
    pub file_exists: bool,
    pub flac_valid: bool,
    pub tags_match: bool,
    pub cover_present: bool,
    pub cover_size_bytes: Option<usize>,
    pub cover_mime: Option<String>,
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

/// Apply FLAC tags directly into the FLAC file using metaflac for complete Symfonium compatibility.
///
/// Uses VorbisComments (XiphComment) for FLAC files following exact Symfonium tag naming rules.
/// Preserves unrelated tags, frame boundaries, and padding cleanly.
pub fn apply_flac_tags(file_path: &Path, metadata: &FlacMetadata) -> std::result::Result<(), String> {
    use metaflac::block::PictureType;

    let mut tag = metaflac::Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to open audio file for tagging: {}", e))?;

    let comments = tag.vorbis_comments_mut();

    if is_valid_tag_val(&metadata.title) {
        comments.set_title(vec![metadata.title.clone()]);
    }
    if is_valid_tag_val(&metadata.artist) {
        comments.set_artist(vec![metadata.artist.clone()]);
    }
    if is_valid_tag_val(&metadata.album) {
        comments.set_album(vec![metadata.album.clone()]);
    }

    if let Some(ref album_artist) = metadata.album_artist {
        if is_valid_tag_val(album_artist) {
            comments.set("ALBUMARTIST", vec![album_artist.clone()]);
        }
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
            comments.set("GENRE", vec![genre.clone()]);
        }
    }

    if let Some(ref style) = metadata.style {
        if is_valid_tag_val(style) {
            comments.set("STYLE", vec![style.clone()]);
        }
    }

    if let Some(ref mood) = metadata.mood {
        if is_valid_tag_val(mood) {
            comments.set("MOOD", vec![mood.clone()]);
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
            comments.set("RELEASECOUNTRY", vec![release_country.clone()]);
        }
    }

    if let Some(ref language) = metadata.language {
        if is_valid_tag_val(language) {
            comments.set("LANGUAGE", vec![language.clone()]);
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
        }
    }

    if let Some(ref barcode) = metadata.barcode {
        if is_valid_tag_val(barcode) {
            comments.set("BARCODE", vec![barcode.clone()]);
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

    if metadata.track_total > 0 {
        comments.set("TRACKTOTAL", vec![metadata.track_total.to_string()]);
    }

    if metadata.disc_number > 0 {
        comments.set("DISCNUMBER", vec![metadata.disc_number.to_string()]);
    }

    if metadata.disc_total > 0 {
        comments.set("DISCTOTAL", vec![metadata.disc_total.to_string()]);
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
            comments.set("BPM", vec![bpm.to_string()]);
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
            comments.set("MUSICBRAINZ_RELEASETRACKID", vec![mbid.clone()]);
        }
    }

    if let Some(ref mbid) = metadata.musicbrainz_artist_id {
        if !mbid.trim().is_empty() {
            comments.set("MUSICBRAINZ_ARTISTID", vec![mbid.clone()]);
        }
    }

    if let Some(ref mbid) = metadata.musicbrainz_album_id {
        if !mbid.trim().is_empty() {
            comments.set("MUSICBRAINZ_ALBUMID", vec![mbid.clone()]);
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

    // Embed cover art avoiding duplication (CoverFront MUST always be static JPEG/PNG)
    if let Some(ref cover_bytes) = metadata.cover_data {
        if !cover_bytes.is_empty() {
            if cover_bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                tag.remove_picture_type(PictureType::CoverFront);
                tag.add_picture("image/png", PictureType::CoverFront, cover_bytes.clone());
            } else if cover_bytes.starts_with(b"\xff\xd8\xff") || (!cover_bytes.starts_with(b"RIFF") && !cover_bytes.starts_with(b"GIF")) {
                tag.remove_picture_type(PictureType::CoverFront);
                tag.add_picture("image/jpeg", PictureType::CoverFront, cover_bytes.clone());
            } else if cover_bytes.starts_with(b"RIFF") && cover_bytes.len() > 12 && &cover_bytes[8..12] == b"WEBP" {
                // Preserve static CoverFront and embed WebP as PictureType::Other sidecar frame
                tag.remove_picture_type(PictureType::Other);
                tag.add_picture("image/webp", PictureType::Other, cover_bytes.clone());
            }
        }
    }

    tag.write_to_path(file_path)
        .map_err(|e| format!("Failed to save FLAC tags: {}", e))?;

    info!("Symfonium-compatible VorbisComments tags written to {:?}", file_path);
    Ok(())
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
        if comments.get("BPM").is_some() {
            verification.bpm_present = true;
        }

        // Compare expected vs actual for populated fields
        let mut check_field = |key: &str, expected_val: Option<&str>| {
            if let Some(exp) = expected_val {
                if !exp.trim().is_empty() {
                    let actual = read_val(key).unwrap_or_default();
                    if actual != exp {
                        verification.mismatches.push((key.to_string(), exp.to_string(), actual));
                    }
                }
            }
        };

        check_field("TITLE", Some(&expected.title));
        check_field("ARTIST", Some(&expected.artist));
        check_field("ALBUM", Some(&expected.album));
        check_field("ALBUMARTIST", expected.album_artist.as_deref());
        check_field("GENRE", expected.genre.as_deref());
        check_field("STYLE", expected.style.as_deref());
        check_field("MOOD", expected.mood.as_deref());
        check_field("RELEASETYPE", expected.release_type.as_deref());
        check_field("RELEASESTATUS", expected.release_status.as_deref());
        check_field("RELEASECOUNTRY", expected.release_country.as_deref());
        check_field("LANGUAGE", expected.language.as_deref());
        check_field("LABEL", expected.label.as_deref());
        check_field("BARCODE", expected.barcode.as_deref());
        check_field("CATALOGNUMBER", expected.catalog_number.as_deref());
        check_field("ORIGINALDATE", expected.original_date.as_deref());
        check_field("ISRC", expected.isrc.as_deref());
        check_field("YEAR", expected.release_year.as_deref());
        if let Some(bpm) = expected.bpm {
            check_field("BPM", Some(&bpm.to_string()));
        }
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
        let path = std::env::temp_dir().join(format!("test_tag_writer_{}.flac", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let mut tag = metaflac::Tag::new();
        tag.vorbis_comments_mut().set_title(vec!["Test Track".to_string()]);
        tag.write_to_path(&path).expect("Failed to write initial FLAC tag");
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
}

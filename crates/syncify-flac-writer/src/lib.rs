//! FLAC Metadata DTO & Tag Writer — source-agnostic VorbisComment tagging
//! with non-destructive METADATA_BLOCK_PICTURE and animated WebP preservation.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info};

pub use syncify_core_domain::cover_rules::{CoverPreservationPolicy, CoverType, CoverUpdateDecision};
pub use syncify_core_domain::byte_validators::WebpByteValidator;

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
            let genres = syncify_metadata_domain::fuse_genres(&[genre.as_str()]);
            if !genres.is_empty() {
                comments.set("GENRE", genres);
            }
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
            match syncify_metadata_domain::resolve_country(release_country) {
                syncify_metadata_domain::CountryResolution::Country { canonical_name, .. } => {
                    comments.set("RELEASECOUNTRY", vec![canonical_name.clone()]);
                    comments.set("COUNTRY", vec![canonical_name]);
                }
                syncify_metadata_domain::CountryResolution::Region { region_name, region_code } => {
                    let reg_val = region_code.unwrap_or(region_name.clone());
                    comments.set("RELEASEREGION", vec![reg_val]);
                    comments.set("RELEASECOUNTRY", vec![region_name.clone()]);
                    comments.set("COUNTRY", vec![region_name]);
                }
                syncify_metadata_domain::CountryResolution::Unknown(_) => {
                    comments.set("RELEASECOUNTRY", vec![release_country.clone()]);
                    comments.set("COUNTRY", vec![release_country.clone()]);
                }
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
            let norm_lang = syncify_metadata_domain::resolve_language(language)
                .unwrap_or_else(|| language.clone());
            comments.set("LANGUAGE", vec![norm_lang]);
        }
    }

    if let Some(comp) = metadata.compilation {
        if comp {
            comments.set("COMPILATION", vec!["1".to_string()]);
        }
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
        if !mbid.trim().is_empty() {
            comments.set("MUSICBRAINZ_ARTISTID", vec![mbid.clone()]);
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

    // Embed cover art avoiding destructive loss of animated WebP:
    // INVARIANT: If the FLAC file already has an animated image/webp CoverFront,
    // NEVER overwrite it with a static JPEG/PNG unless the incoming cover is explicitly a WebP.
    if let Some(ref cover_bytes) = metadata.cover_data {
        if !cover_bytes.is_empty() {
            let incoming_type = WebpByteValidator::detect_cover_type(cover_bytes);

            let existing_front_type = tag.pictures()
                .find(|p| p.picture_type == PictureType::CoverFront)
                .map(|p| WebpByteValidator::detect_cover_type(&p.data))
                .unwrap_or(CoverType::None);

            let decision = CoverPreservationPolicy::evaluate(existing_front_type, incoming_type);

            if decision == CoverUpdateDecision::Overwrite {
                tag.remove_picture_type(PictureType::CoverFront);
                tag.add_picture(incoming_type.mime_type(), PictureType::CoverFront, cover_bytes.clone());
            } else {
                debug!("Preserving existing animated image/webp CoverFront block against static JPEG/PNG incoming payload in {:?}", file_path);
            }
        }
    }

    tag.write_to_path(file_path)
        .map_err(|e| format!("Failed to save FLAC tags: {}", e))?;

    info!("Symfonium-compatible VorbisComments tags written to {:?}", file_path);
    Ok(())
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

        check_field(&mut mismatches, "TITLE", Some(&expected.title), read_val("TITLE"));
        check_field(&mut mismatches, "ARTIST", Some(&expected.artist), read_val("ARTIST"));
        check_field(&mut mismatches, "ALBUM", Some(&expected.album), read_val("ALBUM"));
        check_field(&mut mismatches, "ALBUMARTIST", expected.album_artist.as_deref(), read_val("ALBUMARTIST"));
        check_field(&mut mismatches, "COMPOSER", expected.composer.as_deref(), read_val("COMPOSER"));
        check_field(&mut mismatches, "PERFORMER", expected.performers.as_deref(), read_val("PERFORMER"));
        if expected.track_number > 0 {
            check_field(&mut mismatches, "TRACKNUMBER", Some(&expected.track_number.to_string()), read_val("TRACKNUMBER"));
        }
        if expected.track_total > 0 {
            check_field(&mut mismatches, "TRACKTOTAL", Some(&expected.track_total.to_string()), read_val("TRACKTOTAL"));
        }
        if expected.disc_number > 0 {
            check_field(&mut mismatches, "DISCNUMBER", Some(&expected.disc_number.to_string()), read_val("DISCNUMBER"));
        }
        if expected.disc_total > 0 {
            check_field(&mut mismatches, "DISCTOTAL", Some(&expected.disc_total.to_string()), read_val("DISCTOTAL"));
        }
        if let Some(exp_genre) = expected.genre.as_deref() {
            if !exp_genre.trim().is_empty() {
                let actual_genres = comments.get("GENRE").cloned().unwrap_or_default();
                let exp_genres = syncify_metadata_domain::fuse_genres(&[exp_genre]);
                let matches = if actual_genres.len() > 1 && exp_genres.len() > 1 {
                    actual_genres == exp_genres
                } else {
                    actual_genres.first().map(|s| s.as_str()) == Some(exp_genre)
                        || actual_genres.join("; ") == exp_genre
                        || actual_genres.join(";") == exp_genre
                        || actual_genres == exp_genres
                };
                if !matches {
                    mismatches.push((
                        "GENRE".to_string(),
                        exp_genre.to_string(),
                        actual_genres.join("; "),
                    ));
                }
            }
        }
        check_field(&mut mismatches, "STYLE", expected.style.as_deref(), read_val("STYLE"));
        check_field(&mut mismatches, "MOOD", expected.mood.as_deref(), read_val("MOOD"));
        check_field(&mut mismatches, "RELEASETYPE", expected.release_type.as_deref(), read_val("RELEASETYPE"));
        check_field(&mut mismatches, "RELEASESTATUS", expected.release_status.as_deref(), read_val("RELEASESTATUS"));
        let norm_country = expected.release_country.as_deref().map(|c| {
            match syncify_metadata_domain::resolve_country(c) {
                syncify_metadata_domain::CountryResolution::Country { canonical_name, .. } => canonical_name,
                syncify_metadata_domain::CountryResolution::Region { region_name, .. } => region_name,
                _ => c.to_string(),
            }
        });
        check_field(&mut mismatches, "RELEASECOUNTRY", norm_country.as_deref().or(expected.release_country.as_deref()), read_val("RELEASECOUNTRY"));
        check_field(&mut mismatches, "COUNTRY", norm_country.as_deref().or(expected.release_country.as_deref()), read_val("COUNTRY"));
        check_field(&mut mismatches, "RELEASEREGION", expected.release_region.as_deref(), read_val("RELEASEREGION"));
        let norm_lang = expected.language.as_deref().and_then(|l| syncify_metadata_domain::resolve_language(l));
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
}

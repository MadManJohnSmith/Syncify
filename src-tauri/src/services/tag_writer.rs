//! FLAC Metadata DTO & Tag Writer for `src-tauri` using `metaflac`.
//!
//! Provides source-agnostic VorbisComment tagging and post-write re-read verification.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

/// Metadata DTO for FLAC tagging in `src-tauri`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlacMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub performers: Option<String>,
    pub label: Option<String>,
    pub barcode: Option<String>,
    pub catalog_number: Option<String>,
    pub original_date: Option<String>,
    pub track_number: u32,
    pub track_total: u32,
    pub disc_number: u32,
    pub disc_total: u32,
    pub isrc: Option<String>,
    pub release_year: Option<String>,
    pub musicbrainz_track_id: Option<String>,
    pub musicbrainz_artist_id: Option<String>,
    pub musicbrainz_album_id: Option<String>,
    pub musicbrainz_release_group_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TagVerification {
    pub file_exists: bool,
    pub flac_valid: bool,
    pub tags_match: bool,
    pub duration_sec: Option<f64>,
    pub mismatches: Vec<(String, String, String)>,
}

/// Apply FLAC tags directly into the FLAC file using `metaflac`.
///
/// Preserves unrelated existing VorbisComments, STREAMINFO, audio frames and Picture blocks.
pub fn apply_flac_tags(file_path: &Path, metadata: &FlacMetadata) -> Result<(), String> {
    if !file_path.exists() {
        return Err(format!("File does not exist: {:?}", file_path));
    }

    let file_meta = std::fs::metadata(file_path)
        .map_err(|e| format!("Failed to read metadata for {:?}: {}", file_path, e))?;
    if file_meta.len() == 0 {
        return Err(format!("File is empty (0 bytes): {:?}", file_path));
    }

    let mut tag = metaflac::Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to parse FLAC file {:?}: {}", file_path, e))?;

    let streaminfo = tag
        .get_streaminfo()
        .ok_or_else(|| format!("FLAC file has no valid STREAMINFO header: {:?}", file_path))?;
    if streaminfo.sample_rate == 0 {
        return Err(format!("Invalid sample rate in STREAMINFO: {:?}", file_path));
    }

    let comments = tag.vorbis_comments_mut();

    // Standard Tag Fields
    if !metadata.title.trim().is_empty() {
        comments.set_title(vec![metadata.title.clone()]);
    }
    if !metadata.artist.trim().is_empty() {
        comments.set_artist(vec![metadata.artist.clone()]);
    }
    if !metadata.album.trim().is_empty() {
        comments.set_album(vec![metadata.album.clone()]);
    }

    if let Some(ref album_artist) = metadata.album_artist {
        if !album_artist.trim().is_empty() {
            comments.set("ALBUMARTIST", vec![album_artist.clone()]);
        }
    }

    if let Some(ref performers) = metadata.performers {
        if !performers.trim().is_empty() {
            comments.set("PERFORMER", vec![performers.clone()]);
        }
    }

    if let Some(ref label) = metadata.label {
        if !label.trim().is_empty() {
            comments.set("LABEL", vec![label.clone()]);
            comments.set("ORGANIZATION", vec![label.clone()]);
        }
    }

    if let Some(ref barcode) = metadata.barcode {
        if !barcode.trim().is_empty() {
            comments.set("BARCODE", vec![barcode.clone()]);
        }
    }

    if let Some(ref cn) = metadata.catalog_number {
        if !cn.trim().is_empty() {
            comments.set("CATALOGNUMBER", vec![cn.clone()]);
        }
    }

    if let Some(ref od) = metadata.original_date {
        if !od.trim().is_empty() {
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

    if let Some(ref isrc) = metadata.isrc {
        if !isrc.trim().is_empty() {
            comments.set("ISRC", vec![isrc.clone()]);
        }
    }

    if let Some(ref year) = metadata.release_year {
        if !year.trim().is_empty() {
            comments.set("YEAR", vec![year.clone()]);
            comments.set("DATE", vec![year.clone()]);
        }
    }

    if let Some(ref mbid) = metadata.musicbrainz_track_id {
        if !mbid.trim().is_empty() {
            comments.set("MUSICBRAINZ_TRACKID", vec![mbid.clone()]);
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

    // Write tags to path
    tag.write_to_path(file_path)
        .map_err(|e| format!("Failed to save FLAC tags: {}", e))?;

    info!("VorbisComments tags written successfully to {:?}", file_path);
    Ok(())
}

/// Re-read FLAC file, verify structure, compare persisted tags against expected metadata, and return TagVerification.
pub fn verify_flac_tags(file_path: &Path, expected: &FlacMetadata) -> Result<TagVerification, String> {
    let mut verification = TagVerification {
        file_exists: file_path.exists(),
        flac_valid: false,
        tags_match: false,
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

    // Check VorbisComments
    if let Some(comments) = tag.vorbis_comments() {
        let read_val = |key: &str| -> Option<String> {
            comments.get(key).and_then(|v| v.first().cloned())
        };

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
        check_field("PERFORMER", expected.performers.as_deref());
        check_field("LABEL", expected.label.as_deref());
        check_field("CATALOGNUMBER", expected.catalog_number.as_deref());
        check_field("ORIGINALDATE", expected.original_date.as_deref());
        check_field("ISRC", expected.isrc.as_deref());
        check_field("BARCODE", expected.barcode.as_deref());
        check_field("YEAR", expected.release_year.as_deref());
        check_field("MUSICBRAINZ_TRACKID", expected.musicbrainz_track_id.as_deref());
        check_field("MUSICBRAINZ_ARTISTID", expected.musicbrainz_artist_id.as_deref());
        check_field("MUSICBRAINZ_ALBUMID", expected.musicbrainz_album_id.as_deref());
        check_field("MUSICBRAINZ_RELEASEGROUPID", expected.musicbrainz_release_group_id.as_deref());

        if expected.track_number > 0 {
            check_field("TRACKNUMBER", Some(&expected.track_number.to_string()));
        }
        if expected.track_total > 0 {
            check_field("TRACKTOTAL", Some(&expected.track_total.to_string()));
        }
        if expected.disc_number > 0 {
            check_field("DISCNUMBER", Some(&expected.disc_number.to_string()));
        }
        if expected.disc_total > 0 {
            check_field("DISCTOTAL", Some(&expected.disc_total.to_string()));
        }
    }

    verification.tags_match = verification.mismatches.is_empty();
    if !verification.tags_match {
        return Err(format!(
            "Tag verification failed with mismatches: {:?}",
            verification.mismatches
        ));
    }

    Ok(verification)
}

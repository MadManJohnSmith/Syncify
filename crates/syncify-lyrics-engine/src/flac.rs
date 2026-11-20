//! Embebido de letras en FLAC con metaflac y verificación obligatoria de
//! relectura post-escritura.

use syncify_lyrics_domain::{LyricsResolution, ResolutionStatus};

/// Validate and embed lyrics into a FLAC audio file using metaflac,
/// with mandatory post-write re-read verification.
pub fn validate_and_embed_flac_lyrics(
    file_path: &std::path::Path,
    resolution: &LyricsResolution,
) -> Result<bool, String> {
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", file_path.display()));
    }

    let metadata = std::fs::metadata(file_path).map_err(|e| {
        format!(
            "Failed to read file metadata for {}: {}",
            file_path.display(),
            e
        )
    })?;
    if metadata.len() == 0 {
        return Err(format!("File is empty (0 bytes): {}", file_path.display()));
    }

    if resolution.status != ResolutionStatus::Resolved {
        return Err(format!(
            "Cannot embed non-resolved lyrics (status: {:?})",
            resolution.status
        ));
    }

    let tags = resolution.to_tag_contract();
    let lrc_to_write = tags.lyrics.as_deref().unwrap_or("");
    let plain_to_write = tags.unsynced_lyrics.as_deref().unwrap_or("");
    let source_to_write = tags.source.as_deref().unwrap_or("");

    if lrc_to_write.is_empty() && plain_to_write.is_empty() && !resolution.is_instrumental {
        return Err("No lyrics content to embed".to_string());
    }

    // Read audio file with metaflac
    let mut tag = metaflac::Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to parse FLAC file: {}", e))?;

    // Verify STREAMINFO block exists
    let streaminfo = tag.get_streaminfo().ok_or_else(|| {
        format!(
            "FLAC file has no valid STREAMINFO header: {}",
            file_path.display()
        )
    })?;
    if streaminfo.sample_rate == 0 {
        return Err(format!(
            "Invalid sample rate in STREAMINFO: {}",
            file_path.display()
        ));
    }

    // Modify VorbisComments
    let comments = tag.vorbis_comments_mut();

    // Remove existing lyrics to avoid duplication
    comments.remove("LYRICS");
    comments.remove("UNSYNCEDLYRICS");
    comments.remove("SYNCIFY_LYRICS_SOURCE");

    // Write LYRICS Vorbis comment (Enhanced/Line-synced LRC)
    if !lrc_to_write.is_empty() {
        comments.set("LYRICS", vec![lrc_to_write.to_string()]);
    }

    // Write UNSYNCEDLYRICS / Plain lyrics Vorbis comment
    if !plain_to_write.is_empty() {
        comments.set("UNSYNCEDLYRICS", vec![plain_to_write.to_string()]);
    }

    // Write SYNCIFY_LYRICS_SOURCE Vorbis comment
    if !source_to_write.is_empty() {
        comments.set("SYNCIFY_LYRICS_SOURCE", vec![source_to_write.to_string()]);
    }

    // Save to path using metaflac
    tag.write_to_path(file_path)
        .map_err(|e| format!("Failed to save FLAC tags to {}: {}", file_path.display(), e))?;

    // --- MANDATORY POST-WRITE RE-READ VERIFICATION ---
    let verified_tag = metaflac::Tag::read_from_path(file_path).map_err(|e| {
        format!(
            "Verification failed: unable to re-read FLAC file {}: {}",
            file_path.display(),
            e
        )
    })?;

    let verified_comments = verified_tag.vorbis_comments().ok_or_else(|| {
        format!(
            "Verification failed: no VorbisComments found in {} after save",
            file_path.display()
        )
    })?;

    if !lrc_to_write.is_empty() {
        let read_lyrics = verified_comments
            .get("LYRICS")
            .and_then(|v| v.first().map(|s| s.as_str()));
        if read_lyrics != Some(lrc_to_write) {
            return Err(format!(
                "Verification failed: LYRICS mismatch after save in {}",
                file_path.display()
            ));
        }
    }

    if !plain_to_write.is_empty() {
        let read_unsynced = verified_comments
            .get("UNSYNCEDLYRICS")
            .and_then(|v| v.first().map(|s| s.as_str()));
        if read_unsynced != Some(plain_to_write) {
            return Err(format!(
                "Verification failed: UNSYNCEDLYRICS mismatch after save in {}",
                file_path.display()
            ));
        }
    }

    if !source_to_write.is_empty() {
        let read_source = verified_comments
            .get("SYNCIFY_LYRICS_SOURCE")
            .and_then(|v| v.first().map(|s| s.as_str()));
        if read_source != Some(source_to_write) {
            return Err(format!(
                "Verification failed: SYNCIFY_LYRICS_SOURCE mismatch after save in {}",
                file_path.display()
            ));
        }
    }

    Ok(true)
}

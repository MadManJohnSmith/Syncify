//! FLAC cover-art sanitization with a real recovery route (D-03, SYNC-AUD-066).
//!
//! [`syncify_flac_writer::sanitize_flac_pictures`] removes any embedded PICTURE block
//! that violates the embedding contract (0x0 dimensions, above the 800 KB limit, WebP)
//! and, when [`syncify_flac_writer::prepare_flac_picture`] cannot rebuild it, the block
//! used to leave the file as pure data loss. Until this module existed the sanitizer had
//! **no caller in the application**: it only ran inside the test suite, and its
//! `FlacPictureSanitizeReport` had no consumer, so neither the pipeline nor the user
//! learned that a cover had been discarded.
//!
//! This service is the caller the contract asked for. It runs the sanitizer over the
//! FLAC that the download/tagging pipeline just materialized in the library and, for
//! every block that is about to be dropped, tries the two recovery routes D-03 leaves
//! open — in this order:
//!
//! 1. **Re-encode on the host**: [`syncify_flac_writer::salvage_cover_to_jpeg`] decodes
//!    the damaged payload with lenient ffmpeg flags and the rebuilt frame goes back
//!    into the file, so the cover art survives.
//! 2. **External sidecar**: when even the tolerant decode yields nothing, the original
//!    bytes are written next to the FLAC as `<track>.unrecovered-cover.<ext>` (content
//!    addressed, so a repeated run is idempotent and two different lost covers of the
//!    same track never collide). The bytes leave the container but not the album.
//!
//! Whatever happens — repair, recovery, or loss — is appended to `repair_history`
//! (S163) with the file hashes before/after, the audio payload hash proving the audio
//! stream was untouched, the list of actions taken and a JSON detail payload. The
//! sanitizer is deliberately non-fatal for the caller: cover art is metadata, and
//! failing a download over it would be a worse regression than a reported loss.

use std::path::{Path, PathBuf};

use metaflac::block::Picture;
use sqlx::SqlitePool;
use syncify_flac_writer::{
    prepare_flac_picture, salvage_cover_to_jpeg, sanitize_flac_pictures_with_recovery,
    FlacPictureSanitizeReport,
};
use tracing::{info, warn};

use crate::services::repair_guardrail::{
    compute_bytes_sha256, extract_audio_content_hash_from_bytes,
};

/// Marker embedded in the file name of the external sidecar that preserves the bytes of
/// a cover block the host could neither repair nor re-encode (D-03, route 2/2).
pub const UNRECOVERABLE_COVER_SIDECAR_STEM: &str = "unrecovered-cover";

/// Identifies the writer that produced the FLAC, so the audit row says which code path
/// ran the sanitization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlacCoverSanitizeContext {
    /// Audit provenance, e.g. `tidal_pipeline.download`.
    pub provenance: String,
    /// Download this track belongs to, when the caller knows it.
    pub download_id: Option<i64>,
    /// Track this file belongs to, when the caller knows it.
    pub track_id: Option<i64>,
}

impl FlacCoverSanitizeContext {
    /// Builds a context for `provenance` with no linked download/track ids.
    pub fn new(provenance: &str) -> Self {
        Self {
            provenance: provenance.to_string(),
            download_id: None,
            track_id: None,
        }
    }
}

/// Result of one sanitization + recovery pass over a FLAC file.
#[derive(Debug, Clone, Default)]
pub struct FlacCoverSanitizeOutcome {
    /// What the sanitizer did to the embedded blocks.
    pub report: FlacPictureSanitizeReport,
    /// External sidecars holding the original bytes of blocks that left the container.
    pub preserved_sidecars: Vec<PathBuf>,
}

impl FlacCoverSanitizeOutcome {
    /// True when embedded cover art was lost instead of repaired or recovered.
    pub fn cover_art_lost(&self) -> bool {
        self.report.dropped_cover_art()
    }

    /// True when at least one block was rebuilt by the host re-encode route.
    pub fn cover_art_recovered(&self) -> bool {
        self.report.recovered_blocks > 0
    }

    /// True when the file on disk was rewritten in any way.
    pub fn modified(&self) -> bool {
        self.report.modified
    }
}

/// Sanitize the embedded PICTURE blocks of `flac_path`, recovering the cover art before
/// any block is discarded (D-03).
///
/// Blocking: the recovery route spawns `ffmpeg`. Async callers go through
/// [`sanitize_and_audit_flac_cover_art`], which runs this on a blocking worker.
pub fn sanitize_flac_cover_art(flac_path: &Path) -> Result<FlacCoverSanitizeOutcome, String> {
    let mut preserved_sidecars: Vec<PathBuf> = Vec::new();
    let mut recovery = |data: &[u8], source: &Picture| -> Result<Picture, String> {
        recover_unrepairable_cover(flac_path, data, source, &mut preserved_sidecars)
    };
    let report = sanitize_flac_pictures_with_recovery(flac_path, Some(&mut recovery))?;
    let outcome = FlacCoverSanitizeOutcome {
        report,
        preserved_sidecars,
    };
    if outcome.modified() {
        info!(
            path = %flac_path.display(),
            repaired = outcome.report.repaired_blocks,
            recovered = outcome.report.recovered_blocks,
            dropped = outcome.report.dropped_unrepairable_blocks.len(),
            sidecars = outcome.preserved_sidecars.len(),
            "[FlacCoverSanitizer] Embedded cover art sanitized"
        );
    }
    Ok(outcome)
}

/// D-03 recovery route applied to a single block the strict repair path gave up on.
///
/// Tries the host re-encode first; if that fails too, the original bytes are preserved
/// in an external sidecar. The returned `Err` is the cause that reaches the sanitizer
/// report, so it always states where the bytes went.
fn recover_unrepairable_cover(
    flac_path: &Path,
    data: &[u8],
    source: &Picture,
    preserved_sidecars: &mut Vec<PathBuf>,
) -> Result<Picture, String> {
    match salvage_cover_to_jpeg(data).and_then(|jpeg| prepare_flac_picture(&jpeg)) {
        Ok(mut salvaged) => {
            salvaged.picture_type = source.picture_type;
            salvaged.description = source.description.clone();
            info!(
                path = %flac_path.display(),
                picture_type = ?source.picture_type,
                bytes = salvaged.data.len(),
                width = salvaged.width,
                height = salvaged.height,
                "[FlacCoverSanitizer] Recovered damaged cover art by re-encoding it on the host"
            );
            Ok(salvaged)
        }
        Err(reencode_err) => match preserve_unrepairable_cover_sidecar(flac_path, data, source) {
            Ok(sidecar) => {
                warn!(
                    path = %flac_path.display(),
                    picture_type = ?source.picture_type,
                    bytes = data.len(),
                    sidecar = %sidecar.display(),
                    error = %reencode_err,
                    "[FlacCoverSanitizer] Cover art left the container; original bytes preserved as an external sidecar"
                );
                preserved_sidecars.push(sidecar.clone());
                Err(format!(
                    "host re-encode failed ({}); original bytes preserved in sidecar {}",
                    reencode_err,
                    sidecar.display()
                ))
            }
            Err(sidecar_err) => Err(format!(
                "host re-encode failed ({}); sidecar preservation failed ({}); cover art bytes discarded",
                reencode_err, sidecar_err
            )),
        },
    }
}

/// Write the bytes of an unrepairable cover block to a sidecar next to `flac_path`.
///
/// The name is content addressed (`<track>.unrecovered-cover.<sha256[..8]>.<ext>`) so a
/// second sanitization pass of the same block rewrites the same file with the same
/// content, and two different lost covers of the same track stay distinguishable. It
/// never collides with the Symfonium sidecars (`cover.webp`, `folder.webp`,
/// `animated.webp`, `cover.jpg`), which the sanitizer leaves untouched.
fn preserve_unrepairable_cover_sidecar(
    flac_path: &Path,
    data: &[u8],
    source: &Picture,
) -> Result<PathBuf, String> {
    let dir = flac_path
        .parent()
        .ok_or_else(|| format!("FLAC has no parent directory: {:?}", flac_path))?;
    let stem = flac_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("FLAC has no usable file name: {:?}", flac_path))?;
    if data.is_empty() {
        return Err("refusing to write an empty sidecar".to_string());
    }
    let digest = compute_bytes_sha256(data);
    let sidecar = dir.join(format!(
        "{}.{}.{}.{}",
        stem,
        UNRECOVERABLE_COVER_SIDECAR_STEM,
        &digest[..8],
        cover_sidecar_extension(source)
    ));
    std::fs::write(&sidecar, data)
        .map_err(|e| format!("Failed to write {}: {}", sidecar.display(), e))?;
    Ok(sidecar)
}

/// Extension for the preserved sidecar: the real container of the payload, falling back
/// to the declared MIME subtype when the bytes carry no recognizable magic number.
fn cover_sidecar_extension(source: &Picture) -> String {
    if source.data.starts_with(b"RIFF") && source.data.len() > 12 && &source.data[8..12] == b"WEBP"
    {
        return "webp".to_string();
    }
    if source.data.starts_with(b"\xFF\xD8\xFF") {
        return "jpg".to_string();
    }
    if source.data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return "png".to_string();
    }
    let mime = source.mime_type.to_ascii_lowercase();
    for candidate in ["jpeg", "jpg", "png", "webp", "gif", "bmp"] {
        if mime.contains(candidate) {
            return candidate.to_string();
        }
    }
    "bin".to_string()
}

/// Sanitize `flac_path` and append the outcome to `repair_history` (S163, D-03).
///
/// The audit row is written only when the pass actually changed the file or lost cover
/// art: a compliant file produces no row, so the repair history stays a record of real
/// interventions. Recording failures are logged and never propagated — the sanitization
/// itself already succeeded, and failing the download over an audit insert would be
/// worse than a missing row.
pub async fn sanitize_and_audit_flac_cover_art(
    pool: Option<&SqlitePool>,
    flac_path: &Path,
    context: &FlacCoverSanitizeContext,
) -> Result<FlacCoverSanitizeOutcome, String> {
    let bytes_before = tokio::fs::read(flac_path)
        .await
        .map_err(|e| format!("Failed to read FLAC before sanitization: {}", e))?;
    let input_file_hash = compute_bytes_sha256(&bytes_before);
    let audio_hash_before = extract_audio_content_hash_from_bytes(&bytes_before).ok();

    let sanitize_path = flac_path.to_path_buf();
    let outcome = tokio::task::spawn_blocking(move || sanitize_flac_cover_art(&sanitize_path))
        .await
        .map_err(|e| format!("Cover sanitization worker failed: {}", e))??;

    if !outcome.modified() {
        return Ok(outcome);
    }

    let bytes_after = tokio::fs::read(flac_path)
        .await
        .map_err(|e| format!("Failed to read FLAC after sanitization: {}", e))?;
    let output_file_hash = compute_bytes_sha256(&bytes_after);
    let audio_hash_after = extract_audio_content_hash_from_bytes(&bytes_after).ok();

    let mut actions = vec![format!(
        "flac_picture_sanitize: repaired={} recovered={} dropped={}",
        outcome.report.repaired_blocks,
        outcome.report.recovered_blocks,
        outcome.report.dropped_unrepairable_blocks.len()
    )];
    for sidecar in &outcome.preserved_sidecars {
        actions.push(format!("cover_sidecar_preserved: {}", sidecar.display()));
    }
    for dropped in &outcome.report.dropped_unrepairable_blocks {
        actions.push(format!("cover_art_dropped: {}", dropped));
    }

    let details_json = serde_json::json!({
        "repaired_blocks": outcome.report.repaired_blocks,
        "recovered_blocks": outcome.report.recovered_blocks,
        "dropped_unrepairable_blocks": outcome.report.dropped_unrepairable_blocks,
        "preserved_sidecars": outcome
            .preserved_sidecars
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
        "recovery_route": "ffmpeg-reencode+external-sidecar",
    })
    .to_string();

    let result = if outcome.cover_art_lost() {
        "partial_success"
    } else {
        "success"
    };
    // The guardrail verdict for this operation: sanitization is a metadata-only edit, so
    // the audio payload hash must come out identical. It is spelled out in three states
    // instead of being collapsed into one, because "the audio changed" must never hide
    // behind a label that reads like a pass.
    let baseline_validation = match (&audio_hash_before, &audio_hash_after) {
        (Some(before), Some(after)) if before == after => "valid",
        (None, None) => "audio_payload_hash_unavailable",
        _ => "audio_payload_changed",
    };
    if baseline_validation == "audio_payload_changed" {
        warn!(
            path = %flac_path.display(),
            before = ?audio_hash_before,
            after = ?audio_hash_after,
            "[FlacCoverSanitizer] Audio payload hash changed across cover sanitization"
        );
    }
    let path_str = flac_path.to_string_lossy().to_string();

    match pool {
        Some(pool) => {
            let repair_id = cover_sanitize_repair_id(context);
            if let Err(e) = crate::services::repair_history::record_applied_repair(
                pool,
                &repair_id,
                context.download_id,
                context.track_id,
                context.track_id,
                &path_str,
                &path_str,
                &input_file_hash,
                Some(&output_file_hash),
                audio_hash_before.as_deref(),
                audio_hash_after.as_deref(),
                baseline_validation,
                &actions,
                None,
                &context.provenance,
                result,
                Some(&details_json),
            )
            .await
            {
                warn!(
                    error = %e,
                    path = %flac_path.display(),
                    "[FlacCoverSanitizer] Failed to record cover sanitization in repair history"
                );
            }
        }
        None => {
            // No ledger to write to (e.g. a CLI/library caller). The loss must still be
            // visible, so it is logged at error level rather than silently dropped.
            if outcome.cover_art_lost() {
                tracing::error!(
                    path = %flac_path.display(),
                    dropped = ?outcome.report.dropped_unrepairable_blocks,
                    sidecars = ?outcome.preserved_sidecars,
                    "[FlacCoverSanitizer] Cover art dropped with no repair history ledger available"
                );
            }
        }
    }

    Ok(outcome)
}

/// Unique `repair_id` for one cover sanitization pass.
fn cover_sanitize_repair_id(context: &FlacCoverSanitizeContext) -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let subject = context
        .download_id
        .map(|id| format!("dl_{}", id))
        .or_else(|| context.track_id.map(|id| format!("track_{}", id)))
        .unwrap_or_else(|| "unlinked".to_string());
    format!("rep_cover_{}_{}", subject, millis)
}

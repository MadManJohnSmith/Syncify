//! FLAC PICTURE Block Extraction and Sidecar Zero-Byte Guard (TASK-147)
//!
//! Provides robust sidecar validation, extraction of embedded cover artwork
//! from FLAC containers, and automatic recovery of 0-byte truncated sidecars
//! (`cover.webp`, `folder.webp`, `animated.webp`, `cover.jpg`).
//!
//! Preserves the Symfonium invariant: CoverFront (0x03) = image/webp animated.

use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use metaflac::Tag;
use metaflac::block::PictureType;
use syncify_core_domain::byte_validators::WebpByteValidator;
use syncify_core_domain::cover_rules::CoverType;

/// Check if a sidecar file exists, is a regular file, and has positive byte length (> 0).
pub fn is_valid_sidecar<P: AsRef<Path>>(path: P) -> bool {
    let p = path.as_ref();
    p.is_file() && p.metadata().map(|m| m.len() > 0).unwrap_or(false)
}

/// Extracted picture information from a FLAC file.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FlacPictureInfo {
    pub mime_type: String,
    pub picture_type: PictureType,
    pub data: Vec<u8>,
    pub cover_type: CoverType,
}

/// Extract cover picture from a FLAC file.
/// Prioritizes `PictureType::CoverFront` (0x03). If not present, falls back to the first available picture.
pub fn extract_cover_picture<P: AsRef<Path>>(flac_path: P) -> Option<FlacPictureInfo> {
    let tag = Tag::read_from_path(flac_path.as_ref()).ok()?;
    let pictures: Vec<_> = tag.pictures().collect();
    if pictures.is_empty() {
        return None;
    }

    let selected = pictures
        .iter()
        .find(|p| p.picture_type == PictureType::CoverFront)
        .or_else(|| pictures.first())
        .copied()?;

    let cover_type = WebpByteValidator::detect_cover_type(&selected.data);

    Some(FlacPictureInfo {
        mime_type: selected.mime_type.clone(),
        picture_type: selected.picture_type,
        data: selected.data.clone(),
        cover_type,
    })
}

/// Ensure a sidecar file exists with valid, non-zero content.
///
/// If the destination file already exists and has size > 0, it is left untouched and returns `Ok(false)`.
/// If the destination does not exist OR is truncated to 0 bytes, it writes `content` and returns `Ok(true)`.
pub fn ensure_valid_sidecar<P: AsRef<Path>>(dest_path: P, content: &[u8]) -> std::io::Result<bool> {
    let p = dest_path.as_ref();
    if is_valid_sidecar(p) {
        return Ok(false);
    }

    if let Some(parent) = p.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    std::fs::write(p, content)?;
    Ok(true)
}

/// Regenerate missing or 0-byte truncated sidecars in `target_dir` (and album root if multi-disc)
/// using the embedded `PICTURE` block of the specified FLAC file.
///
/// If the FLAC contains WebP cover art, ensures:
/// - `cover.webp`
/// - `folder.webp`
/// - `animated.webp`
///
/// If the FLAC contains JPEG cover art, ensures:
/// - `cover.jpg`
///
/// Returns the list of paths that were rewritten/regenerated.
pub fn ensure_flac_sidecars_intact<P: AsRef<Path>, Q: AsRef<Path>>(
    flac_path: P,
    target_dir: Q,
) -> Result<Vec<PathBuf>, String> {
    let flac_path = flac_path.as_ref();
    let target_dir = target_dir.as_ref();

    if !flac_path.exists() {
        return Err(format!("FLAC file not found: {:?}", flac_path));
    }

    let pic = match extract_cover_picture(flac_path) {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };

    if pic.data.is_empty() {
        return Ok(Vec::new());
    }

    let is_webp = pic.cover_type.is_webp() || pic.mime_type.to_lowercase().contains("webp");
    let is_jpeg = pic.cover_type == CoverType::StaticJpeg
        || pic.mime_type.to_lowercase().contains("jpeg")
        || pic.mime_type.to_lowercase().contains("jpg");

    let mut required_sidecars: Vec<&str> = if is_webp {
        vec!["cover.webp", "folder.webp", "animated.webp"]
    } else if is_jpeg {
        vec!["cover.jpg"]
    } else {
        vec!["cover.png"]
    };

    // If cover.animated.webp exists with 0 bytes, also include it for repair
    if is_webp && target_dir.join("cover.animated.webp").exists() && !is_valid_sidecar(target_dir.join("cover.animated.webp")) {
        required_sidecars.push("cover.animated.webp");
    }

    let mut regenerated = Vec::new();

    // 1. Target directory
    for name in &required_sidecars {
        let dest = target_dir.join(name);
        if !is_valid_sidecar(&dest) {
            match ensure_valid_sidecar(&dest, &pic.data) {
                Ok(true) => {
                    debug!(path = %dest.display(), "[FlacPicture] Regenerated sidecar from FLAC PICTURE block");
                    regenerated.push(dest);
                }
                Ok(false) => {}
                Err(e) => warn!(error = %e, path = %dest.display(), "[FlacPicture] Failed to write sidecar"),
            }
        }
    }

    // 2. Multi-disc parent directory propagation (Disc 1, Disc 2, CD 1, etc.)
    if let Some(parent) = target_dir.parent() {
        let dir_name = target_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if dir_name.starts_with("Disc") || dir_name.starts_with("CD") {
            for name in &required_sidecars {
                let dest = parent.join(name);
                if !is_valid_sidecar(&dest) {
                    match ensure_valid_sidecar(&dest, &pic.data) {
                        Ok(true) => {
                            debug!(path = %dest.display(), "[FlacPicture] Regenerated root sidecar for multi-disc album");
                            regenerated.push(dest);
                        }
                        Ok(false) => {}
                        Err(e) => warn!(error = %e, path = %dest.display(), "[FlacPicture] Failed to write root sidecar"),
                    }
                }
            }
        }
    }

    Ok(regenerated)
}

/// Recursively scans an album directory for 0-byte truncated sidecars (`cover.webp`, `folder.webp`,
/// `animated.webp`, `cover.animated.webp`, `cover.jpg`) and repairs them using the embedded
/// `PICTURE` block from the first FLAC track found.
///
/// Returns the list of repaired sidecar paths.
#[allow(dead_code)]
pub fn scan_and_repair_album_sidecars<P: AsRef<Path>>(album_dir: P) -> Result<Vec<PathBuf>, String> {
    let album_dir = album_dir.as_ref();
    if !album_dir.exists() || !album_dir.is_dir() {
        return Err(format!("Directory does not exist: {:?}", album_dir));
    }

    let mut zero_byte_targets: Vec<PathBuf> = Vec::new();
    let mut candidate_flac: Option<PathBuf> = None;

    let mut stack = vec![album_dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = match std::fs::read_dir(&current) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let file_ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

                if file_ext == "flac" && candidate_flac.is_none() {
                    candidate_flac = Some(path.clone());
                }

                if matches!(file_name, "cover.webp" | "folder.webp" | "animated.webp" | "cover.animated.webp" | "cover.jpg") {
                    let is_zero = path.metadata().map(|m| m.len() == 0).unwrap_or(false);
                    if is_zero {
                        zero_byte_targets.push(path);
                    }
                }
            }
        }
    }

    if zero_byte_targets.is_empty() {
        return Ok(Vec::new());
    }

    let flac_path = match candidate_flac {
        Some(f) => f,
        None => return Err(format!("Found {} 0-byte sidecars in {:?}, but no FLAC files available for extraction", zero_byte_targets.len(), album_dir)),
    };

    let pic = match extract_cover_picture(&flac_path) {
        Some(p) => p,
        None => return Err(format!("No PICTURE block found in {:?}", flac_path)),
    };

    let mut repaired = Vec::new();
    for target in zero_byte_targets {
        // Rewrite truncated file with extracted picture data
        if std::fs::write(&target, &pic.data).is_ok() {
            info!(path = %target.display(), "[FlacPicture] Repaired 0-byte sidecar from FLAC PICTURE block");
            repaired.push(target);
        }
    }

    Ok(repaired)
}

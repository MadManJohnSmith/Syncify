//! Repair Integrity Guardrail Service (S159)
//!
//! Provides baseline snapshotting, pre-flight revalidation, audio content hash extraction,
//! and complete audit reporting for repair operations.

use std::path::Path;
use sha2::{Digest, Sha256};
use syncify_core_domain::repair::{RepairFileBaseline, RepairValidationStatus};

/// Compute SHA-256 hash of a file on disk asynchronously.
pub async fn compute_file_sha256(path: &Path) -> Result<String, String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| format!("Failed to read file for hashing {:?}: {}", path, e))?;
    Ok(compute_bytes_sha256(&bytes))
}

/// Compute SHA-256 hash of an in-memory byte slice.
pub fn compute_bytes_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Extract and hash only the raw audio payload/frames of an audio file,
/// allowing invariant verification before and after metadata VorbisComment / Picture tagging.
pub fn extract_audio_content_hash_from_bytes(bytes: &[u8]) -> Result<String, String> {
    if bytes.len() < 4 {
        return Err("File too short for audio payload extraction".to_string());
    }

    // 1. FLAC Container
    if bytes.starts_with(b"fLaC") {
        let mut offset = 4usize;
        let mut stream_md5: Option<[u8; 16]> = None;

        while offset + 4 <= bytes.len() {
            let byte0 = bytes[offset];
            let is_last = (byte0 & 0x80) != 0;
            let block_type = byte0 & 0x7F;
            let length = ((bytes[offset + 1] as usize) << 16)
                | ((bytes[offset + 2] as usize) << 8)
                | (bytes[offset + 3] as usize);

            let data_start = offset + 4;
            let data_end = data_start + length;

            if data_end > bytes.len() {
                // Malformed block boundary fallback
                break;
            }

            // STREAMINFO block
            if block_type == 0 && length >= 34 {
                let mut md5_bytes = [0u8; 16];
                md5_bytes.copy_from_slice(&bytes[data_start + 18..data_start + 34]);
                stream_md5 = Some(md5_bytes);
            }

            offset = data_end;
            if is_last {
                break;
            }
        }

        let audio_frames = if offset <= bytes.len() {
            &bytes[offset..]
        } else {
            &bytes[4..]
        };

        let frames_hash = compute_bytes_sha256(audio_frames);

        if let Some(md5) = stream_md5 {
            if !md5.iter().all(|&b| b == 0) {
                let md5_hex = md5.iter().map(|b| format!("{:02x}", b)).collect::<String>();
                return Ok(format!("flac_md5:{}_frames:{}", md5_hex, frames_hash));
            }
        }

        return Ok(format!("flac_frames:{}", frames_hash));
    }

    // 2. MP4 / M4A Container (search for 'mdat' box)
    if bytes.len() >= 8 && (&bytes[4..8] == b"ftyp" || &bytes[0..4] == b"ftyp") {
        let mut offset = 0usize;
        while offset + 8 <= bytes.len() {
            let box_len = ((bytes[offset] as usize) << 24)
                | ((bytes[offset + 1] as usize) << 16)
                | ((bytes[offset + 2] as usize) << 8)
                | (bytes[offset + 3] as usize);

            let fourcc = &bytes[offset + 4..offset + 8];

            let end = if box_len == 0 {
                bytes.len()
            } else if box_len == 1 && offset + 16 <= bytes.len() {
                // 64-bit extended length box
                let mut len64 = 0usize;
                for i in 8..16 {
                    len64 = (len64 << 8) | (bytes[offset + i] as usize);
                }
                (offset + len64).min(bytes.len())
            } else {
                (offset + box_len).min(bytes.len())
            };

            if fourcc == b"mdat" {
                let mdat_data = &bytes[offset + 8..end];
                return Ok(format!("mp4_mdat:{}", compute_bytes_sha256(mdat_data)));
            }

            if box_len == 0 || end <= offset {
                break;
            }
            offset = end;
        }
    }

    // 3. Fallback generic audio payload hash
    Ok(format!("generic_payload:{}", compute_bytes_sha256(bytes)))
}

/// Compute the audio payload content hash of a file on disk.
pub async fn compute_file_audio_content_hash(path: &Path) -> Result<String, String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| format!("Failed to read audio file {:?}: {}", path, e))?;
    extract_audio_content_hash_from_bytes(&bytes)
}

/// Snapshot baseline calculation for an audio file and optional sidecar LRC during dry-run.
pub async fn compute_repair_baseline(
    audio_path: &Path,
    lrc_path: Option<&Path>,
) -> Result<RepairFileBaseline, String> {
    if !audio_path.exists() {
        return Err(format!("Audio file not found: {:?}", audio_path));
    }

    let meta = tokio::fs::metadata(audio_path)
        .await
        .map_err(|e| format!("Failed to read metadata for {:?}: {}", audio_path, e))?;

    let bytes = tokio::fs::read(audio_path)
        .await
        .map_err(|e| format!("Failed to read audio bytes for {:?}: {}", audio_path, e))?;

    let input_sha256 = compute_bytes_sha256(&bytes);
    let input_size = meta.len();
    let input_modified_at = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let audio_content_hash = extract_audio_content_hash_from_bytes(&bytes).ok();

    // Check LRC sidecar if provided or if existing alongside audio
    let effective_lrc = lrc_path
        .map(|p| p.to_path_buf())
        .or_else(|| {
            let candidate = audio_path.with_extension("lrc");
            if candidate.exists() {
                Some(candidate)
            } else {
                None
            }
        });

    let (lrc_path_str, lrc_sha256, lrc_size, lrc_modified_at) = match effective_lrc {
        Some(ref lp) if lp.exists() => {
            let lrc_meta = tokio::fs::metadata(lp).await.ok();
            let lrc_bytes = tokio::fs::read(lp).await.ok();
            let l_sha = lrc_bytes.map(|b| compute_bytes_sha256(&b));
            let l_size = lrc_meta.as_ref().map(|m| m.len());
            let l_mod = lrc_meta
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64);
            (Some(lp.to_string_lossy().to_string()), l_sha, l_size, l_mod)
        }
        _ => (None, None, None, None),
    };

    Ok(RepairFileBaseline {
        file_path: audio_path.to_string_lossy().to_string(),
        input_sha256,
        input_size,
        input_modified_at,
        audio_content_hash,
        lrc_path: lrc_path_str,
        lrc_sha256,
        lrc_size,
        lrc_modified_at,
    })
}

/// Pre-flight revalidation of the current file state against the baseline.
/// Returns `RepairValidationStatus::Valid` if bit-for-bit unchanged,
/// or `RepairValidationStatus::RepairInputChanged` / `FileNotFound` if altered.
pub async fn validate_repair_baseline(
    baseline: &RepairFileBaseline,
    current_audio_path: &Path,
    current_lrc_path: Option<&Path>,
) -> RepairValidationStatus {
    if !current_audio_path.exists() {
        return RepairValidationStatus::FileNotFound {
            path: current_audio_path.to_string_lossy().to_string(),
        };
    }

    let meta = match tokio::fs::metadata(current_audio_path).await {
        Ok(m) => m,
        Err(e) => {
            return RepairValidationStatus::RepairInputChanged {
                reason: format!("Could not read audio file metadata: {}", e),
            };
        }
    };

    // Check size
    if meta.len() != baseline.input_size {
        return RepairValidationStatus::RepairInputChanged {
            reason: format!(
                "File size changed: baseline {} bytes vs current {} bytes",
                baseline.input_size,
                meta.len()
            ),
        };
    }

    // Check full file SHA-256
    let current_bytes = match tokio::fs::read(current_audio_path).await {
        Ok(b) => b,
        Err(e) => {
            return RepairValidationStatus::RepairInputChanged {
                reason: format!("Could not read audio file bytes: {}", e),
            };
        }
    };

    let current_sha256 = compute_bytes_sha256(&current_bytes);
    if current_sha256 != baseline.input_sha256 {
        return RepairValidationStatus::RepairInputChanged {
            reason: format!(
                "File SHA-256 mismatch: baseline {} vs current {}",
                baseline.input_sha256, current_sha256
            ),
        };
    }

    // Check audio content hash if present in baseline
    if let Some(ref base_audio_hash) = baseline.audio_content_hash {
        let current_audio_hash = extract_audio_content_hash_from_bytes(&current_bytes).unwrap_or_default();
        if &current_audio_hash != base_audio_hash {
            return RepairValidationStatus::RepairInputChanged {
                reason: format!(
                    "Audio content payload changed: baseline {} vs current {}",
                    base_audio_hash, current_audio_hash
                ),
            };
        }
    }

    // Check sidecar LRC if baseline recorded one
    if let Some(ref base_lrc_sha) = baseline.lrc_sha256 {
        let effective_lrc = current_lrc_path
            .map(|p| p.to_path_buf())
            .or_else(|| baseline.lrc_path.as_ref().map(std::path::PathBuf::from))
            .or_else(|| {
                let cand = current_audio_path.with_extension("lrc");
                if cand.exists() {
                    Some(cand)
                } else {
                    None
                }
            });

        let lrc_file = match effective_lrc {
            Some(lp) => lp,
            None => {
                return RepairValidationStatus::RepairInputChanged {
                    reason: "Sidecar LRC was recorded in baseline but is now missing".to_string(),
                };
            }
        };

        if !lrc_file.exists() {
            return RepairValidationStatus::RepairInputChanged {
                reason: format!("Sidecar LRC file was removed: {:?}", lrc_file),
            };
        }

        let lrc_bytes = match tokio::fs::read(&lrc_file).await {
            Ok(b) => b,
            Err(e) => {
                return RepairValidationStatus::RepairInputChanged {
                    reason: format!("Could not read sidecar LRC {:?}: {}", lrc_file, e),
                };
            }
        };

        let current_lrc_sha = compute_bytes_sha256(&lrc_bytes);
        if &current_lrc_sha != base_lrc_sha {
            return RepairValidationStatus::RepairInputChanged {
                reason: format!(
                    "Sidecar LRC SHA-256 changed: baseline {} vs current {}",
                    base_lrc_sha, current_lrc_sha
                ),
            };
        }
    }

    RepairValidationStatus::Valid
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_test_flac(path: &Path, audio_payload: &[u8]) {
        let mut flac_bytes = Vec::new();
        flac_bytes.extend_from_slice(b"fLaC");
        flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // Last block, STREAMINFO, 34 bytes
        let mut streaminfo = [0u8; 34];
        streaminfo[0..2].copy_from_slice(&4608u16.to_be_bytes());
        streaminfo[2..4].copy_from_slice(&4608u16.to_be_bytes());
        streaminfo[10] = 0x0A;
        streaminfo[11] = 0xC4;
        streaminfo[12] = 0x42;
        streaminfo[13] = 0xF0;
        flac_bytes.extend_from_slice(&streaminfo);
        flac_bytes.extend_from_slice(audio_payload);
        std::fs::write(path, &flac_bytes).expect("Failed to write test flac");
    }

    #[tokio::test]
    async fn test_baseline_computation_and_validation() {
        let temp = TempDir::new().unwrap();
        let flac_path = temp.path().join("test.flac");
        let lrc_path = temp.path().join("test.lrc");

        write_test_flac(&flac_path, b"ORIGINAL_AUDIO_PAYLOAD_123");
        tokio::fs::write(&lrc_path, b"[00:01.00] Test Lyrics").await.unwrap();

        let baseline = compute_repair_baseline(&flac_path, Some(&lrc_path)).await.unwrap();
        assert_eq!(baseline.file_path, flac_path.to_string_lossy().to_string());
        assert!(baseline.audio_content_hash.is_some());
        assert!(baseline.lrc_sha256.is_some());

        // 1. Unchanged file is Valid
        let val_ok = validate_repair_baseline(&baseline, &flac_path, Some(&lrc_path)).await;
        assert!(val_ok.is_valid());

        // 2. Modified audio payload fails with RepairInputChanged
        write_test_flac(&flac_path, b"MODIFIED_AUDIO_PAYLOAD_456");
        let val_audio_changed = validate_repair_baseline(&baseline, &flac_path, Some(&lrc_path)).await;
        assert!(!val_audio_changed.is_valid());
        match val_audio_changed {
            RepairValidationStatus::RepairInputChanged { reason } => {
                assert!(reason.contains("File SHA-256 mismatch") || reason.contains("size changed"));
            }
            other => panic!("Expected RepairInputChanged, got {:?}", other),
        }
    }
}

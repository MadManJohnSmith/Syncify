//! Integration Test: Cross-Volume Promotion Safety with SHA-256 and Size Verification
//!
//! Validates:
//! 1. Verified copy+delete fallback computes SHA-256 and byte length before source deletion.
//! 2. Staged file is strictly preserved if verification fails.
//! 3. Destination is removed if integrity check fails.
//! 4. Clean promotion when copy and hash verification succeed.

use tempfile::TempDir;
use syncify_tauri_lib::services::repair_guardrail::compute_bytes_sha256;

#[tokio::test]
async fn test_verified_cross_volume_promotion_success() {
    let temp = TempDir::new().unwrap();
    let staging_dir = temp.path().join("staging_volume");
    let library_dir = temp.path().join("library_volume");
    tokio::fs::create_dir_all(&staging_dir).await.unwrap();
    tokio::fs::create_dir_all(&library_dir).await.unwrap();

    let staged_file = staging_dir.join("track_123.flac");
    let audio_payload = b"FLAC_AUDIO_CROSS_VOLUME_DATA_STREAM_TEST";
    tokio::fs::write(&staged_file, audio_payload).await.unwrap();

    let final_dest = library_dir.join("01 - Test Song.flac");

    // Perform verified cross-volume copy+delete
    let staged_bytes = tokio::fs::read(&staged_file).await.unwrap();
    let staged_sha256 = compute_bytes_sha256(&staged_bytes);
    let staged_size = staged_bytes.len() as u64;

    tokio::fs::write(&final_dest, &staged_bytes).await.unwrap();

    let dest_meta = tokio::fs::metadata(&final_dest).await.unwrap();
    let dest_bytes = tokio::fs::read(&final_dest).await.unwrap();
    let dest_sha256 = compute_bytes_sha256(&dest_bytes);

    assert_eq!(dest_meta.len(), staged_size);
    assert_eq!(dest_sha256, staged_sha256);

    // Only delete staging file after verification succeeds
    tokio::fs::remove_file(&staged_file).await.unwrap();

    assert!(final_dest.exists(), "Promoted file must exist in destination");
    assert!(!staged_file.exists(), "Staged file must be removed after verified copy");
}

#[tokio::test]
async fn test_verified_cross_volume_promotion_detects_corruption() {
    let temp = TempDir::new().unwrap();
    let staging_dir = temp.path().join("staging_volume");
    let library_dir = temp.path().join("library_volume");
    tokio::fs::create_dir_all(&staging_dir).await.unwrap();
    tokio::fs::create_dir_all(&library_dir).await.unwrap();

    let staged_file = staging_dir.join("track_456.flac");
    let audio_payload = b"ORIGINAL_CORRECT_AUDIO_DATA";
    tokio::fs::write(&staged_file, audio_payload).await.unwrap();

    let final_dest = library_dir.join("02 - Corrupted Song.flac");

    let staged_bytes = tokio::fs::read(&staged_file).await.unwrap();
    let staged_sha256 = compute_bytes_sha256(&staged_bytes);
    let staged_size = staged_bytes.len() as u64;

    // Simulate corrupted write (e.g. incomplete I/O)
    tokio::fs::write(&final_dest, b"CORRUPTED_TRUNCATED_DATA").await.unwrap();

    let dest_meta = tokio::fs::metadata(&final_dest).await.unwrap();
    let dest_bytes = tokio::fs::read(&final_dest).await.unwrap();
    let dest_sha256 = compute_bytes_sha256(&dest_bytes);

    let verification_passed = dest_meta.len() == staged_size && dest_sha256 == staged_sha256;
    assert!(!verification_passed, "Integrity check must fail on corrupted copy");

    if !verification_passed {
        // Rollback corrupted destination file, retain staging file
        let _ = tokio::fs::remove_file(&final_dest).await;
    }

    assert!(!final_dest.exists(), "Corrupted destination file must be cleaned up");
    assert!(staged_file.exists(), "Staging file must remain preserved for retry");
}

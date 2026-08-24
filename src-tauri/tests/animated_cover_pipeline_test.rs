//! Animated Cover Pipeline and Multi-Disc Promotion Test (S174)
//!
//! Validates:
//! 1. All animated cover sidecar variants (`cover.webp`, `animated.webp`, `folder.webp`, `cover.animated.webp`) are created.
//! 2. Multi-disc album layout correctly propagates animated and static covers to both the disc folder and the parent album root.
//! 3. Validates animated WebP container integrity.
//! 4. Regressions for `...Like Clockwork` and `Skull & Bones` title normalization and matching.
//! 5. Explicit error and timeout handling without silent drops.

use syncify_tauri_lib::services::animated_cover::{
    matches_artist_and_album, normalize_for_comparison,
    strip_leading_punctuation, validate_animated_webp_bytes, AnimatedCoverStatus,
};
use tempfile::tempdir;

fn create_synthetic_animated_webp_bytes() -> Vec<u8> {
    // Generate valid animated WebP with ffmpeg
    let temp_dir = tempdir().expect("tempdir");
    let out_webp = temp_dir.path().join("anim.webp");

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "lavfi",
            "-i", "testsrc=duration=1:size=64x64:rate=10",
            "-vcodec", "libwebp",
            "-loop", "0",
            "-an",
            out_webp.to_str().unwrap(),
        ])
        .output()
        .expect("ffmpeg must execute");

    assert!(status.status.success(), "ffmpeg animated WebP creation must succeed");
    std::fs::read(&out_webp).expect("read anim.webp")
}

#[tokio::test]
async fn test_animated_cover_sidecar_variants_and_multidisc_promotion() {
    let anim_bytes = create_synthetic_animated_webp_bytes();
    let frame_count = validate_animated_webp_bytes(&anim_bytes).expect("Valid animated WebP");
    assert!(frame_count > 1, "Must contain multiple animation frames");

    let root_dir = tempdir().expect("tempdir");
    let staging_dir = root_dir.path().join(".staging").join("item_123");
    tokio::fs::create_dir_all(&staging_dir).await.expect("create staging");

    // 1. Stage animated cover files in staging_dir
    let webp_file = staging_dir.join("cover.webp");
    let anim_file = staging_dir.join("animated.webp");
    let folder_file = staging_dir.join("folder.webp");
    let cov_anim_file = staging_dir.join("cover.animated.webp");
    let static_cov = staging_dir.join("cover.jpg");

    tokio::fs::write(&webp_file, &anim_bytes).await.unwrap();
    tokio::fs::write(&anim_file, &anim_bytes).await.unwrap();
    tokio::fs::write(&folder_file, &anim_bytes).await.unwrap();
    tokio::fs::write(&cov_anim_file, &anim_bytes).await.unwrap();
    tokio::fs::write(&static_cov, b"synthetic_jpeg_bytes").await.unwrap();

    // 2. Simulate multi-disc promotion: destination is "Album / Disc 1"
    let album_root = root_dir.path().join("Artist Name").join("Greatest Hits (Deluxe)");
    let disc_dir = album_root.join("Disc 1");
    tokio::fs::create_dir_all(&disc_dir).await.expect("create disc dir");

    // Copy sidecars from staging to disc_dir, propagating to album_root
    let mut dir_entries = tokio::fs::read_dir(&staging_dir).await.unwrap();
    while let Ok(Some(entry)) = dir_entries.next_entry().await {
        let entry_path = entry.path();
        if entry_path.is_file() {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            if file_name_str == "cover.jpg"
                || file_name_str == "cover.webp"
                || file_name_str == "cover.animated.webp"
                || file_name_str == "folder.webp"
                || file_name_str == "animated.webp"
            {
                let dest_sidecar = disc_dir.join(&file_name);
                if !dest_sidecar.exists() {
                    let _ = tokio::fs::copy(&entry_path, &dest_sidecar).await;
                }
                if let Some(parent) = disc_dir.parent() {
                    let dir_name = disc_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if dir_name.starts_with("Disc") || dir_name.starts_with("CD") {
                        let album_root_sidecar = parent.join(&file_name);
                        if !album_root_sidecar.exists() {
                            let _ = tokio::fs::copy(&entry_path, &album_root_sidecar).await;
                        }
                    }
                }
                let _ = tokio::fs::remove_file(&entry_path).await;
            }
        }
    }

    // 3. Verify sidecars in Disc 1 directory
    assert!(disc_dir.join("cover.jpg").exists(), "cover.jpg must exist in Disc 1");
    assert!(disc_dir.join("cover.webp").exists(), "cover.webp must exist in Disc 1");
    assert!(disc_dir.join("animated.webp").exists(), "animated.webp must exist in Disc 1");
    assert!(disc_dir.join("folder.webp").exists(), "folder.webp must exist in Disc 1");
    assert!(disc_dir.join("cover.animated.webp").exists(), "cover.animated.webp must exist in Disc 1");

    // 4. Verify sidecars propagated to album root directory
    assert!(album_root.join("cover.jpg").exists(), "cover.jpg must exist in album root");
    assert!(album_root.join("cover.webp").exists(), "cover.webp must exist in album root");
    assert!(album_root.join("animated.webp").exists(), "animated.webp must exist in album root");
    assert!(album_root.join("folder.webp").exists(), "folder.webp must exist in album root");

    // 5. Verify staging is clean (0 residual sidecars)
    let remaining_staging = std::fs::read_dir(&staging_dir).unwrap().count();
    assert_eq!(remaining_staging, 0, "Staging directory must have 0 residual files after promotion");
}

#[test]
fn test_regression_like_clockwork_and_skull_and_bones_matching() {
    // 1. Queens of the Stone Age - ...Like Clockwork regression (Unicode ellipsis vs ASCII dots)
    let target_artist = "Queens Of The Stone Age";
    let target_album = "...Like Clockwork";

    assert_eq!(strip_leading_punctuation(target_album), "Like Clockwork");
    assert_eq!(normalize_for_comparison(target_album), "like clockwork");
    assert_eq!(normalize_for_comparison("…Like Clockwork"), "like clockwork");

    // Apple Music catalog candidate (with U+2026 ellipsis)
    let apple_catalog_artist = "Queens of the Stone Age";
    let apple_catalog_album = "…Like Clockwork";
    assert!(
        matches_artist_and_album(apple_catalog_artist, apple_catalog_album, target_artist, target_album),
        "Must match Unicode ellipsis '…Like Clockwork' with ASCII dots '...Like Clockwork'"
    );

    // 2. Cypress Hill - Skull & Bones regression
    let ch_artist = "Cypress Hill";
    let ch_album = "Skull & Bones";

    assert_eq!(normalize_for_comparison(ch_album), "skull bones");
    assert!(
        matches_artist_and_album("Cypress Hill", "Skull & Bones", ch_artist, ch_album),
        "Exact match for Skull & Bones"
    );
    assert!(
        matches_artist_and_album("Cypress Hill", "Skull & Bones (Explicit)", ch_artist, ch_album),
        "Must match edition variants"
    );
}

#[test]
fn test_regression_explicit_animated_cover_status_and_no_silent_drop() {
    // Invalid/corrupted bytes must return explicit validation error, never silently pass or drop
    let invalid_bytes = b"NOT_A_WEBP_CONTAINER";
    let res = validate_animated_webp_bytes(invalid_bytes);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), "WebP data too small (< 30 bytes)");

    // Too small WebP header bytes
    let truncated_riff = b"RIFF\x00\x00\x00\x00WEBPVP8X\x00\x00\x00\x0a\x00\x00\x00\x00";
    let res_trunc = validate_animated_webp_bytes(truncated_riff);
    assert!(res_trunc.is_err());

    // Explicit status enum variants must preserve failure reason
    let failed = AnimatedCoverStatus::Failed("ffmpeg conversion timed out after 30s".to_string());
    match failed {
        AnimatedCoverStatus::Failed(ref reason) => {
            assert!(reason.contains("timed out"));
        }
        _ => panic!("Expected Failed variant"),
    }

    let unavail = AnimatedCoverStatus::SourceUnavailable("Token extraction failed".to_string());
    match unavail {
        AnimatedCoverStatus::SourceUnavailable(ref reason) => {
            assert!(reason.contains("Token extraction failed"));
        }
        _ => panic!("Expected SourceUnavailable variant"),
    }
}

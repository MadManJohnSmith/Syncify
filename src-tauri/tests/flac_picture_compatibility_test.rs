//! FLAC PICTURE Compatibility and Embedded Art Size Limit Test (TASK-71)
//!
//! Validates:
//! 1. Rejection / conversion of images with 0x0 dimensions.
//! 2. Rejection of corrupted/invalid WebP embedded without valid dimensions.
//! 3. Picture block size limit strictly bounded to <= 800 KB (MAX_EMBEDDED_PICTURE_BYTES).
//! 4. Conversion of incoming animated WebP to static JPEG for FLAC embedded PICTURE block
//!    while preserving external animated WebP sidecars (`cover.webp`, `animated.webp`) for Symfonium.
//! 5. Sanitization of existing/legacy FLAC files containing 0x0 or oversized picture blocks.
//!
//! The animated WebP payload is the tracked fixture `fixtures/animated-cover.webp`
//! instead of a payload encoded at test time by the runner's FFmpeg: encoding it
//! inline tied this suite to an FFmpeg build whose output the validator (or the
//! WebP -> JPEG conversion) may not accept, which is exactly the capability the CI
//! preflight reports as unverified.

use metaflac::block::PictureType;
use metaflac::Tag;
use std::path::Path;
use syncify_tauri_lib::services::animated_cover::validate_animated_webp_bytes;
use syncify_tauri_lib::services::tag_writer::{
    apply_flac_tags, prepare_flac_picture, sanitize_flac_pictures, FlacMetadata,
    MAX_EMBEDDED_PICTURE_BYTES,
};
use tempfile::tempdir;

/// Versioned animated WebP (3 ANMF frames), the same fixture the CI preflight decodes.
const ANIMATED_WEBP_FIXTURE: &[u8] = include_bytes!("fixtures/animated-cover.webp");

/// The CI preflight sets `SYNCIFY_ANIMATED_WEBP_E2E=0` when the runner's FFmpeg cannot
/// decode animated WebP. Container validation is pure and hermetic; only the
/// WebP -> JPEG conversion needs that decoder, so it is skipped explicitly instead of
/// failing. Same gate as `animated_cover_mp4_sidecar_test.rs`.
fn animated_webp_decode_available() -> bool {
    std::env::var_os("SYNCIFY_ANIMATED_WEBP_E2E").as_deref() != Some(std::ffi::OsStr::new("0"))
}

fn create_synthetic_flac(path: &Path) {
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=0.2",
            "-c:a",
            "flac",
            path.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("ffmpeg must execute to create synthetic FLAC");
    assert!(status.success(), "ffmpeg synthetic FLAC creation failed");
}

fn create_animated_webp_fixture() -> Vec<u8> {
    ANIMATED_WEBP_FIXTURE.to_vec()
}

fn create_large_jpeg() -> Vec<u8> {
    let output = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "mandelbrot=size=2500x2500",
            "-vframes",
            "1",
            "-q:v",
            "1",
            "-f",
            "image2",
            "-c:v",
            "mjpeg",
            "pipe:1",
        ])
        .output()
        .expect("ffmpeg must generate large jpeg");
    assert!(
        output.status.success(),
        "ffmpeg large jpeg generation must succeed"
    );
    assert!(
        output.stdout.len() > MAX_EMBEDDED_PICTURE_BYTES,
        "Generated JPEG must exceed 800 KB to test size boundary (was {} bytes)",
        output.stdout.len()
    );
    output.stdout
}

#[test]
fn test_reject_zero_dimensions_picture() {
    // Construct JPEG with SOF0 stating 0x0 dimensions
    let mut jpeg_zero = Vec::new();
    jpeg_zero.extend_from_slice(&[
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x08, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
    ]);
    jpeg_zero.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]); // SOF0
    jpeg_zero.extend_from_slice(&0u16.to_be_bytes()); // height 0
    jpeg_zero.extend_from_slice(&0u16.to_be_bytes()); // width 0
    jpeg_zero.extend_from_slice(&[0x03, 0xFF, 0xD9]);

    let res = prepare_flac_picture(&jpeg_zero);
    assert!(res.is_err(), "0x0 dimension JPEG must be rejected");

    let dir = tempdir().expect("tempdir");
    let flac_path = dir.path().join("zero_dim_test.flac");
    create_synthetic_flac(&flac_path);

    let meta = FlacMetadata {
        title: "Zero Dim Track".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        cover_data: Some(jpeg_zero),
        ..Default::default()
    };

    let apply_res = apply_flac_tags(&flac_path, &meta);
    assert!(
        apply_res.is_err(),
        "apply_flac_tags must reject 0x0 dimension cover art"
    );

    let tag = Tag::read_from_path(&flac_path).expect("read FLAC");
    assert_eq!(
        tag.pictures().count(),
        0,
        "No 0x0 PICTURE block must be embedded"
    );
}

#[test]
fn test_reject_corrupted_webp_without_valid_dimensions() {
    // Corrupt WebP payload with invalid chunk structure / dimensions
    let corrupt_webp =
        b"RIFF\x14\x00\x00\x00WEBPVP8X\x0a\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"
            .to_vec();

    let res = prepare_flac_picture(&corrupt_webp);
    assert!(
        res.is_err(),
        "Corrupted WebP with invalid headers must be rejected"
    );

    let dir = tempdir().expect("tempdir");
    let flac_path = dir.path().join("corrupt_webp_test.flac");
    create_synthetic_flac(&flac_path);

    let meta = FlacMetadata {
        title: "Corrupt WebP Track".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        cover_data: Some(corrupt_webp),
        ..Default::default()
    };

    let apply_res = apply_flac_tags(&flac_path, &meta);
    assert!(
        apply_res.is_err(),
        "apply_flac_tags must reject corrupted WebP"
    );

    let tag = Tag::read_from_path(&flac_path).expect("read FLAC");
    assert_eq!(
        tag.pictures().count(),
        0,
        "No picture block should be written for corrupted WebP"
    );
}

#[test]
fn test_picture_block_size_limit_bounded_to_800kb() {
    let large_jpeg = create_large_jpeg();
    let initial_size = large_jpeg.len();
    assert!(initial_size > MAX_EMBEDDED_PICTURE_BYTES);

    let pic_block = prepare_flac_picture(&large_jpeg).expect("Large JPEG must be recompressed");
    assert!(
        pic_block.data.len() <= MAX_EMBEDDED_PICTURE_BYTES,
        "Embedded picture block size must be <= 800 KB (actual: {} bytes)",
        pic_block.data.len()
    );
    assert!(
        pic_block.width > 0 && pic_block.height > 0,
        "Width and height must be > 0"
    );

    let dir = tempdir().expect("tempdir");
    let flac_path = dir.path().join("large_cover_test.flac");
    create_synthetic_flac(&flac_path);

    let meta = FlacMetadata {
        title: "Large Cover Track".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        cover_data: Some(large_jpeg),
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &meta)
        .expect("Tagging with oversized cover must succeed via recompression");

    let tag = Tag::read_from_path(&flac_path).expect("read FLAC");
    let pics: Vec<_> = tag.pictures().collect();
    assert_eq!(pics.len(), 1);
    let embedded = pics[0];
    assert!(embedded.data.len() <= MAX_EMBEDDED_PICTURE_BYTES);
    assert!(embedded.width > 0 && embedded.height > 0);
    assert_eq!(embedded.picture_type, PictureType::CoverFront);
}

#[test]
fn test_convert_animated_webp_to_static_jpeg_while_preserving_external_sidecar() {
    let anim_webp = create_animated_webp_fixture();
    let frame_count =
        validate_animated_webp_bytes(&anim_webp).expect("Must be valid animated WebP");
    assert!(frame_count > 1, "Must have > 1 animation frames");

    if !animated_webp_decode_available() {
        eprintln!(
            "skipping animated WebP -> JPEG conversion: CI preflight reported no decoder support"
        );
        return;
    }

    let dir = tempdir().expect("tempdir");
    let album_dir = dir.path();
    let flac_path = album_dir.join("01 - Test Track.flac");
    create_synthetic_flac(&flac_path);

    // Save external animated sidecar (as downloaded by pipeline)
    let sidecar_cover_webp = album_dir.join("cover.webp");
    let sidecar_animated_webp = album_dir.join("animated.webp");
    std::fs::write(&sidecar_cover_webp, &anim_webp).expect("Write cover.webp sidecar");
    std::fs::write(&sidecar_animated_webp, &anim_webp).expect("Write animated.webp sidecar");

    // Tag FLAC track with the animated webp cover payload
    let meta = FlacMetadata {
        title: "Animated Track".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        cover_data: Some(anim_webp.clone()),
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &meta).expect("FLAC tagging with animated WebP payload");

    // 1. Check embedded FLAC picture: must be converted to static JPEG with real dimensions
    let tag = Tag::read_from_path(&flac_path).expect("read FLAC");
    let pics: Vec<_> = tag.pictures().collect();
    assert_eq!(pics.len(), 1, "Must have exactly 1 PICTURE block");
    let embedded = pics[0];

    assert_eq!(embedded.picture_type, PictureType::CoverFront);
    assert_eq!(
        embedded.mime_type, "image/jpeg",
        "Embedded artwork must be converted to image/jpeg for DAP/Symfonium compatibility"
    );
    assert!(
        embedded.width > 0,
        "Embedded picture width must be > 0 (was {})",
        embedded.width
    );
    assert!(
        embedded.height > 0,
        "Embedded picture height must be > 0 (was {})",
        embedded.height
    );
    assert!(
        embedded.data.len() <= MAX_EMBEDDED_PICTURE_BYTES,
        "Embedded picture size must be <= 800 KB"
    );

    // 2. Check external sidecars: MUST be preserved intact with original animated WebP content
    assert!(
        sidecar_cover_webp.exists(),
        "Sidecar cover.webp must still exist"
    );
    assert!(
        sidecar_animated_webp.exists(),
        "Sidecar animated.webp must still exist"
    );

    let sidecar_bytes = std::fs::read(&sidecar_cover_webp).expect("Read cover.webp");
    assert_eq!(
        sidecar_bytes, anim_webp,
        "Sidecar cover.webp must not be modified or overwritten with static JPEG"
    );

    let sidecar_frames = validate_animated_webp_bytes(&sidecar_bytes)
        .expect("Sidecar must retain animated WebP frames");
    assert_eq!(
        sidecar_frames, frame_count,
        "Sidecar must preserve all animation frames for Symfonium"
    );
}

#[test]
fn test_sanitize_flac_pictures_remediates_legacy_corrupt_blocks() {
    let dir = tempdir().expect("tempdir");
    let flac_path = dir.path().join("legacy_corrupt.flac");
    create_synthetic_flac(&flac_path);

    // Manually inject a legacy corrupt PICTURE block: WebP with 0x0 dimensions
    let mut tag = Tag::read_from_path(&flac_path).expect("read FLAC");
    let anim_webp = create_animated_webp_fixture();

    let mut legacy_pic = metaflac::block::Picture::new();
    legacy_pic.picture_type = PictureType::CoverFront;
    legacy_pic.mime_type = "image/webp".to_string();
    legacy_pic.width = 0; // Legacy 0x0 bug
    legacy_pic.height = 0;
    legacy_pic.data = anim_webp;
    tag.push_block(metaflac::Block::Picture(legacy_pic));
    tag.write_to_path(&flac_path).expect("write legacy FLAC");

    // Verify file is initially in the corrupt state
    let read_tag = Tag::read_from_path(&flac_path).expect("read FLAC");
    let pics: Vec<_> = read_tag.pictures().collect();
    assert_eq!(pics.len(), 1);
    assert_eq!(pics[0].width, 0);
    assert_eq!(pics[0].height, 0);

    // Apply sanitize_flac_pictures
    let report = sanitize_flac_pictures(&flac_path).expect("sanitize_flac_pictures must succeed");
    assert!(report.modified, "Sanitizer must report modification/repair");
    assert!(
        report.dropped_unrepairable_blocks.is_empty(),
        "A block that can be transcoded must be repaired, never dropped: {:?}",
        report.dropped_unrepairable_blocks
    );

    if !animated_webp_decode_available() {
        // The repair contract below needs the runner's FFmpeg to transcode the legacy
        // WebP payload, a capability the CI preflight reports as absent when the gate
        // is 0. Everything asserted up to here is hermetic and still ran; the
        // transcoding outcome is left to the E2E-capable run.
        eprintln!(
            "skipping legacy block repair assertions: CI preflight reported no decoder support"
        );
        return;
    }

    // Verify repaired file has valid dimensions and JPEG mime type
    let cleaned_tag = Tag::read_from_path(&flac_path).expect("read cleaned FLAC");
    let cleaned_pics: Vec<_> = cleaned_tag.pictures().collect();
    assert_eq!(cleaned_pics.len(), 1);
    let clean = cleaned_pics[0];
    assert!(clean.width > 0, "Repaired width must be > 0");
    assert!(clean.height > 0, "Repaired height must be > 0");
    assert_eq!(clean.mime_type, "image/jpeg");
    assert!(clean.data.len() <= MAX_EMBEDDED_PICTURE_BYTES);

    // Idempotence test
    let second_run = sanitize_flac_pictures(&flac_path).expect("second sanitize run");
    assert!(
        !second_run.modified,
        "Second run on already compliant file must report no modification"
    );
}

/// SYNC-AUD-066: an irreparable PICTURE block is removed from the file, and that
/// loss is now part of the returned report instead of a log line plus `Ok(true)`.
#[test]
fn test_sanitize_flac_pictures_reports_unrepairable_blocks() {
    let dir = tempdir().expect("tempdir");
    let flac_path = dir.path().join("unrepairable_cover.flac");
    create_synthetic_flac(&flac_path);

    // Corrupt WebP payload: `prepare_flac_picture` rejects it, and the failure is
    // pure container validation, so this needs no FFmpeg decoder on the runner.
    let mut tag = Tag::read_from_path(&flac_path).expect("read FLAC");
    let mut pic = metaflac::block::Picture::new();
    pic.picture_type = PictureType::CoverFront;
    pic.mime_type = "image/webp".to_string();
    pic.width = 0;
    pic.height = 0;
    pic.data =
        b"RIFF\x14\x00\x00\x00WEBPVP8X\x0a\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"
            .to_vec();
    tag.push_block(metaflac::block::Block::Picture(pic));
    tag.write_to_path(&flac_path)
        .expect("write FLAC with unrepairable cover");

    let report = sanitize_flac_pictures(&flac_path).expect("sanitize_flac_pictures must succeed");

    assert!(report.modified, "the block is removed from the file");
    assert_eq!(report.repaired_blocks, 0, "nothing could be repaired");
    assert!(
        report.dropped_cover_art(),
        "losing the cover art must be visible to the caller"
    );
    assert_eq!(
        report.dropped_unrepairable_blocks.len(),
        1,
        "exactly the irrepairable block must be reported: {:?}",
        report.dropped_unrepairable_blocks
    );
    assert!(
        report.dropped_unrepairable_blocks[0].contains("CoverFront"),
        "the report must identify the lost block: {:?}",
        report.dropped_unrepairable_blocks
    );

    let after = Tag::read_from_path(&flac_path).expect("read FLAC after sanitize");
    assert_eq!(
        after.pictures().count(),
        0,
        "the irrepairable block must not stay embedded"
    );

    // A compliant file reports no modification and no loss.
    let clean = sanitize_flac_pictures(&flac_path).expect("second sanitize run");
    assert!(!clean.modified);
    assert!(clean.dropped_unrepairable_blocks.is_empty());
}

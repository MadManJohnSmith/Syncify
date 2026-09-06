//! Sidecar Zero-Byte Guard and FLAC PICTURE Block Regeneration Test (TASK-147)
//!
//! Validates:
//! 1. Detection of 0-byte truncated sidecars (`cover.webp`, `folder.webp`, `animated.webp`).
//! 2. Automatic regeneration from FLAC `PICTURE` block (`CoverFront` 0x03) into library directories.
//! 3. Preservation of existing valid (> 0 bytes) sidecar files (no unnecessary overwrite).
//! 4. Remediation of the "3 albums, 9 files" scenario where 9 truncated files across 3 albums are repaired.
//! 5. Multi-disc root propagation of regenerated sidecars (`Disc 1/` -> album root).
//! 6. Symfonium Invariant preservation: CoverFront (0x03) = animated `image/webp`.

use std::path::Path;
use tempfile::tempdir;
use metaflac::Tag;
use metaflac::block::PictureType;
use syncify_tauri_lib::services::animated_cover::validate_animated_webp_bytes;
use syncify_tauri_lib::services::flac_picture::{
    ensure_flac_sidecars_intact, extract_cover_picture,
    is_valid_sidecar, scan_and_repair_album_sidecars,
};

/// Create a minimal synthetic animated WebP (RIFF WEBP VP8X + ANIM + ANMF frames).
fn create_synthetic_animated_webp(width: u16, height: u16, frame_count: u16) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&0u32.to_le_bytes()); // placeholder size
    data.extend_from_slice(b"WEBP");
    data.extend_from_slice(b"VP8X");
    data.extend_from_slice(&10u32.to_le_bytes()); // VP8X chunk size
    data.push(0x02); // animation flag bit (bit 1)
    data.extend_from_slice(&[0u8; 3]); // reserved
    data.extend_from_slice(&(width as u32 - 1).to_le_bytes()[..3]);
    data.extend_from_slice(&(height as u32 - 1).to_le_bytes()[..3]);

    data.extend_from_slice(b"ANIM");
    data.extend_from_slice(&6u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes()); // bg color
    data.extend_from_slice(&0u16.to_le_bytes()); // loop count

    for _ in 0..frame_count {
        data.extend_from_slice(b"ANMF");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()[..3]); // frame x
        data.extend_from_slice(&0u32.to_le_bytes()[..3]); // frame y
        data.extend_from_slice(&(width as u32 - 1).to_le_bytes()[..3]);
        data.extend_from_slice(&(height as u32 - 1).to_le_bytes()[..3]);
        data.extend_from_slice(&100u32.to_le_bytes()[..3]); // duration ms
        data.push(0x00); // flags
    }

    let file_size = (data.len() - 8) as u32;
    data[4..8].copy_from_slice(&file_size.to_le_bytes());
    data
}

/// Helper to generate a valid synthetic FLAC file using ffmpeg.
fn create_synthetic_flac(path: &Path) {
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "lavfi",
            "-i", "sine=frequency=440:duration=0.2",
            "-c:a", "flac",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("ffmpeg FLAC creation must execute");
    assert!(status.status.success(), "ffmpeg FLAC synthesis must succeed");
}

/// Attach an image/webp CoverFront picture block to a FLAC file.
fn attach_flac_cover_front(flac_path: &Path, webp_bytes: &[u8]) {
    let mut tag = Tag::read_from_path(flac_path).expect("Read FLAC tag");
    tag.add_picture("image/webp", PictureType::CoverFront, webp_bytes.to_vec());
    tag.save().expect("Save FLAC tag with CoverFront picture");
}

#[test]
fn test_is_valid_sidecar_guard() {
    let dir = tempdir().unwrap();
    let zero_file = dir.path().join("zero.webp");
    let valid_file = dir.path().join("valid.webp");
    let missing_file = dir.path().join("missing.webp");

    std::fs::write(&zero_file, b"").unwrap();
    std::fs::write(&valid_file, b"VALID_BYTES").unwrap();

    assert!(!is_valid_sidecar(&missing_file), "Missing file must not be valid");
    assert!(!is_valid_sidecar(&zero_file), "0-byte file must NOT be considered valid sidecar");
    assert!(is_valid_sidecar(&valid_file), "Positive-size file must be considered valid sidecar");
}

#[test]
fn test_sidecar_zero_byte_guard_detects_and_regenerates() {
    let dir = tempdir().unwrap();
    let album_dir = dir.path().join("Test_Album");
    std::fs::create_dir_all(&album_dir).unwrap();

    let flac_path = album_dir.join("01 - Test Track.flac");
    create_synthetic_flac(&flac_path);

    let webp_art = create_synthetic_animated_webp(400, 400, 6);
    attach_flac_cover_front(&flac_path, &webp_art);

    // Simulate interrupted downloads creating 0-byte truncated sidecars
    let cover_webp = album_dir.join("cover.webp");
    let folder_webp = album_dir.join("folder.webp");
    let animated_webp = album_dir.join("animated.webp");

    std::fs::write(&cover_webp, b"").unwrap();
    std::fs::write(&folder_webp, b"").unwrap();
    std::fs::write(&animated_webp, b"").unwrap();

    assert_eq!(std::fs::metadata(&cover_webp).unwrap().len(), 0);
    assert_eq!(std::fs::metadata(&folder_webp).unwrap().len(), 0);
    assert_eq!(std::fs::metadata(&animated_webp).unwrap().len(), 0);

    // Run the regeneration guard
    let regenerated = ensure_flac_sidecars_intact(&flac_path, &album_dir).unwrap();
    assert_eq!(regenerated.len(), 3, "All 3 zero-byte sidecars must be regenerated");

    // Verify files now have positive length and exact matching bytes
    for p in &[&cover_webp, &folder_webp, &animated_webp] {
        assert!(p.exists());
        let meta = std::fs::metadata(p).unwrap();
        assert!(meta.len() > 0, "File {:?} must have size > 0", p);
        let contents = std::fs::read(p).unwrap();
        assert_eq!(contents, webp_art, "Regenerated bytes must match FLAC PICTURE block");
        let frames = validate_animated_webp_bytes(&contents).expect("Must be valid animated WebP");
        assert_eq!(frames, 6, "Must retain original frame count");
    }
}

#[test]
fn test_sidecar_guard_does_not_overwrite_valid_files() {
    let dir = tempdir().unwrap();
    let album_dir = dir.path().join("Preserve_Album");
    std::fs::create_dir_all(&album_dir).unwrap();

    let flac_path = album_dir.join("01 - Track.flac");
    create_synthetic_flac(&flac_path);

    let flac_art = create_synthetic_animated_webp(300, 300, 4);
    attach_flac_cover_front(&flac_path, &flac_art);

    // Pre-existing valid sidecars with distinct custom content
    let cover_webp = album_dir.join("cover.webp");
    let custom_valid_bytes = b"PREEXISTING_VALID_CUSTOM_WEBP_ARTWORK_DO_NOT_TOUCH";
    std::fs::write(&cover_webp, custom_valid_bytes).unwrap();

    // Zero-byte sidecars that SHOULD be regenerated
    let folder_webp = album_dir.join("folder.webp");
    let animated_webp = album_dir.join("animated.webp");
    std::fs::write(&folder_webp, b"").unwrap();
    std::fs::write(&animated_webp, b"").unwrap();

    let regenerated = ensure_flac_sidecars_intact(&flac_path, &album_dir).unwrap();

    // Only the 2 zero-byte sidecars should have been regenerated
    assert_eq!(regenerated.len(), 2);
    assert!(!regenerated.contains(&cover_webp), "Valid cover.webp must NOT be overwritten");

    // Verify cover.webp was untouched
    let preserved_bytes = std::fs::read(&cover_webp).unwrap();
    assert_eq!(preserved_bytes, custom_valid_bytes, "Valid sidecar content must be preserved untouched");

    // Verify folder.webp and animated.webp were regenerated with flac_art
    assert_eq!(std::fs::read(&folder_webp).unwrap(), flac_art);
    assert_eq!(std::fs::read(&animated_webp).unwrap(), flac_art);
}

#[test]
fn test_three_albums_nine_zero_byte_files_remediation() {
    // Simulates the exact diagnostic condition:
    // 3 albums, each having 3 truncated 0-byte sidecars (cover.webp, folder.webp, animated.webp) = 9 files
    let root_dir = tempdir().unwrap();
    let albums = ["Album_Alpha", "Album_Beta", "Album_Gamma"];
    let mut all_zero_byte_files = Vec::new();

    for (i, album_name) in albums.iter().enumerate() {
        let album_path = root_dir.path().join(album_name);
        std::fs::create_dir_all(&album_path).unwrap();

        // Create FLAC with distinct animated artwork per album
        let flac_file = album_path.join("track.flac");
        create_synthetic_flac(&flac_file);
        let art = create_synthetic_animated_webp(200 + (i as u16 * 50), 200 + (i as u16 * 50), 3 + i as u16);
        attach_flac_cover_front(&flac_file, &art);

        // Create 3 truncated 0-byte sidecars per album
        let sidecar_names = ["cover.webp", "folder.webp", "animated.webp"];
        for name in &sidecar_names {
            let sc_path = album_path.join(name);
            std::fs::write(&sc_path, b"").unwrap();
            all_zero_byte_files.push(sc_path);
        }
    }

    assert_eq!(all_zero_byte_files.len(), 9, "Must have exactly 9 zero-byte files initially");
    for f in &all_zero_byte_files {
        assert_eq!(std::fs::metadata(f).unwrap().len(), 0);
    }

    // Run scanner/repair over the root directory containing the 3 albums
    let repaired = scan_and_repair_album_sidecars(root_dir.path()).unwrap();
    assert_eq!(repaired.len(), 9, "All 9 zero-byte sidecars across the 3 albums must be repaired");

    // Assert that NO 0-byte files remain
    for f in &all_zero_byte_files {
        assert!(f.exists());
        let len = std::fs::metadata(f).unwrap().len();
        assert!(len > 0, "File {:?} must have non-zero size after repair", f);
        let contents = std::fs::read(f).unwrap();
        let frames = validate_animated_webp_bytes(&contents).expect("Repaired file must be valid animated WebP");
        assert!(frames > 0);
    }
}

#[test]
fn test_multidisc_sidecar_zero_byte_repair_and_root_propagation() {
    let dir = tempdir().unwrap();
    let album_root = dir.path().join("Double_Album");
    let disc1_dir = album_root.join("Disc 1");
    std::fs::create_dir_all(&disc1_dir).unwrap();

    let flac_path = disc1_dir.join("01 - Track 1.flac");
    create_synthetic_flac(&flac_path);

    let art = create_synthetic_animated_webp(300, 300, 5);
    attach_flac_cover_front(&flac_path, &art);

    // Disc 1 has 0-byte sidecars
    let disc1_cover = disc1_dir.join("cover.webp");
    std::fs::write(&disc1_cover, b"").unwrap();

    // Album root has 0-byte sidecars
    let root_cover = album_root.join("cover.webp");
    std::fs::write(&root_cover, b"").unwrap();

    // Call ensure_flac_sidecars_intact for Disc 1
    let repaired = ensure_flac_sidecars_intact(&flac_path, &disc1_dir).unwrap();

    assert!(repaired.contains(&disc1_cover), "Disc 1 cover.webp must be repaired");
    assert!(repaired.contains(&root_cover), "Album root cover.webp must be repaired via multi-disc propagation");

    assert!(std::fs::metadata(&disc1_cover).unwrap().len() > 0);
    assert!(std::fs::metadata(&root_cover).unwrap().len() > 0);
}

#[test]
fn test_symfonium_invariant_coverfront_animated_webp_preserved() {
    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("symfonium_track.flac");
    create_synthetic_flac(&flac_path);

    let anim_art = create_synthetic_animated_webp(500, 500, 8);
    attach_flac_cover_front(&flac_path, &anim_art);

    let extracted = extract_cover_picture(&flac_path).expect("Cover picture must be extracted");
    assert_eq!(extracted.picture_type, PictureType::CoverFront, "Must be CoverFront (0x03)");
    assert_eq!(extracted.mime_type, "image/webp", "Must be image/webp");
    assert!(extracted.cover_type.is_animated(), "Must be classified as animated WebP");
    assert_eq!(extracted.data, anim_art, "Extracted payload must be bit-identical");
}

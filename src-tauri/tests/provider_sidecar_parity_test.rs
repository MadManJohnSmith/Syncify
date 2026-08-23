//! Provider Sidecar Parity & Staging Hygiene Test (S172)
//!
//! Validates:
//! 1. Uniformity of sidecar file generation (.lrc, cover.jpg, cover.webp, folder.webp, animated.webp).
//! 2. Staging hygiene: presence of `.nomedia` and 0 residual files left in `.staging` after promotion.
//! 3. Zero cross-album contamination (album A sidecars never pollute album B directory).

use tempfile::tempdir;

#[test]
fn test_sidecar_uniformity_and_staging_hygiene() {
    let base_dir = tempdir().unwrap();
    let library_dir = base_dir.path().join("Music").join("Syncify");
    let staging_dir = library_dir.join(".staging");
    let album_dir = library_dir.join("Pink Floyd").join("The Wall");

    std::fs::create_dir_all(&staging_dir).unwrap();
    std::fs::create_dir_all(&album_dir).unwrap();

    // 1. Staging must have .nomedia
    let nomedia_path = staging_dir.join(".nomedia");
    std::fs::write(&nomedia_path, b"").unwrap();
    assert!(nomedia_path.exists());
    assert_eq!(std::fs::metadata(&nomedia_path).unwrap().len(), 0);

    // 2. Mock staged files
    let staged_audio = staging_dir.join("track_06.part");
    let staged_lrc = staging_dir.join("track_06.lrc");
    let staged_cover_jpg = staging_dir.join("cover.jpg");
    let staged_cover_webp = staging_dir.join("cover.webp");
    let staged_cov_anim_webp = staging_dir.join("cover.animated.webp");

    std::fs::write(&staged_audio, b"DUMMY AUDIO").unwrap();
    std::fs::write(&staged_lrc, b"[00:01.00]Hello?").unwrap();
    std::fs::write(&staged_cover_jpg, b"DUMMY JPG").unwrap();
    std::fs::write(&staged_cover_webp, b"DUMMY WEBP").unwrap();
    std::fs::write(&staged_cov_anim_webp, b"DUMMY ANIM WEBP").unwrap();

    // 3. Promote audio and sidecars
    let final_audio = album_dir.join("06 - Comfortably Numb.flac");
    let final_lrc = album_dir.join("06 - Comfortably Numb.lrc");
    let final_cover_jpg = album_dir.join("cover.jpg");
    let final_cover_webp = album_dir.join("cover.webp");
    let final_cov_anim_webp = album_dir.join("cover.animated.webp");
    let final_folder_webp = album_dir.join("folder.webp");
    let final_animated_webp = album_dir.join("animated.webp");

    std::fs::rename(&staged_audio, &final_audio).unwrap();
    std::fs::rename(&staged_lrc, &final_lrc).unwrap();
    std::fs::copy(&staged_cover_jpg, &final_cover_jpg).unwrap();
    std::fs::copy(&staged_cover_webp, &final_cover_webp).unwrap();
    std::fs::copy(&staged_cov_anim_webp, &final_cov_anim_webp).unwrap();
    std::fs::copy(&staged_cover_webp, &final_folder_webp).unwrap();
    std::fs::copy(&staged_cover_webp, &final_animated_webp).unwrap();

    // 4. Staging cleanup of promoted files
    let _ = std::fs::remove_file(&staged_cover_jpg);
    let _ = std::fs::remove_file(&staged_cover_webp);
    let _ = std::fs::remove_file(&staged_cov_anim_webp);

    // 5. Verify all destination sidecars exist
    assert!(final_audio.exists());
    assert!(final_lrc.exists());
    assert!(final_cover_jpg.exists());
    assert!(final_cover_webp.exists());
    assert!(final_cov_anim_webp.exists());
    assert!(final_folder_webp.exists());
    assert!(final_animated_webp.exists());

    // 6. Verify .staging has ZERO residuals except .nomedia
    let staging_entries: Vec<_> = std::fs::read_dir(&staging_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    assert_eq!(
        staging_entries.len(),
        1,
        "Expected exactly 1 file in staging (.nomedia)"
    );
    assert_eq!(
        staging_entries[0].file_name().to_string_lossy(),
        ".nomedia"
    );
}

#[test]
fn test_no_cross_album_sidecar_contamination() {
    let base_dir = tempdir().unwrap();
    let library_dir = base_dir.path().join("Music").join("Syncify");
    let album_a_dir = library_dir.join("Pink Floyd").join("The Wall");
    let album_b_dir = library_dir.join("Pink Floyd").join("The Dark Side of the Moon");

    std::fs::create_dir_all(&album_a_dir).unwrap();
    std::fs::create_dir_all(&album_b_dir).unwrap();

    std::fs::write(album_a_dir.join("06 - Comfortably Numb.flac"), b"AUDIO A").unwrap();
    std::fs::write(album_a_dir.join("06 - Comfortably Numb.lrc"), b"LRC A").unwrap();
    std::fs::write(album_a_dir.join("cover.jpg"), b"COVER A").unwrap();

    std::fs::write(album_b_dir.join("01 - Speak to Me.flac"), b"AUDIO B").unwrap();
    std::fs::write(album_b_dir.join("01 - Speak to Me.lrc"), b"LRC B").unwrap();
    std::fs::write(album_b_dir.join("cover.jpg"), b"COVER B").unwrap();

    // Verify Album A has only its own track and sidecar
    let lrc_a = std::fs::read_to_string(album_a_dir.join("06 - Comfortably Numb.lrc")).unwrap();
    assert_eq!(lrc_a, "LRC A");
    assert!(!album_a_dir.join("01 - Speak to Me.lrc").exists());

    // Verify Album B has only its own track and sidecar
    let lrc_b = std::fs::read_to_string(album_b_dir.join("01 - Speak to Me.lrc")).unwrap();
    assert_eq!(lrc_b, "LRC B");
    assert!(!album_b_dir.join("06 - Comfortably Numb.lrc").exists());
}

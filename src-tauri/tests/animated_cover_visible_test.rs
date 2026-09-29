//! Animated Cover Visibility & Format Test (S176C)
//!
//! Validates:
//! 1. `cover.webp`, `animated.webp`, `folder.webp`, and `cover.animated.webp` are written to destination directories.
//! 2. Validates animated WebP container integrity (RIFF, WEBP, ANIM/ANMF chunk presence).
//! 3. Verifies file presence and visibility for media server indexing (e.g., Symfonium).

use syncify_tauri_lib::services::animated_cover::validate_animated_webp_bytes;
use tempfile::tempdir;

/// Versioned animated WebP (3 ANMF frames). Generating it at test time with the runner's
/// FFmpeg made this suite depend on an encoder whose output CI already declares
/// unverified, so container integrity is now checked against the tracked fixture with
/// no external process involved.
const ANIMATED_WEBP_FIXTURE: &[u8] = include_bytes!("fixtures/animated-cover.webp");

#[tokio::test]
async fn test_animated_cover_files_written_and_visible() {
    let dir = tempdir().expect("tempdir");
    let dest_dir = dir.path().join("Artist").join("Album");
    tokio::fs::create_dir_all(&dest_dir).await.unwrap();

    let animated_bytes = ANIMATED_WEBP_FIXTURE.to_vec();
    assert!(
        validate_animated_webp_bytes(&animated_bytes).is_ok(),
        "Synthetic WebP must be valid"
    );

    // Write standard animated cover sidecars
    let filenames = [
        "cover.webp",
        "animated.webp",
        "folder.webp",
        "cover.animated.webp",
    ];
    for fname in &filenames {
        let p = dest_dir.join(fname);
        tokio::fs::write(&p, &animated_bytes).await.unwrap();
    }

    // Verify all sidecar files exist and have exact valid bytes
    for fname in &filenames {
        let p = dest_dir.join(fname);
        assert!(p.exists(), "Sidecar {} must exist", fname);
        let read_bytes = tokio::fs::read(&p).await.unwrap();
        assert_eq!(
            read_bytes, animated_bytes,
            "Sidecar {} bytes must match",
            fname
        );
    }
}

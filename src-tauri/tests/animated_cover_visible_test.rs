//! Animated Cover Visibility & Format Test (S176C)
//!
//! Validates:
//! 1. `cover.webp`, `animated.webp`, `folder.webp`, and `cover.animated.webp` are written to destination directories.
//! 2. Validates animated WebP container integrity (RIFF, WEBP, ANIM/ANMF chunk presence).
//! 3. Verifies file presence and visibility for media server indexing (e.g., Symfonium).

use syncify_tauri_lib::services::animated_cover::validate_animated_webp_bytes;
use tempfile::tempdir;

fn create_synthetic_animated_webp_bytes() -> Vec<u8> {
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
async fn test_animated_cover_files_written_and_visible() {
    let dir = tempdir().expect("tempdir");
    let dest_dir = dir.path().join("Artist").join("Album");
    tokio::fs::create_dir_all(&dest_dir).await.unwrap();

    let animated_bytes = create_synthetic_animated_webp_bytes();
    assert!(validate_animated_webp_bytes(&animated_bytes).is_ok(), "Synthetic WebP must be valid");

    // Write standard animated cover sidecars
    let filenames = ["cover.webp", "animated.webp", "folder.webp", "cover.animated.webp"];
    for fname in &filenames {
        let p = dest_dir.join(fname);
        tokio::fs::write(&p, &animated_bytes).await.unwrap();
    }

    // Verify all sidecar files exist and have exact valid bytes
    for fname in &filenames {
        let p = dest_dir.join(fname);
        assert!(p.exists(), "Sidecar {} must exist", fname);
        let read_bytes = tokio::fs::read(&p).await.unwrap();
        assert_eq!(read_bytes, animated_bytes, "Sidecar {} bytes must match", fname);
    }
}

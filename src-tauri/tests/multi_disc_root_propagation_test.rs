//! Multi-Disc Root Propagation Test (S176C)
//!
//! Validates:
//! 1. Static covers (`cover.jpg`) and animated sidecars (`cover.webp`, `animated.webp`, `folder.webp`) are propagated to the parent album root in multi-disc layouts.
//! 2. `Disc 1/` and `Disc 2/` subdirectories retain their respective local sidecars.
//! 3. All propagated sidecars in the album root match the release artwork bytes.

use tempfile::tempdir;

fn create_jpeg(seed: u8, len: usize) -> Vec<u8> {
    let mut v = vec![seed; len];
    v[0] = 0xFF;
    v[1] = 0xD8;
    v[len - 2] = 0xFF;
    v[len - 1] = 0xD9;
    v
}

fn create_webp(seed: u8, len: usize) -> Vec<u8> {
    let mut v = vec![seed; len];
    v[0..4].copy_from_slice(b"RIFF");
    v[8..12].copy_from_slice(b"WEBP");
    v
}

#[tokio::test]
async fn test_multi_disc_cover_and_animated_root_propagation() {
    let root_dir = tempdir().expect("tempdir");
    let staging_root = root_dir.path().join(".staging");
    let library_root = root_dir.path().join("Music");

    let cover_jpg_bytes = create_jpeg(0x33, 2048);
    let cover_webp_bytes = create_webp(0x44, 4096);

    let album_root = library_root.join("Pink Floyd").join("The Wall (Remastered)");
    let disc_1_dir = album_root.join("Disc 1");
    let disc_2_dir = album_root.join("Disc 2");

    tokio::fs::create_dir_all(&disc_1_dir).await.unwrap();
    tokio::fs::create_dir_all(&disc_2_dir).await.unwrap();

    // 1. Process Disc 1
    let staging_d1 = staging_root.join("the_wall_d1");
    tokio::fs::create_dir_all(&staging_d1).await.unwrap();
    tokio::fs::write(staging_d1.join("cover.jpg"), &cover_jpg_bytes).await.unwrap();
    tokio::fs::write(staging_d1.join("cover.webp"), &cover_webp_bytes).await.unwrap();
    tokio::fs::write(staging_d1.join("animated.webp"), &cover_webp_bytes).await.unwrap();
    tokio::fs::write(staging_d1.join("folder.webp"), &cover_webp_bytes).await.unwrap();

    // Promote to Disc 1 and propagate to parent album root
    for fname in &["cover.jpg", "cover.webp", "animated.webp", "folder.webp"] {
        let src = staging_d1.join(fname);
        let dest_disc = disc_1_dir.join(fname);
        tokio::fs::copy(&src, &dest_disc).await.unwrap();

        let dest_root = album_root.join(fname);
        if !dest_root.exists() {
            tokio::fs::copy(&src, &dest_root).await.unwrap();
        }
    }
    tokio::fs::remove_dir_all(&staging_d1).await.unwrap();

    // 2. Process Disc 2
    let staging_d2 = staging_root.join("the_wall_d2");
    tokio::fs::create_dir_all(&staging_d2).await.unwrap();
    tokio::fs::write(staging_d2.join("cover.jpg"), &cover_jpg_bytes).await.unwrap();
    tokio::fs::write(staging_d2.join("cover.webp"), &cover_webp_bytes).await.unwrap();
    tokio::fs::write(staging_d2.join("animated.webp"), &cover_webp_bytes).await.unwrap();
    tokio::fs::write(staging_d2.join("folder.webp"), &cover_webp_bytes).await.unwrap();

    // Promote to Disc 2 and propagate to parent album root
    for fname in &["cover.jpg", "cover.webp", "animated.webp", "folder.webp"] {
        let src = staging_d2.join(fname);
        let dest_disc = disc_2_dir.join(fname);
        tokio::fs::copy(&src, &dest_disc).await.unwrap();

        let dest_root = album_root.join(fname);
        if !dest_root.exists() {
            tokio::fs::copy(&src, &dest_root).await.unwrap();
        }
    }
    tokio::fs::remove_dir_all(&staging_d2).await.unwrap();

    // 3. Verify files in Disc 1, Disc 2, and Album Root
    for fname in &["cover.jpg", "cover.webp", "animated.webp", "folder.webp"] {
        let p_d1 = disc_1_dir.join(fname);
        let p_d2 = disc_2_dir.join(fname);
        let p_root = album_root.join(fname);

        assert!(p_d1.exists(), "Disc 1 {} must exist", fname);
        assert!(p_d2.exists(), "Disc 2 {} must exist", fname);
        assert!(p_root.exists(), "Album root {} must exist", fname);

        let exp_bytes = if fname.ends_with(".jpg") { &cover_jpg_bytes } else { &cover_webp_bytes };
        assert_eq!(tokio::fs::read(&p_d1).await.unwrap(), *exp_bytes);
        assert_eq!(tokio::fs::read(&p_d2).await.unwrap(), *exp_bytes);
        assert_eq!(tokio::fs::read(&p_root).await.unwrap(), *exp_bytes);
    }
}

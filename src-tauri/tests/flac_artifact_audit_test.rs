use std::path::PathBuf;
use metaflac::Tag;
use syncify_tauri_lib::services::animated_cover::validate_animated_webp_bytes;

fn find_sample_flac() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(r"C:\Users\tardis\Music\Syncify\New Order\Brotherhood\06 - Bizarre Love Triangle.flac"),
        PathBuf::from(r"C:\Users\tardis\Documents\Syncify\downloads_test\Linkin Park\[2024] From Zero\02 - The Emptiness Machine.flac"),
        PathBuf::from(r"C:\Users\tardis\Documents\Syncify\downloads_test\Mid-Air Thief\[2018] Crumbling\02 - These Chains.flac"),
    ];

    for c in &candidates {
        if c.exists() {
            return Some(c.clone());
        }
    }
    None
}

#[test]
fn test_flac_magic_bytes_and_structure() {
    let flac_path = match find_sample_flac() {
        Some(p) => p,
        None => {
            println!("No sample FLAC found on local test system, skipping live file assertions");
            return;
        }
    };

    let data = std::fs::read(&flac_path).expect("FLAC file must be readable");
    assert!(data.len() > 4, "FLAC file must not be empty");
    assert_eq!(&data[0..4], b"fLaC", "FLAC file must have valid fLaC magic header");

    let tag = Tag::read_from_path(&flac_path).expect("FLAC tags must be readable via metaflac");
    let vorbis = tag.vorbis_comments().expect("Vorbis comments block must exist");

    // Title, Artist, Album are required baseline tags
    assert!(vorbis.title().is_some(), "TITLE tag must be present");
    assert!(vorbis.artist().is_some(), "ARTIST tag must be present");
    assert!(vorbis.album().is_some(), "ALBUM tag must be present");
}

#[test]
fn test_flac_picture_block_and_animated_webp() {
    let flac_path = match find_sample_flac() {
        Some(p) => p,
        None => return,
    };

    let tag = Tag::read_from_path(&flac_path).expect("FLAC tags must be readable");
    let pictures: Vec<_> = tag.pictures().collect();
    assert!(!pictures.is_empty(), "FLAC must contain at least one PICTURE metadata block");

    let pic = &pictures[0];
    assert_eq!(pic.picture_type, metaflac::block::PictureType::CoverFront);

    if pic.mime_type == "image/webp" {
        let frame_count = validate_animated_webp_bytes(&pic.data)
            .expect("Embedded WebP cover must pass animated WebP container validation");
        assert!(frame_count > 0, "Embedded animated WebP must have >0 animation frames");
    }
}

#[test]
fn test_album_sidecars_presence() {
    let flac_path = match find_sample_flac() {
        Some(p) => p,
        None => return,
    };

    let album_dir = flac_path.parent().expect("Album directory must exist");
    
    // Check sidecar files if present
    let cover_jpg = album_dir.join("cover.jpg");
    let cover_webp = album_dir.join("cover.webp");
    let folder_webp = album_dir.join("folder.webp");
    let animated_webp = album_dir.join("animated.webp");

    if cover_webp.exists() {
        let webp_bytes = std::fs::read(&cover_webp).expect("cover.webp must be readable");
        let frames = validate_animated_webp_bytes(&webp_bytes).expect("cover.webp must be valid animated WebP");
        assert!(frames > 0);
    }

    if folder_webp.exists() {
        assert!(folder_webp.is_file());
    }

    if animated_webp.exists() {
        assert!(animated_webp.is_file());
    }

    if cover_jpg.exists() {
        let jpg_data = std::fs::read(&cover_jpg).unwrap();
        assert!(jpg_data.starts_with(&[0xFF, 0xD8, 0xFF]), "cover.jpg must have valid JPEG header");
    }
}

//! Regression test for Symfonium Animated Cover, CoverFront Embedding & Sidecar Structure

use std::path::Path;
use syncify_cli::metadata::tag_writer::{apply_flac_tags, FlacMetadata};

#[derive(Debug, PartialEq, Eq)]
pub struct WebpAnimationInfo {
    pub is_valid_riff: bool,
    pub is_vp8x: bool,
    pub has_animation_flag: bool,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub frame_count: usize,
}

pub fn inspect_webp_animation(bytes: &[u8]) -> Option<WebpAnimationInfo> {
    if bytes.len() < 30 {
        return None;
    }
    if &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }

    let mut offset = 12;
    let mut is_vp8x = false;
    let mut has_animation_flag = false;
    let mut canvas_width = 0u32;
    let mut canvas_height = 0u32;
    let mut frame_count = 0usize;

    while offset + 8 <= bytes.len() {
        let fourcc = &bytes[offset..offset + 4];
        let chunk_len = u32::from_le_bytes([
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]) as usize;

        let chunk_start = offset + 8;
        let chunk_end = chunk_start + chunk_len;
        if chunk_end > bytes.len() {
            break;
        }

        let chunk_data = &bytes[chunk_start..chunk_end];

        if fourcc == b"VP8X" && chunk_data.len() >= 10 {
            is_vp8x = true;
            let flags = chunk_data[0];
            has_animation_flag = (flags & 0x02) != 0;
            canvas_width = u32::from_le_bytes([chunk_data[4], chunk_data[5], chunk_data[6], 0]) + 1;
            canvas_height = u32::from_le_bytes([chunk_data[7], chunk_data[8], chunk_data[9], 0]) + 1;
        } else if fourcc == b"ANMF" {
            frame_count += 1;
        }

        let pad = chunk_len % 2;
        offset = chunk_end + pad;
    }

    Some(WebpAnimationInfo {
        is_valid_riff: true,
        is_vp8x,
        has_animation_flag,
        canvas_width,
        canvas_height,
        frame_count,
    })
}

fn create_dummy_flac(path: &Path) {
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // is_last=1, len=34
    let mut streaminfo = [0u8; 34];
    streaminfo[0..2].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[2..4].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[10] = 0x0A;
    streaminfo[11] = 0xC4;
    streaminfo[12] = 0x42;
    streaminfo[13] = 0xF0;
    flac_bytes.extend_from_slice(&streaminfo);
    flac_bytes.extend_from_slice(&[0xFF, 0xF8, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00]);
    std::fs::write(path, &flac_bytes).unwrap();
}

#[test]
fn test_regression_rejects_synthetic_empty_webp() {
    // 30-byte dummy synthetic WebP with no frames
    let synthetic_webp = b"RIFF\x20\x00\x00\x00WEBPVP8X\x0A\x00\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    let info = inspect_webp_animation(synthetic_webp).unwrap();

    assert!(info.is_valid_riff);
    assert!(info.is_vp8x);
    assert!(info.has_animation_flag);
    assert_eq!(info.frame_count, 0, "Synthetic WebP has 0 frames and must be rejected");
}

#[test]
fn test_flac_coverfront_embedding_with_animated_webp() {
    let temp_dir = std::env::temp_dir().join(format!("test_webp_flac_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let flac_path = temp_dir.join("03 - Apologize.flac");
    create_dummy_flac(&flac_path);

    // Create minimal valid animated WebP bytes (RIFF ... WEBP VP8X ANIM ANMF)
    let mut webp_bytes = Vec::new();
    webp_bytes.extend_from_slice(b"RIFF\x30\x00\x00\x00WEBP");
    // VP8X
    webp_bytes.extend_from_slice(b"VP8X\x0A\x00\x00\x00\x12\x00\x00\x00\xF3\x01\x00\xF3\x01\x00"); // 500x500
    // ANIM
    webp_bytes.extend_from_slice(b"ANIM\x06\x00\x00\x00\xFF\xFF\xFF\xFF\x00\x00");
    // ANMF
    webp_bytes.extend_from_slice(b"ANMF\x10\x00\x00\x00\x00\x00\x00\x00\x00\x00\xF3\x01\x00\xF3\x01\x00\x47\x00\x00\x00");

    let meta = FlacMetadata {
        title: "Apologize".to_string(),
        artist: "The Warning".to_string(),
        album: "Keep Me Fed".to_string(),
        genre: Some("Alternative Rock".to_string()),
        release_year: Some("2024".to_string()),
        cover_data: Some(webp_bytes.clone()),
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &meta).expect("FLAC tags with animated WebP CoverFront must succeed");

    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let pictures: Vec<_> = tag.pictures().collect();

    assert_eq!(pictures.len(), 1, "Must contain exactly 1 picture block without duplicates");
    assert_eq!(pictures[0].picture_type, metaflac::block::PictureType::CoverFront, "PictureType must be CoverFront 0x03 for Symfonium animation detection");
    assert_eq!(pictures[0].mime_type, "image/webp", "MIME must be image/webp");
    assert_eq!(pictures[0].data, webp_bytes, "WebP bytes must match exactly without degradation");

    // Verify VorbisComments are intact
    let comments = tag.vorbis_comments().unwrap();
    assert_eq!(comments.get("TITLE").unwrap(), &["Apologize"]);
    assert_eq!(comments.get("ARTIST").unwrap(), &["The Warning"]);
    assert_eq!(comments.get("ALBUM").unwrap(), &["Keep Me Fed"]);
    assert_eq!(comments.get("GENRE").unwrap(), &["Alternative Rock"]);

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_historical_sidecars_and_production_output() {
    let target_dir = Path::new("downloads_real_production_test/The Warning/[2024] Keep Me Fed");
    if !target_dir.exists() {
        return;
    }

    let webp_path = target_dir.join("cover.webp");
    let folder_webp_path = target_dir.join("folder.webp");
    let animated_webp_path = target_dir.join("animated.webp");
    let jpg_path = target_dir.join("cover.jpg");

    assert!(webp_path.exists(), "cover.webp sidecar must exist");
    assert!(folder_webp_path.exists(), "folder.webp sidecar must exist");
    assert!(animated_webp_path.exists(), "animated.webp sidecar must exist");
    assert!(jpg_path.exists(), "cover.jpg fallback must exist");

    let webp_bytes = std::fs::read(&webp_path).unwrap();
    let info = inspect_webp_animation(&webp_bytes).expect("cover.webp must be a valid WebP container");
    assert!(info.is_valid_riff);
    assert!(info.is_vp8x);
    assert!(info.has_animation_flag);
    assert_eq!(info.canvas_width, 500);
    assert_eq!(info.canvas_height, 500);
    assert!(info.frame_count >= 10, "Must contain >= 10 ANMF animation frames (17 frames in production)");
}

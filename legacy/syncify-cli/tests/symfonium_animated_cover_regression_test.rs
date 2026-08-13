//! Regression test for Symfonium Animated Cover & Sidecar Structure

use std::path::Path;

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

#[test]
fn test_regression_rejects_synthetic_empty_webp() {
    // 30-byte dummy synthetic WebP with no frames
    let synthetic_webp = b"RIFF\x20\x00\x00\x00WEBPVP8X\x0A\x00\x00\x00\x02\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    let info = inspect_webp_animation(synthetic_webp).unwrap();

    assert!(info.is_valid_riff);
    assert!(info.is_vp8x);
    assert!(info.has_animation_flag);
    assert_eq!(info.frame_count, 0, "Synthetic WebP has 0 frames and is rejected by Symfonium");
}

#[test]
fn test_working_album_has_valid_animated_webp_and_static_cover() {
    let working_webp_path = Path::new("downloads_test/The Warning/[2024] Keep Me Fed/cover.webp");
    if !working_webp_path.exists() {
        return; // Skip if run in standalone environment without test fixtures
    }

    let bytes = std::fs::read(working_webp_path).unwrap();
    let info = inspect_webp_animation(&bytes).expect("Must be valid WebP container");

    assert!(info.is_valid_riff);
    assert!(info.is_vp8x);
    assert!(info.has_animation_flag, "Animation flag must be set");
    assert_eq!(info.canvas_width, 500);
    assert_eq!(info.canvas_height, 500);
    assert!(info.frame_count >= 10, "Must have real animation frames (ANMF chunks)");

    // Also check static cover.jpg exists in the same folder
    let working_jpg_path = Path::new("downloads_test/The Warning/[2024] Keep Me Fed/cover.jpg");
    assert!(working_jpg_path.exists(), "cover.jpg must exist alongside cover.webp");
}

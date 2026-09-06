//! Integration test suite for FLAC PICTURE block real dimension extraction (TASK-131).
//!
//! Validates:
//! 1. Robust image dimension extraction for JPEG (SOF0, SOF2), WebP (VP8X/animated, VP8, VP8L), and PNG.
//! 2. Defensive fallback to (0, 0) on corrupt or truncated headers.
//! 3. Tagging a FLAC file populates real, non-zero physical dimensions (width > 0, height > 0).
//! 4. Preservation of Symfonium invariant: CoverFront (0x03) image/webp animated is preserved
//!    intact, and legacy 0x0 dimensions are healed to real physical dimensions.

use metaflac::block::PictureType;
use metaflac::Tag;
use std::path::Path;
use syncify_flac_writer::{
    apply_flac_tags, extract_image_dimensions, write_flac_metadata, FlacMetadata, FlacTagExt,
};

fn create_synthetic_png(width: u32, height: u32) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    data.extend_from_slice(&13u32.to_be_bytes()); // IHDR length
    data.extend_from_slice(b"IHDR");
    data.extend_from_slice(&width.to_be_bytes());
    data.extend_from_slice(&height.to_be_bytes());
    data.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit truecolor
    data.extend_from_slice(&[0, 0, 0, 0]); // CRC placeholder
    data
}

fn create_synthetic_jpeg_sof0(width: u16, height: u16) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&[0xFF, 0xD8]); // SOI
    // APP0 JFIF
    data.extend_from_slice(&[
        0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
        0x00, 0x01, 0x00, 0x00,
    ]);
    // SOF0 (Baseline DCT)
    data.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
    data.extend_from_slice(&height.to_be_bytes());
    data.extend_from_slice(&width.to_be_bytes());
    data.extend_from_slice(&[0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01]);
    data.extend_from_slice(&[0xFF, 0xD9]); // EOI
    data
}

fn create_synthetic_jpeg_sof2(width: u16, height: u16) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&[0xFF, 0xD8]); // SOI
    // SOF2 (Progressive DCT)
    data.extend_from_slice(&[0xFF, 0xC2, 0x00, 0x11, 0x08]);
    data.extend_from_slice(&height.to_be_bytes());
    data.extend_from_slice(&width.to_be_bytes());
    data.extend_from_slice(&[0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01]);
    data.extend_from_slice(&[0xFF, 0xD9]); // EOI
    data
}

fn create_synthetic_animated_webp(width: u32, height: u32, frame_count: u32) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&[0, 0, 0, 0]); // File size - 8 placeholder
    data.extend_from_slice(b"WEBP");

    // VP8X chunk
    data.extend_from_slice(b"VP8X");
    data.extend_from_slice(&10u32.to_le_bytes());
    let flags: u8 = 0x02; // Animation flag
    data.extend_from_slice(&[flags, 0, 0, 0]);
    let w_raw = (width.saturating_sub(1)) & 0xFFFFFF;
    let h_raw = (height.saturating_sub(1)) & 0xFFFFFF;
    data.push((w_raw & 0xFF) as u8);
    data.push(((w_raw >> 8) & 0xFF) as u8);
    data.push(((w_raw >> 16) & 0xFF) as u8);
    data.push((h_raw & 0xFF) as u8);
    data.push(((h_raw >> 8) & 0xFF) as u8);
    data.push(((h_raw >> 16) & 0xFF) as u8);

    // ANIM chunk
    data.extend_from_slice(b"ANIM");
    data.extend_from_slice(&6u32.to_le_bytes());
    data.extend_from_slice(&[0, 0, 0, 0]); // BG color
    data.extend_from_slice(&0u16.to_le_bytes()); // Loop count

    // ANMF frames
    for _ in 0..frame_count {
        data.extend_from_slice(b"ANMF");
        let sub_payload = b"VP8 \x01\x00\x00\x00\x00";
        let anmf_len = 16 + sub_payload.len() as u32;
        data.extend_from_slice(&anmf_len.to_le_bytes());
        // Frame X, Y
        data.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        // Frame Width-1, Height-1
        data.push((w_raw & 0xFF) as u8);
        data.push(((w_raw >> 8) & 0xFF) as u8);
        data.push(((w_raw >> 16) & 0xFF) as u8);
        data.push((h_raw & 0xFF) as u8);
        data.push(((h_raw >> 8) & 0xFF) as u8);
        data.push(((h_raw >> 16) & 0xFF) as u8);
        // Duration: 100ms + Flags: 0x00
        data.extend_from_slice(&[100, 0, 0, 0]);
        data.extend_from_slice(sub_payload);
        if (sub_payload.len() & 1) != 0 {
            data.push(0);
        }
    }

    let file_len = (data.len() - 8) as u32;
    data[4..8].copy_from_slice(&file_len.to_le_bytes());
    data
}

fn create_synthetic_vp8_lossy(width: u16, height: u16) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&[0, 0, 0, 0]);
    data.extend_from_slice(b"WEBP");

    data.extend_from_slice(b"VP8 ");
    data.extend_from_slice(&10u32.to_le_bytes());
    // Keyframe tag (bit 0 = 0)
    data.extend_from_slice(&[0x00, 0x00, 0x00]);
    // Start code: 0x9D, 0x01, 0x2A
    data.extend_from_slice(&[0x9D, 0x01, 0x2A]);
    let w_field = width & 0x3FFF;
    let h_field = height & 0x3FFF;
    data.extend_from_slice(&w_field.to_le_bytes());
    data.extend_from_slice(&h_field.to_le_bytes());

    let file_len = (data.len() - 8) as u32;
    data[4..8].copy_from_slice(&file_len.to_le_bytes());
    data
}

fn create_synthetic_vp8l_lossless(width: u32, height: u32) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&[0, 0, 0, 0]);
    data.extend_from_slice(b"WEBP");

    data.extend_from_slice(b"VP8L");
    data.extend_from_slice(&5u32.to_le_bytes());
    data.push(0x2F); // Signature
    let w_m1 = (width - 1) & 0x3FFF;
    let h_m1 = (height - 1) & 0x3FFF;
    let b1 = (w_m1 & 0xFF) as u8;
    let b2 = (((w_m1 >> 8) & 0x3F) | ((h_m1 & 0x03) << 6)) as u8;
    let b3 = ((h_m1 >> 2) & 0xFF) as u8;
    let b4 = ((h_m1 >> 10) & 0x0F) as u8;
    data.extend_from_slice(&[b1, b2, b3, b4]);
    data.push(0);

    let file_len = (data.len() - 8) as u32;
    data[4..8].copy_from_slice(&file_len.to_le_bytes());
    data
}

fn create_synthetic_flac(path: &Path) {
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[
        0x80, 0x00, 0x00, 0x22, // Last metadata block (STREAMINFO), length 34
        0x10, 0x00, 0x10, 0x00, // min/max block size (4)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // min/max frame size (6)
        0x0A, 0xC4, 0x42, 0xF0, // 44.1kHz, 2 channels, 16 bits, 0 samples (4)
        0x00, 0x00, 0x00, 0x00, // (4)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // MD5 (8)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // MD5 (8) (total = 4+6+4+4+16 = 34)
    ]);
    std::fs::write(path, &flac_bytes).expect("Failed to write initial FLAC bytes");
}

#[test]
fn test_extract_dimensions_png() {
    let png_data = create_synthetic_png(800, 600);
    assert_eq!(extract_image_dimensions(&png_data), (800, 600));

    let png_large = create_synthetic_png(1920, 1080);
    assert_eq!(extract_image_dimensions(&png_large), (1920, 1080));
}

#[test]
fn test_extract_dimensions_jpeg() {
    let jpeg_sof0 = create_synthetic_jpeg_sof0(640, 480);
    assert_eq!(extract_image_dimensions(&jpeg_sof0), (640, 480));

    let jpeg_sof2 = create_synthetic_jpeg_sof2(1024, 768);
    assert_eq!(extract_image_dimensions(&jpeg_sof2), (1024, 768));
}

#[test]
fn test_extract_dimensions_webp() {
    // 1. Animated WebP via VP8X chunk
    let anim_webp = create_synthetic_animated_webp(500, 500, 3);
    assert_eq!(extract_image_dimensions(&anim_webp), (500, 500));

    let anim_rect = create_synthetic_animated_webp(640, 360, 2);
    assert_eq!(extract_image_dimensions(&anim_rect), (640, 360));

    // 2. Lossy VP8 keyframe
    let vp8_lossy = create_synthetic_vp8_lossy(320, 240);
    assert_eq!(extract_image_dimensions(&vp8_lossy), (320, 240));

    // 3. Lossless VP8L
    let vp8l_lossless = create_synthetic_vp8l_lossless(400, 300);
    assert_eq!(extract_image_dimensions(&vp8l_lossless), (400, 300));
}

#[test]
fn test_extract_dimensions_fallbacks() {
    // Empty data
    assert_eq!(extract_image_dimensions(&[]), (0, 0));

    // Arbitrary garbage bytes
    assert_eq!(extract_image_dimensions(b"NOT AN IMAGE FILE"), (0, 0));

    // Truncated headers
    assert_eq!(extract_image_dimensions(b"\x89PNG\r\n\x1a\n"), (0, 0));
    assert_eq!(extract_image_dimensions(b"RIFF\x20\x00\x00\x00WEBP"), (0, 0));
    assert_eq!(extract_image_dimensions(&[0xFF, 0xD8, 0xFF, 0xC0]), (0, 0));
}

#[test]
fn test_write_flac_metadata_populates_picture_dimensions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let flac_path = dir.path().join("flac_dims_test.flac");
    create_synthetic_flac(&flac_path);

    let jpeg_bytes = create_synthetic_jpeg_sof0(800, 600);

    let meta = FlacMetadata {
        title: "Test Dimensions Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        cover_data: Some(jpeg_bytes.clone()),
        ..Default::default()
    };

    write_flac_metadata(&flac_path, &meta).expect("write_flac_metadata must succeed");

    let read_tag = Tag::read_from_path(&flac_path).expect("read FLAC tags");
    let pictures: Vec<_> = read_tag.pictures().collect();
    assert_eq!(pictures.len(), 1, "Must contain exactly 1 PICTURE block");

    let pic = pictures[0];
    assert_eq!(pic.picture_type, PictureType::CoverFront);
    assert!(
        pic.width > 0,
        "Embedded picture width must be > 0 (was {})",
        pic.width
    );
    assert!(
        pic.height > 0,
        "Embedded picture height must be > 0 (was {})",
        pic.height
    );
    assert_eq!(pic.width, 800, "Embedded picture width must match source");
    assert_eq!(pic.height, 600, "Embedded picture height must match source");
}

#[test]
fn test_symfonium_invariant_animated_webp_preservation_with_real_dimensions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let flac_path = dir.path().join("symfonium_animated_invariant.flac");
    create_synthetic_flac(&flac_path);

    // 1. Construct synthetic animated WebP (500x500)
    let anim_webp = create_synthetic_animated_webp(500, 500, 4);

    // 2. Inject legacy PICTURE block with CoverFront (0x03) and 0x0 bug
    let mut initial_tag = Tag::read_from_path(&flac_path).expect("read initial FLAC");
    let mut legacy_pic = metaflac::block::Picture::new();
    legacy_pic.picture_type = PictureType::CoverFront; // 0x03
    legacy_pic.mime_type = "image/webp".to_string();
    legacy_pic.description = "Front Cover".to_string();
    legacy_pic.width = 0; // Legacy 0x0 bug
    legacy_pic.height = 0; // Legacy 0x0 bug
    legacy_pic.data = anim_webp.clone();
    initial_tag.push_block(metaflac::Block::Picture(legacy_pic));
    initial_tag
        .write_to_path(&flac_path)
        .expect("write legacy FLAC");

    // Verify initial file state has 0x0 dimensions and image/webp CoverFront
    let verify_tag = Tag::read_from_path(&flac_path).expect("read initial FLAC for check");
    let initial_pics: Vec<_> = verify_tag.pictures().collect();
    assert_eq!(initial_pics.len(), 1);
    assert_eq!(initial_pics[0].picture_type, PictureType::CoverFront);
    assert_eq!(initial_pics[0].mime_type, "image/webp");
    assert_eq!(initial_pics[0].width, 0, "Initial width must be 0");
    assert_eq!(initial_pics[0].height, 0, "Initial height must be 0");

    // 3. Tag with an incoming static JPEG payload (e.g. standard album art from API)
    let incoming_jpeg = create_synthetic_jpeg_sof0(600, 600);
    let meta = FlacMetadata {
        title: "Animated Invariant Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        cover_data: Some(incoming_jpeg),
        ..Default::default()
    };

    // Apply metadata tags
    apply_flac_tags(&flac_path, &meta).expect("apply_flac_tags must succeed");

    // 4. Read back and assert Symfonium invariant & healed dimensions
    let result_tag = Tag::read_from_path(&flac_path).expect("read resulting FLAC");
    let result_pics: Vec<_> = result_tag.pictures().collect();
    assert_eq!(
        result_pics.len(),
        1,
        "Must contain exactly 1 CoverFront block"
    );

    let final_pic = result_pics[0];

    // INVARIANTE SYMFONIUM: CoverFront (0x03) = image/webp animado MUST be preserved!
    assert_eq!(
        final_pic.picture_type,
        PictureType::CoverFront,
        "PictureType must remain CoverFront (0x03)"
    );
    assert_eq!(
        final_pic.mime_type, "image/webp",
        "MIME type must remain image/webp to preserve Now Playing animation in Symfonium"
    );
    assert_eq!(
        final_pic.data, anim_webp,
        "WebP payload data must be preserved byte-for-byte"
    );

    // DIMENSION HEALING: Must have real non-zero dimensions
    assert_eq!(
        final_pic.width, 500,
        "Legacy 0x0 width must be healed to real WebP canvas width (500)"
    );
    assert_eq!(
        final_pic.height, 500,
        "Legacy 0x0 height must be healed to real WebP canvas height (500)"
    );
}

#[test]
fn test_heal_preexisting_flac_picture_dimensions_without_incoming_cover() {
    let dir = tempfile::tempdir().expect("tempdir");
    let flac_path = dir.path().join("heal_without_cover.flac");
    create_synthetic_flac(&flac_path);

    let png_bytes = create_synthetic_png(640, 480);

    // Inject legacy 0x0 block
    let mut tag = Tag::read_from_path(&flac_path).expect("read flac");
    let mut legacy_pic = metaflac::block::Picture::new();
    legacy_pic.picture_type = PictureType::CoverFront;
    legacy_pic.mime_type = "image/png".to_string();
    legacy_pic.width = 0;
    legacy_pic.height = 0;
    legacy_pic.data = png_bytes;
    tag.push_block(metaflac::Block::Picture(legacy_pic));
    tag.write_to_path(&flac_path).expect("write flac");

    // Tag without cover_data
    let meta = FlacMetadata {
        title: "No Cover Track".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        cover_data: None,
        ..Default::default()
    };
    write_flac_metadata(&flac_path, &meta).expect("write_flac_metadata");

    // Verify healed
    let read_tag = Tag::read_from_path(&flac_path).expect("read flac");
    let pic = read_tag.pictures().next().expect("picture block");
    assert_eq!(pic.width, 640);
    assert_eq!(pic.height, 480);
}

#[test]
fn test_flac_tag_ext_add_picture_with_dimensions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let flac_path = dir.path().join("ext_tag_test.flac");
    create_synthetic_flac(&flac_path);

    let jpeg_bytes = create_synthetic_jpeg_sof0(1200, 800);

    let mut tag = Tag::read_from_path(&flac_path).expect("read flac");
    tag.add_picture_with_dimensions("image/jpeg", PictureType::CoverFront, jpeg_bytes);
    tag.write_to_path(&flac_path).expect("write flac");

    let read_tag = Tag::read_from_path(&flac_path).expect("read flac");
    let pic = read_tag.pictures().next().expect("picture block");
    assert_eq!(pic.width, 1200);
    assert_eq!(pic.height, 800);
}

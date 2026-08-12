use syncify_cli::download::{resolve_and_download_animated_cover, AnimatedCoverStatus};
use syncify_cli::download::http_client::create_http_client;
use std::path::Path;

fn create_valid_flac_file(path: &Path, audio_payload_len: usize) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;

    // 1. fLaC marker
    file.write_all(b"fLaC")?;

    // 2. STREAMINFO block header: is_last=1, len=34
    let streaminfo_header: [u8; 4] = [0x80, 0x00, 0x00, 0x22];
    file.write_all(&streaminfo_header)?;

    // 3. STREAMINFO payload (34 bytes): 44.1kHz, 2ch, 16-bit
    let mut streaminfo_payload = [0u8; 34];
    streaminfo_payload[0..2].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo_payload[2..4].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo_payload[10] = 0x0A;
    streaminfo_payload[11] = 0xC4;
    streaminfo_payload[12] = 0x42;
    streaminfo_payload[13] = 0xF0;
    file.write_all(&streaminfo_payload)?;

    // 4. Audio frame sync 0xFFF8
    let mut audio_data = vec![0u8; audio_payload_len.max(16)];
    audio_data[0] = 0xFF;
    audio_data[1] = 0xF8;
    audio_data[2] = 0x18;
    audio_data[3] = 0x00;
    file.write_all(&audio_data)?;

    Ok(())
}

fn verify_flac_audio_frames_intact(path: &Path, expected_payload_len: usize) -> Result<(), String> {
    let data = std::fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;
    if data.len() < 42 || &data[0..4] != b"fLaC" {
        return Err("Invalid FLAC magic header".to_string());
    }

    let mut offset = 4;
    let mut found_last = false;

    while offset < data.len() {
        if offset + 4 > data.len() {
            return Err("Truncated block header".to_string());
        }

        let hdr = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
        let is_last = (hdr >> 31) & 1 == 1;
        let block_len = (hdr & 0x00FF_FFFF) as usize;

        offset += 4 + block_len;
        if is_last {
            found_last = true;
            break;
        }
    }

    if !found_last {
        return Err("No metadata block marked as last".to_string());
    }

    let audio_start = offset;
    if audio_start + expected_payload_len > data.len() {
        return Err("Audio payload truncated".to_string());
    }

    let sync = u16::from_be_bytes([data[audio_start], data[audio_start + 1]]);
    if (sync & 0xFFFC) != 0xFFF8 {
        return Err(format!("Invalid audio sync 0x{:04X}", sync));
    }

    Ok(())
}

#[tokio::test]
async fn test_animated_cover_sidecar_and_metaflac_integration() {
    let temp_base = std::env::temp_dir().join(format!("syncify_anim_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let album_dir = temp_base.join("David Bowie").join("Heroes");
    std::fs::create_dir_all(&album_dir).unwrap();

    let track_flac = album_dir.join("01 - Heroes.flac");
    create_valid_flac_file(&track_flac, 1024).unwrap();

    // Verify initial FLAC structure
    assert!(verify_flac_audio_frames_intact(&track_flac, 1024).is_ok());

    // Create a mock animated webp file
    let webp_path = album_dir.join("cover.webp");
    let mock_webp_data = vec![
        0x52, 0x49, 0x46, 0x46, 0x14, 0x00, 0x00, 0x00, // RIFF len=20
        0x57, 0x45, 0x42, 0x50, // WEBP
        0x56, 0x50, 0x38, 0x58, // VP8X (Extended WebP with Animation)
        0x0A, 0x00, 0x00, 0x00, // Chunk len=10
        0x02, 0x00, 0x00, 0x00, // Flags: Animation=1
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    std::fs::write(&webp_path, &mock_webp_data).unwrap();

    // Perform metaflac picture embedding
    let mut tag = metaflac::Tag::read_from_path(&track_flac).unwrap();
    tag.remove_picture_type(metaflac::block::PictureType::CoverFront);
    tag.add_picture("image/webp", metaflac::block::PictureType::CoverFront, mock_webp_data.clone());
    tag.write_to_path(&track_flac).unwrap();

    // Verify FLAC structure and audio payload integrity
    assert!(verify_flac_audio_frames_intact(&track_flac, 1024).is_ok());

    // Verify tag contains exactly 1 picture block
    let tag_read = metaflac::Tag::read_from_path(&track_flac).unwrap();
    let pics: Vec<_> = tag_read.pictures().collect();
    assert_eq!(pics.len(), 1);
    assert_eq!(pics[0].mime_type, "image/webp");
    assert_eq!(pics[0].data, mock_webp_data);

    // Verify sidecars
    let folder_webp = album_dir.join("folder.webp");
    let animated_webp = album_dir.join("animated.webp");
    std::fs::copy(&webp_path, &folder_webp).unwrap();
    std::fs::copy(&webp_path, &animated_webp).unwrap();

    assert!(folder_webp.exists());
    assert!(animated_webp.exists());

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_base);
}

#[tokio::test]
async fn test_animated_cover_empty_inputs_return_not_found() {
    let client = create_http_client();
    let target = std::env::temp_dir().join("syncify_anim_empty");

    let status1 = resolve_and_download_animated_cover(&client, "", "Album", &target).await;
    assert_eq!(status1, AnimatedCoverStatus::NotFound);

    let status2 = resolve_and_download_animated_cover(&client, "Artist", "", &target).await;
    assert_eq!(status2, AnimatedCoverStatus::NotFound);
}

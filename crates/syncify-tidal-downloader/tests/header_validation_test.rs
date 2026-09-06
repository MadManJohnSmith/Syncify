use std::io::Write;
use syncify_tidal_downloader::{
    read_audio_header_bounded, validate_audio_file_header, validate_audio_header_magic,
    AUDIO_HEADER_PROBE_SIZE,
};
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_bounded_header_read_on_large_file_prevents_oom() {
    // Simulate a large 256 MB audio download using a sparse file
    let mut temp = NamedTempFile::new().expect("Create tempfile");
    
    // Write a valid FLAC header (4 bytes 'fLaC' + 4 bytes metadata block header)
    temp.write_all(b"fLaC\x00\x00\x00\x22")
        .expect("Write FLAC magic");
    
    // Extend file to 256 MB (sparse file on supported OS, instant O(1) allocation)
    const TARGET_SIZE: u64 = 256 * 1024 * 1024;
    temp.as_file()
        .set_len(TARGET_SIZE)
        .expect("Set sparse file len to 256 MB");
    temp.flush().expect("Flush file");

    let path = temp.path();
    let meta = std::fs::metadata(path).expect("Read metadata");
    assert_eq!(meta.len(), TARGET_SIZE, "File on disk must report 256 MB");

    // Reading header must strictly read bounded 64 bytes without loading 256 MB into memory
    let (buf, bytes_read) = read_audio_header_bounded(path)
        .await
        .expect("Read bounded header");

    assert_eq!(bytes_read, AUDIO_HEADER_PROBE_SIZE);
    assert_eq!(&buf[..4], b"fLaC");

    // Full validation should succeed instantaneously with O(1) memory
    let validation_result = validate_audio_file_header(path, "flac").await;
    assert!(
        validation_result.is_ok(),
        "Validation of 256 MB sparse FLAC file must succeed without OOM: {:?}",
        validation_result.err()
    );
}

#[tokio::test]
async fn test_valid_audio_magic_headers() {
    // 1. Native FLAC
    let mut flac_temp = NamedTempFile::new().expect("Create tempfile");
    flac_temp
        .write_all(b"fLaC\x00\x00\x00\x22\x00\x00\x00\x00")
        .expect("Write FLAC header");
    flac_temp.flush().expect("Flush");
    assert!(validate_audio_file_header(flac_temp.path(), "flac")
        .await
        .is_ok());

    // 2. ISOBMFF FLAC container (Tidal DASH segment)
    let mut dash_temp = NamedTempFile::new().expect("Create tempfile");
    dash_temp
        .write_all(b"\x00\x00\x00\x18ftypdash\x00\x00\x00\x00")
        .expect("Write ISOBMFF header");
    dash_temp.flush().expect("Flush");
    assert!(validate_audio_file_header(dash_temp.path(), "flac")
        .await
        .is_ok());

    // 3. MP3 with ID3v2 tag
    let mut mp3_id3_temp = NamedTempFile::new().expect("Create tempfile");
    mp3_id3_temp
        .write_all(b"ID3\x03\x00\x00\x00\x00\x00\x00")
        .expect("Write ID3 header");
    mp3_id3_temp.flush().expect("Flush");
    assert!(validate_audio_file_header(mp3_id3_temp.path(), "mp3")
        .await
        .is_ok());

    // 4. MP3 with raw MPEG sync frame (0xFF 0xFB)
    let mut mp3_frame_temp = NamedTempFile::new().expect("Create tempfile");
    mp3_frame_temp
        .write_all(&[0xFF, 0xFB, 0x90, 0x64, 0x00, 0x00, 0x00, 0x00])
        .expect("Write MP3 sync header");
    mp3_frame_temp.flush().expect("Flush");
    assert!(validate_audio_file_header(mp3_frame_temp.path(), "mp3")
        .await
        .is_ok());

    // 5. MP4 / M4A (ftyp box)
    let mut m4a_temp = NamedTempFile::new().expect("Create tempfile");
    m4a_temp
        .write_all(b"\x00\x00\x00\x20ftypM4A \x00\x00\x00\x00")
        .expect("Write M4A header");
    m4a_temp.flush().expect("Flush");
    assert!(validate_audio_file_header(m4a_temp.path(), "m4a")
        .await
        .is_ok());
    assert!(validate_audio_file_header(m4a_temp.path(), "mp4")
        .await
        .is_ok());
}

#[tokio::test]
async fn test_corrupt_headers_rejection() {
    // 1. Non-FLAC payload with .flac extension
    let mut bad_flac = NamedTempFile::new().expect("Create tempfile");
    bad_flac
        .write_all(b"RIFF\x24\x00\x00\x00WAVEfmt ")
        .expect("Write non-flac header");
    bad_flac.flush().expect("Flush");
    let res = validate_audio_file_header(bad_flac.path(), "flac").await;
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("fails FLAC magic header verification"),
        "Unexpected error: {}",
        err_msg
    );

    // 2. Non-MP3 payload with .mp3 extension
    let mut bad_mp3 = NamedTempFile::new().expect("Create tempfile");
    bad_mp3
        .write_all(b"OggS\x00\x02\x00\x00\x00\x00\x00\x00")
        .expect("Write non-mp3 header");
    bad_mp3.flush().expect("Flush");
    let res = validate_audio_file_header(bad_mp3.path(), "mp3").await;
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("fails MP3 frame header verification"),
        "Unexpected error: {}",
        err_msg
    );

    // 3. Non-M4A payload with .m4a extension
    let mut bad_m4a = NamedTempFile::new().expect("Create tempfile");
    bad_m4a
        .write_all(b"fLaC\x00\x00\x00\x22\x00\x00\x00\x00")
        .expect("Write non-m4a header");
    bad_m4a.flush().expect("Flush");
    let res = validate_audio_file_header(bad_m4a.path(), "m4a").await;
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("fails MP4/AAC magic header verification"),
        "Unexpected error: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_empty_and_truncated_files_rejection() {
    // 1. Empty file (0 bytes)
    let empty_file = NamedTempFile::new().expect("Create tempfile");
    let res = validate_audio_file_header(empty_file.path(), "flac").await;
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("too small to contain valid audio headers"),
        "Unexpected error: {}",
        err_msg
    );

    // 2. Truncated file (2 bytes)
    let mut truncated = NamedTempFile::new().expect("Create tempfile");
    truncated.write_all(b"fL").expect("Write 2 bytes");
    truncated.flush().expect("Flush");
    let res = validate_audio_file_header(truncated.path(), "flac").await;
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("too small to contain valid audio headers"),
        "Unexpected error: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_nonexistent_file_rejection() {
    let non_existent = std::path::Path::new("/tmp/non_existent_audio_file_12345.flac");
    let res = validate_audio_file_header(non_existent, "flac").await;
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("Cannot read downloaded file header"),
        "Unexpected error: {}",
        err_msg
    );
}

#[test]
fn test_validate_audio_header_magic_unit() {
    // Direct in-memory buffer check
    assert!(validate_audio_header_magic(b"fLaC\x00\x00\x00\x22", "flac").is_ok());
    assert!(validate_audio_header_magic(b"\x00\x00\x00\x18ftypdash", "flac").is_ok());
    assert!(validate_audio_header_magic(b"ID3\x03\x00\x00\x00", "mp3").is_ok());
    assert!(validate_audio_header_magic(&[0xFF, 0xFB, 0x90, 0x64], "mp3").is_ok());
    assert!(validate_audio_header_magic(b"\x00\x00\x00\x20ftypM4A ", "m4a").is_ok());

    // Buffer smaller than 4 bytes
    assert!(validate_audio_header_magic(b"fL", "flac").is_err());
    assert!(validate_audio_header_magic(b"", "flac").is_err());
}

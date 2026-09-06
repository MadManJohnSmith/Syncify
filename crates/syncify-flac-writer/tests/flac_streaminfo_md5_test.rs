use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use syncify_flac_writer::{
    apply_flac_tags, compute_pcm_stream_md5, inspect_and_verify_flac_stream,
    populate_streaminfo_md5, verify_flac_integrity_stream, write_flac_metadata, FlacMetadata,
};
use tempfile::TempDir;

/// Helper to generate a valid FLAC audio file using ffmpeg.
fn generate_test_flac(
    dir: &Path,
    filename: &str,
    sample_rate: u32,
    bit_depth: u8,
    channels: u8,
) -> PathBuf {
    let path = dir.join(filename);
    let freq_filter = format!("sine=frequency=1000:duration=1:sample_rate={}", sample_rate);

    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-f", "lavfi", "-i", &freq_filter]);

    if channels == 1 {
        cmd.args(["-ac", "1"]);
    } else {
        cmd.args(["-ac", "2"]);
    }

    if bit_depth == 24 {
        cmd.args(["-af", &format!("aformat=sample_fmts=s32:sample_rates={}", sample_rate)]);
    }

    cmd.args(["-c:a", "flac"]).arg(&path);

    let status = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("Failed to execute ffmpeg");

    assert!(status.success(), "ffmpeg failed to generate test FLAC at {:?}", path);
    path
}

/// Helper to check if `flac` binary is available in PATH.
fn has_flac_binary() -> bool {
    Command::new("flac")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn test_flac_preserves_md5_after_applying_tags() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = generate_test_flac(temp_dir.path(), "test_preservation.flac", 44100, 16, 2);

    // 1. Read initial STREAMINFO MD5
    let tag = metaflac::Tag::read_from_path(&flac_path).expect("Failed to read FLAC");
    let streaminfo = tag.get_streaminfo().expect("Missing STREAMINFO");
    assert_eq!(streaminfo.md5.len(), 16);
    assert!(streaminfo.md5.iter().any(|&b| b != 0), "Initial MD5 must not be all zeros");
    let initial_md5 = streaminfo.md5.clone();

    // 2. Apply metadata tags
    let metadata = FlacMetadata {
        title: "Bit-Exact Preservation Track".to_string(),
        artist: "Master Audio Artist".to_string(),
        album: "High-Fidelity Master".to_string(),
        track_number: 1,
        track_total: 10,
        genre: Some("Audiophile".to_string()),
        explicit: Some(false),
        ..Default::default()
    };
    apply_flac_tags(&flac_path, &metadata).expect("apply_flac_tags failed");

    // 3. Verify STREAMINFO MD5 is preserved bit-for-bit
    let tag_after = metaflac::Tag::read_from_path(&flac_path).expect("Failed to re-read FLAC");
    let si_after = tag_after.get_streaminfo().expect("Missing STREAMINFO after tags");
    assert_eq!(
        si_after.md5, initial_md5,
        "STREAMINFO MD5 must remain identical after applying tags"
    );

    // 4. Verify physical stream integrity
    let report = inspect_and_verify_flac_stream(&flac_path).expect("inspect_and_verify failed");
    assert!(report.verified, "FLAC stream must be verified");
    assert!(report.streaminfo_md5_valid, "STREAMINFO MD5 must be marked valid");
    assert_eq!(report.check_mode, "streaminfo_md5");

    assert!(
        verify_flac_integrity_stream(&flac_path).expect("verify_flac_integrity_stream failed"),
        "Integrity stream verification must succeed"
    );

    // 5. Verify flac -t CLI passes if available
    if has_flac_binary() {
        let flac_check = Command::new("flac")
            .args(["-t", "-s"])
            .arg(&flac_path)
            .status()
            .expect("Failed to run flac -t");
        assert!(flac_check.success(), "flac -t must succeed on tagged file");
    }
}

#[test]
fn test_flac_preserves_md5_after_write_flac_metadata_with_cover() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = generate_test_flac(temp_dir.path(), "test_cover_preservation.flac", 44100, 16, 2);

    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let initial_md5 = tag.get_streaminfo().unwrap().md5.clone();

    // Minimal 1x1 valid JPEG
    let minimal_jpg: &[u8] = &[
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01, 0x00,
        0x48, 0x00, 0x48, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06,
        0x05, 0x08, 0x07, 0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B,
        0x0C, 0x19, 0x12, 0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
        0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29, 0x2C, 0x30, 0x31,
        0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32, 0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF,
        0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00,
        0x1F, 0x00, 0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
        0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, 0xBF, 0x80, 0xFF, 0xD9,
    ];

    let metadata = FlacMetadata {
        title: "Track with Embedded Cover".to_string(),
        artist: "Cover Artist".to_string(),
        album: "Cover Album".to_string(),
        cover_data: Some(minimal_jpg.to_vec()),
        cover_source: Some("cover.jpg".to_string()),
        ..Default::default()
    };

    write_flac_metadata(&flac_path, &metadata).expect("write_flac_metadata failed");

    let tag_after = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let si_after = tag_after.get_streaminfo().unwrap();
    assert_eq!(
        si_after.md5, initial_md5,
        "STREAMINFO MD5 must survive cover art embedding intact"
    );
    assert_eq!(tag_after.pictures().count(), 1, "Picture block must be embedded");
}

#[test]
fn test_synthetic_flac_with_zero_md5_populated_and_verified() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = generate_test_flac(temp_dir.path(), "test_zero_md5.flac", 44100, 16, 2);

    // Compute ground-truth MD5 first
    let ground_truth_md5 = compute_pcm_stream_md5(&flac_path).expect("PCM MD5 computation failed");

    // Manually zero out MD5 at offset 26
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&flac_path)
            .expect("Failed to open file for zeroing MD5");
        file.seek(SeekFrom::Start(26)).unwrap();
        file.write_all(&[0u8; 16]).unwrap();
        file.flush().unwrap();
    }

    // Verify MD5 is now 000...0
    let tag_zero = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let si_zero = tag_zero.get_streaminfo().unwrap();
    assert_eq!(si_zero.md5, vec![0u8; 16], "MD5 must be all zeros");

    // Inspect in zero-MD5 state: must use decode_check mode
    let report_zero = inspect_and_verify_flac_stream(&flac_path).unwrap();
    assert_eq!(report_zero.check_mode, "decode_check");
    assert!(!report_zero.streaminfo_md5_valid);
    assert!(report_zero.verified);

    // If flac CLI is present, flac -t issues a warning for zeroed MD5
    if has_flac_binary() {
        let output = Command::new("flac")
            .args(["-t"])
            .arg(&flac_path)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("cannot check MD5 signature") || output.status.success(),
            "flac -t should run with warning or pass"
        );
    }

    // Now populate STREAMINFO MD5
    let populated_md5 = populate_streaminfo_md5(&flac_path).expect("populate_streaminfo_md5 failed");
    assert_eq!(populated_md5, ground_truth_md5, "Populated MD5 must match PCM stream MD5");

    // Re-read STREAMINFO from disk
    let tag_populated = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let si_populated = tag_populated.get_streaminfo().unwrap();
    assert_eq!(
        si_populated.md5,
        ground_truth_md5.to_vec(),
        "STREAMINFO MD5 on disk must now contain populated hash"
    );

    // Verify stream integrity now operates in bit-exact "streaminfo_md5" mode
    let report_populated = inspect_and_verify_flac_stream(&flac_path).unwrap();
    assert_eq!(report_populated.check_mode, "streaminfo_md5");
    assert!(report_populated.streaminfo_md5_valid);
    assert!(report_populated.verified);
    assert_eq!(
        report_populated.computed_md5,
        ground_truth_md5.iter().map(|b| format!("{:02x}", b)).collect::<String>()
    );

    // Verify flac -t runs cleanly without warning
    if has_flac_binary() {
        let output = Command::new("flac")
            .args(["-t"])
            .arg(&flac_path)
            .output()
            .unwrap();
        assert!(output.status.success(), "flac -t must pass after population");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("cannot check MD5 signature"),
            "flac -t must not warn about unset MD5 after population"
        );
    }
}

#[test]
fn test_compute_pcm_stream_md5_hi_res_24bit_exactness() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = generate_test_flac(temp_dir.path(), "test_hires_24_96.flac", 96000, 24, 2);

    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let si = tag.get_streaminfo().unwrap();
    assert_eq!(si.bits_per_sample, 24);
    assert_eq!(si.sample_rate, 96000);

    let upstream_md5 = si.md5.clone();
    assert!(upstream_md5.iter().any(|&b| b != 0));

    let computed = compute_pcm_stream_md5(&flac_path).expect("Failed 24-bit PCM MD5 computation");
    assert_eq!(
        computed.to_vec(),
        upstream_md5,
        "24-bit computed PCM MD5 must match upstream STREAMINFO MD5 bit-for-bit"
    );

    let report = inspect_and_verify_flac_stream(&flac_path).unwrap();
    assert!(report.verified);
    assert!(report.streaminfo_md5_valid);
    assert_eq!(report.check_mode, "streaminfo_md5");
}

#[test]
fn test_populate_streaminfo_md5_idempotent() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = generate_test_flac(temp_dir.path(), "test_idempotent.flac", 44100, 16, 2);

    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let orig_md5 = tag.get_streaminfo().unwrap().md5.clone();

    // Call populate on an already valid file
    let result = populate_streaminfo_md5(&flac_path).expect("populate failed");
    assert_eq!(result.to_vec(), orig_md5, "Must return existing MD5 without modification");

    let tag_after = metaflac::Tag::read_from_path(&flac_path).unwrap();
    assert_eq!(tag_after.get_streaminfo().unwrap().md5, orig_md5);
}

#[test]
fn test_corrupted_flac_stream_fails_verification() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = generate_test_flac(temp_dir.path(), "test_corrupt.flac", 44100, 16, 2);

    // Corrupt audio samples in the middle/end of the file
    let file_len = fs::metadata(&flac_path).unwrap().len();
    assert!(file_len > 1000);

    {
        use std::io::{Seek, SeekFrom, Write};
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&flac_path)
            .unwrap();
        // Seek into the audio stream (past metadata)
        file.seek(SeekFrom::Start(file_len - 500)).unwrap();
        file.write_all(&[0xFF; 200]).unwrap();
        file.flush().unwrap();
    }

    // Verification must fail (either MD5 mismatch or decode error)
    let result = verify_flac_integrity_stream(&flac_path);
    assert!(result.is_err(), "Corrupted audio stream must fail integrity verification");
}

#[test]
fn test_synthetic_pure_header_flac_md5_preservation() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = temp_dir.path().join("synthetic_header.flac");

    let custom_md5 = [0x5A; 16];

    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[
        0x80, 0x00, 0x00, 0x22, // Last metadata block (STREAMINFO), length 34
        0x10, 0x00, 0x10, 0x00, // min/max block size
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // min/max frame size
        0x0A, 0xC4, 0x42, 0xF0, // 44.1kHz, 2 channels, 16 bits, 0 samples
        0x00, 0x00, 0x00, 0x00,
    ]);
    flac_bytes.extend_from_slice(&custom_md5);
    fs::write(&flac_path, &flac_bytes).expect("Failed to write synthetic FLAC");

    let meta = FlacMetadata {
        title: "Synthetic Preservation".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        ..Default::default()
    };
    apply_flac_tags(&flac_path, &meta).expect("apply_flac_tags failed on synthetic header");

    let tag = metaflac::Tag::read_from_path(&flac_path).expect("Failed to read tagged synthetic FLAC");
    let si = tag.get_streaminfo().expect("STREAMINFO missing");
    assert_eq!(
        si.md5,
        custom_md5.to_vec(),
        "Custom STREAMINFO MD5 must be preserved intact"
    );
}

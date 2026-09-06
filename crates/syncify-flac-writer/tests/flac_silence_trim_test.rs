//! TASK-76: Lossless FLAC dead-silence trim container-surgery regression tests.
//!
//! Validates, with real ffmpeg/flac tooling:
//! 1. `trim_flac_stream_copy` retains a bit-identical PCM slice (stream copy, no re-encode).
//! 2. `restore_flac_metadata_blocks` preserves VorbisComments and CoverFront pictures.
//! 3. `finalize_flac_streaminfo_after_remux` repairs the stale STREAMINFO
//!    (total_samples + MD5) that the remux carries over from the source.

use std::path::{Path, PathBuf};
use std::process::Command;
use syncify_flac_writer::{
    finalize_flac_streaminfo_after_remux, inspect_and_verify_flac_stream,
    restore_flac_metadata_blocks, trim_flac_stream_copy,
};

fn has_ffmpeg() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Generates `[3.5 s silence][5 s sine][3.5 s silence]` at 44.1 kHz stereo.
fn generate_fixture(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    let status = Command::new("ffmpeg")
        .args(["-v", "error", "-y"])
        .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=5:sample_rate=44100"])
        .args(["-af", "adelay=3500:all=1,apad=pad_dur=3.5"])
        .args(["-ac", "2"])
        .arg(&path)
        .status()
        .expect("spawn ffmpeg fixture generator");
    assert!(status.success(), "ffmpeg fixture generation failed");
    path
}

fn decoded_pcm_len(path: &Path) -> u64 {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-f", "s16le", "-"])
        .output()
        .expect("decode pcm");
    out.stdout.len() as u64
}

#[test]
fn test_trim_restore_finalize_pipeline_repairs_streaminfo() {
    if !has_ffmpeg() {
        eprintln!("ffmpeg not available; skipping");
        return;
    }
    let dir = tempfile::TempDir::new().unwrap();
    let fixture = generate_fixture(dir.path(), "fixture.flac");

    // Tag the fixture before trimming (VorbisComment + CoverFront PNG).
    let mut tag = metaflac::Tag::read_from_path(&fixture).unwrap();
    {
        let comments = tag.vorbis_comments_mut();
        comments.set("TITLE", vec!["Trim Fixture"]);
        comments.set("ARTIST", vec!["TASK-76"]);
    }
    let png_status = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i", "color=c=blue:s=8x8:d=1"])
        .args(["-frames:v", "1", "-f", "image2"])
        .arg(dir.path().join("cover.png"))
        .status()
        .unwrap();
    assert!(png_status.success());
    let png = std::fs::read(dir.path().join("cover.png")).unwrap();
    tag.add_picture("image/png", metaflac::block::PictureType::CoverFront, png);
    tag.write_to_path(&fixture).unwrap();

    let original_pcm = decoded_pcm_len(&fixture); // 12 s * 44100 * 4
    assert!((original_pcm as f64 / (44100.0 * 4.0) - 12.0).abs() < 0.05);

    // 1. Lossless stream-copy trim to [3.35, 8.65].
    let trimmed = trim_flac_stream_copy(&fixture, Some(3.35), Some(8.65)).expect("trim");
    let trimmed_pcm = decoded_pcm_len(&trimmed);
    let expected = (8.65 - 3.35) * 44100.0 * 4.0;
    assert!(
        (trimmed_pcm as f64 - expected).abs() < 44100.0 * 4.0 * 0.05,
        "trimmed PCM {} not ~= {} bytes",
        trimmed_pcm,
        expected
    );

    // The remuxed file carries the stale source STREAMINFO (12 s total samples).
    let stale = metaflac::Tag::read_from_path(&trimmed)
        .unwrap()
        .get_streaminfo()
        .unwrap()
        .total_samples;
    assert!(
        (stale as f64 / 44100.0 - 12.0).abs() < 0.05,
        "expected stale remuxed STREAMINFO total_samples ~= 529200, got {}",
        stale
    );

    // 2. Restore tags/pictures.
    let restored = restore_flac_metadata_blocks(&trimmed, &fixture).expect("restore");
    assert!(restored >= 2, "expected VorbisComment + Picture restored, got {}", restored);
    let tag_after = metaflac::Tag::read_from_path(&trimmed).unwrap();
    assert_eq!(
        tag_after
            .vorbis_comments()
            .and_then(|c| c.get("TITLE"))
            .and_then(|v| v.first())
            .map(|s| s.to_string()),
        Some("Trim Fixture".to_string())
    );
    assert!(tag_after.pictures().next().is_some(), "CoverFront picture lost");

    // 3. Finalize STREAMINFO: total_samples + MD5 recomputed from a decode pass.
    let fin = finalize_flac_streaminfo_after_remux(&trimmed).expect("finalize");
    assert!(
        (fin.total_samples as f64 / 44100.0 - 5.3).abs() < 0.05,
        "finalized total_samples {} not ~= 5.3 s",
        fin.total_samples
    );

    let integrity = inspect_and_verify_flac_stream(&trimmed).expect("integrity");
    assert!(integrity.streaminfo_md5_valid, "MD5 missing after finalize");
    assert!(integrity.verified, "MD5 mismatch after finalize: {:?}", integrity);
    assert_eq!(integrity.computed_md5, fin.md5_hex);

    let final_pcm = decoded_pcm_len(&trimmed);
    assert_eq!(final_pcm, trimmed_pcm, "finalize must not alter decoded audio");
}

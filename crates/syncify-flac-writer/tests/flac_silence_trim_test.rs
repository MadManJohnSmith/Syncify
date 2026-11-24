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
        .args([
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=5:sample_rate=44100",
        ])
        .args(["-af", "adelay=3500:all=1,apad=pad_dur=3.5"])
        .args(["-ac", "2"])
        .arg(&path)
        .status()
        .expect("spawn ffmpeg fixture generator");
    assert!(status.success(), "ffmpeg fixture generation failed");
    path
}

fn decoded_pcm(path: &Path) -> Vec<u8> {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-f", "s16le", "-"])
        .output()
        .expect("decode pcm");
    out.stdout
}

fn decoded_pcm_len(path: &Path) -> u64 {
    decoded_pcm(path).len() as u64
}

/// True si `needle` aparece como slice contiguo de `haystack`.
///
/// La sonda corta evita comparar el needle entero en cada ventana: el
/// stream copy tiene que producir un slice bit a bit del PCM original.
fn contains_contiguous_slice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    let probe = &needle[..needle.len().min(64)];
    let mut search_from = 0;
    while let Some(offset) = haystack[search_from..]
        .windows(probe.len())
        .position(|window| window == probe)
    {
        let start = search_from + offset;
        if haystack[start..].starts_with(needle) {
            return true;
        }
        search_from = start + 1;
    }
    false
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
        .args([
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=8x8:d=1",
        ])
        .args(["-frames:v", "1", "-f", "image2"])
        .arg(dir.path().join("cover.png"))
        .status()
        .unwrap();
    assert!(png_status.success());
    let png = std::fs::read(dir.path().join("cover.png")).unwrap();
    tag.add_picture("image/png", metaflac::block::PictureType::CoverFront, png);
    tag.write_to_path(&fixture).unwrap();

    let original_pcm = decoded_pcm(&fixture); // 12 s * 44100 * 4
    assert!((original_pcm.len() as f64 / (44100.0 * 4.0) - 12.0).abs() < 0.05);

    // 1. Lossless stream-copy trim to [3.35, 8.65].
    let trimmed = trim_flac_stream_copy(&fixture, Some(3.35), Some(8.65)).expect("trim");
    let trimmed_pcm = decoded_pcm(&trimmed);
    let expected = (8.65 - 3.35) * 44100.0 * 4.0;
    // El stream copy corta en frontera de frame FLAC: no puede partir una
    // muestra por dentro, así que el peor caso pierde un frame en cada extremo.
    // El tamaño máximo lo declara el STREAMINFO del propio fichero (4608
    // muestras es el tope del subset FLAC); los ffmpeg de CI y local eligen
    // frames distintos y un margen de 50 ms era menor que un frame.
    let max_block = metaflac::Tag::read_from_path(&trimmed)
        .unwrap()
        .get_streaminfo()
        .unwrap()
        .max_block_size as f64;
    let tolerance_bytes = 2.0 * max_block * 4.0; // 2 canales, 16 bits
    assert!(
        (trimmed_pcm.len() as f64 - expected).abs() <= tolerance_bytes,
        "trimmed PCM {} not within 2 FLAC frames of {} bytes (block={} muestras)",
        trimmed_pcm.len(),
        expected,
        max_block
    );
    // Y lo que sí es exacto en un stream copy: el PCM trimado es un slice
    // contiguo bit a bit del original, sin recodificación.
    assert!(
        contains_contiguous_slice(&original_pcm, &trimmed_pcm),
        "trimmed PCM is not a contiguous, bit-identical slice of the source"
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
    assert!(
        restored >= 2,
        "expected VorbisComment + Picture restored, got {}",
        restored
    );
    let tag_after = metaflac::Tag::read_from_path(&trimmed).unwrap();
    assert_eq!(
        tag_after
            .vorbis_comments()
            .and_then(|c| c.get("TITLE"))
            .and_then(|v| v.first())
            .map(|s| s.to_string()),
        Some("Trim Fixture".to_string())
    );
    assert!(
        tag_after.pictures().next().is_some(),
        "CoverFront picture lost"
    );

    // 3. Finalize STREAMINFO: total_samples + MD5 recomputed from a decode pass.
    let fin = finalize_flac_streaminfo_after_remux(&trimmed).expect("finalize");
    let tolerance_seconds = 2.0 * max_block / 44100.0;
    assert!(
        (fin.total_samples as f64 / 44100.0 - 5.3).abs() <= tolerance_seconds,
        "finalized total_samples {} not within 2 FLAC frames of 5.3 s",
        fin.total_samples
    );

    let integrity = inspect_and_verify_flac_stream(&trimmed).expect("integrity");
    assert!(integrity.streaminfo_md5_valid, "MD5 missing after finalize");
    assert!(
        integrity.verified,
        "MD5 mismatch after finalize: {:?}",
        integrity
    );
    assert_eq!(integrity.computed_md5, fin.md5_hex);

    let final_pcm = decoded_pcm_len(&trimmed);
    assert_eq!(
        final_pcm,
        trimmed_pcm.len() as u64,
        "finalize must not alter decoded audio"
    );
}

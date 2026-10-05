//! A truncated transfer must degrade to "unknown metrics", never take the worker
//! down.
//!
//! `inspect_physical_audio_file` is now called on whatever the manifest writer
//! finds next to a completed download, including partially written files left
//! behind by a crash. `metaflac` walks the metadata block chain by slicing the
//! file, so it panicked (`range end index 34 out of range for slice of length 32`)
//! on a stub whose declared STREAMINFO block never arrived.

use std::path::Path;
use syncify_tauri_lib::download::audio_inspector::inspect_physical_audio_file;
use tempfile::TempDir;

/// The exact stub shape that used to panic: `fLaC` magic plus a block header that
/// announces a 34-byte STREAMINFO the file never carries.
const TRUNCATED_FLAC: &[u8] =
    b"fLaC\x00\x00\x00\"\x10\x00\x10\x00\x00\x00\x00\x00\x00\x00\x0a\xc4\x42\xf0\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

fn complete_flac() -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"fLaC");
    bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // last STREAMINFO block, 34 bytes
    bytes.extend_from_slice(&4096u16.to_be_bytes());
    bytes.extend_from_slice(&4096u16.to_be_bytes());
    bytes.extend_from_slice(&[0u8; 3]);
    bytes.extend_from_slice(&[0u8; 3]);
    let packed: u64 = (44100u64 << 44) | (1u64 << 41) | (15u64 << 36);
    bytes.extend_from_slice(&packed.to_be_bytes());
    bytes.extend_from_slice(&[0u8; 16]);
    bytes
}

#[test]
fn inspecting_a_truncated_flac_returns_instead_of_panicking() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("cut-short.flac");
    std::fs::write(&path, TRUNCATED_FLAC).unwrap();

    // No assertion on the value: the contract under test is "does not panic and
    // does not invent metrics for a file that carries none".
    let _ = inspect_physical_audio_file(Path::new(&path));
}

#[test]
fn inspecting_a_flac_shorter_than_the_magic_does_not_panic() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("almost-empty.flac");
    std::fs::write(&path, b"fLaC").unwrap();

    let _ = inspect_physical_audio_file(Path::new(&path));
}

#[test]
fn a_complete_flac_is_still_measured() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("complete.flac");
    std::fs::write(&path, complete_flac()).unwrap();

    let meta = inspect_physical_audio_file(Path::new(&path)).expect("metadata for a complete FLAC");

    assert_eq!(meta.format, "FLAC");
    assert_eq!(meta.sample_rate, 44100);
    assert_eq!(meta.bit_depth, 16);
    assert_eq!(meta.channels, 2);
}

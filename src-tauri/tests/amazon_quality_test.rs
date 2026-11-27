//! D13 (#125): Amazon quality validation against the real DoubleDouble shape.
//!
//! The public status payload observed in 2026-11 has no quality fields. These
//! tests pin both halves of the contract:
//!
//! - a status payload without quality is parsed but does NOT invent 24/48;
//! - a payload that does declare format/bitDepth/sampleRate is accepted;
//! - incomplete, invalid, or malformed payloads fail with an explicit error;
//! - the delivered file is the authoritative quality; a physical FLAC fixture
//!   overrides a different declaration, and an uninspectable file with no
//!   declaration fails closed.

use std::path::{Path, PathBuf};

use syncify_tauri_lib::download::amazon::{
    effective_quality, parse_status_payload, AmazonDeclaredQuality,
};
use syncify_tauri_lib::download::audio_inspector::inspect_physical_audio_file;

const STATUS_DONE_UNDECLARED: &str =
    include_str!("fixtures/amazon/status_done_undeclared_quality.json");
const STATUS_DONE_DECLARED: &str =
    include_str!("fixtures/amazon/status_done_declared_quality.json");
const STATUS_DONE_NESTED: &str = include_str!("fixtures/amazon/status_done_nested_quality.json");
const STATUS_DONE_PARTIAL: &str = include_str!("fixtures/amazon/status_done_partial_quality.json");
const STATUS_DONE_INVALID_QUALITY: &str =
    include_str!("fixtures/amazon/status_done_invalid_quality.json");
const STATUS_DONE_WITHOUT_URL: &str = include_str!("fixtures/amazon/status_done_without_url.json");
const STATUS_ERROR: &str = include_str!("fixtures/amazon/status_error.json");
const STATUS_MALFORMED: &str = include_str!("fixtures/amazon/status_malformed.json");

fn flac_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/silence.flac")
}

#[test]
fn declared_quality_is_extracted_from_the_status_payload() {
    let status = parse_status_payload(STATUS_DONE_DECLARED).expect("valid status payload");
    assert_eq!(status.status, "done");
    assert_eq!(
        status.url.as_deref(),
        Some("https://files.example.test/amazon/track.flac")
    );
    assert_eq!(
        status.declared_quality,
        Some(AmazonDeclaredQuality {
            format: "FLAC".to_string(),
            bit_depth: 24,
            sample_rate: 96_000,
        })
    );
}

#[test]
fn declared_quality_is_extracted_from_current_track() {
    let status = parse_status_payload(STATUS_DONE_NESTED).expect("valid status payload");
    let quality = status
        .declared_quality
        .expect("current.bit_depth/current.sample_rate are evidence");
    assert_eq!(quality.format, "FLAC");
    assert_eq!(quality.bit_depth, 16);
    assert_eq!(quality.sample_rate, 44_100);
}

/// The real payload shape (status/friendlyStatus/current/url) is valid even
/// though it declares no quality. It must remain usable for the job lifecycle
/// while quality comes from the delivered file.
#[test]
fn real_shaped_payload_without_quality_is_valid_but_does_not_invent_values() {
    let status = parse_status_payload(STATUS_DONE_UNDECLARED).expect("valid status payload");
    assert_eq!(status.status, "done");
    assert!(status.declared_quality.is_none());

    let completion = status.into_completion().expect("terminal done with url");
    let err = effective_quality(completion.declared_quality.as_ref(), None)
        .expect_err("sin payload ni fichero no hay calidad que declarar");
    let message = err.to_string();
    assert!(message.contains("not declared"), "mensaje: {message}");
    assert!(
        !message.contains("24-bit") && !message.contains("48"),
        "no se inventa 24/48: {message}"
    );
}

#[test]
fn partial_quality_declaration_is_a_payload_error() {
    let err = parse_status_payload(STATUS_DONE_PARTIAL).expect_err("falta format y sampleRate");
    assert!(
        err.to_string().contains("incomplete audio quality"),
        "mensaje: {err}"
    );
}

#[test]
fn invalid_quality_values_are_rejected() {
    let err = parse_status_payload(STATUS_DONE_INVALID_QUALITY)
        .expect_err("20 bits no es un bit depth de audio soportado");
    assert!(
        err.to_string().contains("invalid bit depth 20"),
        "mensaje: {err}"
    );
}

#[test]
fn malformed_payload_fails_closed() {
    let err = parse_status_payload(STATUS_MALFORMED).expect_err("JSON inválido");
    assert!(
        err.to_string().contains("payload is invalid"),
        "mensaje: {err}"
    );
}

#[test]
fn completed_payload_without_url_fails_closed() {
    let status = parse_status_payload(STATUS_DONE_WITHOUT_URL).expect("JSON válido");
    let err = status
        .into_completion()
        .expect_err("done sin url no entrega nada");
    assert!(
        err.to_string().contains("without a download URL"),
        "mensaje: {err}"
    );
}

#[test]
fn backend_error_keeps_its_message() {
    let status = parse_status_payload(STATUS_ERROR).expect("error payload válido");
    assert_eq!(status.status, "error");
    assert_eq!(
        status.message.as_deref(),
        Some("Rip failed on the source side")
    );
}

#[test]
fn delivered_file_overrides_a_different_payload_declaration() {
    let physical =
        inspect_physical_audio_file(&flac_fixture()).expect("silence.flac inspeccionable");
    let declared = AmazonDeclaredQuality {
        format: "FLAC".to_string(),
        bit_depth: 24,
        sample_rate: 96_000,
    };

    let effective = effective_quality(Some(&declared), Some(&physical)).expect("calidad efectiva");
    assert_eq!(effective.format, "FLAC");
    assert_eq!(
        effective.bit_depth, 16,
        "manda el STREAMINFO físico, no lo declarado"
    );
    assert_eq!(
        effective.sample_rate, 8_000,
        "manda el sample rate físico, no 48 kHz"
    );
}

#[test]
fn uninspectable_file_without_declaration_fails_closed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("broken.flac");
    std::fs::write(&path, b"not a FLAC stream").expect("fixture corrupta");

    let physical = inspect_physical_audio_file(&path);
    assert!(physical.is_none(), "un fichero corrupto no declara calidad");

    let err = effective_quality(None, physical.as_ref())
        .expect_err("sin evidencia física ni declarada no se puede completar");
    assert!(err.to_string().contains("not declared"), "mensaje: {err}");
}

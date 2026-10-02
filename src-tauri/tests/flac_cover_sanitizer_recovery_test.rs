//! D-03 / SYNC-AUD-066 — FLAC cover-art sanitization with a real recovery route.
//!
//! Item 5.3 of the post-audit plan. Two things are locked here, because both were the
//! defect:
//!
//! 1. **The sanitizer runs in the app.** `sanitize_flac_pictures` had no caller outside
//!    the test suite, so a PICTURE block removed from a real library file was invisible.
//!    `test_production_write_paths_call_the_cover_sanitizer` is the source-level guard
//!    that keeps it wired into every FLAC write path.
//! 2. **A block that has to leave the file does not simply disappear.** The recovery
//!    route is tried first (re-encode on the host), and the original bytes are preserved
//!    in an external sidecar when even that fails, with the whole outcome appended to
//!    `repair_history`.
//!
//! Fixtures and helpers mirror `flac_picture_compatibility_test.rs`: the synthetic FLAC
//! comes from the runner's FFmpeg, the animated cover is the tracked
//! `fixtures/animated-cover.webp`, and the irreparable payload is the same pure
//! container-validation stub that suite uses (so the sidecar route is asserted on every
//! runner, with no decoder capability involved).

use std::path::{Path, PathBuf};

use metaflac::block::PictureType;
use metaflac::Tag;
use sqlx::sqlite::SqlitePoolOptions;
use syncify_flac_writer::sanitize_flac_pictures_with_recovery;
use syncify_tauri_lib::services::flac_cover_sanitizer::{
    sanitize_and_audit_flac_cover_art, sanitize_flac_cover_art, FlacCoverSanitizeContext,
    UNRECOVERABLE_COVER_SIDECAR_STEM,
};
use syncify_tauri_lib::services::repair_guardrail::extract_audio_content_hash_from_bytes;
use syncify_tauri_lib::services::tag_writer::{
    apply_flac_tags, prepare_flac_picture, FlacMetadata, MAX_EMBEDDED_PICTURE_BYTES,
};
use tempfile::{tempdir, TempDir};

/// Versioned animated WebP (3 ANMF frames), the same fixture the CI preflight decodes.
const ANIMATED_WEBP_FIXTURE: &[u8] = include_bytes!("fixtures/animated-cover.webp");

/// A WebP container that no decoder can turn into a frame: the RIFF/VP8X header is
/// there, the canvas is empty and no image chunk follows. `prepare_flac_picture`
/// rejects it on pure container validation, so the drop is reproducible on every runner.
const IRREPARABLE_WEBP_STUB: &[u8] =
    b"RIFF\x14\x00\x00\x00WEBPVP8X\x0a\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

/// Production FLAC write paths that must sanitize the cover art they just wrote.
/// Each pair is (path relative to the crate root, human label) so a failure names the
/// path that lost the wiring.
const PRODUCTION_WRITE_PATHS: &[(&str, &str)] = &[
    (
        "src/services/tidal_pipeline.rs",
        "tidal_pipeline (post-promotion)",
    ),
    ("src/download/qobuz.rs", "qobuz download finalization"),
    ("src/services/enrichment.rs", "enrichment FLAC re-tag"),
    ("src/commands/tags.rs", "manual tag editor"),
    ("src/download/tidal.rs", "tidal downloader no-DB fallback"),
];

fn create_synthetic_flac(path: &Path) {
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=0.2",
            "-c:a",
            "flac",
            path.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("ffmpeg must execute to create synthetic FLAC");
    assert!(status.success(), "ffmpeg synthetic FLAC creation failed");
}

/// A small, real JPEG produced by the runner's FFmpeg: a valid replacement for a
/// damaged cover block.
fn create_valid_jpeg() -> Vec<u8> {
    let output = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "mandelbrot=size=320x320",
            "-vframes",
            "1",
            "-q:v",
            "4",
            "-f",
            "image2",
            "-c:v",
            "mjpeg",
            "pipe:1",
        ])
        .output()
        .expect("ffmpeg must generate the replacement JPEG");
    assert!(
        output.status.success(),
        "ffmpeg replacement JPEG generation must succeed"
    );
    assert!(
        !output.stdout.is_empty(),
        "replacement JPEG must not be empty"
    );
    output.stdout
}

/// Builds a library FLAC whose only cover block is the irreparable WebP stub, and
/// returns it together with the exact bytes that were embedded.
fn flac_with_irreparable_cover(dir: &TempDir) -> (PathBuf, Vec<u8>) {
    let flac_path = dir.path().join("01 - Irreparable Cover.flac");
    create_synthetic_flac(&flac_path);
    let mut tag = Tag::read_from_path(&flac_path).expect("read FLAC");
    let mut pic = metaflac::block::Picture::new();
    pic.picture_type = PictureType::CoverFront;
    pic.mime_type = "image/webp".to_string();
    pic.width = 0;
    pic.height = 0;
    pic.data = IRREPARABLE_WEBP_STUB.to_vec();
    tag.push_block(metaflac::block::Block::Picture(pic));
    tag.write_to_path(&flac_path)
        .expect("write FLAC with irreparable cover");
    (flac_path, IRREPARABLE_WEBP_STUB.to_vec())
}

async fn create_test_db() -> (sqlx::Pool<sqlx::Sqlite>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("cover_sanitizer_test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    (pool, temp_dir)
}

async fn repair_history_rows(pool: &sqlx::Pool<sqlx::Sqlite>) -> Vec<(String, String, String)> {
    use sqlx::Row;
    sqlx::query("SELECT provenance, result, baseline_validation FROM repair_history ORDER BY id")
        .fetch_all(pool)
        .await
        .expect("query repair_history")
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("provenance"),
                row.get::<String, _>("result"),
                row.get::<String, _>("baseline_validation"),
            )
        })
        .collect()
}

/// Item 5.3, part 1: the sanitizer must be part of the production FLAC write paths.
/// This is the exact regression the audit found — the function existed, was documented
/// as mandatory, and had zero callers in `src-tauri/src`.
#[test]
fn test_production_write_paths_call_the_cover_sanitizer() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for (path, label) in PRODUCTION_WRITE_PATHS {
        let full_path = crate_root.join(path);
        let source = std::fs::read_to_string(&full_path)
            .unwrap_or_else(|e| panic!("{} ({}) must be readable: {}", label, path, e));
        assert!(
            source.contains("flac_cover_sanitizer::sanitize_and_audit_flac_cover_art"),
            "{} ({}) no longer sanitizes the cover art it writes: the D-03 recovery route \
             would be dead code again",
            label,
            path
        );
    }
}

/// Item 5.3, part 2: when the cover cannot be repaired, the bytes are preserved outside
/// the container and the loss is recorded in `repair_history`.
#[tokio::test]
async fn test_unrecoverable_cover_is_preserved_as_sidecar_and_audited() {
    let dir = tempdir().expect("tempdir");
    let (flac_path, stub_bytes) = flac_with_irreparable_cover(&dir);
    let (pool, _db_dir) = create_test_db().await;

    let bytes_before = std::fs::read(&flac_path).expect("read FLAC before sanitization");
    let audio_before = extract_audio_content_hash_from_bytes(&bytes_before)
        .expect("audio payload hash must be computable on a real FLAC");

    let context = FlacCoverSanitizeContext {
        provenance: "test.cover_sanitizer".to_string(),
        download_id: None,
        track_id: Some(4242),
    };
    let outcome = sanitize_and_audit_flac_cover_art(Some(&pool), &flac_path, &context)
        .await
        .expect("sanitization must not fail");

    assert!(outcome.modified(), "the irreparable block leaves the file");
    assert!(
        outcome.cover_art_lost(),
        "the loss must be visible to the caller: {:?}",
        outcome.report.dropped_unrepairable_blocks
    );
    assert_eq!(
        outcome.report.dropped_unrepairable_blocks.len(),
        1,
        "exactly the injected block is reported: {:?}",
        outcome.report.dropped_unrepairable_blocks
    );
    let drop_entry = &outcome.report.dropped_unrepairable_blocks[0];
    assert!(
        drop_entry.contains("CoverFront"),
        "the report must identify the lost block: {}",
        drop_entry
    );
    assert!(
        drop_entry.contains(UNRECOVERABLE_COVER_SIDECAR_STEM),
        "the report must name the sidecar that kept the bytes: {}",
        drop_entry
    );

    // Route 2/2: the original bytes are outside the container, byte for byte.
    assert_eq!(
        outcome.preserved_sidecars.len(),
        1,
        "one sidecar per irrecoverable block"
    );
    let sidecar = &outcome.preserved_sidecars[0];
    assert_eq!(
        sidecar.parent(),
        flac_path.parent(),
        "the sidecar belongs next to the FLAC the user sees"
    );
    let sidecar_name = sidecar
        .file_name()
        .and_then(|n| n.to_str())
        .expect("sidecar name");
    assert!(
        sidecar_name.contains(UNRECOVERABLE_COVER_SIDECAR_STEM),
        "unexpected sidecar name: {}",
        sidecar_name
    );
    assert!(
        sidecar_name.starts_with("01 - Irreparable Cover."),
        "the sidecar is tied to its track: {}",
        sidecar_name
    );
    assert_eq!(
        std::fs::read(sidecar).expect("read preserved sidecar"),
        stub_bytes,
        "the sidecar must hold the exact bytes that left the container"
    );

    // The block is really gone from the container, and the audio is untouched.
    let after = Tag::read_from_path(&flac_path).expect("read FLAC after sanitization");
    assert_eq!(
        after.pictures().count(),
        0,
        "the irrepairable block must not stay embedded"
    );
    let bytes_after = std::fs::read(&flac_path).expect("read FLAC after sanitization");
    assert_ne!(
        compute_test_sha256(&bytes_before),
        compute_test_sha256(&bytes_after),
        "the file was rewritten, which is what the audit row must show"
    );
    assert_eq!(
        extract_audio_content_hash_from_bytes(&bytes_after).expect("audio hash after"),
        audio_before,
        "cover sanitization is a metadata-only edit: the audio payload must be identical"
    );

    // And the whole thing is in the ledger.
    let rows = repair_history_rows(&pool).await;
    assert_eq!(rows.len(), 1, "one audit row per intervention: {:?}", rows);
    assert_eq!(rows[0].0, "test.cover_sanitizer");
    assert_eq!(
        rows[0].1, "partial_success",
        "a lost cover block is not a plain success"
    );
    assert_eq!(
        rows[0].2, "valid",
        "the audio payload hash matched across the sanitization"
    );
    let detail: (Option<String>, Option<String>, Option<String>, String) = sqlx::query_as(
        "SELECT audio_payload_hash_before, audio_payload_hash_after, details_json, actions \
         FROM repair_history LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("query audit detail");
    assert_eq!(
        detail.0, detail.1,
        "audio payload hash must be reported as stable"
    );
    assert!(
        detail.0.is_some(),
        "audio payload hash must be computable here"
    );
    let details = detail.2.expect("details_json must be recorded");
    assert!(
        details.contains("dropped_unrepairable_blocks"),
        "the drop must be in the audit payload: {}",
        details
    );
    assert!(
        details.contains(UNRECOVERABLE_COVER_SIDECAR_STEM),
        "the audit payload must name the sidecar: {}",
        details
    );
    let actions: Vec<String> = serde_json::from_str(&detail.3).expect("actions json");
    assert!(
        actions
            .iter()
            .any(|a| a.starts_with("cover_sidecar_preserved:")),
        "the sidecar action must be auditable: {:?}",
        actions
    );
    assert!(
        actions.iter().any(|a| a.starts_with("cover_art_dropped:")),
        "the loss itself must be auditable: {:?}",
        actions
    );
}

/// SHA-256 of the test inputs, mirroring the service's file hashing without depending
/// on a private helper.
fn compute_test_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// A second pass over an already sanitized file must be inert: no rewrite, no repeated
/// drop, no second sidecar and no second ledger row.
#[tokio::test]
async fn test_second_pass_is_inert() {
    let dir = tempdir().expect("tempdir");
    let (flac_path, _) = flac_with_irreparable_cover(&dir);
    let (pool, _db_dir) = create_test_db().await;
    let context = FlacCoverSanitizeContext {
        provenance: "test.cover_sanitizer".to_string(),
        download_id: None,
        track_id: Some(7),
    };

    let first = sanitize_and_audit_flac_cover_art(Some(&pool), &flac_path, &context)
        .await
        .expect("first pass");
    let bytes_after_first = std::fs::read(&flac_path).expect("read after first pass");
    let sidecars_after_first: Vec<PathBuf> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.contains(UNRECOVERABLE_COVER_SIDECAR_STEM))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(sidecars_after_first.len(), 1);

    let second = sanitize_and_audit_flac_cover_art(Some(&pool), &flac_path, &context)
        .await
        .expect("second pass");
    assert!(
        !second.modified(),
        "an already sanitized file must not be rewritten: {:?}",
        second.report
    );
    assert!(
        second.preserved_sidecars.is_empty(),
        "no block is lost on a second pass, so no sidecar may appear"
    );
    assert!(!second.cover_art_lost());
    assert_eq!(
        second.report,
        syncify_flac_writer::FlacPictureSanitizeReport::default(),
        "an inert pass reports nothing at all: {:?} (first pass was {:?})",
        second.report,
        first.report
    );
    assert_eq!(
        std::fs::read(&flac_path).expect("read after second pass"),
        bytes_after_first,
        "the file bytes must be identical after the second pass"
    );
    let sidecars_after_second: Vec<PathBuf> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.contains(UNRECOVERABLE_COVER_SIDECAR_STEM))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(
        sidecars_after_second, sidecars_after_first,
        "the content-addressed sidecar must not be duplicated"
    );
    assert_eq!(
        repair_history_rows(&pool).await.len(),
        1,
        "an inert pass must not append to the ledger"
    );
}

/// A FLAC written by the production writer carries a compliant block, so the new
/// sanitization call must leave it alone — no rewrite, no ledger noise.
#[tokio::test]
async fn test_compliant_file_is_untouched_and_writes_no_audit_row() {
    let dir = tempdir().expect("tempdir");
    let flac_path = dir.path().join("02 - Compliant Cover.flac");
    create_synthetic_flac(&flac_path);
    let meta = FlacMetadata {
        title: "Compliant Track".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        cover_data: Some(create_valid_jpeg()),
        ..Default::default()
    };
    apply_flac_tags(&flac_path, &meta).expect("apply_flac_tags must succeed");
    let (pool, _db_dir) = create_test_db().await;
    let bytes_before = std::fs::read(&flac_path).expect("read FLAC");

    let context = FlacCoverSanitizeContext {
        provenance: "test.cover_sanitizer".to_string(),
        download_id: None,
        track_id: Some(9),
    };
    let outcome = sanitize_and_audit_flac_cover_art(Some(&pool), &flac_path, &context)
        .await
        .expect("sanitization of a compliant file must not fail");

    assert!(
        !outcome.modified(),
        "a compliant file must not be rewritten: {:?}",
        outcome.report
    );
    assert!(!outcome.cover_art_lost());
    assert!(outcome.preserved_sidecars.is_empty());
    assert_eq!(
        std::fs::read(&flac_path).expect("read FLAC after"),
        bytes_before,
        "the file must be byte-identical"
    );
    assert!(
        repair_history_rows(&pool).await.is_empty(),
        "a no-op pass must not append to the ledger"
    );
    let dir_entries: Vec<PathBuf> = std::fs::read_dir(dir.path())
        .expect("read dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    assert_eq!(
        dir_entries.len(),
        1,
        "no sidecar may be written for a compliant file: {:?}",
        dir_entries
    );
}

/// D-03, route 1/2: when the recovery route produces a block, the cover never leaves the
/// container. Injected here as a deterministic route because the *strict* repair path
/// needs a runner whose FFmpeg can transcode the fixture, which the compatibility suite
/// already documents as a per-runner capability gap.
#[test]
fn test_successful_recovery_route_keeps_the_cover_in_the_file() {
    let dir = tempdir().expect("tempdir");
    let (flac_path, _) = flac_with_irreparable_cover(&dir);
    let replacement_jpeg = create_valid_jpeg();

    let mut recovery = |data: &[u8], _source: &metaflac::block::Picture| {
        // The route is only consulted for the block the strict path gave up on, and
        // only with that block's bytes.
        assert_eq!(
            data, IRREPARABLE_WEBP_STUB,
            "the route sees the offending block"
        );
        prepare_flac_picture(&replacement_jpeg)
    };
    let report = sanitize_flac_pictures_with_recovery(&flac_path, Some(&mut recovery))
        .expect("sanitize_flac_pictures_with_recovery must succeed");

    assert_eq!(report.recovered_blocks, 1, "the route restored one block");
    assert_eq!(
        report.repaired_blocks, 0,
        "the strict path repaired nothing here"
    );
    assert!(
        !report.dropped_cover_art(),
        "a recovered block is not a loss: {:?}",
        report.dropped_unrepairable_blocks
    );
    assert!(report.modified);

    let after = Tag::read_from_path(&flac_path).expect("read FLAC after recovery");
    let pics: Vec<_> = after.pictures().cloned().collect();
    assert_eq!(pics.len(), 1, "the cover art stayed in the file");
    let pic = &pics[0];
    assert_eq!(pic.picture_type, PictureType::CoverFront);
    assert_eq!(pic.mime_type, "image/jpeg");
    assert!(
        pic.width > 0 && pic.height > 0,
        "recovered block has real dimensions"
    );
    assert!(pic.data.len() <= MAX_EMBEDDED_PICTURE_BYTES);
}

/// The host re-encode route is capability dependent by nature (it needs an FFmpeg that
/// can decode the payload), so the contract asserted here is the one that holds on every
/// runner: it either returns a compliant JPEG or a cause — never a bad image.
#[test]
fn test_host_reencode_route_is_either_compliant_or_explicitly_refused() {
    match syncify_flac_writer::salvage_cover_to_jpeg(IRREPARABLE_WEBP_STUB) {
        Ok(jpeg) => panic!(
            "a 24-byte WebP header with no image chunk must never yield a picture ({} bytes)",
            jpeg.len()
        ),
        Err(cause) => assert!(!cause.trim().is_empty(), "a refusal must say why"),
    }

    match syncify_flac_writer::salvage_cover_to_jpeg(ANIMATED_WEBP_FIXTURE) {
        Ok(jpeg) => {
            let dims = syncify_flac_writer::extract_image_dimensions(&jpeg);
            assert!(
                dims.0 > 0 && dims.1 > 0,
                "a salvaged image must have real dimensions, got {:?}",
                dims
            );
            assert!(
                jpeg.len() <= MAX_EMBEDDED_PICTURE_BYTES,
                "a salvaged image must respect the embedding limit ({} bytes)",
                jpeg.len()
            );
            let block = prepare_flac_picture(&jpeg)
                .expect("a salvaged JPEG must satisfy the embedding contract");
            assert_eq!(block.mime_type, "image/jpeg");
        }
        Err(cause) => {
            eprintln!(
                "this runner's ffmpeg cannot salvage the animated WebP fixture ({}); \
                 asserting the refusal contract only",
                cause
            );
        }
    }
}

/// The service wrapper must expose exactly the same report the crate produced, with the
/// sidecars it produced — the pipeline branch logs on these two methods.
#[test]
fn test_service_outcome_exposes_recovery_and_loss() {
    let dir = tempdir().expect("tempdir");
    let (flac_path, _) = flac_with_irreparable_cover(&dir);
    let outcome = sanitize_flac_cover_art(&flac_path).expect("sanitize_flac_cover_art");

    assert!(outcome.cover_art_lost());
    assert!(!outcome.cover_art_recovered());
    assert_eq!(outcome.preserved_sidecars.len(), 1);
    assert_eq!(
        outcome.report.dropped_unrepairable_blocks.len(),
        1,
        "the crate report reaches the service unchanged"
    );
}

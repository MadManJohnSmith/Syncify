//! TASK-7.1 / CR-5: the country/region repair is exposed (dry-run + apply) and it
//! lands exactly where the tag writers would have written it.
//!
//! The dry run must not touch the file; the apply must produce the wire values of
//! `syncify_metadata_domain` and leave every other tag alone.

use std::fs;
use std::path::PathBuf;
use syncify_flac_writer::FlacMetadata;
use syncify_tauri_lib::commands::tags::run_country_tag_repair_on_path;
use tempfile::TempDir;

struct FlacGuard {
    path: PathBuf,
}
impl Drop for FlacGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn create_synthetic_flac(dir: &std::path::Path, name: &str) -> FlacGuard {
    let path = dir.join(name);
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[
        0x80, 0x00, 0x00, 0x22, // last metadata block (STREAMINFO), length 34
        0x10, 0x00, 0x10, 0x00, // min/max block size
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // min/max frame size
        0x0A, 0xC4, 0x42, 0xF0, // 44.1kHz, 2 channels, 16 bits, 0 samples
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    fs::write(&path, &flac_bytes).expect("synthetic FLAC");
    FlacGuard { path }
}

fn tag_value(path: &std::path::Path, key: &str) -> Option<String> {
    let tag = metaflac::Tag::read_from_path(path).expect("read");
    tag.get_vorbis(key)
        .and_then(|mut it| it.next().map(|s| s.to_string()))
}

#[test]
fn dry_run_reports_the_plan_and_never_touches_the_file() {
    let dir = TempDir::new().unwrap();
    let flac = create_synthetic_flac(dir.path(), "legacy_alpha2.flac");
    // Pre-2026-08-24 writer contract: ISO alpha-2 on the wire.
    syncify_flac_writer::apply_flac_tags(
        &flac.path,
        &FlacMetadata {
            title: "Heroes".to_string(),
            artist: "David Bowie".to_string(),
            album: "Heroes".to_string(),
            release_country: Some("US".to_string()),
            ..Default::default()
        },
    )
    .expect("seed tags");

    // Rewrite the country tag to the legacy alpha-2 form the repair must fix.
    let mut tag = metaflac::Tag::read_from_path(&flac.path).unwrap();
    tag.set_vorbis("RELEASECOUNTRY", vec!["US".to_string()]);
    tag.set_vorbis("COUNTRY", vec!["US".to_string()]);
    tag.write_to_path(&flac.path).unwrap();

    let report = run_country_tag_repair_on_path(1, flac.path.to_str().unwrap(), false)
        .expect("dry run must succeed");

    assert!(!report.applied);
    assert!(report.needs_repair);
    assert_eq!(report.plan.target_country.as_deref(), Some("United States"));
    assert!(
        report.applied_tags.is_empty(),
        "dry run reports nothing applied"
    );
    // The file is untouched.
    assert_eq!(
        tag_value(&flac.path, "RELEASECOUNTRY").as_deref(),
        Some("US")
    );
}

#[test]
fn apply_writes_the_canonical_country_and_keeps_every_other_tag() {
    let dir = TempDir::new().unwrap();
    let flac = create_synthetic_flac(dir.path(), "apply_alpha2.flac");
    syncify_flac_writer::apply_flac_tags(
        &flac.path,
        &FlacMetadata {
            title: "Heroes".to_string(),
            artist: "David Bowie".to_string(),
            album: "Heroes".to_string(),
            isrc: Some("GBAYE7700021".to_string()),
            track_number: 3,
            ..Default::default()
        },
    )
    .expect("seed tags");
    let mut tag = metaflac::Tag::read_from_path(&flac.path).unwrap();
    tag.set_vorbis("RELEASECOUNTRY", vec!["US".to_string()]);
    tag.set_vorbis("COUNTRY", vec!["US".to_string()]);
    tag.write_to_path(&flac.path).unwrap();

    let report =
        run_country_tag_repair_on_path(1, flac.path.to_str().unwrap(), true).expect("apply");

    assert!(report.applied);
    assert!(report.needs_repair);
    assert_eq!(report.plan.target_country.as_deref(), Some("United States"));
    assert_eq!(
        report.applied_tags.get("RELEASECOUNTRY"),
        Some(&vec!["United States".to_string()])
    );

    // On disk: both country tags carry the canonical name...
    assert_eq!(
        tag_value(&flac.path, "RELEASECOUNTRY").as_deref(),
        Some("United States")
    );
    assert_eq!(
        tag_value(&flac.path, "COUNTRY").as_deref(),
        Some("United States")
    );
    // ...RELEASEREGION is untouched (no region on this release)...
    assert_eq!(tag_value(&flac.path, "RELEASEREGION"), None);
    // ...and unrelated tags survive the repair.
    assert_eq!(
        tag_value(&flac.path, "ISRC").as_deref(),
        Some("GBAYE7700021")
    );
    assert_eq!(tag_value(&flac.path, "TITLE").as_deref(), Some("Heroes"));
    assert_eq!(tag_value(&flac.path, "TRACKNUMBER").as_deref(), Some("3"));
}

#[test]
fn apply_completes_the_region_pair_without_dropping_the_country_tag() {
    let dir = TempDir::new().unwrap();
    let flac = create_synthetic_flac(dir.path(), "region_pair.flac");
    syncify_flac_writer::apply_flac_tags(
        &flac.path,
        &FlacMetadata {
            title: "Nightcall".to_string(),
            artist: "Kavinsky".to_string(),
            album: "OutRun".to_string(),
            ..Default::default()
        },
    )
    .expect("seed tags");
    let mut tag = metaflac::Tag::read_from_path(&flac.path).unwrap();
    tag.set_vorbis("RELEASECOUNTRY", vec!["Europe".to_string()]);
    tag.set_vorbis("COUNTRY", vec!["Europe".to_string()]);
    tag.remove_vorbis("RELEASEREGION");
    tag.write_to_path(&flac.path).unwrap();

    let report =
        run_country_tag_repair_on_path(2, flac.path.to_str().unwrap(), true).expect("apply");

    assert!(report.applied);
    assert_eq!(report.plan.target_country.as_deref(), Some("Europe"));
    assert_eq!(report.plan.target_region.as_deref(), Some("XE"));
    assert_eq!(
        tag_value(&flac.path, "RELEASECOUNTRY").as_deref(),
        Some("Europe")
    );
    assert_eq!(
        tag_value(&flac.path, "RELEASEREGION").as_deref(),
        Some("XE")
    );
}

#[test]
fn a_file_already_on_the_wire_contract_is_left_alone() {
    let dir = TempDir::new().unwrap();
    let flac = create_synthetic_flac(dir.path(), "already_ok.flac");
    syncify_flac_writer::apply_flac_tags(
        &flac.path,
        &FlacMetadata {
            title: "Heroes".to_string(),
            artist: "David Bowie".to_string(),
            album: "Heroes".to_string(),
            release_country: Some("United States".to_string()),
            ..Default::default()
        },
    )
    .expect("seed tags");

    let report =
        run_country_tag_repair_on_path(3, flac.path.to_str().unwrap(), true).expect("apply");

    assert!(
        !report.applied,
        "nothing to repair must not rewrite the file"
    );
    assert!(!report.needs_repair);
    assert_eq!(
        tag_value(&flac.path, "RELEASECOUNTRY").as_deref(),
        Some("United States")
    );
}

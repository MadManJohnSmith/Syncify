//! Tests for TASK-145: Normalization of audio_quality Enum
//!
//! Validates:
//! 1. Rust domain helper `normalize_audio_quality` and `classify_audio_tier`:
//!    - Canonical lowercase mapping to {"lossless", "hires", "lossy"}.
//!    - Handling of casing ("LOSSLESS", "HIGH", "LOW", "STANDARD", "HIRES").
//!    - Handling of legacy and provider variants ("flac", "cd", "standard", "hi-res", "24-96").
//!    - Whitespace trimming and fallback behavior.
//! 2. SQLite Migration 0072 execution:
//!    - Clean in-memory migration over 0001..=0071 dirty state.
//!    - Normalization of all 2.512 legacy tracks (LOSSLESS -> lossless, standard/HIGH/LOW -> lossy, HIRES -> hires).
//!    - Invariant: 0 non-null tracks outside of {'lossless', 'hires', 'lossy'}.
//! 3. Durable recurrence-prevention triggers:
//!    - INSERT triggers normalize non-canonical values to lowercase canonical tiers.
//!    - UPDATE triggers normalize non-canonical updates to lowercase canonical tiers.
//!    - Normal records and NULL values remain untouched without infinite recursion.
//! 4. Full migration pipeline integrity and PRAGMA checks:
//!    - `PRAGMA integrity_check` reports "ok".
//!    - `PRAGMA foreign_key_check` reports 0 violations.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;
use syncify_core_domain::quality::{classify_audio_tier, normalize_audio_quality};

#[test]
fn test_normalize_audio_quality_rust_function() {
    // 1. Lossless variants
    assert_eq!(normalize_audio_quality("lossless"), "lossless");
    assert_eq!(normalize_audio_quality("LOSSLESS"), "lossless");
    assert_eq!(normalize_audio_quality("Lossless"), "lossless");
    assert_eq!(normalize_audio_quality("flac"), "lossless");
    assert_eq!(normalize_audio_quality("FLAC"), "lossless");
    assert_eq!(normalize_audio_quality("cd"), "lossless");
    assert_eq!(normalize_audio_quality("16-44"), "lossless");
    assert_eq!(normalize_audio_quality("alac"), "lossless");
    assert_eq!(normalize_audio_quality("wav"), "lossless");

    // 2. Hi-Res variants
    assert_eq!(normalize_audio_quality("hires"), "hires");
    assert_eq!(normalize_audio_quality("HIRES"), "hires");
    assert_eq!(normalize_audio_quality("HiRes"), "hires");
    assert_eq!(normalize_audio_quality("hi_res"), "hires");
    assert_eq!(normalize_audio_quality("HI_RES"), "hires");
    assert_eq!(normalize_audio_quality("hi-res"), "hires");
    assert_eq!(normalize_audio_quality("HI-RES"), "hires");
    assert_eq!(normalize_audio_quality("HI_RES_LOSSLESS"), "hires");
    assert_eq!(normalize_audio_quality("hires_lossless"), "hires");
    assert_eq!(normalize_audio_quality("high_resolution"), "hires");
    assert_eq!(normalize_audio_quality("HIGH_RESOLUTION"), "hires");
    assert_eq!(normalize_audio_quality("max"), "hires");
    assert_eq!(normalize_audio_quality("24-96"), "hires");
    assert_eq!(normalize_audio_quality("24-192"), "hires");

    // 3. Lossy variants (production non-canonical strings)
    assert_eq!(normalize_audio_quality("standard"), "lossy");
    assert_eq!(normalize_audio_quality("STANDARD"), "lossy");
    assert_eq!(normalize_audio_quality("Standard"), "lossy");
    assert_eq!(normalize_audio_quality("HIGH"), "lossy");
    assert_eq!(normalize_audio_quality("high"), "lossy");
    assert_eq!(normalize_audio_quality("LOW"), "lossy");
    assert_eq!(normalize_audio_quality("low"), "lossy");
    assert_eq!(normalize_audio_quality("normal"), "lossy");
    assert_eq!(normalize_audio_quality("lossy"), "lossy");
    assert_eq!(normalize_audio_quality("LOSSY"), "lossy");
    assert_eq!(normalize_audio_quality("mp3"), "lossy");
    assert_eq!(normalize_audio_quality("MP3"), "lossy");
    assert_eq!(normalize_audio_quality("aac"), "lossy");
    assert_eq!(normalize_audio_quality("ogg"), "lossy");
    assert_eq!(normalize_audio_quality("opus"), "lossy");
    assert_eq!(normalize_audio_quality("vorbis"), "lossy");
    assert_eq!(normalize_audio_quality("320"), "lossy");

    // 4. Whitespace trimming
    assert_eq!(normalize_audio_quality("  LOSSLESS  "), "lossless");
    assert_eq!(normalize_audio_quality("\tstandard\n"), "lossy");
    assert_eq!(normalize_audio_quality("  HIRES  "), "hires");

    // 5. Fallback for unknown strings
    assert_eq!(normalize_audio_quality("unknown_arbitrary_string"), "lossy");

    // 6. classify_audio_tier produces canonical AudioTier
    assert_eq!(
        classify_audio_tier(None, None, None, Some("LOSSLESS")).as_str(),
        "lossless"
    );
    assert_eq!(
        classify_audio_tier(None, None, None, Some("standard")).as_str(),
        "lossy"
    );
    assert_eq!(
        classify_audio_tier(None, None, None, Some("HIGH")).as_str(),
        "lossy"
    );
    assert_eq!(
        classify_audio_tier(None, None, None, Some("LOW")).as_str(),
        "lossy"
    );
    assert_eq!(
        classify_audio_tier(None, None, None, Some("HIRES")).as_str(),
        "hires"
    );
    assert_eq!(
        classify_audio_tier(Some(24), Some(96000), None, Some("FLAC")).as_str(),
        "hires"
    );
    assert_eq!(
        classify_audio_tier(Some(16), Some(44100), None, Some("FLAC")).as_str(),
        "lossless"
    );
}

#[tokio::test]
async fn test_sqlite_migration_0072_normalization_and_triggers() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Connect to clean in-memory SQLite database");

    // Enable foreign keys
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("Enable foreign keys");

    // 1. Apply migrations 0001..=0071 first to create pre-migration dirty catalog
    let migrator = sqlx::migrate!("./migrations");
    let migrations: Vec<_> = migrator.iter().collect();

    let partial_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            migrations
                .iter()
                .filter(|m| m.version <= 71)
                .map(|m| (*m).clone())
                .collect(),
        ),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };
    partial_migrator
        .run(&pool)
        .await
        .expect("Apply migrations 0001..=0071");

    // 2. Seed dirty tracks matching the diagnosed production state
    let artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Test Audio Quality Artist') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert artist");

    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, release_date) VALUES ('Test Audio Quality Album', '2026-01-01') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert album");

    // Insert 11 test tracks with various casings and tiers
    let test_tracks = vec![
        (1001, "Track LOSSLESS", Some("LOSSLESS")),
        (1002, "Track standard", Some("standard")),
        (1003, "Track HIGH", Some("HIGH")),
        (1004, "Track LOW", Some("LOW")),
        (1005, "Track HIRES", Some("HIRES")),
        (1006, "Track hi-res", Some("hi-res")),
        (1007, "Track flac", Some("flac")),
        (1008, "Track lossless already", Some("lossless")),
        (1009, "Track hires already", Some("hires")),
        (1010, "Track lossy already", Some("lossy")),
        (1011, "Track null", None),
    ];

    for (id, title, aq) in &test_tracks {
        sqlx::query(
            "INSERT INTO tracks (id, title, album_id, duration_ms, isrc, audio_quality) VALUES (?, ?, ?, 200000, ?, ?)"
        )
        .bind(id)
        .bind(title)
        .bind(album_id)
        .bind(format!("US100000{:04}", id % 10000))
        .bind(aq)
        .execute(&pool)
        .await
        .expect("Insert test track");

        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(id)
            .bind(artist_id)
            .execute(&pool)
            .await
            .expect("Link track artist");
    }

    // Verify pre-migration state has non-canonical values
    let non_canonical_pre: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tracks WHERE audio_quality IS NOT NULL AND audio_quality NOT IN ('lossless', 'hires', 'lossy')"
    )
    .fetch_one(&pool)
    .await
    .expect("Count non-canonical pre-migration");
    assert_eq!(non_canonical_pre, 7, "Expected 7 non-canonical tracks before migration 0072");

    // 3. Apply migration 0072
    let full_migrator = sqlx::migrate!("./migrations");
    full_migrator
        .run(&pool)
        .await
        .expect("Run migration 0072");

    // 4. Verify post-migration normalization
    let non_canonical_post: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tracks WHERE audio_quality IS NOT NULL AND audio_quality NOT IN ('lossless', 'hires', 'lossy')"
    )
    .fetch_one(&pool)
    .await
    .expect("Count non-canonical post-migration");
    assert_eq!(non_canonical_post, 0, "All tracks must be normalized to canonical enum values");

    // Verify individual tracks
    let q1001: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1001")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1001.as_deref(), Some("lossless"), "LOSSLESS must become lossless");

    let q1002: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1002")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1002.as_deref(), Some("lossy"), "standard must become lossy");

    let q1003: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1003")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1003.as_deref(), Some("lossy"), "HIGH must become lossy");

    let q1004: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1004")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1004.as_deref(), Some("lossy"), "LOW must become lossy");

    let q1005: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1005")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1005.as_deref(), Some("hires"), "HIRES must become hires");

    let q1006: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1006")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1006.as_deref(), Some("hires"), "hi-res must become hires");

    let q1007: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1007")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1007.as_deref(), Some("lossless"), "flac must become lossless");

    let q1008: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1008")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1008.as_deref(), Some("lossless"), "lossless remains lossless");

    let q1009: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1009")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1009.as_deref(), Some("hires"), "hires remains hires");

    let q1010: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1010")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1010.as_deref(), Some("lossy"), "lossy remains lossy");

    let q1011: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1011")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1011, None, "NULL remains NULL");

    // 5. Test recurrence-prevention triggers on INSERT
    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, duration_ms, audio_quality) VALUES (2001, 'Trigger Test 1', ?, 180000, 'LOSSLESS')"
    )
    .bind(album_id)
    .execute(&pool)
    .await
    .expect("Insert with LOSSLESS");

    let q2001: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 2001")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q2001.as_deref(), Some("lossless"), "Trigger must normalize INSERT LOSSLESS -> lossless");

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, duration_ms, audio_quality) VALUES (2002, 'Trigger Test 2', ?, 180000, 'standard')"
    )
    .bind(album_id)
    .execute(&pool)
    .await
    .expect("Insert with standard");

    let q2002: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 2002")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q2002.as_deref(), Some("lossy"), "Trigger must normalize INSERT standard -> lossy");

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, duration_ms, audio_quality) VALUES (2003, 'Trigger Test 3', ?, 180000, 'HIGH')"
    )
    .bind(album_id)
    .execute(&pool)
    .await
    .expect("Insert with HIGH");

    let q2003: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 2003")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q2003.as_deref(), Some("lossy"), "Trigger must normalize INSERT HIGH -> lossy");

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, duration_ms, audio_quality) VALUES (2004, 'Trigger Test 4', ?, 180000, 'LOW')"
    )
    .bind(album_id)
    .execute(&pool)
    .await
    .expect("Insert with LOW");

    let q2004: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 2004")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q2004.as_deref(), Some("lossy"), "Trigger must normalize INSERT LOW -> lossy");

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, duration_ms, audio_quality) VALUES (2005, 'Trigger Test 5', ?, 180000, 'HIRES')"
    )
    .bind(album_id)
    .execute(&pool)
    .await
    .expect("Insert with HIRES");

    let q2005: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 2005")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q2005.as_deref(), Some("hires"), "Trigger must normalize INSERT HIRES -> hires");

    // 6. Test recurrence-prevention triggers on UPDATE
    sqlx::query("UPDATE tracks SET audio_quality = 'HIGH' WHERE id = 1008")
        .execute(&pool)
        .await
        .expect("Update with HIGH");

    let q1008_upd: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1008")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1008_upd.as_deref(), Some("lossy"), "Trigger must normalize UPDATE HIGH -> lossy");

    sqlx::query("UPDATE tracks SET audio_quality = 'LOSSLESS' WHERE id = 1008")
        .execute(&pool)
        .await
        .expect("Update with LOSSLESS");

    let q1008_upd2: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1008")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1008_upd2.as_deref(), Some("lossless"), "Trigger must normalize UPDATE LOSSLESS -> lossless");

    sqlx::query("UPDATE tracks SET audio_quality = 'HI_RES' WHERE id = 1008")
        .execute(&pool)
        .await
        .expect("Update with HI_RES");

    let q1008_upd3: Option<String> = sqlx::query_scalar("SELECT audio_quality FROM tracks WHERE id = 1008")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q1008_upd3.as_deref(), Some("hires"), "Trigger must normalize UPDATE HI_RES -> hires");

    // 7. Structural integrity and foreign key checks
    let integrity_row: (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .expect("PRAGMA integrity_check");
    assert_eq!(integrity_row.0, "ok", "Database must pass integrity check");

    let fk_rows = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .expect("PRAGMA foreign_key_check");
    assert!(fk_rows.is_empty(), "Foreign key check must report 0 violations");
}

#[tokio::test]
async fn test_clean_database_migration_pipeline_0072() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Connect to clean in-memory SQLite database");

    // Run complete migration sequence from 0001 to 0072 on fresh database
    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(&pool)
        .await
        .expect("Full migration sequence 0001..=0072 must apply cleanly");

    // Verify migration 0072 is recorded and succeeded
    let row = sqlx::query("SELECT version, description, success FROM _sqlx_migrations WHERE version = 72")
        .fetch_optional(&pool)
        .await
        .expect("Query _sqlx_migrations for version 72");

    assert!(row.is_some(), "Migration 0072 must be recorded in _sqlx_migrations");
    let row = row.unwrap();
    let version: i64 = row.get("version");
    let description: String = row.get("description");
    let success: bool = row.get("success");

    assert_eq!(version, 72);
    assert!(
        description.contains("normalize") && description.contains("audio") && description.contains("quality"),
        "Description was: {}",
        description
    );
    assert!(success);

    // Verify triggers exist in sqlite_master
    let ins_trig: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'trigger' AND name = 'trg_tracks_normalize_audio_quality_ins'"
    )
    .fetch_optional(&pool)
    .await
    .expect("Check insert trigger");
    assert_eq!(ins_trig.as_deref(), Some("trg_tracks_normalize_audio_quality_ins"));

    let upd_trig: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'trigger' AND name = 'trg_tracks_normalize_audio_quality_upd'"
    )
    .fetch_optional(&pool)
    .await
    .expect("Check update trigger");
    assert_eq!(upd_trig.as_deref(), Some("trg_tracks_normalize_audio_quality_upd"));

    // Verify database passes integrity checks
    let integrity_row: (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .expect("PRAGMA integrity_check");
    assert_eq!(integrity_row.0, "ok");

    let fk_rows = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .expect("PRAGMA foreign_key_check");
    assert!(fk_rows.is_empty());
}

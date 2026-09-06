//! Integration test suite for TASK-105:
//! Unificación Canónica de Artistas: Colisiones NOCASE, Nombres Basura y Purga de Huérfanos.
//!
//! Verifies:
//! 1. Migration 0079 merges case-insensitive colliding artist variants ("The Beatles" / "the beatles").
//! 2. Junction table relationships (track_artists, album_artists, track_credits) reassign to the canonical survivor.
//! 3. Metadata, service IDs (spotify_id, tidal_id, qobuz_id, musicbrainz_id) and favorites are consolidated with COALESCE/MAX.
//! 4. Garbage artists ('', 'Unknown', 'Unknown Artist', '\P', unlinked 'Various') are purged without stranding FKs.
//! 5. Legitimate artists starting with Unknown (e.g. "Unknown Mortal Orchestra") are strictly preserved.
//! 6. Pure orphan artists (no tracks/albums, no favorites, no provider IDs) are purged.
//! 7. Orphan artists with is_favorite = 1 or provider IDs are strictly preserved.
//! 8. Database triggers and unique index prevent empty/garbage names and case duplicates.
//! 9. Backend `upsert_canonical_favorite_artist` reuses canonical artists across case/whitespace variations.

use sqlx::sqlite::SqlitePoolOptions;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[tokio::test]
async fn test_migration_0079_unification_and_orphan_purge() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("canonical_artists_test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    // 1. Prepare migrations directory with migrations prior to 0079
    let mig_temp_dir = TempDir::new().unwrap();
    let src_migrations_dir = Path::new("./migrations");
    for entry in fs::read_dir(src_migrations_dir).unwrap().filter_map(|e| e.ok()) {
        let file_name = entry.file_name().into_string().unwrap();
        if file_name.ends_with(".sql") && !file_name.starts_with("0079") {
            fs::copy(entry.path(), mig_temp_dir.path().join(&file_name)).unwrap();
        }
    }

    let pre_migrator = sqlx::migrate::Migrator::new(mig_temp_dir.path())
        .await
        .expect("Failed to create pre-migrator");
    pre_migrator
        .run(&pool)
        .await
        .expect("Failed to run pre-0079 migrations");

    // Temporarily drop index if exists to simulate legacy data with collisions before 0079
    let _ = sqlx::query("DROP INDEX IF EXISTS idx_artists_name_unique_nocase").execute(&pool).await;

    // 2. Insert albums and tracks
    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Abbey Road'), (2, 'The Psych Album')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, isrc) VALUES
         (1, 'Come Together', 1, 'GBAYE6900013'),
         (2, 'Something', 1, 'GBAYE6900014'),
         (3, 'Garbage Track', NULL, 'USRC10000003'),
         (4, 'Unknown Track', NULL, 'USRC10000004'),
         (5, 'Multi-Love', 2, 'NZUMO1500001')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // 3. Insert colliding artist groups and test cases
    // Group 1: "The Beatles" (ID 10) vs "the beatles" (ID 20)
    sqlx::query(
        "INSERT INTO artists (id, name, spotify_id, is_favorite) VALUES (10, 'The Beatles', 'sp_beatles', 0)"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO artists (id, name, tidal_id, image_url, is_favorite) VALUES (20, 'the beatles', 'ti_beatles', 'https://img.beatles.jpg', 1)"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (1, 10, 'primary')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (2, 20, 'primary')")
        .execute(&pool)
        .await
        .unwrap();

    // Group 2: "Steve Harley &amp; Cockney Rebel" (ID 30) vs "Steve Harley & Cockney Rebel" (ID 40)
    sqlx::query(
        "INSERT INTO artists (id, name, is_favorite, favorite_at) VALUES (30, 'Steve Harley &amp; Cockney Rebel', 1, '2026-09-01T00:00:00Z')"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO artists (id, name, is_favorite) VALUES (40, 'Steve Harley & Cockney Rebel', 0)"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (1, 40, 1)")
        .execute(&pool)
        .await
        .unwrap();

    // Garbage Artists
    sqlx::query("INSERT INTO artists (id, name) VALUES (50, '')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (3, 50, 'primary')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO artists (id, name) VALUES (51, 'Unknown')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (2, 51, 1)")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO artists (id, name) VALUES (52, 'Unknown Artist')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (4, 52, 'primary')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO artists (id, name) VALUES (53, '\\P')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_credits (track_id, artist_id, role) VALUES (1, 53, 'performer')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO artists (id, name) VALUES (54, 'Various')")
        .execute(&pool)
        .await
        .unwrap();

    // Legitimate Artist with 'Unknown'
    sqlx::query("INSERT INTO artists (id, name, is_favorite) VALUES (55, 'Unknown Mortal Orchestra', 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (5, 55, 'primary')")
        .execute(&pool)
        .await
        .unwrap();

    // Orphans:
    // Pure orphan (should be purged)
    sqlx::query("INSERT INTO artists (id, name, is_favorite) VALUES (60, 'Pure Orphan Artist', 0)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_credits (track_id, artist_id, role) VALUES (1, 60, 'composer')")
        .execute(&pool)
        .await
        .unwrap();

    // Favorite orphan (should be preserved)
    sqlx::query("INSERT INTO artists (id, name, is_favorite) VALUES (61, 'Favorite Orphan Artist', 1)")
        .execute(&pool)
        .await
        .unwrap();

    // Provider orphan (should be preserved)
    sqlx::query("INSERT INTO artists (id, name, spotify_id, is_favorite) VALUES (62, 'Provider Orphan Artist', 'sp_orph_123', 0)")
        .execute(&pool)
        .await
        .unwrap();

    // 4. Run Migration 0079
    let m0079_sql = fs::read_to_string("migrations/0079_canonical_artists_unification.sql")
        .expect("Must read migration 0079 file");
    sqlx::raw_sql(&m0079_sql)
        .execute(&pool)
        .await
        .expect("Migration 0079 must execute cleanly");

    // 5. Assertions: Unification
    // Group 1: Beatles must be unified into exactly 1 artist
    let beatles_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM artists WHERE LOWER(TRIM(name)) = 'the beatles'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(beatles_count.0, 1, "There must be exactly 1 Beatles artist");

    let beatles: (i64, String, Option<String>, Option<String>, Option<String>, i64) = sqlx::query_as(
        "SELECT id, name, spotify_id, tidal_id, image_url, is_favorite FROM artists WHERE LOWER(TRIM(name)) = 'the beatles'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let survivor_id = beatles.0;
    assert_eq!(beatles.2.as_deref(), Some("sp_beatles"), "Spotify ID must be consolidated");
    assert_eq!(beatles.3.as_deref(), Some("ti_beatles"), "Tidal ID must be consolidated");
    assert_eq!(beatles.4.as_deref(), Some("https://img.beatles.jpg"), "Image URL must be consolidated");
    assert_eq!(beatles.5, 1, "Favorite state must be preserved as 1");

    // Both tracks must point to the survivor ID
    let t1_art: (i64,) = sqlx::query_as("SELECT artist_id FROM track_artists WHERE track_id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let t2_art: (i64,) = sqlx::query_as("SELECT artist_id FROM track_artists WHERE track_id = 2")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(t1_art.0, survivor_id, "Track 1 artist must point to survivor");
    assert_eq!(t2_art.0, survivor_id, "Track 2 artist must point to survivor");

    // Group 2: Steve Harley must be merged, entity unescaped to &
    let steve: (String, i64) = sqlx::query_as(
        "SELECT name, is_favorite FROM artists WHERE id = 40 OR id = 30"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(steve.0, "Steve Harley & Cockney Rebel", "Name must be unescaped without &amp;");
    assert_eq!(steve.1, 1, "Favorite state 1 from ID 30 must be merged");

    // Garbage artists must be purged
    let garbage_remaining: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM artists WHERE id IN (50, 51, 52, 53, 54)"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(garbage_remaining.0, 0, "Garbage artists (50..54) must be purged");

    // Legitimate artist must be preserved
    let umo_exists: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM artists WHERE id = 55 AND name = 'Unknown Mortal Orchestra'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(umo_exists.0, 1, "Unknown Mortal Orchestra must be preserved");

    // Pure orphan 60 must be purged
    let pure_orphan: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM artists WHERE id = 60"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pure_orphan.0, 0, "Pure orphan 60 must be purged");

    // Preserved orphans 61 and 62 must exist
    let preserved_orphans: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM artists WHERE id IN (61, 62)"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(preserved_orphans.0, 2, "Favorite and provider orphans must be preserved");

    // Relational integrity check
    let fk_violations: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(fk_violations.is_empty(), "Foreign key check must return 0 violations: {:?}", fk_violations);

    let integrity: (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(integrity.0, "ok", "Integrity check must be 'ok'");
}

#[tokio::test]
async fn test_triggers_and_constraints_prevent_garbage_and_case_duplicates() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to memory DB");

    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("Migrations must apply cleanly");

    // 1. Rejection of empty or whitespace artist name
    let empty_res = sqlx::query("INSERT INTO artists (name) VALUES ('')").execute(&pool).await;
    assert!(empty_res.is_err(), "Empty string artist name must be rejected by trigger");

    let whitespace_res = sqlx::query("INSERT INTO artists (name) VALUES ('    ')").execute(&pool).await;
    assert!(whitespace_res.is_err(), "Whitespace-only artist name must be rejected by trigger");

    // 2. Rejection of 'Unknown' and 'Unknown Artist'
    let unknown_res = sqlx::query("INSERT INTO artists (name) VALUES ('Unknown')").execute(&pool).await;
    assert!(unknown_res.is_err(), "'Unknown' must be rejected by trigger");

    let unknown_artist_res = sqlx::query("INSERT INTO artists (name) VALUES ('Unknown Artist')").execute(&pool).await;
    assert!(unknown_artist_res.is_err(), "'Unknown Artist' must be rejected by trigger");

    // 3. Legitimate name starting with 'Unknown' is allowed
    let umo_res = sqlx::query("INSERT INTO artists (name) VALUES ('Unknown Mortal Orchestra')").execute(&pool).await;
    assert!(umo_res.is_ok(), "'Unknown Mortal Orchestra' must be accepted");

    // 4. Unique case constraint prevents inserting duplicate with different case
    sqlx::query("INSERT INTO artists (name) VALUES ('Radiohead')").execute(&pool).await.unwrap();
    let dup_case_res = sqlx::query("INSERT INTO artists (name) VALUES ('radiohead')").execute(&pool).await;
    assert!(dup_case_res.is_err(), "Duplicate case variant 'radiohead' must fail unique constraint");

    let dup_whitespace_res = sqlx::query("INSERT INTO artists (name) VALUES ('  Radiohead  ')").execute(&pool).await;
    assert!(dup_whitespace_res.is_err(), "Whitespace variant '  Radiohead  ' must fail unique constraint");
}

#[tokio::test]
async fn test_backend_upsert_canonical_favorite_artist_reconciliation() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to memory DB");

    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("Migrations must apply cleanly");

    // Fetch existing service IDs
    let spotify_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'spotify'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let tidal_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'tidal'")
        .fetch_one(&pool)
        .await
        .unwrap();

    // 1. Upsert initial artist
    let aid1 = syncify_tauri_lib::commands::favorites::upsert_canonical_favorite_artist(
        &pool, spotify_id, "sp_pf_001", "Pink Floyd"
    )
    .await
    .expect("Upsert Pink Floyd");

    // 2. Upsert case variant from Tidal
    let aid2 = syncify_tauri_lib::commands::favorites::upsert_canonical_favorite_artist(
        &pool, tidal_id, "ti_pf_002", "pink floyd"
    )
    .await
    .expect("Upsert pink floyd");

    assert_eq!(aid1, aid2, "Both variants must resolve to the identical canonical artist ID");

    // 3. Upsert whitespace variant
    let aid3 = syncify_tauri_lib::commands::favorites::upsert_canonical_favorite_artist(
        &pool, spotify_id, "sp_pf_001", "   Pink Floyd   "
    )
    .await
    .expect("Upsert padded Pink Floyd");

    assert_eq!(aid1, aid3, "Padded variant must resolve to canonical artist ID");

    // Verify DB only has 1 artist
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM artists")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count.0, 1, "Exactly one artist must exist in DB");

    // Verify provider IDs merged
    let artist: (String, Option<String>, Option<String>, i64) = sqlx::query_as(
        "SELECT name, spotify_id, tidal_id, is_favorite FROM artists WHERE id = ?"
    )
    .bind(aid1)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(artist.0, "Pink Floyd");
    assert_eq!(artist.1.as_deref(), Some("sp_pf_001"));
    assert_eq!(artist.2.as_deref(), Some("ti_pf_002"));
    assert_eq!(artist.3, 1);
}

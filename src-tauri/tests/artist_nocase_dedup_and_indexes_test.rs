//! Test suite for TASK-83: Deduplicación Case-Insensitive (NOCASE) de Artistas y Creación de Índices Inversos
//!
//! Validates:
//! 1. Clean application of all canonical migrations including 0076.
//! 2. Existence of NOCASE unique index and inverse secondary indexes on track_artists, album_artists, and track_credits.
//! 3. Deduplication of case-insensitive and whitespace variant artists:
//!    - Metadata/external service IDs consolidation onto the canonical survivor.
//!    - Favorited state unification (`is_favorite = 1`).
//!    - Junction table reference reassignment without composite primary key conflicts.
//!    - Total absence of case duplicates post-migration (`HAVING COUNT(*) > 1` = 0).
//! 4. EXPLAIN QUERY PLAN verification confirming index usage for reverse artist lookups.
//! 5. Structural and relational integrity: PRAGMA foreign_key_check = 0 and PRAGMA integrity_check = ok.
//! 6. Recurrence prevention: NOCASE uniqueness enforcement and automatic name trimming trigger.

use sqlx::sqlite::SqlitePoolOptions;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

#[tokio::test]
async fn test_migration_0076_clean_run_schema_and_indexes() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite DB");

    // 1. Run all migrations including 0076
    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(&pool)
        .await
        .expect("Canonical migrations 0001..=0076 must apply cleanly");

    // 2. Verify migration version in _sqlx_migrations >= 76
    let max_v: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("Must fetch max migration version");
    assert!(max_v.0 >= 76, "Migration version must be >= 76");

    // 3. Verify required indexes exist
    let required_indexes = [
        "idx_artists_name_unique_nocase",
        "idx_track_artists_artist",
        "idx_album_artists_artist",
        "idx_track_credits_artist",
    ];

    for idx_name in required_indexes {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?",
        )
        .bind(idx_name)
        .fetch_one(&pool)
        .await
        .expect("Index lookup query failed");
        assert_eq!(count.0, 1, "Index '{}' must exist in sqlite_master", idx_name);
    }

    // 4. Verify old binary unique index is dropped
    let old_idx_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_artists_name_unique'",
    )
    .fetch_one(&pool)
    .await
    .expect("Old index check query failed");
    assert_eq!(old_idx_count.0, 0, "Old index idx_artists_name_unique must be dropped");

    // 5. Verify PRAGMA integrity_check and foreign_key_check
    let integrity: (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .expect("PRAGMA integrity_check failed");
    assert_eq!(integrity.0, "ok", "Database integrity check must be 'ok'");

    let fk_violations: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .expect("PRAGMA foreign_key_check failed");
    assert!(fk_violations.is_empty(), "Database must have 0 foreign key violations");
}

#[tokio::test]
async fn test_migration_0076_deduplicates_nocase_artist_groups_and_reassigns_links() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("dedup_test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test SQLite DB");

    // 1. Prepare temp migrations dir containing 0001 through 0075
    let mig_temp_dir = TempDir::new().unwrap();
    let src_migrations_dir = Path::new("./migrations");
    for entry in fs::read_dir(src_migrations_dir).unwrap().filter_map(|e| e.ok()) {
        let file_name = entry.file_name().into_string().unwrap();
        if file_name.ends_with(".sql") {
            let version = file_name.split('_').next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
            if version > 0 && version < 76 {
                fs::copy(entry.path(), mig_temp_dir.path().join(&file_name)).unwrap();
            }
        }
    }

    let pre_migrator = sqlx::migrate::Migrator::new(mig_temp_dir.path())
        .await
        .expect("Failed to build pre-migrator for 0001..=0075");
    pre_migrator
        .run(&pool)
        .await
        .expect("Failed running migrations 0001..=0075");

    // 2. Insert test albums and tracks for linking
    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Rock Album 1'), (2, 'Rock Album 2')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tracks (id, title, album_id, isrc) VALUES (1, 'Track 1', 1, 'USRC10000001'), (2, 'Track 2', 1, 'USRC10000002')")
        .execute(&pool)
        .await
        .unwrap();

    // 3. Insert simulated case-insensitive and whitespace duplicate artist groups:
    // Group A: grandson (canonical) vs Grandson (loser, favorite=1, no tracks)
    sqlx::query(
        "INSERT INTO artists (id, name, qobuz_id, spotify_id, is_favorite)
         VALUES (64, 'grandson', 'qobuz_grandson', 'spotify_grandson', 0)"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO artists (id, name, tidal_id, is_favorite)
         VALUES (93676, 'Grandson', 'tidal_grandson', 1)"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Group B: TOTO (winner, has tidal_id) vs Toto (loser, has musicbrainz_id)
    sqlx::query(
        "INSERT INTO artists (id, name, tidal_id, is_favorite)
         VALUES (11541, 'TOTO', 'tidal_toto', 1)"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO artists (id, name, musicbrainz_id, is_favorite)
         VALUES (93680, 'Toto', 'mb_toto_123', 0)"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Group C: Whitespace duplicate: 'Oasis ' vs 'Oasis'
    sqlx::query("INSERT INTO artists (id, name, is_favorite) VALUES (923, 'Oasis ', 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO artists (id, name) VALUES (925, 'Oasis')")
        .execute(&pool)
        .await
        .unwrap();

    // 4. Link artists into junction tables including overlapping entries:
    // 4a. track_artists: track 1 linked to both 64 and 93676 with role 'primary'
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (1, 64, 'primary')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (1, 93676, 'primary')")
        .execute(&pool)
        .await
        .unwrap();
    // track 2 linked to canonical artist 64 with role 'featured'
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (2, 64, 'featured')")
        .execute(&pool)
        .await
        .unwrap();

    // 4b. album_artists: album 1 linked to both 64 and 93676
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (1, 64, 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (1, 93676, 1)")
        .execute(&pool)
        .await
        .unwrap();
    // album 1 also linked to 11541 (Toto), album 2 linked to loser 93680 (Toto) to test album reassignment
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (1, 11541, 1)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (2, 93680, 1)")
        .execute(&pool)
        .await
        .unwrap();

    // 4c. track_credits: track 1 credited to both 11541 and 93680 with role 'composer'
    sqlx::query("INSERT INTO track_credits (track_id, artist_id, role) VALUES (1, 11541, 'composer')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_credits (track_id, artist_id, role) VALUES (1, 93680, 'composer')")
        .execute(&pool)
        .await
        .unwrap();

    // Verify duplicates exist before migration 0076
    let pre_dups: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM (SELECT LOWER(TRIM(name)) FROM artists GROUP BY LOWER(TRIM(name)) HAVING COUNT(*) > 1)"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pre_dups.0, 3, "Must have 3 duplicate artist groups pre-migration");

    // 5. Now apply migration 0076 via canonical sqlx migrator
    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(&pool)
        .await
        .expect("Migration 0076 must apply cleanly via sqlx migrator");

    // 6. Assert acceptance criteria:
    // 6a. Duplicates count must be exactly 0
    let post_dups: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM (SELECT LOWER(name) FROM artists GROUP BY LOWER(name) HAVING COUNT(*) > 1)"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(post_dups.0, 0, "Deduplication must result in 0 case-insensitive duplicates");

    // 6b. Check survivor 64 (grandson) consolidated all fields and favorite status
    let survivor_grandson: (String, Option<String>, Option<String>, Option<String>, i64) = sqlx::query_as(
        "SELECT name, spotify_id, qobuz_id, tidal_id, is_favorite FROM artists WHERE id = 64"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(survivor_grandson.1.as_deref(), Some("spotify_grandson"));
    assert_eq!(survivor_grandson.2.as_deref(), Some("qobuz_grandson"));
    assert_eq!(survivor_grandson.3.as_deref(), Some("tidal_grandson"), "Loser tidal_id must merge to winner");
    assert_eq!(survivor_grandson.4, 1, "is_favorite must be consolidated to 1");

    // 6c. Check survivor 11541 (TOTO) consolidated musicbrainz_id
    let survivor_toto: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT tidal_id, musicbrainz_id FROM artists WHERE id = 11541"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(survivor_toto.0.as_deref(), Some("tidal_toto"));
    assert_eq!(survivor_toto.1.as_deref(), Some("mb_toto_123"), "musicbrainz_id must merge to winner");

    // 6d. Check loser rows are deleted (93676, 93680, 925) and winner 923 trimmed
    let loser_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM artists WHERE id IN (93676, 93680, 925)"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(loser_count.0, 0, "Loser artist records must be deleted");

    let oasis_winner: (String,) = sqlx::query_as("SELECT name FROM artists WHERE id = 923")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(oasis_winner.0, "Oasis", "Winner name must be trimmed");

    // 6e. Check track_artists links
    let ta_rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT track_id, artist_id, role FROM track_artists ORDER BY track_id, role"
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    // Track 1 should have exactly one link to survivor 64 (deduplicated primary)
    // Track 2 should have reassigned link to survivor 64 with role 'featured'
    assert_eq!(ta_rows.len(), 2);
    assert_eq!(ta_rows[0], (1, 64, "primary".to_string()));
    assert_eq!(ta_rows[1], (2, 64, "featured".to_string()));

    // 6f. Check album_artists links
    let aa_rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT album_id, artist_id FROM album_artists ORDER BY album_id, artist_id"
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(aa_rows.len(), 3);
    assert_eq!(aa_rows[0], (1, 64), "Album 1 must link to survivor 64");
    assert_eq!(aa_rows[1], (1, 11541), "Album 1 must link to survivor 11541");
    assert_eq!(aa_rows[2], (2, 11541), "Album 2 must reassign to survivor 11541");

    // 6g. Check track_credits links
    let tc_rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT track_id, artist_id, role FROM track_credits"
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(tc_rows.len(), 1);
    assert_eq!(tc_rows[0], (1, 11541, "composer".to_string()), "Track 1 credit must deduplicate to winner 11541");

    // 7. Verify PRAGMAs
    let integrity: (String,) = sqlx::query_as("PRAGMA integrity_check").fetch_one(&pool).await.unwrap();
    assert_eq!(integrity.0, "ok");
    let fk_violations: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check").fetch_all(&pool).await.unwrap();
    assert!(fk_violations.is_empty(), "Zero FK violations");
}

#[tokio::test]
async fn test_explain_query_plan_uses_new_indexes() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.unwrap();

    // 1. track_credits query plan
    let qp_credits: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "EXPLAIN QUERY PLAN SELECT * FROM track_credits WHERE artist_id = ?"
    )
    .bind(64)
    .fetch_all(&pool)
    .await
    .unwrap();
    let detail_credits = qp_credits.iter().map(|r| r.3.clone()).collect::<Vec<_>>().join("; ");
    assert!(
        detail_credits.contains("USING INDEX idx_track_credits_artist")
            || detail_credits.contains("USING COVERING INDEX idx_track_credits_artist"),
        "track_credits lookup by artist_id must use idx_track_credits_artist. Detail: {}",
        detail_credits
    );

    // 2. track_artists query plan
    let qp_track_artists: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "EXPLAIN QUERY PLAN SELECT * FROM track_artists WHERE artist_id = ?"
    )
    .bind(64)
    .fetch_all(&pool)
    .await
    .unwrap();
    let detail_ta = qp_track_artists.iter().map(|r| r.3.clone()).collect::<Vec<_>>().join("; ");
    assert!(
        detail_ta.contains("USING INDEX idx_track_artists_artist")
            || detail_ta.contains("USING COVERING INDEX idx_track_artists_artist")
            || detail_ta.contains("idx_track_artists_artist_id"),
        "track_artists lookup by artist_id must use index. Detail: {}",
        detail_ta
    );

    // 3. album_artists query plan
    let qp_album_artists: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "EXPLAIN QUERY PLAN SELECT * FROM album_artists WHERE artist_id = ?"
    )
    .bind(64)
    .fetch_all(&pool)
    .await
    .unwrap();
    let detail_aa = qp_album_artists.iter().map(|r| r.3.clone()).collect::<Vec<_>>().join("; ");
    assert!(
        detail_aa.contains("USING INDEX idx_album_artists_artist")
            || detail_aa.contains("USING COVERING INDEX idx_album_artists_artist")
            || detail_aa.contains("idx_album_artists_artist_id"),
        "album_artists lookup by artist_id must use index. Detail: {}",
        detail_aa
    );

    // 4. artists query plan with COLLATE NOCASE
    let qp_artist_nocase: Vec<(i64, i64, i64, String)> = sqlx::query_as(
        "EXPLAIN QUERY PLAN SELECT id FROM artists WHERE name = ? COLLATE NOCASE"
    )
    .bind("Grandson")
    .fetch_all(&pool)
    .await
    .unwrap();
    let detail_art = qp_artist_nocase.iter().map(|r| r.3.clone()).collect::<Vec<_>>().join("; ");
    assert!(
        detail_art.contains("USING INDEX idx_artists_name_unique_nocase")
            || detail_art.contains("USING COVERING INDEX idx_artists_name_unique_nocase")
            || detail_art.contains("idx_artists_name_search"),
        "artists lookup by name COLLATE NOCASE must use NOCASE index. Detail: {}",
        detail_art
    );
}

#[tokio::test]
async fn test_nocase_unique_constraint_and_recurrence_prevention() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.unwrap();

    // 1. Insert initial artist
    sqlx::query("INSERT INTO artists (name) VALUES ('Radiohead')")
        .execute(&pool)
        .await
        .expect("Initial insert must succeed");

    // 2. Direct duplicate insert with different case must be rejected by idx_artists_name_unique_nocase
    let dup_res = sqlx::query("INSERT INTO artists (name) VALUES ('RADIOHEAD')")
        .execute(&pool)
        .await;
    assert!(
        dup_res.is_err(),
        "Direct insert of 'RADIOHEAD' must violate UNIQUE constraint idx_artists_name_unique_nocase"
    );

    // 3. Upsert with ON CONFLICT (name COLLATE NOCASE) must resolve cleanly to existing artist
    let resolved_id: (i64,) = sqlx::query_as(
        "INSERT INTO artists (name) VALUES (?)
         ON CONFLICT (name COLLATE NOCASE) DO UPDATE SET id = id
         RETURNING id"
    )
    .bind("radiohead")
    .fetch_one(&pool)
    .await
    .expect("ON CONFLICT(name COLLATE NOCASE) upsert must resolve without error");

    let initial_id: (i64,) = sqlx::query_as("SELECT id FROM artists WHERE name = 'Radiohead'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(resolved_id.0, initial_id.0, "Upsert must return existing artist ID");

    // 4. Test recurrence prevention triggers: automatic whitespace trimming
    let coldplay_id: (i64,) = sqlx::query_as(
        "INSERT INTO artists (name) VALUES ('   Coldplay   ') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let clean_name: (String,) = sqlx::query_as("SELECT name FROM artists WHERE id = ?")
        .bind(coldplay_id.0)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(clean_name.0, "Coldplay", "Trigger must automatically trim artist name on insert");

    // Trigger on update
    sqlx::query("UPDATE artists SET name = '   The Strokes   ' WHERE id = ?")
        .bind(coldplay_id.0)
        .execute(&pool)
        .await
        .unwrap();

    let updated_name: (String,) = sqlx::query_as("SELECT name FROM artists WHERE id = ?")
        .bind(coldplay_id.0)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(updated_name.0, "The Strokes", "Trigger must automatically trim artist name on update");
}

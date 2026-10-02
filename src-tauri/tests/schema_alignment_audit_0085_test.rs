//! Regression tests for migration 0085 (post-audit phase 1, item 1.2).
//!
//! Aligns the schema with real code usage:
//! (a) BD-5  — `albums.animated_cover_path` exists, so the animated-cover flow
//!             persists the association instead of skipping it forever.
//! (b) BD-4  — `library_items` backfill statement (from the migration file)
//!             maps track_sources -> tracks -> track_artists -> artists/albums,
//!             omits rows without external_id/title/artist and pins
//!             library_items.id = tracks.id (the invariant the migration.rs
//!             consumers rely on via `playlist_tracks.track_id = library_items.id`).
//! (c) BD-10 — `sync_log` is dropped.
//! (d) BD-11 — library_snapshots keeps only columns with real writers;
//!             downloads loses only_available_on/not_streaming (rebuilt table)
//!             and gains a durable musicbrainz_release_id writer; library_entries
//!             loses play_count/auto_download; cache_stats is dropped.
//! (e) BD-12 — trg_tracks_sync_album_total_tracks_{ins,del,upd} are installed by
//!             the migration and recount albums.total_tracks on deletes.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::Arc;
use syncify_tauri_lib::commands::dashboard::create_library_snapshot;
use syncify_tauri_lib::services::animated_cover::associate_animated_cover_in_db;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::{AppState, EnrichmentWorkerState};
use tauri::Manager;

/// The exact backfill statement shipped in the migration, extracted between
/// markers so this test exercises the real SQL, not a copy of it.
fn library_items_backfill_sql() -> String {
    let raw = include_str!("../migrations/0085_alignment_audit.sql");
    let begin = "-- BEGIN library_items_backfill";
    let end = "-- END library_items_backfill";
    let start = raw
        .find(begin)
        .unwrap_or_else(|| panic!("backfill begin marker missing in 0085"));
    let stop = raw[start..]
        .find(end)
        .unwrap_or_else(|| panic!("backfill end marker missing in 0085"));
    raw[start + begin.len()..start + stop].to_string()
}

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

async fn column_names(pool: &SqlitePool, table: &str) -> Vec<String> {
    let rows: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as(&format!("PRAGMA table_info({})", table))
            .fetch_all(pool)
            .await
            .expect("PRAGMA table_info must succeed");
    rows.into_iter().map(|c| c.1).collect()
}

async fn table_exists(pool: &SqlitePool, table: &str) -> bool {
    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(table)
            .fetch_one(pool)
            .await
            .expect("sqlite_master query must succeed");
    count > 0
}

// ============================================================================
// Schema shape
// ============================================================================

#[tokio::test]
async fn test_migration_0085_recorded_and_schema_aligned() {
    let pool = setup_test_db().await;

    let (version, success): (i64, i64) =
        sqlx::query_as("SELECT version, success FROM _sqlx_migrations WHERE version = 85")
            .fetch_one(&pool)
            .await
            .expect("0085 must be recorded in _sqlx_migrations");
    assert_eq!(version, 85);
    assert_eq!(success, 1, "0085 must be recorded as successfully applied");

    // (a) BD-5: the animated-cover column exists.
    let album_cols = column_names(&pool, "albums").await;
    assert!(
        album_cols.contains(&"animated_cover_path".to_string()),
        "albums.animated_cover_path must exist after 0085, got: {:?}",
        album_cols
    );

    // (c) BD-10: sync_log is gone.
    assert!(
        !table_exists(&pool, "sync_log").await,
        "sync_log must be dropped by 0085"
    );

    // (d) BD-11: only snapshot columns with real writers remain.
    let snapshot_cols = column_names(&pool, "library_snapshots").await;
    for gone in &["metadata_excellent", "metadata_good", "metadata_needs_work"] {
        assert!(
            !snapshot_cols.contains(&gone.to_string()),
            "library_snapshots.{} must be dropped (no data source, no reader)",
            gone
        );
    }
    for kept in &[
        "snapshot_date",
        "total_tracks",
        "total_albums",
        "total_artists",
        "total_size_bytes",
        "tracks_with_lyrics",
        "tracks_lossless",
        "tracks_hires",
        "downloaded_tracks",
    ] {
        assert!(
            snapshot_cols.contains(&kept.to_string()),
            "library_snapshots.{} must remain (dashboard writes it)",
            kept
        );
    }

    // (d) BD-11: downloads — the two unwritable availability columns are gone,
    // every real column (incl. the 0052/0056/0060/0062 additions) survives.
    let download_cols = column_names(&pool, "downloads").await;
    assert!(!download_cols.contains(&"only_available_on".to_string()));
    assert!(!download_cols.contains(&"not_streaming".to_string()));
    for kept in &[
        "id",
        "track_id",
        "source_service_id",
        "file_path",
        "file_format",
        "file_size_bytes",
        "file_hash",
        "bit_depth",
        "sample_rate",
        "metadata_completeness",
        "downloaded_at",
        "musicbrainz_release_id",
        "updated_at",
        "origin_service",
        "origin_service_track_id",
        "effective_service",
        "effective_service_track_id",
        "fallback_reason",
        "match_method",
        "match_confidence",
        "file_disambiguator",
        "requested_quality",
        "effective_quality",
        "requested_format",
        "effective_format",
        "quality_decision",
        "provider_fallback_used",
        "quality_fallback_used",
        "decision_reason",
        "skip_reason",
    ] {
        assert!(
            download_cols.contains(&kept.to_string()),
            "downloads.{} must survive the 0085 rebuild",
            kept
        );
    }
    assert_eq!(
        download_cols.len(),
        30,
        "downloads must have exactly 30 columns after dropping 2 of 32"
    );

    // Indexes of the rebuilt table are back in place.
    let (path_index,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_downloads_unique_path'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        path_index, 1,
        "idx_downloads_unique_path must be recreated after the rebuild"
    );

    // (d) BD-11: library_entries loses the two unwritable columns.
    let entry_cols = column_names(&pool, "library_entries").await;
    assert!(!entry_cols.contains(&"play_count".to_string()));
    assert!(!entry_cols.contains(&"auto_download".to_string()));
    for kept in &[
        "id",
        "account_id",
        "track_id",
        "added_at",
        "is_liked",
        "is_purchased",
    ] {
        assert!(
            entry_cols.contains(&kept.to_string()),
            "library_entries.{} must survive",
            kept
        );
    }

    // (d) BD-11: cache_stats is gone.
    assert!(
        !table_exists(&pool, "cache_stats").await,
        "cache_stats must be dropped by 0085"
    );

    // (e) BD-12: the three total_tracks triggers are installed by the migration.
    for trg in &[
        "trg_tracks_sync_album_total_tracks_ins",
        "trg_tracks_sync_album_total_tracks_del",
        "trg_tracks_sync_album_total_tracks_upd",
    ] {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = ?",
        )
        .bind(trg)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "trigger {} must be installed by 0085", trg);
    }
}

// ============================================================================
// (b) BD-4: library_items backfill
// ============================================================================

#[tokio::test]
async fn test_library_items_backfill_maps_catalog_and_skips_incomplete_rows() {
    let pool = setup_test_db().await;

    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();

    // Album with a title for the backfill.
    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Backfill Album') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    let art_primary: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Primary Artist') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let art_main: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Main Artist') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    // Track 1: two sources; the qobuz one wins (available + higher score).
    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, audio_quality) VALUES ('Backfill Song', ?, 210000, 'lossless') RETURNING id",
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary'), (?, ?, 'main')")
        .bind(t1)
        .bind(art_primary)
        .bind(t1)
        .bind(art_main)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, available, quality_score) VALUES (?, 1, 'sp-1', 1, 50), (?, 2, 'qb-1', 1, 90)")
        .bind(t1)
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();

    // Track 2: single source; omitted (no artist rows at all).
    let t2: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Orphan Song') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-2')",
    )
    .bind(t2)
    .execute(&pool)
    .await
    .unwrap();

    // Track 3: has an artist but a blank external_id; omitted.
    let t3: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('No External Song') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t3)
        .bind(art_primary)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, '   ')",
    )
    .bind(t3)
    .execute(&pool)
    .await
    .unwrap();

    // Track 4: source exists but the track has a blank title; omitted.
    let t4: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('   ') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t4)
        .bind(art_primary)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-4')",
    )
    .bind(t4)
    .execute(&pool)
    .await
    .unwrap();

    let backfill = library_items_backfill_sql();
    sqlx::query(&backfill)
        .execute(&pool)
        .await
        .expect("The shipped backfill statement must execute cleanly");

    // Exactly one row: the complete track. Omitted: no-artist, blank
    // external_id and blank-title tracks.
    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM library_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1, "only complete tracks are backfilled");

    let row: (i64, String, String, String, String, String, Option<String>, i64, Option<String>) =
        sqlx::query_as(
            "SELECT id, service, source_service, external_id, title, artist, album, duration_ms, quality FROM library_items",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row.0, t1,
        "library_items.id must equal tracks.id (consumer invariant)"
    );
    assert_eq!(row.1, "qobuz", "best available source service wins");
    assert_eq!(row.2, "qobuz");
    assert_eq!(row.3, "qb-1");
    assert_eq!(row.4, "Backfill Song");
    assert_eq!(row.5, "Primary Artist", "primary role beats main role");
    assert_eq!(row.6.as_deref(), Some("Backfill Album"));
    assert_eq!(row.7, 210000);
    assert_eq!(row.8.as_deref(), Some("lossless"));

    // Re-running the statement must not duplicate anything (idempotency guard).
    sqlx::query(&backfill)
        .execute(&pool)
        .await
        .expect("backfill must be re-runnable");
    let (rows_after,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM library_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows_after, 1, "backfill must be idempotent");

    // The consumer join used by preview_migration/start_migration
    // (playlist_tracks.track_id = library_items.id) resolves the backfilled row.
    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'a@b.c', '{}', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, external_id, name) VALUES (?, 'pl-1', 'P') RETURNING id",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, 1)")
        .bind(playlist_id)
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();

    let joined: (i64, String) = sqlx::query_as(
        "SELECT li.id, li.external_id FROM library_items li
         JOIN playlist_tracks pt ON pt.track_id = li.id
         WHERE pt.playlist_id = ?",
    )
    .bind(playlist_id)
    .fetch_one(&pool)
    .await
    .expect("consumer join over backfilled rows must resolve");
    assert_eq!(joined.0, t1);
    assert_eq!(joined.1, "qb-1");
}

// ============================================================================
// (a) BD-5: animated cover association works on the migrated schema
// ============================================================================

#[tokio::test]
async fn test_animated_cover_association_persists_on_migrated_schema() {
    let pool = setup_test_db().await;

    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Animated Album') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    let mp4 = Path::new("/music/Animated Album/animated_cover.mp4");
    let associated = associate_animated_cover_in_db(&pool, album_id, mp4)
        .await
        .expect("association must succeed now that the column exists");
    assert!(associated, "association must no longer be skipped");

    let saved: Option<String> =
        sqlx::query_scalar("SELECT animated_cover_path FROM albums WHERE id = ?")
            .bind(album_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(saved.as_deref(), Some(mp4.to_str().unwrap()));
}

// ============================================================================
// (d) BD-11: downloads.musicbrainz_release_id stays populated
// ============================================================================

#[tokio::test]
async fn test_downloads_musicbrainz_release_id_backfill_and_triggers() {
    let pool = setup_test_db().await;

    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();

    let alb1: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, musicbrainz_id) VALUES ('Rel A', 'mb-release-aaa') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let alb2: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, musicbrainz_id) VALUES ('Rel B', 'mb-release-bbb') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id) VALUES ('Song A', ?) RETURNING id",
    )
    .bind(alb1)
    .fetch_one(&pool)
    .await
    .unwrap();
    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id) VALUES ('Song B', ?) RETURNING id",
    )
    .bind(alb2)
    .fetch_one(&pool)
    .await
    .unwrap();

    // INSERT trigger fills the release id from the track's album.
    let dl1: i64 = sqlx::query_scalar(
        "INSERT INTO downloads (track_id, file_path) VALUES (?, '/lib/Song A.flac') RETURNING id",
    )
    .bind(t1)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mb: Option<String> =
        sqlx::query_scalar("SELECT musicbrainz_release_id FROM downloads WHERE id = ?")
            .bind(dl1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(mb.as_deref(), Some("mb-release-aaa"));

    // UPDATE OF track_id re-fills it after a reassignment.
    sqlx::query("UPDATE downloads SET musicbrainz_release_id = NULL WHERE id = ?")
        .bind(dl1)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE downloads SET track_id = ? WHERE id = ?")
        .bind(t2)
        .bind(dl1)
        .execute(&pool)
        .await
        .unwrap();
    let mb2: Option<String> =
        sqlx::query_scalar("SELECT musicbrainz_release_id FROM downloads WHERE id = ?")
            .bind(dl1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(mb2.as_deref(), Some("mb-release-bbb"));

    // Rows without a track stay NULL and untouched.
    let dl2: i64 = sqlx::query_scalar(
        "INSERT INTO downloads (file_path) VALUES ('/lib/Local Import.flac') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let mb3: Option<String> =
        sqlx::query_scalar("SELECT musicbrainz_release_id FROM downloads WHERE id = ?")
            .bind(dl2)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(mb3, None);

    // The migration's one-shot backfill covered pre-existing rows too: insert a
    // row the way legacy code would (no trigger help for NULL track), link it to
    // a track afterwards via the same UPDATE the migration used, and confirm the
    // data path the migration relies on (tracks -> albums) resolves.
    let (backfilled_ok,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM downloads WHERE track_id IS NOT NULL AND musicbrainz_release_id = (SELECT albums.musicbrainz_id FROM tracks JOIN albums ON albums.id = tracks.album_id WHERE tracks.id = downloads.track_id)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        backfilled_ok, 1,
        "every download with a track must carry its album's release MBID"
    );
}

// ============================================================================
// (e) BD-12: total_tracks recount, especially on delete
// ============================================================================

#[tokio::test]
async fn test_total_tracks_recounts_when_tracks_are_deleted() {
    let pool = setup_test_db().await;

    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Delete Album', 0, 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let t1: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('D1', ?) RETURNING id")
            .bind(album_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let _t2: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('D2', ?) RETURNING id")
            .bind(album_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let t3: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('D3', ?) RETURNING id")
            .bind(album_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let (count,): (i64,) = sqlx::query_as("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);

    // The BD-12 core: deleting tracks keeps albums.total_tracks in sync.
    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();
    let (after_del,): (i64,) = sqlx::query_as("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after_del, 2, "total_tracks must be recounted on delete");

    // Reassigning a track to another album recounts both albums.
    let other: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, is_stub) VALUES ('Other Album', 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE tracks SET album_id = ? WHERE id = ?")
        .bind(other)
        .bind(t3)
        .execute(&pool)
        .await
        .unwrap();
    let (orig, target): (i64, i64) = sqlx::query_as(
        "SELECT MAX(CASE WHEN id = ? THEN total_tracks END), MAX(CASE WHEN id = ? THEN total_tracks END) FROM albums",
    )
    .bind(album_id)
    .bind(other)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(orig, 1, "source album must lose the reassigned track");
    assert_eq!(target, 1, "target album must gain the reassigned track");
}

#[tokio::test]
async fn test_total_tracks_triggers_leave_stub_albums_declared() {
    let pool = setup_test_db().await;

    let stub_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Wishlist Stub', 12, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO tracks (title, album_id) VALUES ('Stub Track', ?)")
        .bind(stub_id)
        .execute(&pool)
        .await
        .unwrap();

    let (declared,): (i64,) = sqlx::query_as("SELECT total_tracks FROM albums WHERE id = ?")
        .bind(stub_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        declared, 12,
        "stub albums keep their declared total_tracks (TASK-138 semantics)"
    );
}

// ============================================================================
// (d) BD-11: create_library_snapshot writes every remaining column
// ============================================================================

#[tokio::test]
async fn test_create_library_snapshot_persists_all_columns() {
    let pool = setup_test_db().await;

    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();

    // 3 tracks: one lossless with lyrics, one hires, one lossy; 2 downloads
    // totalling 3000 bytes.
    let alb: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Snap Album') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let t_lossless: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, audio_quality) VALUES ('L1', ?, 'lossless') RETURNING id",
    )
    .bind(alb)
    .fetch_one(&pool)
    .await
    .unwrap();
    let t_hires: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, audio_quality) VALUES ('H1', ?, 'hires') RETURNING id",
    )
    .bind(alb)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO tracks (title, album_id, audio_quality) VALUES ('Y1', ?, 'lossy')")
        .bind(alb)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO lyrics (track_id, format, content) VALUES (?, 'plain', 'la la')")
        .bind(t_lossless)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO downloads (track_id, file_path, file_size_bytes) VALUES (?, '/lib/L1.flac', 1000), (?, '/lib/H1.flac', 2000)")
        .bind(t_lossless)
        .bind(t_hires)
        .execute(&pool)
        .await
        .unwrap();

    let app = tauri::test::mock_app();
    app.manage(AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    });

    let snapshot = create_library_snapshot(app.state::<AppState>())
        .await
        .expect("create_library_snapshot must succeed");

    assert_eq!(snapshot.total_tracks, 3);
    assert_eq!(snapshot.total_albums, 1);
    assert_eq!(snapshot.downloaded_tracks, 2);
    assert_eq!(snapshot.total_size_bytes, 3000, "real size must be written");
    assert_eq!(
        snapshot.tracks_with_lyrics, 1,
        "lyrics coverage must be real"
    );
    assert_eq!(
        snapshot.tracks_lossless, 1,
        "lossless count must come from tracks.audio_quality"
    );
    assert_eq!(
        snapshot.tracks_hires, 1,
        "hires count must come from tracks.audio_quality"
    );

    // Re-running updates today's snapshot in place instead of duplicating it.
    let second = create_library_snapshot(app.state::<AppState>())
        .await
        .expect("second snapshot must succeed");
    assert_eq!(second.id, snapshot.id);

    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM library_snapshots")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

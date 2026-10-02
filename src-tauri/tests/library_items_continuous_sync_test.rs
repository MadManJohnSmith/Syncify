//! Regression tests for audit item 4.1 (BD-4 continuation): library_items stays
//! populated in continuous operation.
//!
//! Phase 1 (migration 0085) backfilled library_items once, but nothing kept it
//! populated afterwards, so any track imported/synced/liked after that backfill
//! never reached the migration feature (preview_migration, start_migration and
//! search_destination_track all read library_items — BD-4).
//!
//! Migration 0086 installs triggers on track_sources (and tracks) that fire at
//! exactly the write points of the sync/import/favorites flows — INSERT (also
//! INSERT OR IGNORE / INSERT OR REPLACE), UPDATE of the identity/availability
//! columns and DELETE — and maintain the mirror row under the 0085 backfill
//! rules:
//!   * one row per canonical track with library_items.id = tracks.id (the
//!     invariant the consumers pin via `playlist_tracks.track_id = library_items.id`);
//!   * the row carries the track's best source (available DESC,
//!     quality_score DESC, id ASC — the backfill's exact ordering);
//!   * rows lacking external_id/title/artist are omitted without failing the
//!     mirrored write;
//!   * a track that loses its last source or is deleted leaves no ghost row.
//!
//! Every test below fails against the pre-0086 schema (library_items stays
//! empty / stale).

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // The application enforces foreign keys on every connection (db.rs) and the
    // catalog-cascade expectations below rely on them.
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

/// Seeds services 1 = spotify, 2 = qobuz and returns the pool ids resolved.
async fn seed_services(pool: &SqlitePool) {
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(pool)
        .await
        .unwrap();
}

/// Creates an album + two artists (primary first) + a complete track and
/// returns (track_id, primary_artist_name).
async fn seed_complete_track(
    pool: &SqlitePool,
    title: &str,
    album_title: &str,
    duration_ms: i64,
    quality: Option<&str>,
) -> (i64, String) {
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES (?) RETURNING id")
        .bind(album_title)
        .fetch_one(pool)
        .await
        .unwrap();
    let primary: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
        .bind(format!("{} Primary", title))
        .fetch_one(pool)
        .await
        .unwrap();
    let main: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
        .bind(format!("{} Main", title))
        .fetch_one(pool)
        .await
        .unwrap();
    let track_id: i64 = match quality {
        Some(q) => {
            sqlx::query_scalar(
                "INSERT INTO tracks (title, album_id, duration_ms, audio_quality) VALUES (?, ?, ?, ?) RETURNING id",
            )
            .bind(title)
            .bind(album_id)
            .bind(duration_ms)
            .bind(q)
            .fetch_one(pool)
            .await
            .unwrap()
        }
        None => {
            sqlx::query_scalar(
                "INSERT INTO tracks (title, album_id, duration_ms) VALUES (?, ?, ?) RETURNING id",
            )
            .bind(title)
            .bind(album_id)
            .bind(duration_ms)
            .fetch_one(pool)
            .await
            .unwrap()
        }
    };
    // 'main' inserted first on purpose: 'primary' must still win the artist pick.
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'main'), (?, ?, 'primary')")
        .bind(track_id)
        .bind(main)
        .bind(track_id)
        .bind(primary)
        .execute(pool)
        .await
        .unwrap();
    let primary_name: String = sqlx::query_scalar("SELECT name FROM artists WHERE id = ?")
        .bind(primary)
        .fetch_one(pool)
        .await
        .unwrap();
    (track_id, primary_name)
}

async fn mirror_row(
    pool: &SqlitePool,
    track_id: i64,
) -> Option<(String, String, String, String, String, i64, Option<String>)> {
    sqlx::query_as(
        "SELECT service, source_service, external_id, title, artist, duration_ms, quality FROM library_items WHERE id = ?",
    )
    .bind(track_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn mirror_count(pool: &SqlitePool) -> i64 {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM library_items")
        .fetch_one(pool)
        .await
        .unwrap();
    count
}

// ============================================================================
// Schema: the continuous-sync triggers are installed
// ============================================================================

#[tokio::test]
async fn test_library_items_continuous_sync_triggers_installed() {
    let pool = setup_test_db().await;

    for trigger in &[
        "trg_track_sources_sync_library_items_ins",
        "trg_track_sources_sync_library_items_upd",
        "trg_track_sources_sync_library_items_del",
        "trg_tracks_sync_library_items_del",
    ] {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = ?",
        )
        .bind(trigger)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1, "trigger {} must be installed by 0086", trigger);
    }
}

// ============================================================================
// INSERT writes (sync/import/favorites) populate the mirror
// ============================================================================

#[tokio::test]
async fn test_track_sources_insert_populates_library_items_with_best_source_metadata() {
    let pool = setup_test_db().await;
    seed_services(&pool).await;

    let (t1, primary_artist) = seed_complete_track(
        &pool,
        "Mirror Song",
        "Mirror Album",
        210_000,
        Some("lossless"),
    )
    .await;

    // First identity write — the INSERT OR IGNORE shape used by the favorites
    // sync (commands/favorites.rs:851).
    sqlx::query("INSERT OR IGNORE INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-mirror')")
        .bind(t1)
        .execute(&pool)
        .await
        .expect("favorites-style track_sources write must succeed");

    let (service, source_service, external_id, title, artist, duration_ms, quality) =
        mirror_row(&pool, t1)
            .await
            .expect("mirror row must exist after the write");
    assert_eq!(service, "spotify");
    assert_eq!(source_service, "spotify", "trigger must fill both columns");
    assert_eq!(external_id, "sp-mirror");
    assert_eq!(title, "Mirror Song");
    assert_eq!(artist, primary_artist, "primary role beats insertion order");
    assert_eq!(duration_ms, 210_000);
    assert_eq!(quality.as_deref(), Some("lossless"));

    // A better source arrives — the INSERT OR REPLACE shape used by the
    // service import paths (commands/service.rs:761 and friends). The mirror
    // must repoint to the new best source and MUST NOT duplicate rows.
    sqlx::query("INSERT OR REPLACE INTO track_sources (track_id, service_id, service_track_id, available, quality_score) VALUES (?, 2, 'qb-mirror', 1, 90)")
        .bind(t1)
        .execute(&pool)
        .await
        .expect("import-style track_sources write must succeed");

    assert_eq!(
        mirror_count(&pool).await,
        1,
        "one row per canonical track — no duplicates"
    );
    let (service, _source, external_id, _title, _artist, _duration, _quality) =
        mirror_row(&pool, t1).await.unwrap();
    assert_eq!(
        service, "qobuz",
        "better source wins, like the 0085 backfill"
    );
    assert_eq!(external_id, "qb-mirror");

    // The consumer invariant: playlist joins resolve through library_items.id.
    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'sync@test.dev', '{}', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, external_id, name) VALUES (?, 'pl-mirror', 'Mirror Playlist') RETURNING id",
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
    .expect("consumer join must resolve the continuously-synced row");
    assert_eq!(joined.0, t1, "library_items.id must equal tracks.id");
    assert_eq!(joined.1, "qb-mirror");
}

// ============================================================================
// Incomplete rows: the mirrored write succeeds, the mirror is skipped
// ============================================================================

#[tokio::test]
async fn test_incomplete_tracks_are_skipped_without_failing_the_write() {
    let pool = setup_test_db().await;
    seed_services(&pool).await;

    let artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Heal Artist') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    // Track without any artist rows (artist is NOT NULL in library_items).
    let no_artist: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('No Artist Song') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-noartist')")
        .bind(no_artist)
        .execute(&pool)
        .await
        .expect("write must succeed even though the mirror skips the row");
    assert!(mirror_row(&pool, no_artist).await.is_none());

    // Complete track with a blank external id.
    let blank_ext: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Blank Ext Song') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(blank_ext)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, '   ')",
    )
    .bind(blank_ext)
    .execute(&pool)
    .await
    .expect("write must succeed for a blank external id");
    assert!(mirror_row(&pool, blank_ext).await.is_none());

    assert_eq!(
        mirror_count(&pool).await,
        0,
        "incomplete tracks are not mirrored"
    );

    // Self-heal: once the missing artist link exists, the next identity write
    // (INSERT OR REPLACE, as the importer does on re-sync) mirrors the track.
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(no_artist)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT OR REPLACE INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-noartist')")
        .bind(no_artist)
        .execute(&pool)
        .await
        .unwrap();
    let row = mirror_row(&pool, no_artist)
        .await
        .expect("row appears once metadata is complete");
    assert_eq!(row.2, "sp-noartist");
    assert_eq!(row.3, "No Artist Song");
    assert_eq!(row.4, "Heal Artist");
}

// ============================================================================
// UPDATE: availability refreshes and catalog merges keep the mirror honest
// ============================================================================

#[tokio::test]
async fn test_source_updates_recompute_or_drop_the_mirror_row() {
    let pool = setup_test_db().await;
    seed_services(&pool).await;

    let (t1, _) = seed_complete_track(&pool, "Update Song", "Update Album", 180_000, None).await;
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, available, quality_score) VALUES (?, 1, 'sp-upd', 1, 50), (?, 2, 'qb-upd', 1, 90)")
        .bind(t1)
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(mirror_row(&pool, t1).await.unwrap().0, "qobuz");

    // Availability refresh marks the best source unavailable (the exact UPDATE
    // shape of the availability checker, commands/library.rs:3661): the mirror
    // repoints to the best remaining source.
    sqlx::query("UPDATE track_sources SET available = 0 WHERE track_id = ? AND service_id = 2")
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();
    let row = mirror_row(&pool, t1).await.unwrap();
    assert_eq!(
        row.0, "spotify",
        "unavailable best source must lose the mirror slot"
    );
    assert_eq!(row.2, "sp-upd");

    // A catalog merge reassigns the source to another track (the exact UPDATE
    // shape of library.rs:2917): the losing track's mirror row is recomputed
    // from its remaining sources and the winning track gains its own.
    let (t2, _) = seed_complete_track(&pool, "Merge Target", "Update Album", 100_000, None).await;
    sqlx::query("UPDATE track_sources SET track_id = ? WHERE track_id = ? AND service_id = 2")
        .bind(t2)
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();

    let t1_row = mirror_row(&pool, t1).await.unwrap();
    assert_eq!(
        t1_row.2, "sp-upd",
        "t1's mirror repoints to its remaining source"
    );
    let t2_row = mirror_row(&pool, t2)
        .await
        .expect("t2 gains the moved identity");
    assert_eq!(t2_row.2, "qb-upd");
    assert_eq!(
        mirror_count(&pool).await,
        2,
        "one row per track, no leftovers"
    );

    // Deleting t1's last source removes its mirror row (catalog repair DELETE,
    // commands/library.rs:2945).
    sqlx::query("DELETE FROM track_sources WHERE track_id = ? AND service_id = 1")
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        mirror_row(&pool, t1).await.is_none(),
        "a track without service identity must leave the mirror"
    );
    assert_eq!(mirror_count(&pool).await, 1);
}

// ============================================================================
// DELETE: track deletion (incl. the catalog cascade) leaves no ghosts
// ============================================================================

#[tokio::test]
async fn test_track_deletion_removes_the_mirror_row() {
    let pool = setup_test_db().await;
    seed_services(&pool).await;

    let (t1, _) = seed_complete_track(&pool, "Doomed Song", "Doomed Album", 90_000, None).await;
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-doomed')")
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();
    assert!(mirror_row(&pool, t1).await.is_some());

    // Deleting the track cascades to track_sources (PRAGMA foreign_keys = ON,
    // as the app enforces) and the mirror row must not survive.
    sqlx::query("DELETE FROM tracks WHERE id = ?")
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        mirror_count(&pool).await,
        0,
        "deleting a track must remove its library_items ghost"
    );
}

// ============================================================================
// The full library reset leaves the mirror consistent (no ghosts, no loss)
// ============================================================================

#[tokio::test]
async fn test_reset_style_bulk_source_delete_empties_the_mirror() {
    let pool = setup_test_db().await;
    seed_services(&pool).await;

    let (t1, _) = seed_complete_track(&pool, "Reset Song", "Reset Album", 120_000, None).await;
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-reset')")
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(mirror_count(&pool).await, 1);

    // The full library reset wipes track_sources before tracks
    // (commands/library.rs:1331).
    sqlx::query("DELETE FROM track_sources")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(
        mirror_count(&pool).await,
        0,
        "the reset must not leave stale library_items rows behind"
    );

    // And the mirror comes back to life with the next sync write.
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp-reset2')")
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();
    let row = mirror_row(&pool, t1)
        .await
        .expect("mirror repopulates after the reset");
    assert_eq!(row.2, "sp-reset2");
    assert_eq!(row.3, "Reset Song");
}

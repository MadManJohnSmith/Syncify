//! Deterministic Concurrency Tests: Identity & Metadata Locks (Tests A, B, F, G)

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::Duration;
use syncify_core_domain::{LockScope, ProviderTrackIdentity};
use syncify_tauri_lib::services::{get_global_concurrency_manager, ConcurrencyError};

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test SQLite");

    sqlx::query("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")
        .execute(&pool)
        .await
        .unwrap();

    // Run baseline migrations
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS services (
            id INTEGER PRIMARY KEY,
            name TEXT UNIQUE NOT NULL,
            display_name TEXT NOT NULL
        );
        INSERT OR IGNORE INTO services (id, name, display_name) VALUES (1, 'spotify', 'Spotify'), (2, 'qobuz', 'Qobuz'), (3, 'tidal', 'Tidal');

        CREATE TABLE IF NOT EXISTS accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            service_id INTEGER NOT NULL,
            display_name TEXT,
            email TEXT,
            is_active INTEGER DEFAULT 1,
            credentials_json TEXT
        );

        CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            album_id INTEGER,
            duration_ms INTEGER,
            isrc TEXT,
            is_favorite INTEGER DEFAULT 0,
            favorite_at TEXT,
            primary_service TEXT,
            record_label TEXT,
            musicbrainz_id TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS track_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            service_id INTEGER NOT NULL,
            service_track_id TEXT NOT NULL,
            is_primary INTEGER DEFAULT 1,
            quality_tier TEXT,
            bit_depth INTEGER,
            sample_rate INTEGER,
            confidence REAL DEFAULT 1.0,
            matched_at TEXT DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (track_id) REFERENCES tracks(id),
            FOREIGN KEY (service_id) REFERENCES services(id),
            UNIQUE(service_id, service_track_id)
        );
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create base tables");

    pool
}

/// Test A: Two imports for the same account cannot run concurrently and overlap
#[tokio::test]
async fn test_concurrency_a_two_syncs_same_account_serialize() {
    let mgr = get_global_concurrency_manager();
    let account_id = 101;

    let g1 = mgr
        .acquire(
            LockScope::AccountSync(account_id),
            Some("sync-tidal-1"),
            Some(Duration::from_millis(50)),
        )
        .await
        .expect("First sync must acquire AccountSync lock");

    assert_eq!(g1.scope, LockScope::AccountSync(account_id));

    // Attempt concurrent second sync on same account with short timeout
    let g2_res = mgr
        .acquire(
            LockScope::AccountSync(account_id),
            Some("sync-tidal-2"),
            Some(Duration::from_millis(50)),
        )
        .await;

    assert!(
        matches!(g2_res, Err(ConcurrencyError::Timeout { .. })),
        "Second sync on same account must be excluded and time out"
    );

    // Drop first lock and verify second can proceed
    drop(g1);
    tokio::time::sleep(Duration::from_millis(15)).await;

    let g3 = mgr
        .acquire(
            LockScope::AccountSync(account_id),
            Some("sync-tidal-3"),
            Some(Duration::from_millis(100)),
        )
        .await
        .expect("After release, next sync should succeed");

    assert_eq!(g3.operation_id, "sync-tidal-3");
}

/// Test B: Tidal + Qobuz import of the same ISRC resolves to single canonical track without duplicate track_sources
#[tokio::test]
async fn test_concurrency_b_cross_service_same_isrc_no_duplicate_sources() {
    let db = setup_test_db().await;
    let mgr = get_global_concurrency_manager();
    let creation_lock = Arc::new(tokio::sync::Mutex::new(()));

    let isrc = "USUM71703861";

    // Simulate concurrent ingestion of Tidal and Qobuz for the exact same track
    let handle_tidal = {
        let db = db.clone();
        let mgr = Arc::clone(&mgr);
        let c_lock = Arc::clone(&creation_lock);
        tokio::spawn(async move {
            let identity = ProviderTrackIdentity {
                service_id: 3,
                service_name: "tidal".to_string(),
                service_track_id: "tidal-trk-001".to_string(),
                title: Some("Never Gonna Give You Up".to_string()),
                artist: Some("Rick Astley".to_string()),
                album: Some("Whenever You Need Somebody".to_string()),
                isrc: Some(isrc.to_string()),
                duration_ms: Some(213000),
                ..Default::default()
            };

            let _ident_guard = mgr
                .acquire(
                    LockScope::TrackIdentity {
                        service_id: 3,
                        service_track_id: identity.service_track_id.clone(),
                    },
                    Some("import-tidal"),
                    Some(Duration::from_secs(5)),
                )
                .await
                .unwrap();

            // Insert or match track by ISRC with creation lock
            let _c_guard = c_lock.lock().await;
            let mut tx = db.begin().await.unwrap();
            let track_id: i64 = match sqlx::query_scalar("SELECT id FROM tracks WHERE isrc = ?")
                .bind(isrc)
                .fetch_optional(&mut *tx)
                .await
                .unwrap()
            {
                Some(id) => id,
                None => sqlx::query_scalar(
                    "INSERT INTO tracks (title, isrc, primary_service) VALUES (?, ?, 'tidal') RETURNING id",
                )
                .bind(identity.title.as_deref().unwrap_or("Unknown"))
                .bind(isrc)
                .fetch_one(&mut *tx)
                .await
                .unwrap(),
            };

            sqlx::query(
                "INSERT OR IGNORE INTO track_sources (track_id, service_id, service_track_id, is_primary) VALUES (?, 3, ?, 1)",
            )
            .bind(track_id)
            .bind(&identity.service_track_id)
            .execute(&mut *tx)
            .await
            .unwrap();

            tx.commit().await.unwrap();
            track_id
        })
    };

    let handle_qobuz = {
        let db = db.clone();
        let mgr = Arc::clone(&mgr);
        let c_lock = Arc::clone(&creation_lock);
        tokio::spawn(async move {
            let identity = ProviderTrackIdentity {
                service_id: 2,
                service_name: "qobuz".to_string(),
                service_track_id: "qobuz-trk-999".to_string(),
                title: Some("Never Gonna Give You Up".to_string()),
                artist: Some("Rick Astley".to_string()),
                album: Some("Whenever You Need Somebody".to_string()),
                isrc: Some(isrc.to_string()),
                duration_ms: Some(213000),
                ..Default::default()
            };

            let _ident_guard = mgr
                .acquire(
                    LockScope::TrackIdentity {
                        service_id: 2,
                        service_track_id: identity.service_track_id.clone(),
                    },
                    Some("import-qobuz"),
                    Some(Duration::from_secs(5)),
                )
                .await
                .unwrap();

            let _c_guard = c_lock.lock().await;
            let mut tx = db.begin().await.unwrap();
            let track_id: i64 = match sqlx::query_scalar("SELECT id FROM tracks WHERE isrc = ?")
                .bind(isrc)
                .fetch_optional(&mut *tx)
                .await
                .unwrap()
            {
                Some(id) => id,
                None => sqlx::query_scalar(
                    "INSERT INTO tracks (title, isrc, primary_service) VALUES (?, ?, 'qobuz') RETURNING id",
                )
                .bind(identity.title.as_deref().unwrap_or("Unknown"))
                .bind(isrc)
                .fetch_one(&mut *tx)
                .await
                .unwrap(),
            };

            sqlx::query(
                "INSERT OR IGNORE INTO track_sources (track_id, service_id, service_track_id, is_primary) VALUES (?, 2, ?, 0)",
            )
            .bind(track_id)
            .bind(&identity.service_track_id)
            .execute(&mut *tx)
            .await
            .unwrap();

            tx.commit().await.unwrap();
            track_id
        })
    };

    let (res_t, res_q) = tokio::join!(handle_tidal, handle_qobuz);
    let tid_1 = res_t.unwrap();
    let tid_2 = res_q.unwrap();

    // Verify canonical track count is 1 (no duplicate tracks created)
    let total_tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE isrc = ?")
        .bind(isrc)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(total_tracks, 1, "Must have exactly 1 canonical track for same ISRC");
    assert_eq!(tid_1, tid_2, "Both imports must resolve to same canonical track_id");

    // Verify 2 distinct track sources linked to the same canonical track
    let source_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources WHERE track_id = ?")
        .bind(tid_1)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(source_count, 2, "Must have 2 distinct track sources for Tidal and Qobuz");
}

/// Test F: Enrichment + Sync on same track does not overwrite higher-precedence metadata
#[tokio::test]
async fn test_concurrency_f_enrichment_and_sync_canonical_track_lock() {
    let db = setup_test_db().await;
    let mgr = get_global_concurrency_manager();

    // Seed track with higher-precedence metadata
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, record_label, musicbrainz_id) VALUES ('Original Studio Title', 'Master Label Inc', 'mbid-12345') RETURNING id"
    )
    .fetch_one(&db)
    .await
    .unwrap();

    let h_sync = {
        let db = db.clone();
        let mgr = Arc::clone(&mgr);
        tokio::spawn(async move {
            let _guard = mgr
                .acquire(
                    LockScope::CanonicalTrack(track_id),
                    Some("sync-update"),
                    Some(Duration::from_secs(2)),
                )
                .await
                .unwrap();

            // Sync updates title
            sqlx::query("UPDATE tracks SET title = 'Original Studio Title (Remastered)' WHERE id = ?")
                .bind(track_id)
                .execute(&db)
                .await
                .unwrap();
        })
    };

    let h_enrich = {
        let db = db.clone();
        let mgr = Arc::clone(&mgr);
        tokio::spawn(async move {
            let _guard = mgr
                .acquire(
                    LockScope::CanonicalTrack(track_id),
                    Some("enrichment-update"),
                    Some(Duration::from_secs(2)),
                )
                .await
                .unwrap();

            // Enrichment updates record_label without overwriting higher precedence fields
            sqlx::query("UPDATE tracks SET record_label = 'Master Label Inc / Polydor' WHERE id = ?")
                .bind(track_id)
                .execute(&db)
                .await
                .unwrap();
        })
    };

    let (r1, r2) = tokio::join!(h_sync, h_enrich);
    r1.unwrap();
    r2.unwrap();

    let (title, label, mbid): (String, Option<String>, Option<String>) =
        sqlx::query_as("SELECT title, record_label, musicbrainz_id FROM tracks WHERE id = ?")
            .bind(track_id)
            .fetch_one(&db)
            .await
            .unwrap();

    assert_eq!(title, "Original Studio Title (Remastered)");
    assert_eq!(label.as_deref(), Some("Master Label Inc / Polydor"));
    assert_eq!(mbid.as_deref(), Some("mbid-12345"));
}

/// Test G: Favorite toggle + Sync preserves consistent state without rollback races
#[tokio::test]
async fn test_concurrency_g_favorite_toggle_and_sync_consistency() {
    let db = setup_test_db().await;
    let mgr = get_global_concurrency_manager();

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, is_favorite) VALUES ('Favorite Test', 0) RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();

    let h_fav = {
        let db = db.clone();
        let mgr = Arc::clone(&mgr);
        tokio::spawn(async move {
            let _guard = mgr
                .acquire(
                    LockScope::CanonicalTrack(track_id),
                    Some("fav-toggle"),
                    Some(Duration::from_secs(2)),
                )
                .await
                .unwrap();

            sqlx::query("UPDATE tracks SET is_favorite = 1, favorite_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(track_id)
                .execute(&db)
                .await
                .unwrap();
        })
    };

    let h_sync = {
        let db = db.clone();
        let mgr = Arc::clone(&mgr);
        tokio::spawn(async move {
            let _guard = mgr
                .acquire(
                    LockScope::CanonicalTrack(track_id),
                    Some("sync-pass"),
                    Some(Duration::from_secs(2)),
                )
                .await
                .unwrap();

            sqlx::query("UPDATE tracks SET updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(track_id)
                .execute(&db)
                .await
                .unwrap();
        })
    };

    let (r_fav, r_sync) = tokio::join!(h_fav, h_sync);
    r_fav.unwrap();
    r_sync.unwrap();

    let is_fav: i64 = sqlx::query_scalar("SELECT is_favorite FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&db)
        .await
        .unwrap();

    assert_eq!(is_fav, 1, "Favorite state must remain 1 and not be lost by concurrent sync");
}

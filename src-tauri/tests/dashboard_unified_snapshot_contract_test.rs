//! FE-PERF-04 (#51): contrato de la lectura unificada del Dashboard.
//!
//! `get_dashboard_snapshot` debe devolver, en un solo payload, EXACTAMENTE
//! las mismas cifras que antes se leían por separado:
//!   - `stats`     == `get_library_stats` (vista `library_stats`)
//!   - `metadata`  == `get_metadata_stats` (misma consulta, sin re-cálculos)
//!   - `lyrics`    == `get_lyrics_stats`
//!   - `snapshots` == `get_library_snapshots(days)`
//!   - `snapshot`  == el snapshot del día que escribía `create_library_snapshot`
//!
//! Además fija la diferencia de semántica que ya existía entre la vista y el
//! snapshot para `total_artists` (la vista cuenta todos los artists; el
//! snapshot solo los con presencia o favorito) — las dos cifras conviven en
//! el payload sin mezclarse.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::sync::Arc;
use syncify_tauri_lib::commands::dashboard::get_dashboard_snapshot;
use syncify_tauri_lib::commands::library::get_library_stats;
use syncify_tauri_lib::commands::lyrics::get_lyrics_stats;
use syncify_tauri_lib::commands::metadata::get_metadata_stats;
use syncify_tauri_lib::services::ConcurrencyManager;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::{AppState, EnrichmentWorkerState};
use tauri::Manager;

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

/// Biblioteca sembrada con recuentos no triviales:
///   - 2 álbumes, 4 artistas (a4 sin presencia y sin favorito: la vista lo
///     cuenta, el snapshot NO), 4 pistas (2 lossless, 1 hires, 1 lossy),
///   - 2 descargas (3000 bytes en total), cola con 1 queued + 1 downloading,
///   - 2 pistas con letra (1 sincronizada lrc).
async fn seed_library(pool: &SqlitePool) {
    let alb1: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Album One') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    let alb2: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Album Two') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();

    let a1: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Artist One') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    let a2: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Artist Two') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    // Favorito sin presencia en pistas: lo cuentan AMBAS semánticas.
    sqlx::query("INSERT INTO artists (name, is_favorite) VALUES ('Fav Artist', 1)")
        .execute(pool)
        .await
        .unwrap();
    // Huérfano: ni pistas ni favorito. La vista lo cuenta; el snapshot no.
    sqlx::query("INSERT INTO artists (name) VALUES ('Orphan Artist')")
        .execute(pool)
        .await
        .unwrap();

    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, audio_quality, genre, isrc, release_year) VALUES ('T1', ?, 'lossless', 'Rock', 'ISRC1', 2020) RETURNING id",
    )
    .bind(alb1)
    .fetch_one(pool)
    .await
    .unwrap();
    let t2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, audio_quality, genre) VALUES ('T2', ?, 'hires', 'Pop') RETURNING id",
    )
    .bind(alb1)
    .fetch_one(pool)
    .await
    .unwrap();
    let t3: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, audio_quality) VALUES ('T3', ?, 'lossy') RETURNING id",
    )
    .bind(alb2)
    .fetch_one(pool)
    .await
    .unwrap();
    let t4: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, audio_quality) VALUES ('T4', ?, 'lossless') RETURNING id",
    )
    .bind(alb2)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary'), (?, ?, 'primary'), (?, ?, 'primary'), (?, ?, 'primary')")
        .bind(t1).bind(a1)
        .bind(t2).bind(a1)
        .bind(t3).bind(a2)
        .bind(t4).bind(a2)
        .execute(pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO lyrics (track_id, format, content) VALUES (?, 'lrc', '[00:01] la'), (?, 'plain', 'la la')")
        .bind(t1)
        .bind(t3)
        .execute(pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO downloads (track_id, file_path, file_size_bytes) VALUES (?, '/lib/T1.flac', 1000), (?, '/lib/T2.flac', 2000)")
        .bind(t1)
        .bind(t2)
        .execute(pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO download_queue (track_id, status) VALUES (?, 'queued'), (?, 'downloading')",
    )
    .bind(t3)
    .bind(t4)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn test_dashboard_snapshot_unified_payload_matches_separate_reads() {
    let pool = setup_test_db().await;
    seed_library(&pool).await;

    let app = tauri::test::mock_app();
    app.manage(AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(ConcurrencyManager::new()),
    });

    let payload = get_dashboard_snapshot(app.state::<AppState>(), 30)
        .await
        .expect("get_dashboard_snapshot must succeed");

    // stats == get_library_stats (vista library_stats), campo a campo.
    let stats = get_library_stats(app.state::<AppState>())
        .await
        .expect("get_library_stats must succeed");
    assert_eq!(payload.stats.total_tracks, stats.total_tracks);
    assert_eq!(payload.stats.total_artists, stats.total_artists);
    assert_eq!(payload.stats.total_albums, stats.total_albums);
    assert_eq!(payload.stats.total_downloads, stats.total_downloads);
    assert_eq!(payload.stats.queued_downloads, stats.queued_downloads);
    assert_eq!(payload.stats.active_downloads, stats.active_downloads);
    assert_eq!(payload.stats.library_entries, stats.library_entries);
    assert_eq!(payload.stats.playlists, stats.playlists);
    assert_eq!(payload.stats.services_with_data, stats.services_with_data);

    // metadata == get_metadata_stats (misma consulta, sin re-cálculos).
    let metadata = get_metadata_stats(app.state::<AppState>())
        .await
        .expect("get_metadata_stats must succeed");
    assert_eq!(payload.metadata.total_tracks, metadata.total_tracks);
    assert_eq!(payload.metadata.with_isrc, metadata.with_isrc);
    assert_eq!(
        payload.metadata.with_musicbrainz_id,
        metadata.with_musicbrainz_id
    );
    assert_eq!(payload.metadata.with_album, metadata.with_album);
    assert_eq!(payload.metadata.with_year, metadata.with_year);
    assert_eq!(payload.metadata.with_genre, metadata.with_genre);
    assert_eq!(payload.metadata.with_art, metadata.with_art);
    assert!(
        (payload.metadata.average_completeness - metadata.average_completeness).abs() < 1e-9,
        "average_completeness diverged: {} vs {}",
        payload.metadata.average_completeness,
        metadata.average_completeness
    );

    // lyrics == get_lyrics_stats.
    let lyrics = get_lyrics_stats(app.state::<AppState>())
        .await
        .expect("get_lyrics_stats must succeed");
    assert_eq!(payload.lyrics.total_tracks, lyrics.total_tracks);
    assert_eq!(payload.lyrics.with_lyrics, lyrics.with_lyrics);
    assert_eq!(payload.lyrics.synced_lyrics, lyrics.synced_lyrics);
    assert_eq!(payload.lyrics.embedded_lyrics, lyrics.embedded_lyrics);

    // snapshots == get_library_snapshots(days), y el snapshot del día
    // recién escrito aparece en el historial (como en la app: se crea y
    // luego se lee la ventana de días).
    let snapshots =
        syncify_tauri_lib::commands::dashboard::get_library_snapshots(app.state::<AppState>(), 30)
            .await
            .expect("get_library_snapshots must succeed");
    assert_eq!(payload.snapshots.len(), snapshots.len());
    for (a, b) in payload.snapshots.iter().zip(snapshots.iter()) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.snapshot_date, b.snapshot_date);
        assert_eq!(a.total_tracks, b.total_tracks);
        assert_eq!(a.downloaded_tracks, b.downloaded_tracks);
    }
    assert!(
        payload
            .snapshots
            .iter()
            .any(|s| s.id == payload.snapshot.id),
        "today's snapshot must be in the history window"
    );

    // snapshot == las cifras que escribía create_library_snapshot, con la
    // MISMA semántica por columna (verificado contra el seed).
    let snap = &payload.snapshot;
    assert_eq!(snap.total_tracks, 4, "COUNT(*) FROM tracks");
    assert_eq!(
        snap.total_albums, 2,
        "COUNT(*) FROM albums (semántica snapshot, no la CASE de la vista)"
    );
    // El snapshot cuenta artistas con presencia o favorito (3); la vista
    // cuenta TODOS los artists (4). Ambas cifras conviven sin mezclarse.
    assert_eq!(snap.total_artists, 3, "snapshot: presencia o favorito");
    assert_eq!(
        payload.stats.total_artists, 4,
        "vista: COUNT(*) FROM artists"
    );
    assert_eq!(snap.downloaded_tracks, 2, "COUNT(*) FROM downloads");
    assert_eq!(snap.total_size_bytes, 3000);
    assert_eq!(snap.tracks_with_lyrics, 2, "letras con contenido no vacío");
    assert_eq!(snap.tracks_lossless, 2);
    assert_eq!(snap.tracks_hires, 1);

    // Los recuentos de cola de la vista llegan en stats, no en el snapshot.
    assert_eq!(payload.stats.queued_downloads, 1);
    assert_eq!(payload.stats.active_downloads, 1);
}

#[tokio::test]
async fn test_dashboard_snapshot_upserts_todays_row() {
    let pool = setup_test_db().await;
    seed_library(&pool).await;

    let app = tauri::test::mock_app();
    app.manage(AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(ConcurrencyManager::new()),
    });

    let first = get_dashboard_snapshot(app.state::<AppState>(), 30)
        .await
        .expect("first read must succeed");
    let second = get_dashboard_snapshot(app.state::<AppState>(), 30)
        .await
        .expect("second read must succeed");

    assert_eq!(
        second.snapshot.id, first.snapshot.id,
        "re-running updates today's snapshot in place"
    );

    let (rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM library_snapshots")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1, "one snapshot row for today, not one per call");
}

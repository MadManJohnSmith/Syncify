//! Guard against treating `metadata_score` as a materialized SQLite column.
//! PMS-01: every score here is derived by the reader, not stored by migration/sync.

use sqlx::{sqlite::SqlitePoolOptions, Row};
use std::sync::Arc;
use syncify_tauri_lib::{
    commands::get_library, enrichment_worker::EnrichmentWorkerState, services::ConcurrencyManager,
    worker::DownloadWorkerState, AppState,
};
use tauri::Manager;

#[tokio::test]
async fn metadata_score_is_a_live_alias_not_a_column_to_backfill() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    for table in ["tracks", "downloads", "metadata_preferences"] {
        let pragma = format!("PRAGMA table_info({table})");
        let columns = sqlx::query(&pragma).fetch_all(&pool).await.unwrap();
        assert!(
            columns
                .iter()
                .all(|col| col.get::<String, _>("name") != "metadata_score"),
            "{table} unexpectedly materialized metadata_score"
        );
    }
    let downloads_columns = sqlx::query("PRAGMA table_info(downloads)")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(downloads_columns
        .iter()
        .any(|col| col.get::<String, _>("name") == "metadata_completeness"));

    let artist: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Artist') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let album: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Album') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let track: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Track', ?) RETURNING id")
            .bind(album)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track)
        .bind(artist)
        .execute(&pool)
        .await
        .unwrap();
    let service: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'spotify'")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, metadata_completeness) VALUES (?, ?, '/tmp/synthetic.flac', 7)")
        .bind(track).bind(service).execute(&pool).await.unwrap();

    let app = tauri::test::mock_app();
    app.manage(AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(ConcurrencyManager::new()),
    });
    let page = get_library(app.state::<AppState>(), None, None)
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.tracks[0].metadata_score, Some(30));

    // No UPDATE of any score column. Changing source metadata changes the SELECT alias.
    sqlx::query("UPDATE tracks SET isrc = 'US1234567890' WHERE id = ?")
        .bind(track)
        .execute(&pool)
        .await
        .unwrap();
    let page = get_library(app.state::<AppState>(), None, None)
        .await
        .unwrap();
    assert_eq!(page.tracks[0].metadata_score, Some(50));
    let persisted: i64 =
        sqlx::query_scalar("SELECT metadata_completeness FROM downloads WHERE track_id = ?")
            .bind(track)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        persisted, 7,
        "the materialized download column has a separate meaning"
    );
}

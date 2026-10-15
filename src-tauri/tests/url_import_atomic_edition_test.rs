//! Offline URL import atomicity and preservation of independent provider items.
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::str::FromStr;
use std::sync::Arc;
use syncify_tauri_lib::commands::url_import::perform_import_from_url;
use syncify_tauri_lib::download::{orchestrator::DownloadOrchestrator, songlink::SongLinkClient};

fn offline_orchestrator() -> DownloadOrchestrator {
    DownloadOrchestrator::new().with_songlink(Arc::new(
        SongLinkClient::new().with_base_url("http://127.0.0.1:1".to_string()),
    ))
}

async fn fixture() -> SqlitePool {
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    db
}

async fn counts(db: &SqlitePool) -> (i64, i64, i64) {
    sqlx::query_as("SELECT (SELECT count(*) FROM tracks), (SELECT count(*) FROM track_sources), (SELECT count(*) FROM download_queue)")
        .fetch_one(db).await.unwrap()
}

#[tokio::test]
async fn queue_provenance_insert_failure_rolls_back_every_row() {
    let db = fixture().await;
    sqlx::query("CREATE TRIGGER reject_url_provenance BEFORE INSERT ON download_queue WHEN NEW.origin_service IS NOT NULL BEGIN SELECT RAISE(FAIL, 'synthetic provenance failure'); END")
        .execute(&db).await.unwrap();
    let result = perform_import_from_url(
        &db,
        Some(&offline_orchestrator()),
        "https://tidal.com/browse/track/34782012",
    )
    .await;
    assert!(result.is_err(), "{result:?}");
    assert_eq!(counts(&db).await, (0, 0, 0));
}

#[tokio::test]
async fn preexisting_rows_survive_failed_import_and_two_provider_items_stay_distinct() {
    let db = fixture().await;
    let original = perform_import_from_url(
        &db,
        Some(&offline_orchestrator()),
        "https://tidal.com/browse/track/10101",
    )
    .await
    .unwrap();
    let before = counts(&db).await;
    sqlx::query("CREATE TRIGGER reject_second_provenance BEFORE INSERT ON download_queue WHEN NEW.service_track_id = '20202' BEGIN SELECT RAISE(FAIL, 'synthetic second edition failure'); END")
        .execute(&db).await.unwrap();
    assert!(perform_import_from_url(
        &db,
        Some(&offline_orchestrator()),
        "https://tidal.com/browse/track/20202"
    )
    .await
    .is_err());
    assert_eq!(counts(&db).await, before);
    sqlx::query("DROP TRIGGER reject_second_provenance")
        .execute(&db)
        .await
        .unwrap();
    let second = perform_import_from_url(
        &db,
        Some(&offline_orchestrator()),
        "https://tidal.com/browse/track/20202",
    )
    .await
    .unwrap();
    assert_ne!(original.track_id, second.track_id);
    assert_eq!(counts(&db).await, (2, 2, 2));
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT service_track_id FROM track_sources ORDER BY service_track_id")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(ids, ["10101", "20202"]);
    let repeated = perform_import_from_url(
        &db,
        Some(&offline_orchestrator()),
        "https://tidal.com/browse/track/10101",
    )
    .await
    .unwrap();
    assert_eq!(repeated.queue_id, original.queue_id);
    assert_eq!(counts(&db).await, (2, 2, 2));
}

#[tokio::test]
async fn existing_target_failure_preserves_existing_source_and_queue() {
    let db = fixture().await;
    let orchestrator = offline_orchestrator();
    let original = perform_import_from_url(
        &db,
        Some(&orchestrator),
        "https://tidal.com/browse/track/30303",
    )
    .await
    .unwrap();
    let existing_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks(title) VALUES ('synthetic preexisting target') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_sources(track_id, service_id, service_track_id, available) VALUES (?, (SELECT id FROM services WHERE name = 'tidal'), '40404', 1)")
        .bind(existing_id).execute(&db).await.unwrap();
    let before = counts(&db).await;
    sqlx::query("CREATE TRIGGER reject_existing_queue BEFORE INSERT ON download_queue WHEN NEW.service_track_id = '40404' BEGIN SELECT RAISE(FAIL, 'synthetic existing target failure'); END")
        .execute(&db).await.unwrap();
    assert!(perform_import_from_url(
        &db,
        Some(&orchestrator),
        "https://tidal.com/browse/track/40404"
    )
    .await
    .is_err());
    assert_eq!(counts(&db).await, before);
    let surviving: i64 = sqlx::query_scalar("SELECT count(*) FROM download_queue WHERE id = ?")
        .bind(original.queue_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(surviving, 1);
    let source: i64 =
        sqlx::query_scalar("SELECT track_id FROM track_sources WHERE service_track_id = '40404'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(source, existing_id);
}

#[tokio::test]
async fn concurrent_imports_reuse_one_source_and_queue_after_restart() {
    // Separate connections to one disposable file exercise SQLite writer
    // contention. The temp dir is self-contained so the test does not depend on
    // the runner defining an isolated TMPDIR.
    let fixture = tempfile::tempdir().expect("isolated test directory");
    let path = fixture.path().join("url-import.sqlite");
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
        .unwrap()
        .create_if_missing(true)
        .busy_timeout(std::time::Duration::from_secs(10));
    let db = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options.clone())
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    let url = "https://tidal.com/browse/track/50505";
    let (a, b) = tokio::join!(
        async { perform_import_from_url(&db, Some(&offline_orchestrator()), url).await },
        async { perform_import_from_url(&db, Some(&offline_orchestrator()), url).await },
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.queue_id, b.queue_id);
    assert_eq!(counts(&db).await, (1, 1, 1));
    db.close().await;
    let reopened = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .unwrap();
    let c = perform_import_from_url(&reopened, Some(&offline_orchestrator()), url)
        .await
        .unwrap();
    assert_eq!(c.queue_id, a.queue_id);
    assert_eq!(counts(&reopened).await, (1, 1, 1));
    reopened.close().await;
}

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::services::enrichment::{
    EnrichmentEngine, OriginTrackMetadata, SyncTrackInput,
};

async fn setup() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}
fn input(
    service_id: i64,
    service: &str,
    item: &str,
    isrc: Option<&str>,
    release: Option<&str>,
) -> SyncTrackInput {
    SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Identical Song".into()),
            artist: Some("Test Artist".into()),
            album: Some("Album".into()),
            isrc: isrc.map(str::to_owned),
            source_name: service.to_owned(),
            ..Default::default()
        },
        service_track_id: item.into(),
        service_name: service.into(),
        service_id,
        account_id: 1,
        is_favorite: false,
        is_purchased: false,
        format: None,
        bit_depth: None,
        sample_rate: None,
        quality_score: None,
        audio_quality: None,
        cover_art_url: None,
        duration_ms: Some(200000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: release.map(str::to_owned),
    }
}
#[tokio::test]
async fn distinct_items_without_recording_evidence_stay_distinct() {
    let pool = setup().await;
    let sid: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let account: i64 = sqlx::query_scalar(
        "INSERT INTO accounts(service_id,display_name) VALUES (?, 'fixture') RETURNING id",
    )
    .bind(sid)
    .fetch_one(&pool)
    .await
    .unwrap();
    let engine = EnrichmentEngine::new();
    let mut a = input(sid, "tidal", "item-a", None, Some("release-a"));
    a.account_id = account;
    let mut b = input(sid, "tidal", "item-b", None, Some("release-b"));
    b.account_id = account;
    let first = engine
        .enrich_and_persist_sync_track(&pool, a)
        .await
        .unwrap();
    let second = engine
        .enrich_and_persist_sync_track(&pool, b)
        .await
        .unwrap();
    assert_ne!(first.track_id, second.track_id);
    let links: i64 = sqlx::query_scalar("SELECT count(*) FROM edition_item_tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(links, 2);
}
#[tokio::test]
async fn same_isrc_different_item_fails_closed_without_overwrite() {
    let pool = setup().await;
    let sid: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let account: i64 = sqlx::query_scalar(
        "INSERT INTO accounts(service_id,display_name) VALUES (?, 'fixture') RETURNING id",
    )
    .bind(sid)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut first = input(
        sid,
        "qobuz",
        "edition-a",
        Some("ED02ISRC0001"),
        Some("release-a"),
    );
    first.account_id = account;
    let engine = EnrichmentEngine::new();
    let original = engine
        .enrich_and_persist_sync_track(&pool, first.clone())
        .await
        .unwrap();
    let repeated = engine
        .enrich_and_persist_sync_track(&pool, first)
        .await
        .unwrap();
    assert_eq!(original.track_id, repeated.track_id);
    let mut second = input(
        sid,
        "qobuz",
        "edition-b",
        Some("ED02ISRC0001"),
        Some("release-b"),
    );
    second.account_id = account;
    let error = engine
        .enrich_and_persist_sync_track(&pool, second)
        .await
        .unwrap_err();
    assert!(error.contains("needs_review"), "{error}");
    let source: (i64, String) =
        sqlx::query_as("SELECT track_id,service_track_id FROM track_sources WHERE service_id=?")
            .bind(sid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(source, (original.track_id, "edition-a".into()));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM edition_provider_items WHERE service_id=?")
            .bind(sid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    let rolled_back: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM edition_releases WHERE provider_release_id='release-b'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rolled_back, 0);
}

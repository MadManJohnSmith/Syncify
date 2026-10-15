//! Synthetic same-title, different-UPC release separation during sync.
use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::services::enrichment::{
    EnrichmentEngine, OriginTrackMetadata, SyncTrackInput,
};

#[tokio::test]
async fn distinct_upcs_never_merge_same_titled_albums() {
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();
    let sid: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&db)
        .await
        .unwrap();
    let account: i64 = sqlx::query_scalar("INSERT INTO accounts(service_id, display_name, email) VALUES (?, 'Synthetic', 'synthetic@test.invalid') RETURNING id")
        .bind(sid).fetch_one(&db).await.unwrap();
    let engine = EnrichmentEngine::new();
    let mut albums = Vec::new();
    for (item, upc) in [("edition-1", "000000000001"), ("edition-2", "000000000002")] {
        let result = engine
            .enrich_and_persist_sync_track(
                &db,
                SyncTrackInput {
                    origin_meta: OriginTrackMetadata {
                        title: Some("Same Recording".into()),
                        artist: Some("Synthetic Artist".into()),
                        album_artist: Some("Synthetic Artist".into()),
                        album: Some("Shared Title".into()),
                        isrc: None,
                        barcode: Some(upc.into()),
                        source_name: "qobuz".into(),
                        ..Default::default()
                    },
                    service_track_id: item.into(),
                    service_name: "qobuz".into(),
                    service_id: sid,
                    account_id: account,
                    query_musicbrainz: false,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let album: i64 = sqlx::query_scalar("SELECT album_id FROM tracks WHERE id = ?")
            .bind(result.track_id)
            .fetch_one(&db)
            .await
            .unwrap();
        albums.push(album);
    }
    assert_ne!(albums[0], albums[1]);
    let upcs: Vec<String> =
        sqlx::query_scalar("SELECT upc FROM albums WHERE title = 'Shared Title' ORDER BY upc")
            .fetch_all(&db)
            .await
            .unwrap();
    assert_eq!(upcs, ["000000000001", "000000000002"]);
}

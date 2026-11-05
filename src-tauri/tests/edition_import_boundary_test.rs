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
async fn same_isrc_different_item_links_edition_without_overwriting_slot() {
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
    second.is_favorite = true;
    // Route 2 (2026-10-18): the same recording on another release links to the
    // canonical track instead of failing closed, keeps the favorite entry and
    // never overwrites the legacy source slot.
    let linked = engine
        .enrich_and_persist_sync_track(&pool, second)
        .await
        .unwrap();
    assert_eq!(linked.track_id, original.track_id);
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
    assert_eq!(count, 2);
    let links: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM edition_item_tracks link JOIN edition_provider_items item ON item.id = link.item_id WHERE item.service_id = ? AND link.track_id = ?",
    )
    .bind(sid)
    .bind(original.track_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(links, 2);
    let release_kept: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM edition_releases WHERE provider_release_id='release-b'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(release_kept, 1);
    let liked: Option<i64> = sqlx::query_scalar(
        "SELECT is_liked FROM library_entries WHERE account_id = ? AND track_id = ?",
    )
    .bind(account)
    .bind(original.track_id)
    .fetch_optional(&pool)
    .await
    .unwrap();
    assert_eq!(liked, Some(1));
}

#[tokio::test]
async fn cross_service_same_isrc_reuses_canonical_track_and_keeps_album_identity() {
    let pool = setup().await;
    let qobuz_sid: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let tidal_sid: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let qobuz_account: i64 = sqlx::query_scalar(
        "INSERT INTO accounts(service_id,display_name) VALUES (?, 'qobuz-fixture') RETURNING id",
    )
    .bind(qobuz_sid)
    .fetch_one(&pool)
    .await
    .unwrap();
    let tidal_account: i64 = sqlx::query_scalar(
        "INSERT INTO accounts(service_id,display_name) VALUES (?, 'tidal-fixture') RETURNING id",
    )
    .bind(tidal_sid)
    .fetch_one(&pool)
    .await
    .unwrap();

    let engine = EnrichmentEngine::new();
    let mut qobuz_input = input(
        qobuz_sid,
        "qobuz",
        "qobuz-item",
        Some("ED02CROSS0001"),
        Some("release-q"),
    );
    qobuz_input.account_id = qobuz_account;
    let first = engine
        .enrich_and_persist_sync_track(&pool, qobuz_input)
        .await
        .unwrap();
    let album_after_first: Option<i64> =
        sqlx::query_scalar("SELECT album_id FROM tracks WHERE id = ?")
            .bind(first.track_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(first.is_new_global_track);

    let mut tidal_input = input(
        tidal_sid,
        "tidal",
        "tidal-item",
        Some("ED02CROSS0001"),
        Some("release-t"),
    );
    tidal_input.account_id = tidal_account;
    tidal_input.origin_meta.title = Some("Different Provider Title".into());
    tidal_input.origin_meta.album = Some("Different Provider Album".into());
    let second = engine
        .enrich_and_persist_sync_track(&pool, tidal_input)
        .await
        .unwrap();

    assert_eq!(first.track_id, second.track_id);
    assert!(!second.is_new_global_track);
    assert!(second.is_new_source_for_service);

    let track_count: i64 = sqlx::query_scalar("SELECT count(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(track_count, 1);
    let source_count: i64 = sqlx::query_scalar("SELECT count(*) FROM track_sources")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(source_count, 2);
    let item_count: i64 = sqlx::query_scalar("SELECT count(*) FROM edition_provider_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(item_count, 2);
    let link_count: i64 = sqlx::query_scalar("SELECT count(*) FROM edition_item_tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(link_count, 2);

    let album_after_second: Option<i64> =
        sqlx::query_scalar("SELECT album_id FROM tracks WHERE id = ?")
            .bind(first.track_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(album_after_first, album_after_second);

    let linked_accounts: Vec<i64> =
        sqlx::query_scalar("SELECT account_id FROM library_entries ORDER BY account_id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(linked_accounts, vec![qobuz_account, tidal_account]);

    // Re-importing the same provider item is idempotent: it must not create a
    // second source, ledger item or track.
    let mut tidal_retry = input(
        tidal_sid,
        "tidal",
        "tidal-item",
        Some("ED02CROSS0001"),
        Some("release-t"),
    );
    tidal_retry.account_id = tidal_account;
    let third = engine
        .enrich_and_persist_sync_track(&pool, tidal_retry)
        .await
        .unwrap();
    assert_eq!(third.track_id, first.track_id);
    assert!(!third.is_new_global_track);
    assert!(!third.is_new_source_for_service);
    let source_count: i64 = sqlx::query_scalar("SELECT count(*) FROM track_sources")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(source_count, 2);
    let item_count: i64 = sqlx::query_scalar("SELECT count(*) FROM edition_provider_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(item_count, 2);
}

#[tokio::test]
async fn isrc_case_variants_resolve_to_the_same_canonical_track() {
    let pool = setup().await;
    let spotify_sid: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'spotify'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let qobuz_sid: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let spotify_account: i64 = sqlx::query_scalar(
        "INSERT INTO accounts(service_id,display_name) VALUES (?, 'spotify-fixture') RETURNING id",
    )
    .bind(spotify_sid)
    .fetch_one(&pool)
    .await
    .unwrap();
    let qobuz_account: i64 = sqlx::query_scalar(
        "INSERT INTO accounts(service_id,display_name) VALUES (?, 'qobuz-fixture') RETURNING id",
    )
    .bind(qobuz_sid)
    .fetch_one(&pool)
    .await
    .unwrap();

    let engine = EnrichmentEngine::new();
    // Spotify writes lowercase ISRCs and Qobuz uppercase; the unique index is
    // `isrc COLLATE NOCASE`, so the lookup must ignore case or the INSERT dies
    // with `UNIQUE constraint failed: tracks.isrc` and the item is lost.
    let mut spotify_input = input(
        spotify_sid,
        "spotify",
        "spotify-item",
        Some("uscgh2139727"),
        Some("release-s"),
    );
    spotify_input.account_id = spotify_account;
    let first = engine
        .enrich_and_persist_sync_track(&pool, spotify_input)
        .await
        .unwrap();
    assert!(first.is_new_global_track);

    let mut qobuz_input = input(
        qobuz_sid,
        "qobuz",
        "qobuz-item",
        Some("USCGH2139727"),
        Some("release-q"),
    );
    qobuz_input.account_id = qobuz_account;
    let second = engine
        .enrich_and_persist_sync_track(&pool, qobuz_input)
        .await
        .unwrap();
    assert_eq!(first.track_id, second.track_id);
    assert!(!second.is_new_global_track);

    let track_count: i64 = sqlx::query_scalar("SELECT count(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(track_count, 1);
}

#[tokio::test]
async fn favorite_import_records_provider_item_with_its_release() {
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

    let mut fav = input(
        sid,
        "tidal",
        "fav-item-1",
        Some("ED02FAV00001"),
        Some("release-1"),
    );
    fav.account_id = account;
    fav.is_favorite = true;
    fav.origin_meta.album = Some("Storyteller".into());
    engine
        .enrich_and_persist_sync_track(&pool, fav.clone())
        .await
        .unwrap();

    let row: (String, Option<String>, String) = sqlx::query_as(
        "SELECT title, album_name, item_type FROM favorites WHERE account_id = ? AND service_id = ? AND service_item_id = 'fav-item-1'",
    )
    .bind(account)
    .bind(sid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        (
            "Identical Song".to_string(),
            Some("Storyteller".to_string()),
            "track".to_string()
        )
    );

    // Re-importing the same item is idempotent.
    engine
        .enrich_and_persist_sync_track(&pool, fav)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM favorites WHERE account_id = ?")
        .bind(account)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // A non-favorite import must not create favorite rows.
    let mut plain = input(
        sid,
        "tidal",
        "plain-item",
        Some("ED02FAV00002"),
        Some("release-2"),
    );
    plain.account_id = account;
    engine
        .enrich_and_persist_sync_track(&pool, plain)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM favorites WHERE account_id = ?")
        .bind(account)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

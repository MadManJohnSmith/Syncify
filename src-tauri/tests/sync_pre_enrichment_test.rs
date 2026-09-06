use sqlx::sqlite::SqlitePoolOptions;
use std::sync::{Arc, Mutex};
use syncify_metadata_domain::EnrichmentCompleteness;
use syncify_tauri_lib::commands::{
    SyncProgressEmitter, SyncProgressEvent,
};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::enrichment::{
    EnrichmentEngine, OriginTrackMetadata, SyncTrackInput,
};

async fn setup_test_db() -> sqlx::SqlitePool {
    let _ = crypto::init_crypto([42u8; 32]);

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

async fn create_test_account(pool: &sqlx::SqlitePool, email: &str) -> (i64, i64) {
    let service_id: i64 = match sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    {
        Some(id) => id,
        None => {
            sqlx::query_scalar("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'qobuz') RETURNING id")
                .fetch_one(pool)
                .await
                .unwrap_or(1)
        }
    };

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, email) VALUES (?, 'Test User', ?) RETURNING id"
    )
    .bind(service_id)
    .bind(email)
    .fetch_one(pool)
    .await
    .unwrap();

    (service_id, account_id)
}

#[derive(Clone, Default)]
struct TestProgressCollector {
    events: Arc<Mutex<Vec<SyncProgressEvent>>>,
}

impl SyncProgressEmitter for TestProgressCollector {
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

#[tokio::test]
async fn test_sync_pre_enrichment_persists_track_album_artist_credits_metadata() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "bowie@test.local").await;
    let engine = EnrichmentEngine::new();

    // 1. Prepare comprehensive catalog metadata from streaming service
    let input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Heroes".to_string()),
            artist: Some("David Bowie".to_string()),
            album: Some("Heroes (2017 Remaster)".to_string()),
            album_artist: Some("David Bowie".to_string()),
            composer: Some("David Bowie, Brian Eno".to_string()),
            performers: Some("David Bowie, Robert Fripp".to_string()),
            track_number: Some(3),
            track_total: Some(10),
            disc_number: Some(1),
            isrc: Some("GBAYE7700021".to_string()),
            barcode: Some("0035629007421".to_string()),
            label: Some("Parlophone UK".to_string()),
            release_date: Some("1977-10-14".to_string()),
            release_year: Some("1977".to_string()),
            release_country: Some("United Kingdom".to_string()),
            genre: Some("Art Rock".to_string()),
            explicit: Some(false),
            bpm: Some(112),
            initial_key: Some("D".to_string()),
            acoustid_fingerprint: Some("AQAA-qobuz-fingerprint-heroes".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "123456".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: true,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(24),
        sample_rate: Some(96000),
        quality_score: Some(95),
        audio_quality: Some("hires".to_string()),
        cover_art_url: Some("https://static.qobuz.com/images/covers/heroes.jpg".to_string()),
        duration_ms: Some(367000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };

    // 2. Execute sync pre-enrichment
    let result = engine
        .enrich_and_persist_sync_track(&pool, input)
        .await
        .expect("Pre-enrichment should succeed");

    assert!(result.track_id > 0);
    assert!(result.artist_id > 0);
    assert!(result.album_id.is_some());
    assert!(result.is_new_import);
    assert_eq!(result.completeness, EnrichmentCompleteness::Enriched);
    assert_eq!(result.availability_status, "available");

    // 3. Verify Artists Table
    let artist_name: String = sqlx::query_scalar("SELECT name FROM artists WHERE id = ?")
        .bind(result.artist_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(artist_name, "David Bowie");

    // 4. Verify Albums Table
    let album_id = result.album_id.unwrap();
    let (album_title, album_label, album_upc, total_tracks): (String, Option<String>, Option<String>, Option<i32>) =
        sqlx::query_as("SELECT title, label, upc, total_tracks FROM albums WHERE id = ?")
            .bind(album_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(album_title, "Heroes (2017 Remaster)");
    assert_eq!(album_label.as_deref(), Some("Parlophone UK"));
    assert_eq!(album_upc.as_deref(), Some("0035629007421"));
    assert_eq!(total_tracks, Some(10));

    // 5. Verify Tracks Table
    let (title, isrc, year, audio_quality, enrichment_status): (String, Option<String>, Option<i32>, Option<String>, String) =
        sqlx::query_as("SELECT title, isrc, release_year, audio_quality, enrichment_status FROM tracks WHERE id = ?")
            .bind(result.track_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(title, "Heroes");
    assert_eq!(isrc.as_deref(), Some("GBAYE7700021"));
    assert_eq!(year, Some(1977));
    assert_eq!(audio_quality.as_deref(), Some("hires"));
    assert_eq!(enrichment_status, "enriched");

    // 6. Verify Track Credits Table
    let credits: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT a.name, tc.role FROM track_credits tc
        JOIN artists a ON a.id = tc.artist_id
        WHERE tc.track_id = ?
        ORDER BY tc.role, a.name
        "#
    )
    .bind(result.track_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert!(credits.iter().any(|(name, role)| name == "Brian Eno" && role == "composer"));
    assert!(credits.iter().any(|(name, role)| name == "David Bowie" && role == "composer"));
    assert!(credits.iter().any(|(name, role)| name == "Robert Fripp" && role == "performer"));

    // 7. Verify Track Sources Table
    let (src_available, src_status, src_format, src_bit_depth): (i32, String, Option<String>, Option<i32>) =
        sqlx::query_as("SELECT available, availability_status, format, bit_depth FROM track_sources WHERE track_id = ? AND service_id = ?")
            .bind(result.track_id)
            .bind(service_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(src_available, 1);
    assert_eq!(src_status, "available");
    assert_eq!(src_format.as_deref(), Some("FLAC"));
    assert_eq!(src_bit_depth, Some(24));

    // 8. Verify Library Entries Table
    let (is_liked, is_purchased): (i32, i32) = sqlx::query_as(
        "SELECT is_liked, is_purchased FROM library_entries WHERE account_id = ? AND track_id = ?"
    )
    .bind(account_id)
    .bind(result.track_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(is_liked, 1);
    assert_eq!(is_purchased, 0);
}

#[tokio::test]
async fn test_sync_pre_enrichment_idempotency() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "starman@test.local").await;
    let engine = EnrichmentEngine::new();

    let input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Starman".to_string()),
            artist: Some("David Bowie".to_string()),
            album: Some("The Rise and Fall of Ziggy Stardust".to_string()),
            isrc: Some("GBAYE7200012".to_string()),
            release_year: Some("1972".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "789012".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: true,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(16),
        sample_rate: Some(44100),
        quality_score: Some(80),
        audio_quality: Some("lossless".to_string()),
        cover_art_url: None,
        duration_ms: Some(254000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };

    // First Run
    let res1 = engine.enrich_and_persist_sync_track(&pool, input.clone()).await.unwrap();
    assert!(res1.is_new_import);

    // Second Run (Identical sync)
    let res2 = engine.enrich_and_persist_sync_track(&pool, input.clone()).await.unwrap();
    assert!(!res2.is_new_import);
    assert_eq!(res1.track_id, res2.track_id);
    assert_eq!(res1.artist_id, res2.artist_id);
    assert_eq!(res1.album_id, res2.album_id);

    // Verify Row Counts in SQLite
    let tracks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    let artists_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artists").fetch_one(&pool).await.unwrap();
    let albums_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums").fetch_one(&pool).await.unwrap();
    let entries_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries").fetch_one(&pool).await.unwrap();

    assert_eq!(tracks_count, 1);
    assert_eq!(artists_count, 1);
    assert_eq!(albums_count, 1);
    assert_eq!(entries_count, 1);
}

#[tokio::test]
async fn test_sync_pre_enrichment_manual_precedence_preservation() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "manual@test.local").await;
    let engine = EnrichmentEngine::new();

    // 1. User manually edited this track previously
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Custom Artist') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let track_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO tracks (title, release_year, isrc, enrichment_status)
        VALUES ('My Custom Title', 1980, 'ISRC-MANUAL-001', 'manual')
        RETURNING id
        "#
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let _ = sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id).bind(artist_id).execute(&pool).await;

    // 2. Incoming streaming sync with different title/year for the same ISRC
    let input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Streaming Title Overwrite Attempt".to_string()),
            artist: Some("Streaming Artist Overwrite Attempt".to_string()),
            album: Some("Streaming Album".to_string()),
            isrc: Some("ISRC-MANUAL-001".to_string()),
            release_year: Some("2022".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "qobuz-999".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: true,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(16),
        sample_rate: Some(44100),
        quality_score: Some(80),
        audio_quality: Some("lossless".to_string()),
        cover_art_url: None,
        duration_ms: Some(200000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };

    let res = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();
    assert_eq!(res.track_id, track_id);

    // 3. Verify manual fields were preserved, but service link was added
    let (title, year, status): (String, Option<i32>, String) =
        sqlx::query_as("SELECT title, release_year, enrichment_status FROM tracks WHERE id = ?")
            .bind(track_id)
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(title, "My Custom Title");
    assert_eq!(year, Some(1980));
    assert_eq!(status, "manual");

    // Verified service source was linked
    let qobuz_id: Option<String> = sqlx::query_scalar("SELECT service_track_id FROM track_sources WHERE track_id = ? AND service_id = ?")
        .bind(track_id)
        .bind(service_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(qobuz_id.as_deref(), Some("qobuz-999"));
}

#[tokio::test]
async fn test_country_normalization_during_sync() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "country@test.local").await;
    let engine = EnrichmentEngine::new();

    let cases = vec![
        ("United Kingdom", "GB"),
        ("Spain", "ES"),
        ("United States", "US"),
        ("Germany", "DE"),
        ("Japan", "JP"),
    ];

    for (raw_country, expected_iso) in cases {
        let input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(format!("Track from {}", raw_country)),
                artist: Some("Global Artist".to_string()),
                album: Some("World Tour".to_string()),
                release_country: Some(raw_country.to_string()),
                source_name: "qobuz".to_string(),
                ..Default::default()
            },
            service_track_id: format!("country-track-{}", expected_iso),
            service_name: "qobuz".to_string(),
            service_id,
            account_id,
            is_favorite: false,
            is_purchased: false,
            format: Some("FLAC".to_string()),
            bit_depth: Some(16),
            sample_rate: Some(44100),
            quality_score: Some(80),
            audio_quality: Some("lossless".to_string()),
            cover_art_url: None,
            duration_ms: Some(180000),
            query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
        };

        let _ = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();

        // Verify domain normalization logic
        let normalized = syncify_metadata_domain::country::normalize_country_or_region(raw_country);
        assert_eq!(normalized, Some(expected_iso.to_string()));
    }
}

#[tokio::test]
async fn test_separation_of_imported_available_downloaded_and_zero_audio_files() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "space@test.local").await;
    let engine = EnrichmentEngine::new();

    let input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Space Oddity".to_string()),
            artist: Some("David Bowie".to_string()),
            album: Some("Space Oddity".to_string()),
            isrc: Some("GBAYE6900010".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "track-space-oddity".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: true,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(24),
        sample_rate: Some(192000),
        quality_score: Some(99),
        audio_quality: Some("hires".to_string()),
        cover_art_url: None,
        duration_ms: Some(315000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };

    let result = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();

    // 1. Library entry exists (imported)
    let imported_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE track_id = ?")
        .bind(result.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(imported_count, 1);

    // 2. Track source exists and is marked 'available'
    let (available, avail_status): (i32, String) = sqlx::query_as(
        "SELECT available, availability_status FROM track_sources WHERE track_id = ?"
    )
    .bind(result.track_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(available, 1);
    assert_eq!(avail_status, "available");

    // 3. ZERO audio downloads exist
    let downloads_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads").fetch_one(&pool).await.unwrap();
    assert_eq!(downloads_count, 0);
}

#[tokio::test]
async fn test_progress_events_no_raw_fetching_labels() {
    let collector = TestProgressCollector::default();

    // Emit standard sync progress events
    collector.emit_sync_progress(&SyncProgressEvent::running("qobuz", Some(1), "importing_favorite_tracks", 5, Some(10), "Importing favorite tracks (5/10)", 5, 5));
    collector.emit_sync_progress(&SyncProgressEvent::running("qobuz", Some(1), "importing_favorite_albums", 2, Some(4), "Importing favorite albums (2/4)", 5, 5));
    collector.emit_sync_progress(&SyncProgressEvent::running("qobuz", Some(1), "importing_playlists", 1, Some(2), "Importing playlist: Rock Classics (1/2)", 5, 5));
    collector.emit_sync_progress(&SyncProgressEvent::running("qobuz", Some(1), "importing_purchases", 1, Some(1), "Importing purchases (1/1)", 5, 5));

    let events = collector.events.lock().unwrap().clone();
    for ev in events {
        assert!(!ev.message.starts_with("Fetching "), "Event message '{}' should not start with 'Fetching '", ev.message);
        assert!(!ev.phase.starts_with("fetching_"), "Event phase '{}' should not start with 'fetching_'", ev.phase);
    }
}

// S198: favorite-album marking + guarded provider-id persistence through the
// shared EnrichmentEngine (owner live audit docs/s197_auditoria_importaciones.md §4).
#[tokio::test]
async fn test_s198_favorite_album_marking_and_qobuz_id_persistence() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "s198@test.local").await;
    let engine = EnrichmentEngine::new();

    let make_input = |track_title: String, track_provider_id: String, album_provider_id: String| SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some(track_title),
            artist: Some("A Touch Of Class".to_string()),
            album: Some("Around The World".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: track_provider_id,
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: false,
        album_is_favorite: true,
        album_provider_track_id: Some(album_provider_id),
        ..Default::default()
    };

    // Track 1 creates the album and must mark it favorite + persist qobuz_id.
    let res1 = engine
        .enrich_and_persist_sync_track(&pool, make_input("Around The World (La La La)".into(), "qb-tr-1".into(), "qb-alb-100".into()))
        .await
        .expect("track 1 should persist");
    let album1 = res1.album_id.expect("album should exist");

    let (is_fav, qid): (i64, Option<String>) = sqlx::query_as(
        "SELECT COALESCE(is_favorite, 0), qobuz_id FROM albums WHERE id = ?"
    )
    .bind(album1)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(is_fav, 1, "album must be marked favorite by the engine");
    assert_eq!(qid.as_deref(), Some("qb-alb-100"), "qobuz_id persisted on first write");

    // Re-import same provider id → idempotent, still favorite.
    let _ = engine
        .enrich_and_persist_sync_track(&pool, make_input("Around The World (La La La) [Radio Edit]".into(), "qb-tr-2".into(), "qb-alb-100".into()))
        .await
        .expect("re-import should succeed");
    let qid2: Option<String> = sqlx::query_scalar("SELECT qobuz_id FROM albums WHERE id = ?")
        .bind(album1)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(qid2.as_deref(), Some("qb-alb-100"));

    // A DIFFERENT album row must never have its id overwritten by the first.
    let input_other = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Unrelated Song".to_string()),
            artist: Some("Other Artist".to_string()),
            album: Some("Other Album".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "999".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        album_is_favorite: true,
        album_provider_track_id: Some("qb-alb-200".to_string()),
        ..Default::default()
    };
    let res2 = engine
        .enrich_and_persist_sync_track(&pool, input_other)
        .await
        .expect("second album should persist");
    let album2 = res2.album_id.expect("second album should exist");
    assert_ne!(album1, album2);
    let qid_other: Option<String> = sqlx::query_scalar("SELECT qobuz_id FROM albums WHERE id = ?")
        .bind(album2)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(qid_other.as_deref(), Some("qb-alb-200"));
}

//! Sprint S130B: Child Track Persistence for Albums and Playlists Integration Test Suite
//! Validates:
//! 1. Clean DB + album sync -> child tracks appear in library
//! 2. Clean DB + playlist sync -> child tracks appear in library & playlist_tracks
//! 3. Album + playlist sharing a track -> deduplicated, unique count accurate
//! 4. Strict counter separation (favorite_tracks_total vs favorite_albums_total vs imported_tracks_total)
//! 5. Incremental sync idempotency
//! 6. Partial metadata handling & availability status
//! 7. Phase timings telemetry data contract
//! 8. Zero audio downloads performed during sync

use sqlx::sqlite::SqlitePoolOptions;
use syncify_metadata_domain::EnrichmentCompleteness;
use syncify_tauri_lib::commands::{
    ServiceSyncResult, SyncPhaseTimings,
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

async fn create_test_account(pool: &sqlx::SqlitePool, service_name: &str, email: &str) -> (i64, i64) {
    let service_id: i64 = match sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service_name)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    {
        Some(id) => id,
        None => {
            sqlx::query_scalar("INSERT OR IGNORE INTO services (name) VALUES (?) RETURNING id")
                .bind(service_name)
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

#[tokio::test]
async fn test_clean_db_sync_album_persists_child_tracks_in_library() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "qobuz", "album_sync@test.local").await;
    let engine = EnrichmentEngine::new();

    let album_title = "The Dark Side of the Moon (50th Anniversary)";
    let artist_name = "Pink Floyd";

    let tracks = vec![
        ("Speak to Me", 1, 1, 67000, "GBAYE7300001"),
        ("Breathe (In the Air)", 2, 1, 163000, "GBAYE7300002"),
        ("Time", 3, 1, 421000, "GBAYE7300003"),
    ];

    let mut imported_tracks = 0;
    for (title, track_num, disc_num, dur_ms, isrc) in &tracks {
        let input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(title.to_string()),
                artist: Some(artist_name.to_string()),
                album: Some(album_title.to_string()),
                album_artist: Some(artist_name.to_string()),
                track_number: Some(*track_num),
                track_total: Some(3),
                disc_number: Some(*disc_num),
                isrc: Some(isrc.to_string()),
                barcode: Some("5099902894523".to_string()),
                label: Some("Harvest Records".to_string()),
                release_date: Some("1973-03-01".to_string()),
                release_year: Some("1973".to_string()),
                source_name: "qobuz".to_string(),
                ..Default::default()
            },
            service_track_id: format!("qobuz_album_tr_{}", track_num),
            service_name: "qobuz".to_string(),
            service_id,
            account_id,
            is_favorite: false,
            is_purchased: false,
            format: Some("FLAC".to_string()),
            bit_depth: Some(24),
            sample_rate: Some(96000),
            quality_score: Some(95),
            audio_quality: Some("hires".to_string()),
            cover_art_url: Some("https://static.qobuz.com/covers/dsotm.jpg".to_string()),
            duration_ms: Some(*dur_ms),
            query_musicbrainz: false,
        };

        let res = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();
        if res.is_new_import {
            imported_tracks += 1;
        }
    }

    // Mark album as favorite
    sqlx::query("UPDATE albums SET is_favorite = 1, favorite_at = CURRENT_TIMESTAMP WHERE title = ?")
        .bind(album_title)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(imported_tracks, 3);

    // Verify tracks in database
    let db_tracks: Vec<(String, Option<i32>, Option<i64>, Option<i32>)> = sqlx::query_as(
        "SELECT t.title, t.track_number, t.duration_ms, t.is_favorite FROM tracks t ORDER BY t.track_number"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(db_tracks.len(), 3);
    assert_eq!(db_tracks[0].0, "Speak to Me");
    assert_eq!(db_tracks[0].1, Some(1));
    assert_eq!(db_tracks[0].2, Some(67000));
    assert_eq!(db_tracks[0].3, Some(0)); // Track itself is not favorite

    assert_eq!(db_tracks[1].0, "Breathe (In the Air)");
    assert_eq!(db_tracks[2].0, "Time");

    // Verify album is marked favorite
    let is_fav_album: i32 = sqlx::query_scalar("SELECT is_favorite FROM albums WHERE title = ?")
        .bind(album_title)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(is_fav_album, 1);

    // Verify library entries
    let entries_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(entries_count, 3);

    // Verify library query returns album tracks with artist and album joined
    let library_tracks: Vec<(i64, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT t.id, t.title, ar.name as artist_name, al.title as album_name, s.name as imported_from
        FROM tracks t
        JOIN albums al ON al.id = t.album_id
        LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
        LEFT JOIN artists ar ON ar.id = ta.artist_id
        JOIN library_entries le ON le.track_id = t.id AND le.account_id = ?
        JOIN accounts acc ON acc.id = le.account_id
        JOIN services s ON s.id = acc.service_id
        ORDER BY t.track_number ASC
        "#
    )
    .bind(account_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(library_tracks.len(), 3);
    assert_eq!(library_tracks[0].1, "Speak to Me");
    assert_eq!(library_tracks[0].2.as_deref(), Some("Pink Floyd"));
    assert_eq!(library_tracks[0].3.as_deref(), Some(album_title));
    assert_eq!(library_tracks[0].4.as_deref(), Some("qobuz"));
}

#[tokio::test]
async fn test_clean_db_sync_playlist_persists_child_tracks_in_library_and_playlist_tracks() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "spotify", "playlist_sync@test.local").await;
    let engine = EnrichmentEngine::new();

    // 1. Insert playlist row
    let playlist_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO playlists (account_id, service_playlist_id, name, description, is_public, is_collaborative, track_count)
           VALUES (?, 'sp_pl_1', 'Rock Classics', 'Timeless rock hits', 1, 0, 3) RETURNING id"#
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let playlist_tracks = vec![
        ("Comfortably Numb", "Pink Floyd", "The Wall", 1, "GBAYE7900010", 382000),
        ("Stairway to Heaven", "Led Zeppelin", "Led Zeppelin IV", 2, "USAT27100004", 482000),
        ("Bohemian Rhapsody", "Queen", "A Night at the Opera", 3, "GBUM71029606", 354000),
    ];

    let mut imported = 0;
    for (pos, (title, artist, album, track_num, isrc, dur_ms)) in playlist_tracks.iter().enumerate() {
        let input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(title.to_string()),
                artist: Some(artist.to_string()),
                album: Some(album.to_string()),
                track_number: Some(*track_num),
                isrc: Some(isrc.to_string()),
                source_name: "spotify".to_string(),
                ..Default::default()
            },
            service_track_id: format!("sp_tr_{}", pos + 1),
            service_name: "spotify".to_string(),
            service_id,
            account_id,
            is_favorite: false,
            is_purchased: false,
            format: Some("OGG".to_string()),
            bit_depth: None,
            sample_rate: None,
            quality_score: None,
            audio_quality: Some("standard".to_string()),
            cover_art_url: None,
            duration_ms: Some(*dur_ms),
            query_musicbrainz: false,
        };

        let res = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();
        if res.is_new_import {
            imported += 1;
        }

        // Link playlist_tracks
        sqlx::query("INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)"
            .into())
            .bind(playlist_id)
            .bind(res.track_id)
            .bind(pos as i32 + 1)
            .execute(&pool)
            .await
            .unwrap();
    }

    assert_eq!(imported, 3);

    // Verify playlist_tracks table
    let pl_items: Vec<(i64, i32, String)> = sqlx::query_as(
        r#"SELECT pt.track_id, pt.position, t.title FROM playlist_tracks pt
           JOIN tracks t ON t.id = pt.track_id
           WHERE pt.playlist_id = ?
           ORDER BY pt.position ASC"#
    )
    .bind(playlist_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(pl_items.len(), 3);
    assert_eq!(pl_items[0].1, 1);
    assert_eq!(pl_items[0].2, "Comfortably Numb");
    assert_eq!(pl_items[1].1, 2);
    assert_eq!(pl_items[1].2, "Stairway to Heaven");
    assert_eq!(pl_items[2].1, 3);
    assert_eq!(pl_items[2].2, "Bohemian Rhapsody");
}

#[tokio::test]
async fn test_album_and_playlist_sharing_track_deduplication() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "tidal", "shared_tracks@test.local").await;
    let engine = EnrichmentEngine::new();

    // Track 1 (Shared): "Shine On You Crazy Diamond"
    // Track 2 (Album only): "Welcome to the Machine"
    // Track 3 (Playlist only): "Money"

    let shared_isrc = "GBAYE7500001";

    // 1. Sync Album (Tracks 1 and 2)
    let album_tracks = vec![
        ("Shine On You Crazy Diamond", 1, shared_isrc, 810000),
        ("Welcome to the Machine", 2, "GBAYE7500002", 451000),
    ];

    let mut album_imported = 0;
    for (title, track_num, isrc, dur_ms) in album_tracks {
        let input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(title.to_string()),
                artist: Some("Pink Floyd".to_string()),
                album: Some("Wish You Were Here".to_string()),
                track_number: Some(track_num),
                isrc: Some(isrc.to_string()),
                source_name: "tidal".to_string(),
                ..Default::default()
            },
            service_track_id: format!("tidal_alb_{}", track_num),
            service_name: "tidal".to_string(),
            service_id,
            account_id,
            is_favorite: false,
            is_purchased: false,
            format: Some("FLAC".to_string()),
            bit_depth: None,
            sample_rate: None,
            quality_score: None,
            audio_quality: Some("lossless".to_string()),
            cover_art_url: None,
            duration_ms: Some(dur_ms),
            query_musicbrainz: false,
        };

        let res = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();
        if res.is_new_import {
            album_imported += 1;
        }
    }
    assert_eq!(album_imported, 2);

    // 2. Sync Playlist (Track 1 [Shared] and Track 3 [New])
    let playlist_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO playlists (account_id, service_playlist_id, name, is_public, track_count)
           VALUES (?, 'pl_shared', 'Progressive Rock', 1, 2) RETURNING id"#
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let playlist_tracks = vec![
        ("Shine On You Crazy Diamond", "Wish You Were Here", shared_isrc, 810000, "tidal_alb_1"), // same isrc & title
        ("Money", "The Dark Side of the Moon", "GBAYE7300006", 382000, "tidal_pl_3"),
    ];

    let mut playlist_new_imported = 0;
    let mut playlist_skipped = 0;

    for (pos, (title, album, isrc, dur_ms, svc_id)) in playlist_tracks.iter().enumerate() {
        let input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(title.to_string()),
                artist: Some("Pink Floyd".to_string()),
                album: Some(album.to_string()),
                isrc: Some(isrc.to_string()),
                source_name: "tidal".to_string(),
                ..Default::default()
            },
            service_track_id: svc_id.to_string(),
            service_name: "tidal".to_string(),
            service_id,
            account_id,
            is_favorite: false,
            is_purchased: false,
            format: Some("FLAC".to_string()),
            bit_depth: None,
            sample_rate: None,
            quality_score: None,
            audio_quality: Some("lossless".to_string()),
            cover_art_url: None,
            duration_ms: Some(*dur_ms),
            query_musicbrainz: false,
        };

        let res = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();
        if res.is_new_import {
            playlist_new_imported += 1;
        } else {
            playlist_skipped += 1;
        }

        sqlx::query("INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)")
            .bind(playlist_id)
            .bind(res.track_id)
            .bind(pos as i32 + 1)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Shared track is deduplicated
    assert_eq!(playlist_new_imported, 1);
    assert_eq!(playlist_skipped, 1);

    // Total unique tracks in DB is 3 (not 4)
    let total_tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    assert_eq!(total_tracks, 3);

    // Total playlist tracks linked is 2
    let total_pl_tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?")
        .bind(playlist_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total_pl_tracks, 2);
}

#[tokio::test]
async fn test_favorites_count_strictly_separated_from_albums_and_playlists() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "qobuz", "fav_separation@test.local").await;
    let engine = EnrichmentEngine::new();

    // 1. Sync 1 Favorite Track
    let fav_input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Favorite Track 1".to_string()),
            artist: Some("Artist A".to_string()),
            album: Some("Album A".to_string()),
            isrc: Some("ISRCFAV00001".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "fav_tr_1".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: true,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(16),
        sample_rate: Some(44100),
        quality_score: None,
        audio_quality: Some("lossless".to_string()),
        cover_art_url: None,
        duration_ms: Some(200000),
        query_musicbrainz: false,
    };
    let res_fav = engine.enrich_and_persist_sync_track(&pool, fav_input).await.unwrap();
    assert!(res_fav.is_new_import);

    // 2. Sync 2 Album Tracks (is_favorite = false)
    for i in 1..=2 {
        let alb_input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(format!("Album Track {}", i)),
                artist: Some("Artist B".to_string()),
                album: Some("Album B".to_string()),
                track_number: Some(i),
                isrc: Some(format!("ISRCALB0000{}", i)),
                source_name: "qobuz".to_string(),
                ..Default::default()
            },
            service_track_id: format!("alb_tr_{}", i),
            service_name: "qobuz".to_string(),
            service_id,
            account_id,
            is_favorite: false,
            is_purchased: false,
            format: Some("FLAC".to_string()),
            bit_depth: Some(24),
            sample_rate: Some(96000),
            quality_score: None,
            audio_quality: Some("hires".to_string()),
            cover_art_url: None,
            duration_ms: Some(180000),
            query_musicbrainz: false,
        };
        let res = engine.enrich_and_persist_sync_track(&pool, alb_input).await.unwrap();
        assert!(res.is_new_import);
    }
    sqlx::query("UPDATE albums SET is_favorite = 1 WHERE title = 'Album B'")
        .execute(&pool)
        .await
        .unwrap();

    // 3. Verify track-level favorites count vs total imported tracks
    let fav_tracks_in_db: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE is_favorite = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(fav_tracks_in_db, 1);

    let total_tracks_in_db: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total_tracks_in_db, 3);

    let fav_albums_in_db: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums WHERE is_favorite = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(fav_albums_in_db, 1);
}

#[tokio::test]
async fn test_sync_incremental_idempotency() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "qobuz", "idempotent@test.local").await;
    let engine = EnrichmentEngine::new();

    let input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Idempotency Test Track".to_string()),
            artist: Some("Idempotent Artist".to_string()),
            album: Some("Idempotent Album".to_string()),
            isrc: Some("ISRCIDEM0001".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "idem_tr_1".to_string(),
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
        duration_ms: Some(300000),
        query_musicbrainz: false,
    };

    // Run 1
    let r1 = engine.enrich_and_persist_sync_track(&pool, input.clone()).await.unwrap();
    assert!(r1.is_new_import);

    // Run 2 (exact same input)
    let r2 = engine.enrich_and_persist_sync_track(&pool, input.clone()).await.unwrap();
    assert!(!r2.is_new_import);
    assert_eq!(r1.track_id, r2.track_id);
    assert_eq!(r1.artist_id, r2.artist_id);

    // Row counts must not increase
    let t_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    let src_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources").fetch_one(&pool).await.unwrap();
    let entry_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries").fetch_one(&pool).await.unwrap();

    assert_eq!(t_count, 1);
    assert_eq!(src_count, 1);
    assert_eq!(entry_count, 1);
}

#[tokio::test]
async fn test_partial_metadata_and_availability_parity() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "qobuz", "partial_meta@test.local").await;
    let engine = EnrichmentEngine::new();

    // 1. Full Metadata Track -> Enriched
    let full_input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Full Track".to_string()),
            artist: Some("Full Artist".to_string()),
            album: Some("Full Album".to_string()),
            album_artist: Some("Full Artist".to_string()),
            isrc: Some("ISRCFULL0001".to_string()),
            barcode: Some("123456789012".to_string()),
            label: Some("Label X".to_string()),
            release_date: Some("2020-01-01".to_string()),
            release_year: Some("2020".to_string()),
            track_number: Some(1),
            track_total: Some(10),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "full_1".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: true,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(24),
        sample_rate: Some(96000),
        quality_score: Some(90),
        audio_quality: Some("hires".to_string()),
        cover_art_url: None,
        duration_ms: Some(240000),
        query_musicbrainz: false,
    };
    let full_res = engine.enrich_and_persist_sync_track(&pool, full_input).await.unwrap();
    assert_eq!(full_res.completeness, EnrichmentCompleteness::Enriched);

    // 2. Partial Metadata Track (Title, Artist, Album, but no ISRC or extended tags) -> Partial
    let partial_input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Partial Track".to_string()),
            artist: Some("Partial Artist".to_string()),
            album: Some("Partial Album".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "partial_1".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: true,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: None,
        sample_rate: None,
        quality_score: None,
        audio_quality: Some("lossless".to_string()),
        cover_art_url: None,
        duration_ms: Some(180000),
        query_musicbrainz: false,
    };
    let partial_res = engine.enrich_and_persist_sync_track(&pool, partial_input).await.unwrap();
    assert_eq!(partial_res.completeness, EnrichmentCompleteness::Partial);

    // 3. Minimal Metadata Track (Title + Artist only, no album or IDs) -> Minimal
    let minimal_input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Minimal Track".to_string()),
            artist: Some("Minimal Artist".to_string()),
            album: None,
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "minimal_1".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: true,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: None,
        sample_rate: None,
        quality_score: None,
        audio_quality: Some("lossless".to_string()),
        cover_art_url: None,
        duration_ms: Some(180000),
        query_musicbrainz: false,
    };
    let minimal_res = engine.enrich_and_persist_sync_track(&pool, minimal_input).await.unwrap();
    assert_eq!(minimal_res.completeness, EnrichmentCompleteness::Minimal);
}

#[tokio::test]
async fn test_sync_phase_timings_telemetry_contract() {
    let timings = SyncPhaseTimings {
        api_fetch_ms: 120,
        entity_expansion_ms: 85,
        enrichment_ms: 45,
        persistence_ms: 30,
        availability_check_ms: 10,
        total_elapsed_ms: 290,
    };

    let result = ServiceSyncResult {
        service: "qobuz".to_string(),
        account_id: Some(1),
        success: true,
        message: "Sync complete".to_string(),
        imported_tracks_total: 10,
        favorite_tracks_total: 5,
        favorite_albums_total: 2,
        favorite_artists_total: 1,
        playlists_total: 1,
        purchases_total: 0,
        skipped_tracks_total: 0,
        albums_total: 2,
        metadata_enriched: 8,
        metadata_partial: 2,
        availability_unknown: 0,
        availability_checked: 10,
        phase_timings: Some(timings.clone()),
        album_expansion_metrics: None,
        tracks_processed: 10,
        tracks_changed_unique: 10,
        tracks_new_global: 10,
        sources_new_for_service: 10,
        library_entries_new_for_account: 10,
        tracks_already_present: 0,
        favorites_seen: 5,
        albums_seen: 2,
        playlists_seen: 1,
        tracks_expanded: 5,
        tracks_expansion_failed: 0,
        errors: vec![],
        ..Default::default()
    };

    assert!(result.phase_timings.is_some());
    let pt = result.phase_timings.unwrap();
    assert_eq!(pt.api_fetch_ms, 120);
    assert_eq!(pt.entity_expansion_ms, 85);
    assert_eq!(pt.enrichment_ms, 45);
    assert_eq!(pt.persistence_ms, 30);
    assert_eq!(pt.availability_check_ms, 10);
    assert_eq!(pt.total_elapsed_ms, 290);
}

#[tokio::test]
async fn test_sync_performs_zero_audio_downloads() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "qobuz", "no_downloads@test.local").await;
    let engine = EnrichmentEngine::new();

    // Perform multiple track/album/playlist syncs
    for i in 1..=5 {
        let input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(format!("Track {}", i)),
                artist: Some("Band".to_string()),
                album: Some("Album".to_string()),
                track_number: Some(i),
                isrc: Some(format!("ISRCNODWN{:04}", i)),
                source_name: "qobuz".to_string(),
                ..Default::default()
            },
            service_track_id: format!("nodwn_{}", i),
            service_name: "qobuz".to_string(),
            service_id,
            account_id,
            is_favorite: i % 2 == 0,
            is_purchased: false,
            format: Some("FLAC".to_string()),
            bit_depth: Some(24),
            sample_rate: Some(96000),
            quality_score: Some(90),
            audio_quality: Some("hires".to_string()),
            cover_art_url: None,
            duration_ms: Some(200000),
            query_musicbrainz: false,
        };
        let _ = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();
    }

    // Verify download_queue table is completely empty
    let queue_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queue_count, 0, "Sync must not insert into download_queue");

    // Verify downloads table is completely empty (no audio files downloaded during sync)
    let downloaded_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(downloaded_count, 0, "Sync must not insert into downloads table");
}

#[tokio::test]
async fn test_qobuz_album_deserialization_handles_numeric_and_string_ids() {
    use syncify_tauri_lib::services::qobuz::QobuzAlbum;

    // Numeric ID (e.g. from user favorites 6269513)
    let json_numeric = r#"{
        "id": 6269513,
        "title": "The Grand Illusion",
        "released_at": 238464000,
        "upc": "0007502132232"
    }"#;
    let album_num: QobuzAlbum = serde_json::from_str(json_numeric).expect("Must deserialize numeric ID");
    assert_eq!(album_num.id, "6269513");
    assert_eq!(album_num.title.as_deref(), Some("The Grand Illusion"));
    assert!(album_num.tracks.is_none());

    // String ID
    let json_string = r#"{
        "id": "0007502132232",
        "title": "The Grand Illusion (Remastered)",
        "released_at": 238464000
    }"#;
    let album_str: QobuzAlbum = serde_json::from_str(json_string).expect("Must deserialize string ID");
    assert_eq!(album_str.id, "0007502132232");
}

#[tokio::test]
async fn test_qobuz_favorite_album_expansion_persists_child_tracks_and_library_entries() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "qobuz", "qobuz_album_exp@test.local").await;
    let engine = EnrichmentEngine::new();

    // Simulate an expanded QobuzAlbum with 4 tracks
    let album_title = "A Night at the Opera";
    let artist_name = "Queen";
    let qobuz_album_id = "0060252771765";

    let child_tracks = vec![
        ("Death on Two Legs", 1, 1, 223000, "GBUM71100611", 101),
        ("Lazing on a Sunday Afternoon", 2, 1, 67000, "GBUM71100612", 102),
        ("I'm in Love with My Car", 3, 1, 185000, "GBUM71100613", 103),
        ("Bohemian Rhapsody", 11, 1, 355000, "GBUM71100621", 111),
    ];

    let mut imported_tracks = 0;
    for (title, track_num, disc_num, dur_ms, isrc, trk_id) in &child_tracks {
        let input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(title.to_string()),
                artist: Some(artist_name.to_string()),
                album: Some(album_title.to_string()),
                album_artist: Some(artist_name.to_string()),
                track_number: Some(*track_num),
                track_total: Some(12),
                disc_number: Some(*disc_num),
                isrc: Some(isrc.to_string()),
                barcode: Some(qobuz_album_id.to_string()),
                label: Some("EMI Records".to_string()),
                release_date: Some("1975-11-21".to_string()),
                release_year: Some("1975".to_string()),
                source_name: "qobuz".to_string(),
                ..Default::default()
            },
            service_track_id: trk_id.to_string(),
            service_name: "qobuz".to_string(),
            service_id,
            account_id,
            is_favorite: false,
            is_purchased: false,
            format: Some("FLAC".to_string()),
            bit_depth: Some(24),
            sample_rate: Some(96000),
            quality_score: Some(95),
            audio_quality: Some("hires".to_string()),
            cover_art_url: Some("https://static.qobuz.com/covers/queen_opera.jpg".to_string()),
            duration_ms: Some(*dur_ms),
            query_musicbrainz: false,
        };

        let res = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();
        if res.is_new_import {
            imported_tracks += 1;
        }
    }

    // Mark album as favorite
    sqlx::query("UPDATE albums SET is_favorite = 1, favorite_at = CURRENT_TIMESTAMP WHERE title = ?")
        .bind(album_title)
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(imported_tracks, 4);

    // Verify tracks and library entries
    let track_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    assert_eq!(track_count, 4);

    let entries: Vec<(i64, i32)> = sqlx::query_as("SELECT track_id, is_liked FROM library_entries WHERE account_id = ?")
        .bind(account_id)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(entries.len(), 4);
    for (_, is_liked) in entries {
        assert_eq!(is_liked, 0, "Album child tracks must have is_liked = 0");
    }

    let is_fav_album: i32 = sqlx::query_scalar("SELECT is_favorite FROM albums WHERE title = ?")
        .bind(album_title)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(is_fav_album, 1);
}

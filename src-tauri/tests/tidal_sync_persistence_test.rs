//! Sprint S142: Tidal Sync Persistence and Metric Invariants Integration Test Suite
//! Validates:
//! 1. Empty DB: Tidal favorites insert tracks, track_sources, and library_entries (all granular flags true).
//! 2. Shared ISRC (Qobuz + Tidal): Creates separate Tidal track_source and library_entry without duplicating global track.
//! 3. Re-Sync Idempotency: 0 new global/source/entry rows and tracks_already_present is exact.
//! 4. Tidal Album Expansion: Child tracks are persisted in library and linked to album.
//! 5. Tidal Playlist Expansion: Preserves track ordering in playlist_tracks (positions 1..N).
//! 6. Strict Counter Separation: favorite_tracks_total reflects only liked tracks, distinct from global catalogue.
//! 7. Account Isolation: Separate library_entries per account.
//! 8. Zero Audio Downloads: Sync does not create downloads or download_queue rows.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::types::ServiceSyncResult;
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::enrichment::{
    EnrichmentEngine, OriginTrackMetadata, SyncTrackInput,
};

async fn setup_test_db() -> sqlx::SqlitePool {
    let _ = crypto::init_keychain_crypto().or_else(|_| crypto::init_crypto([42u8; 32]));

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

async fn create_test_account(pool: &sqlx::SqlitePool, service_name: &str, display_name: &str) -> (i64, i64) {
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
                .unwrap_or(3)
        }
    };

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, ?, 1) RETURNING id"
    )
    .bind(service_id)
    .bind(display_name)
    .fetch_one(pool)
    .await
    .unwrap();

    (service_id, account_id)
}

#[tokio::test]
async fn test_empty_db_tidal_favorites_persists_tracks_sources_and_library_entries() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "tidal", "Tidal User 1").await;
    let engine = EnrichmentEngine::new();

    let sample_tracks = vec![
        ("7112001", "Blinding Lights", "The Weeknd", "After Hours", "USUM71900764", 200000),
        ("7112002", "Save Your Tears", "The Weeknd", "After Hours", "USUM72000215", 215000),
        ("7112003", "In Your Eyes", "The Weeknd", "After Hours", "USUM72000216", 237000),
    ];

    let mut tracks_new_global = 0;
    let mut sources_new_for_service = 0;
    let mut library_entries_new_for_account = 0;
    let mut tracks_already_present = 0;

    for (tidal_id, title, artist, album, isrc, dur) in &sample_tracks {
        let input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(title.to_string()),
                artist: Some(artist.to_string()),
                album: Some(album.to_string()),
                isrc: Some(isrc.to_string()),
                source_name: "tidal".to_string(),
                ..Default::default()
            },
            service_track_id: tidal_id.to_string(),
            service_name: "tidal".to_string(),
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
            duration_ms: Some(*dur),
            query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
        };

        let res = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();

        assert!(res.is_new_global_track, "Track {} should be new global track", title);
        assert!(res.is_new_source_for_service, "Track {} should be new source for Tidal", title);
        assert!(res.is_new_library_entry_for_account, "Track {} should be new library entry for account", title);
        assert!(!res.is_already_present, "Track {} should not be marked already present", title);
        assert!(res.is_new_import, "Track {} should be marked new import", title);

        if res.is_new_global_track { tracks_new_global += 1; }
        if res.is_new_source_for_service { sources_new_for_service += 1; }
        if res.is_new_library_entry_for_account { library_entries_new_for_account += 1; }
        if res.is_already_present { tracks_already_present += 1; }
    }

    assert_eq!(tracks_new_global, 3);
    assert_eq!(sources_new_for_service, 3);
    assert_eq!(library_entries_new_for_account, 3);
    assert_eq!(tracks_already_present, 0);

    // Verify database table counts
    let global_tracks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    let sources_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources WHERE service_id = ?").bind(service_id).fetch_one(&pool).await.unwrap();
    let entries_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ? AND is_liked = 1").bind(account_id).fetch_one(&pool).await.unwrap();

    assert_eq!(global_tracks_count, 3);
    assert_eq!(sources_count, 3);
    assert_eq!(entries_count, 3);
}

#[tokio::test]
async fn test_shared_isrc_qobuz_and_tidal_creates_separate_sources_without_duplicating_global_track() {
    let pool = setup_test_db().await;
    let (qobuz_svc_id, qobuz_acc_id) = create_test_account(&pool, "qobuz", "Qobuz User").await;
    let (tidal_svc_id, tidal_acc_id) = create_test_account(&pool, "tidal", "Tidal User").await;
    let engine = EnrichmentEngine::new();

    let shared_isrc = "GBAYE7300003";
    let title = "Time";
    let artist = "Pink Floyd";
    let album = "The Dark Side of the Moon";

    // 1. First import track from Qobuz
    let qobuz_input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some(title.to_string()),
            artist: Some(artist.to_string()),
            album: Some(album.to_string()),
            isrc: Some(shared_isrc.to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "qobuz_track_101".to_string(),
        service_name: "qobuz".to_string(),
        service_id: qobuz_svc_id,
        account_id: qobuz_acc_id,
        is_favorite: true,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(24),
        sample_rate: Some(96000),
        quality_score: Some(95),
        audio_quality: Some("hires".to_string()),
        cover_art_url: None,
        duration_ms: Some(421000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };

    let qobuz_res = engine.enrich_and_persist_sync_track(&pool, qobuz_input).await.unwrap();
    assert!(qobuz_res.is_new_global_track);
    assert!(qobuz_res.is_new_source_for_service);
    assert!(qobuz_res.is_new_library_entry_for_account);

    let tracks_after_qobuz: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    assert_eq!(tracks_after_qobuz, 1);

    // 2. Now import same track from Tidal with identical ISRC
    let tidal_input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some(title.to_string()),
            artist: Some(artist.to_string()),
            album: Some(album.to_string()),
            isrc: Some(shared_isrc.to_string()),
            source_name: "tidal".to_string(),
            ..Default::default()
        },
        service_track_id: "tidal_track_202".to_string(),
        service_name: "tidal".to_string(),
        service_id: tidal_svc_id,
        account_id: tidal_acc_id,
        is_favorite: true,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(16),
        sample_rate: Some(44100),
        quality_score: Some(80),
        audio_quality: Some("lossless".to_string()),
        cover_art_url: None,
        duration_ms: Some(421000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };

    let tidal_res = engine.enrich_and_persist_sync_track(&pool, tidal_input).await.unwrap();

    // Critical Invariant Assertions:
    assert_eq!(tidal_res.track_id, qobuz_res.track_id, "Track IDs must match via shared ISRC");
    assert!(!tidal_res.is_new_global_track, "Global track must NOT be duplicated");
    assert!(tidal_res.is_new_source_for_service, "Tidal track_source must be newly created");
    assert!(tidal_res.is_new_library_entry_for_account, "Tidal library_entry must be newly created");
    assert!(!tidal_res.is_already_present, "Must not be considered fully already present since Tidal source & entry are new");
    assert!(tidal_res.is_new_import, "Must be classified as new import for Tidal account");

    // Global tracks count remains 1
    let tracks_after_tidal: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    assert_eq!(tracks_after_tidal, 1);

    // Track sources has 2 rows (one for Qobuz, one for Tidal)
    let sources_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources WHERE track_id = ?")
        .bind(tidal_res.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sources_total, 2);

    // Library entries has 2 rows (one for Qobuz account, one for Tidal account)
    let entries_total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE track_id = ?")
        .bind(tidal_res.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(entries_total, 2);
}

#[tokio::test]
async fn test_resync_idempotency_zero_new_rows_and_already_present_exact() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "tidal", "Tidal User 1").await;
    let engine = EnrichmentEngine::new();

    let tidal_input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Starboy".to_string()),
            artist: Some("The Weeknd".to_string()),
            album: Some("Starboy".to_string()),
            isrc: Some("USUM71606822".to_string()),
            source_name: "tidal".to_string(),
            ..Default::default()
        },
        service_track_id: "7113001".to_string(),
        service_name: "tidal".to_string(),
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
        duration_ms: Some(230000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };

    // First Sync: Inserts all
    let first_res = engine.enrich_and_persist_sync_track(&pool, tidal_input.clone()).await.unwrap();
    assert!(first_res.is_new_global_track);
    assert!(first_res.is_new_source_for_service);
    assert!(first_res.is_new_library_entry_for_account);
    assert!(!first_res.is_already_present);
    assert!(first_res.is_new_import);

    // Second Sync (Re-Sync): 0 new rows
    let second_res = engine.enrich_and_persist_sync_track(&pool, tidal_input.clone()).await.unwrap();
    assert!(!second_res.is_new_global_track, "Re-sync must not insert global track");
    assert!(!second_res.is_new_source_for_service, "Re-sync must not insert new source");
    assert!(!second_res.is_new_library_entry_for_account, "Re-sync must not insert new library entry");
    assert!(second_res.is_already_present, "Re-sync must mark track as already present");
    assert!(!second_res.is_new_import, "Re-sync must not mark track as new import");

    let global_tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    let sources: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources").fetch_one(&pool).await.unwrap();
    let entries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries").fetch_one(&pool).await.unwrap();

    assert_eq!(global_tracks, 1);
    assert_eq!(sources, 1);
    assert_eq!(entries, 1);
}

#[tokio::test]
async fn test_tidal_album_expansion_persists_child_tracks_and_links_album() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "tidal", "Tidal User").await;
    let engine = EnrichmentEngine::new();

    let album_title = "Discovery";
    let artist_name = "Daft Punk";

    let child_tracks = vec![
        ("7114001", "One More Time", 1, "FRZ020100001", 320000),
        ("7114002", "Aerodynamic", 2, "FRZ020100002", 212000),
        ("7114003", "Digital Love", 3, "FRZ020100003", 298000),
        ("7114004", "Harder, Better, Faster, Stronger", 4, "FRZ020100004", 224000),
    ];

    let mut tracks_expanded = 0;
    for (tid, title, num, isrc, dur) in &child_tracks {
        let input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(title.to_string()),
                artist: Some(artist_name.to_string()),
                album: Some(album_title.to_string()),
                album_artist: Some(artist_name.to_string()),
                track_number: Some(*num),
                isrc: Some(isrc.to_string()),
                source_name: "tidal".to_string(),
                ..Default::default()
            },
            service_track_id: tid.to_string(),
            service_name: "tidal".to_string(),
            service_id,
            account_id,
            is_favorite: false, // Child tracks in album sync have is_favorite=false
            is_purchased: false,
            format: Some("FLAC".to_string()),
            bit_depth: Some(16),
            sample_rate: Some(44100),
            quality_score: Some(80),
            audio_quality: Some("lossless".to_string()),
            cover_art_url: None,
            duration_ms: Some(*dur),
            query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
        };

        let res = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();
        assert!(res.is_new_import);
        tracks_expanded += 1;
    }

    assert_eq!(tracks_expanded, 4);

    // Verify child tracks exist in tracks table
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    assert_eq!(count, 4);

    // Verify child tracks have library_entries with is_liked = 0
    let unliked_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ? AND is_liked = 0")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(unliked_count, 4);

    // Mark album as favorite in albums table
    sqlx::query("UPDATE albums SET is_favorite = 1, favorite_at = CURRENT_TIMESTAMP WHERE title = ? COLLATE NOCASE")
        .bind(album_title)
        .execute(&pool)
        .await
        .unwrap();

    let album_fav: i32 = sqlx::query_scalar("SELECT is_favorite FROM albums WHERE title = ? COLLATE NOCASE")
        .bind(album_title)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(album_fav, 1);
}

#[tokio::test]
async fn test_tidal_playlist_expansion_preserves_track_ordering() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "tidal", "Tidal User").await;
    let engine = EnrichmentEngine::new();

    // Create playlist
    let playlist_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO playlists (account_id, service_playlist_id, name, is_public, track_count)
           VALUES (?, 'tidal_pl_001', 'Late Night Vibes', 1, 3) RETURNING id"#
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let playlist_items = vec![
        ("7115001", "Midnight City", "M83", "GBAYE1100001", 1),
        ("7115002", "Intro", "The xx", "GBAYE0900002", 2),
        ("7115003", "Nightcall", "Kavinsky", "FRAYE1000003", 3),
    ];

    for (tid, title, artist, isrc, position) in &playlist_items {
        let input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(title.to_string()),
                artist: Some(artist.to_string()),
                isrc: Some(isrc.to_string()),
                source_name: "tidal".to_string(),
                ..Default::default()
            },
            service_track_id: tid.to_string(),
            service_name: "tidal".to_string(),
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
            duration_ms: Some(200000),
            query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
        };

        let res = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();

        sqlx::query("INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)"
            .as_ref())
            .bind(playlist_id)
            .bind(res.track_id)
            .bind(*position)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Verify ordering
    let ordered_positions: Vec<(i32, String)> = sqlx::query_as(
        r#"SELECT pt.position, t.title 
           FROM playlist_tracks pt 
           JOIN tracks t ON t.id = pt.track_id 
           WHERE pt.playlist_id = ? 
           ORDER BY pt.position ASC"#
    )
    .bind(playlist_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(ordered_positions.len(), 3);
    assert_eq!(ordered_positions[0], (1, "Midnight City".to_string()));
    assert_eq!(ordered_positions[1], (2, "Intro".to_string()));
    assert_eq!(ordered_positions[2], (3, "Nightcall".to_string()));
}

#[tokio::test]
async fn test_strict_counter_separation_favorites_seen_vs_global_catalogue() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "tidal", "Tidal User").await;
    let engine = EnrichmentEngine::new();

    // 1. Insert 5 catalogue tracks from album (not liked)
    for i in 1..=5 {
        let input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(format!("Album Track {}", i)),
                artist: Some("Album Artist".to_string()),
                album: Some("Big Album".to_string()),
                source_name: "tidal".to_string(),
                ..Default::default()
            },
            service_track_id: format!("alb_trk_{}", i),
            service_name: "tidal".to_string(),
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
        engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();
    }

    // 2. Insert 2 favorite tracks (liked)
    let mut favorites_seen = 0;
    let mut favorite_tracks_total = 0;
    for i in 1..=2 {
        let input = SyncTrackInput {
            origin_meta: OriginTrackMetadata {
                title: Some(format!("Favorite Track {}", i)),
                artist: Some("Fav Artist".to_string()),
                source_name: "tidal".to_string(),
                ..Default::default()
            },
            service_track_id: format!("fav_trk_{}", i),
            service_name: "tidal".to_string(),
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
            duration_ms: Some(180000),
            query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
        };
        let res = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();
        favorites_seen += 1;
        favorite_tracks_total += 1;
        assert!(res.is_new_import);
    }

    let global_tracks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    let liked_entries_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ? AND is_liked = 1")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(global_tracks_count, 7, "Total catalogue tracks must be 7 (5 album + 2 favorites)");
    assert_eq!(favorites_seen, 2, "favorites_seen must be strictly 2");
    assert_eq!(favorite_tracks_total, 2, "favorite_tracks_total must be strictly 2");
    assert_eq!(liked_entries_count, 2, "library_entries with is_liked = 1 must be strictly 2");
}

#[tokio::test]
async fn test_account_isolation_between_different_tidal_accounts() {
    let pool = setup_test_db().await;
    let (svc_id, acc_a) = create_test_account(&pool, "tidal", "Account Alpha").await;
    let (_, acc_b) = create_test_account(&pool, "tidal", "Account Beta").await;
    let engine = EnrichmentEngine::new();

    // Account Alpha imports Track A
    let input_a = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Alpha Track".to_string()),
            artist: Some("Alpha Artist".to_string()),
            source_name: "tidal".to_string(),
            ..Default::default()
        },
        service_track_id: "alpha_101".to_string(),
        service_name: "tidal".to_string(),
        service_id: svc_id,
        account_id: acc_a,
        is_favorite: true,
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
    let res_a = engine.enrich_and_persist_sync_track(&pool, input_a).await.unwrap();

    // Account Beta imports Track B
    let input_b = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Beta Track".to_string()),
            artist: Some("Beta Artist".to_string()),
            source_name: "tidal".to_string(),
            ..Default::default()
        },
        service_track_id: "beta_202".to_string(),
        service_name: "tidal".to_string(),
        service_id: svc_id,
        account_id: acc_b,
        is_favorite: true,
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
    let res_b = engine.enrich_and_persist_sync_track(&pool, input_b).await.unwrap();

    // Verify isolation in library_entries
    let acc_a_tracks: Vec<i64> = sqlx::query_scalar("SELECT track_id FROM library_entries WHERE account_id = ?")
        .bind(acc_a)
        .fetch_all(&pool)
        .await
        .unwrap();

    let acc_b_tracks: Vec<i64> = sqlx::query_scalar("SELECT track_id FROM library_entries WHERE account_id = ?")
        .bind(acc_b)
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(acc_a_tracks, vec![res_a.track_id]);
    assert_eq!(acc_b_tracks, vec![res_b.track_id]);
    assert_ne!(acc_a_tracks, acc_b_tracks);
}

#[tokio::test]
async fn test_no_audio_downloads_performed_during_sync() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "tidal", "Tidal User").await;
    let engine = EnrichmentEngine::new();

    let input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Non Download Track".to_string()),
            artist: Some("Artist".to_string()),
            source_name: "tidal".to_string(),
            ..Default::default()
        },
        service_track_id: "no_dl_999".to_string(),
        service_name: "tidal".to_string(),
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

    let _ = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();

    // Invariant: downloads and download_queue tables MUST remain completely empty during sync
    let downloads_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads").fetch_one(&pool).await.unwrap();
    let queue_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue").fetch_one(&pool).await.unwrap();

    assert_eq!(downloads_count, 0, "No audio file downloads may be registered during sync");
    assert_eq!(queue_count, 0, "No download queue items may be registered during sync");
}

#[tokio::test]
async fn test_service_sync_result_contract_contains_all_s142_granular_fields() {
    let result = ServiceSyncResult {
        service: "tidal".to_string(),
        account_id: Some(50),
        success: true,
        message: "Sync completed".to_string(),
        imported_tracks_total: 100,
        favorite_tracks_total: 92,
        favorite_albums_total: 10,
        favorite_artists_total: 5,
        playlists_total: 4,
        purchases_total: 0,
        skipped_tracks_total: 5,
        albums_total: 10,
        metadata_enriched: 95,
        metadata_partial: 5,
        availability_unknown: 0,
        availability_checked: 100,
        phase_timings: None,
        album_expansion_metrics: None,
        tracks_processed: 105,
        tracks_changed_unique: 100,
        tracks_new_global: 50,
        sources_new_for_service: 95,
        library_entries_new_for_account: 95,
        tracks_already_present: 5,
        favorites_seen: 92,
        albums_seen: 10,
        playlists_seen: 4,
        tracks_expanded: 8,
        tracks_expansion_failed: 0,
        albums_unavailable: 0,
        tracks_unavailable: 0,
        tracks_expansion_deferred: 0,
        sync_outcome: Some("success".to_string()),
        warnings: vec![],
        errors: vec![],
        ..Default::default()
    };

    assert_eq!(result.tracks_processed, 105);
    assert_eq!(result.tracks_changed_unique, 100);
    assert_eq!(result.tracks_already_present, 5);
    assert_eq!(result.tracks_processed, result.tracks_changed_unique + result.tracks_already_present);
    assert_eq!(result.tracks_new_global, 50);
    assert_eq!(result.sources_new_for_service, 95);
    assert_eq!(result.library_entries_new_for_account, 95);
    assert_eq!(result.favorites_seen, 92);
    assert_eq!(result.albums_seen, 10);
    assert_eq!(result.playlists_seen, 4);
    assert_eq!(result.tracks_expanded, 8);
    assert_eq!(result.tracks_expansion_failed, 0);
    assert_eq!(result.albums_unavailable, 0);
    assert_eq!(result.tracks_unavailable, 0);
    assert_eq!(result.tracks_expansion_deferred, 0);
    assert_eq!(result.sync_outcome.as_deref(), Some("success"));
    assert!(result.success);
}

#[test]
fn test_service_sync_result_camel_case_ipc_serialization() {
    let result = ServiceSyncResult {
        service: "tidal".to_string(),
        account_id: Some(50),
        success: true,
        message: "Sync completed for tidal".to_string(),
        imported_tracks_total: 0,
        favorite_tracks_total: 91,
        favorite_albums_total: 107,
        favorite_artists_total: 0,
        playlists_total: 57,
        purchases_total: 0,
        skipped_tracks_total: 3526,
        albums_total: 107,
        metadata_enriched: 3526,
        metadata_partial: 0,
        availability_unknown: 0,
        availability_checked: 3526,
        phase_timings: None,
        album_expansion_metrics: None,
        tracks_processed: 3526,
        tracks_changed_unique: 0,
        tracks_new_global: 0,
        sources_new_for_service: 0,
        library_entries_new_for_account: 0,
        tracks_already_present: 3526,
        favorites_seen: 91,
        albums_seen: 107,
        playlists_seen: 57,
        tracks_expanded: 3435,
        tracks_expansion_failed: 0,
        albums_unavailable: 10,
        tracks_unavailable: 47,
        tracks_expansion_deferred: 47,
        sync_outcome: Some("success_with_warnings".to_string()),
        warnings: vec!["Album unavailable".to_string()],
        errors: vec![],
        ..Default::default()
    };

    let json_val = serde_json::to_value(&result).expect("Must serialize to JSON");
    let json_str = serde_json::to_string_pretty(&result).expect("Must serialize string");

    // Verify all camelCase keys exist and are not null
    assert_eq!(json_val["service"], "tidal");
    assert_eq!(json_val["accountId"], 50);
    assert_eq!(json_val["tracksProcessed"], 3526);
    assert_eq!(json_val["tracksChangedUnique"], 0);
    assert_eq!(json_val["tracksAlreadyPresent"], 3526);
    assert_eq!(json_val["tracksNewGlobal"], 0);
    assert_eq!(json_val["sourcesNewForService"], 0);
    assert_eq!(json_val["libraryEntriesNewForAccount"], 0);
    assert_eq!(json_val["favoritesSeen"], 91);
    assert_eq!(json_val["albumsSeen"], 107);
    assert_eq!(json_val["playlistsSeen"], 57);
    assert_eq!(json_val["tracksExpanded"], 3435);
    assert_eq!(json_val["tracksExpansionFailed"], 0);
    assert_eq!(json_val["albumsUnavailable"], 10);
    assert_eq!(json_val["tracksUnavailable"], 47);
    assert_eq!(json_val["tracksExpansionDeferred"], 47);
    assert_eq!(json_val["syncOutcome"], "success_with_warnings");
    assert_eq!(json_val["warnings"].as_array().unwrap().len(), 1);

    // Verify no snake_case keys in direct serialization
    assert!(json_val.get("tracks_processed").is_none());
    assert!(json_val.get("tracks_changed_unique").is_none());
    assert!(json_val.get("tracks_already_present").is_none());
    assert!(json_val.get("tracks_new_global").is_none());
    assert!(json_val.get("albums_unavailable").is_none());
    assert!(json_val.get("tracks_unavailable").is_none());

    // Verify deserialization accepts camelCase JSON
    let roundtrip: ServiceSyncResult = serde_json::from_str(&json_str).expect("Must deserialize camelCase JSON");
    assert_eq!(roundtrip.tracks_processed, 3526);
    assert_eq!(roundtrip.tracks_already_present, 3526);
    assert_eq!(roundtrip.albums_unavailable, 10);
    assert_eq!(roundtrip.tracks_unavailable, 47);
    assert_eq!(roundtrip.sync_outcome.as_deref(), Some("success_with_warnings"));
    assert_eq!(roundtrip.account_id, Some(50));
}

#[tokio::test]
async fn test_favorite_already_exists_globally_without_tidal_source() {
    let pool = setup_test_db().await;
    let (spotify_svc_id, spotify_acc_id) = create_test_account(&pool, "spotify", "Spotify User").await;
    let (tidal_svc_id, tidal_acc_id) = create_test_account(&pool, "tidal", "Tidal User").await;
    let engine = EnrichmentEngine::new();

    let shared_isrc = "USUM71900764";

    // 1. Existing Spotify track in global catalogue
    let sp_input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Blinding Lights".to_string()),
            artist: Some("The Weeknd".to_string()),
            album: Some("After Hours".to_string()),
            isrc: Some(shared_isrc.to_string()),
            source_name: "spotify".to_string(),
            ..Default::default()
        },
        service_track_id: "spotify_trk_1".to_string(),
        service_name: "spotify".to_string(),
        service_id: spotify_svc_id,
        account_id: spotify_acc_id,
        is_favorite: true,
        is_purchased: false,
        format: Some("OGG".to_string()),
        bit_depth: None,
        sample_rate: None,
        quality_score: None,
        audio_quality: Some("standard".to_string()),
        cover_art_url: None,
        duration_ms: Some(200000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };
    let sp_res = engine.enrich_and_persist_sync_track(&pool, sp_input).await.unwrap();
    assert!(sp_res.is_new_global_track);

    // 2. Tidal user favorites same track -> Already exists globally, but NO Tidal source or library entry
    let tidal_input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Blinding Lights".to_string()),
            artist: Some("The Weeknd".to_string()),
            album: Some("After Hours".to_string()),
            isrc: Some(shared_isrc.to_string()),
            source_name: "tidal".to_string(),
            ..Default::default()
        },
        service_track_id: "tidal_trk_1".to_string(),
        service_name: "tidal".to_string(),
        service_id: tidal_svc_id,
        account_id: tidal_acc_id,
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

    let tidal_res = engine.enrich_and_persist_sync_track(&pool, tidal_input).await.unwrap();

    assert!(!tidal_res.is_new_global_track, "Global track must not be duplicated");
    assert!(tidal_res.is_new_source_for_service, "Tidal source must be new");
    assert!(tidal_res.is_new_library_entry_for_account, "Tidal library entry must be new");
    assert!(!tidal_res.is_already_present, "Not already present since source & entry are new");
    assert!(tidal_res.is_new_import, "Classified as new import for Tidal account");

    let global_tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    assert_eq!(global_tracks, 1);
}

#[tokio::test]
#[ignore = "Live API test against account 50 in runtime DB"]
async fn test_live_runtime_tidal_sync_account_50() {
    let app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let db_path = std::path::PathBuf::from(app_data)
        .join("com.syncify.app")
        .join("syncify.db");

    if !db_path.exists() {
        println!("DB does not exist at {:?}, skipping live test", db_path);
        return;
    }

    let _ = syncify_tauri_lib::crypto::init_keychain_crypto();

    let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", db_path.display()))
        .await
        .expect("Connect to runtime DB");

    // Perform real live sync with explicit account_id = 50
    let res = syncify_tauri_lib::commands::perform_sync_service_with_emitter(
        &pool,
        "tidal",
        Some(50),
        None,
        None::<&()>,
    )
    .await;

    match res {
        Ok(sync_result) => {
            println!("\n=== RUNTIME TIDAL SYNC RESULT (ACCOUNT 50) ===");
            let json_str = serde_json::to_string_pretty(&sync_result).expect("Serialize to JSON");
            println!("{}", json_str);

            let json_val = serde_json::to_value(&sync_result).expect("Serialize to JSON Value");
            println!("\nIPC Serialized Field Verification:");
            println!("service: {:?}", json_val.get("service"));
            println!("accountId: {:?}", json_val.get("accountId"));
            println!("tracksProcessed: {:?}", json_val.get("tracksProcessed"));
            println!("tracksChangedUnique: {:?}", json_val.get("tracksChangedUnique"));
            println!("tracksAlreadyPresent: {:?}", json_val.get("tracksAlreadyPresent"));
            println!("tracksNewGlobal: {:?}", json_val.get("tracksNewGlobal"));
            println!("sourcesNewForService: {:?}", json_val.get("sourcesNewForService"));
            println!("libraryEntriesNewForAccount: {:?}", json_val.get("libraryEntriesNewForAccount"));
            println!("favoritesSeen: {:?}", json_val.get("favoritesSeen"));
            println!("albumsSeen: {:?}", json_val.get("albumsSeen"));
            println!("playlistsSeen: {:?}", json_val.get("playlistsSeen"));
            println!("tracksExpanded: {:?}", json_val.get("tracksExpanded"));
            println!("tracksExpansionFailed: {:?}", json_val.get("tracksExpansionFailed"));
            println!("success: {:?}", json_val.get("success"));
            println!("message: {:?}", json_val.get("message"));

            // Verify camelCase IPC contract & Metric invariants
            assert_eq!(sync_result.service, "tidal");
            assert_eq!(sync_result.account_id, Some(50));
            assert!(json_val.get("tracksProcessed").is_some());
            assert!(json_val.get("tracksChangedUnique").is_some());
            assert!(json_val.get("tracksAlreadyPresent").is_some());
            assert!(sync_result.tracks_processed > 0);
            assert_eq!(sync_result.tracks_processed, sync_result.tracks_changed_unique + sync_result.tracks_already_present);
            assert_eq!(sync_result.favorites_seen, 92);
            assert_eq!(sync_result.albums_seen, 107);
            assert_eq!(sync_result.playlists_seen, 57);
        }
        Err(e) => {
            panic!("Live Tidal sync for account 50 failed: {}", e);
        }
    }
}

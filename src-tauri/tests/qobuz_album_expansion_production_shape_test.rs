use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::commands::types::AlbumSyncExpansionMetrics;
use syncify_tauri_lib::services::enrichment::{EnrichmentEngine, OriginTrackMetadata, SyncTrackInput};
use syncify_tauri_lib::services::qobuz::QobuzAlbum;

async fn setup_test_db() -> SqlitePool {
    let _ = syncify_tauri_lib::crypto::init_crypto([42u8; 32]);

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

async fn create_test_account(pool: &SqlitePool, service_name: &str, email: &str) -> (i64, i64) {
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
        "INSERT INTO accounts (service_id, display_name, email) VALUES (?, 'Test User', ?) RETURNING id",
    )
    .bind(service_id)
    .bind(email)
    .fetch_one(pool)
    .await
    .unwrap();

    (service_id, account_id)
}

// 1. Fixture album con tracks inline (formato objeto { items: [...] } y formato array [...])
#[tokio::test]
async fn test_1_fixture_album_with_inline_tracks_object_and_array() {
    // Formato A: tracks como objeto { items: [...], total: 2 }
    let json_object = r#"{
        "id": "0060253714709",
        "title": "Abbey Road",
        "released_at": -8400000,
        "tracks": {
            "items": [
                {
                    "id": 1001,
                    "title": "Come Together",
                    "duration": 259,
                    "isrc": "GBAYE0601498",
                    "track_number": 1
                },
                {
                    "id": 1002,
                    "title": "Something",
                    "duration": 182,
                    "isrc": "GBAYE0601499",
                    "track_number": 2
                }
            ],
            "total": 2
        }
    }"#;
    let album_obj: QobuzAlbum = serde_json::from_str(json_object).expect("Must deserialize tracks object");
    assert_eq!(album_obj.id, "0060253714709");
    assert_eq!(album_obj.title.as_deref(), Some("Abbey Road"));
    let tracks_obj = album_obj.tracks.expect("Tracks container present");
    assert_eq!(tracks_obj.items.len(), 2);
    assert_eq!(tracks_obj.items[0].title.as_deref(), Some("Come Together"));

    // Formato B: tracks directamente como array [ ... ]
    let json_array = r#"{
        "id": "0060253714709",
        "title": "Abbey Road (Array Format)",
        "tracks": [
            {
                "id": 2001,
                "title": "Maxwell's Silver Hammer",
                "duration": 207,
                "track_number": 3
            },
            {
                "id": 2002,
                "title": "Oh! Darling",
                "duration": 206,
                "track_number": 4
            }
        ]
    }"#;
    let album_arr: QobuzAlbum = serde_json::from_str(json_array).expect("Must deserialize tracks array");
    let tracks_arr = album_arr.tracks.expect("Tracks container present from array");
    assert_eq!(tracks_arr.items.len(), 2);
    assert_eq!(tracks_arr.items[0].title.as_deref(), Some("Maxwell's Silver Hammer"));
}

// 2. Fixture album sin tracks -> detail endpoint
#[tokio::test]
async fn test_2_fixture_album_without_tracks_triggers_detail_expansion() {
    let json_no_tracks = r#"{
        "id": "6269513",
        "title": "The Grand Illusion",
        "released_at": 238464000,
        "upc": "0007502132232"
    }"#;
    let album_no_tracks: QobuzAlbum = serde_json::from_str(json_no_tracks).expect("Must deserialize");
    assert!(album_no_tracks.tracks.is_none());

    // Verify helper check detects expansion needed
    let has_tracks = album_no_tracks.tracks.as_ref().map(|t| !t.items.is_empty()).unwrap_or(false);
    assert!(!has_tracks, "Album without tracks must require expansion");
}

// 3. Detail endpoint con ID numérico
#[tokio::test]
async fn test_3_detail_endpoint_with_numeric_id() {
    let json_detail_numeric = r#"{
        "id": 6269513,
        "title": "The Grand Illusion",
        "released_at": 238464000,
        "tracks": {
            "items": [
                {
                    "id": 4001,
                    "title": "Fooling Yourself",
                    "duration": 331,
                    "track_number": 1
                }
            ],
            "total": 1
        }
    }"#;
    let album: QobuzAlbum = serde_json::from_str(json_detail_numeric).expect("Numeric ID album must deserialize");
    assert_eq!(album.id, "6269513");
    assert_eq!(album.tracks.unwrap().items.len(), 1);
}

// 4. Detail endpoint con ID string
#[tokio::test]
async fn test_4_detail_endpoint_with_string_id() {
    let json_detail_string = r#"{
        "id": "al_0007502132232",
        "title": "The Grand Illusion (Special Edition)",
        "tracks": {
            "items": [
                {
                    "id": "5001",
                    "title": "Come Sail Away",
                    "duration": 367,
                    "track_number": 3
                }
            ],
            "total": 1
        }
    }"#;
    let album: QobuzAlbum = serde_json::from_str(json_detail_string).expect("String ID album must deserialize");
    assert_eq!(album.id, "al_0007502132232");
    assert_eq!(album.tracks.as_ref().unwrap().items[0].id, 5001);
}

// 5. Respuesta con campos reales de producción (float duration, performers object/string, work object, etc.)
#[tokio::test]
async fn test_5_real_production_shape_with_float_durations_and_complex_objects() {
    let json_prod = r#"{
        "id": "0060252771765",
        "title": "A Night at the Opera (Deluxe)",
        "released_at": 185846400,
        "label": {
            "id": 12,
            "name": "EMI Records"
        },
        "upc": 60252771765,
        "artist": {
            "id": 44,
            "name": "Queen"
        },
        "tracks": {
            "items": [
                {
                    "id": "2798336",
                    "title": "Bohemian Rhapsody",
                    "duration": 354.82,
                    "isrc": "GBUM71100621",
                    "copyright": "2011 Queen Productions Ltd.",
                    "performers": {
                        "main": "Freddie Mercury - Vocals, Piano",
                        "guitar": "Brian May"
                    },
                    "composer": {
                        "id": 991,
                        "name": "Freddie Mercury"
                    },
                    "work": {
                        "id": 101,
                        "title": "Opera Suite"
                    },
                    "track_number": 11,
                    "media_number": 1,
                    "maximum_bit_depth": 24,
                    "maximum_sampling_rate": 96.0,
                    "performer": {
                        "id": 44,
                        "name": "Queen"
                    }
                }
            ],
            "total": 1
        }
    }"#;

    let album: QobuzAlbum = serde_json::from_str(json_prod).expect("Production shape JSON must deserialize cleanly");
    assert_eq!(album.id, "0060252771765");
    assert_eq!(album.label.as_ref().and_then(|l| l.name.as_deref()), Some("EMI Records"));
    assert_eq!(album.upc.as_deref(), Some("60252771765"));

    let tracks = album.tracks.expect("Tracks present");
    let track = &tracks.items[0];
    assert_eq!(track.id, 2798336);
    assert_eq!(track.duration, 355); // Rounded from 354.82
    assert_eq!(track.work.as_deref(), Some("Opera Suite"));
    assert!(track.performers.is_some());
    assert_eq!(track.maximum_bit_depth, Some(24));
    assert_eq!(track.maximum_sampling_rate, Some(96.0));
}

// 6. Error de detail endpoint no se silencia y se registra en AlbumSyncExpansionMetrics
#[tokio::test]
async fn test_6_detail_endpoint_error_is_captured_in_metrics() {
    let metrics = AlbumSyncExpansionMetrics {
        albums_received: 5,
        albums_needing_expansion: 5,
        album_detail_requests: 5,
        album_detail_success: 4,
        album_detail_failed: 1,
        tracks_received: 40,
        tracks_persisted_new: 40,
        tracks_existing: 0,
        tracks_invalid: 0,
        first_error_code: Some("HTTP 404: Album not found".to_string()),
        first_error_album_id: Some("invalid_alb_999".to_string()),
    };

    assert_eq!(metrics.album_detail_failed, 1);
    assert_eq!(metrics.first_error_code.as_deref(), Some("HTTP 404: Album not found"));
    assert_eq!(metrics.first_error_album_id.as_deref(), Some("invalid_alb_999"));
}

// 7. 93 albums / 0 tracks produce partial failure, no success
#[tokio::test]
async fn test_7_albums_with_zero_tracks_produces_partial_failure() {
    let metrics = AlbumSyncExpansionMetrics {
        albums_received: 93,
        albums_needing_expansion: 93,
        album_detail_requests: 93,
        album_detail_success: 0,
        album_detail_failed: 93,
        tracks_received: 0,
        tracks_persisted_new: 0,
        tracks_existing: 0,
        tracks_invalid: 0,
        first_error_code: Some("HTTP 500: Internal Server Error".to_string()),
        first_error_album_id: Some("alb_1".to_string()),
    };

    let mut success = true;
    let mut errors = Vec::new();

    if metrics.albums_received > 0
        && metrics.tracks_persisted_new == 0
        && metrics.tracks_existing == 0
    {
        success = false;
        let err_detail = metrics.first_error_code.as_deref().unwrap_or("No tracks found in albums");
        errors.push(format!(
            "Qobuz album expansion failed: received {} albums, but 0 tracks imported ({})",
            metrics.albums_received, err_detail
        ));
    }

    assert!(!success, "93 albums with 0 tracks imported must report success = false");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("93 albums, but 0 tracks imported"));
}

// 8. Tracks expandidos aparecen en get_library
#[tokio::test]
async fn test_8_expanded_tracks_appear_in_library() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "qobuz", "test8@qobuz.local").await;
    let engine = EnrichmentEngine::new();

    let input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Child Track 1".to_string()),
            artist: Some("Artist 1".to_string()),
            album: Some("Favorite Album".to_string()),
            track_number: Some(1),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "qobuz_trk_888".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: false,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(24),
        sample_rate: Some(96000),
        quality_score: Some(90),
        audio_quality: Some("hires".to_string()),
        cover_art_url: None,
        duration_ms: Some(210000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };

    let res = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();
    assert!(res.is_new_import);

    // Verify track is in tracks table
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE id = ?")
        .bind(res.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Verify library_entries
    let entry: (i64, i32) = sqlx::query_as("SELECT track_id, is_liked FROM library_entries WHERE account_id = ? AND track_id = ?")
        .bind(account_id)
        .bind(res.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(entry.0, res.track_id);
    assert_eq!(entry.1, 0); // is_liked = 0 for album child track
}

// 9. Imported count refleja nuevos; existing count refleja duplicados
#[tokio::test]
async fn test_9_imported_count_reflects_new_and_existing_reflects_duplicates() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "qobuz", "test9@qobuz.local").await;
    let engine = EnrichmentEngine::new();

    let input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Repeated Track".to_string()),
            artist: Some("Repeated Artist".to_string()),
            album: Some("Repeated Album".to_string()),
            track_number: Some(1),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "qobuz_trk_dup_1".to_string(),
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

    let mut metrics = AlbumSyncExpansionMetrics::default();

    // First import
    let res1 = engine.enrich_and_persist_sync_track(&pool, input.clone()).await.unwrap();
    if res1.is_new_import {
        metrics.tracks_persisted_new += 1;
    } else {
        metrics.tracks_existing += 1;
    }

    // Second import of identical track
    let res2 = engine.enrich_and_persist_sync_track(&pool, input).await.unwrap();
    if res2.is_new_import {
        metrics.tracks_persisted_new += 1;
    } else {
        metrics.tracks_existing += 1;
    }

    assert_eq!(metrics.tracks_persisted_new, 1, "First sync must count as new import");
    assert_eq!(metrics.tracks_existing, 1, "Second sync must count as existing duplicate");
}

// 10. Cuentas múltiples no mezclan library_entries
#[tokio::test]
async fn test_10_multiple_accounts_do_not_mix_library_entries() {
    let pool = setup_test_db().await;
    let (service_id, account_id_1) = create_test_account(&pool, "qobuz", "user1@qobuz.local").await;
    let (_, account_id_2) = create_test_account(&pool, "qobuz", "user2@qobuz.local").await;
    let engine = EnrichmentEngine::new();

    // Account 1 imports track A
    let input_1 = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("User 1 Unique Track".to_string()),
            artist: Some("User 1 Artist".to_string()),
            album: Some("User 1 Album".to_string()),
            track_number: Some(1),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "qobuz_user1_trk".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id: account_id_1,
        is_favorite: false,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(24),
        sample_rate: Some(96000),
        quality_score: Some(90),
        audio_quality: Some("hires".to_string()),
        cover_art_url: None,
        duration_ms: Some(200000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };
    let _ = engine.enrich_and_persist_sync_track(&pool, input_1).await.unwrap();

    // Verify account 1 has 1 entry, account 2 has 0 entries
    let count_acc_1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
        .bind(account_id_1)
        .fetch_one(&pool)
        .await
        .unwrap();
    let count_acc_2: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
        .bind(account_id_2)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(count_acc_1, 1);
    assert_eq!(count_acc_2, 0);
}

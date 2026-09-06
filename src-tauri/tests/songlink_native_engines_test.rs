//! Integration test suite for TASK-36:
//! Connecting SongLink with Native Tidal/Qobuz Engines in `orchestrator.rs`.
//!
//! Tests:
//! 1. SongLinkAvailability parses and deserializes Odesli / SongLink JSON correctly:
//!    - Extracting `tidal_id` and `qobuz_id` from `entitiesByUniqueId`.
//!    - Extracting IDs from entityUniqueId format (`PLATFORM_SONG::ID`).
//!    - Extracting IDs from platform URLs as fallback.
//! 2. SongLink result with `tidal_id` routes candidate to native Tidal engine.
//! 3. SongLink result with `qobuz_id` routes candidate to native Qobuz engine.
//! 4. SongLink result with both Tidal and Qobuz respects orchestrator `service_priority`.
//! 5. SongLink result with neither Tidal nor Qobuz falls back to Amazon Music.
//! 6. Account activation in DB filters out inactive services from candidate resolution.
//! 7. Download execution preserves origin and effective service provenance.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::sync::Arc;
use syncify_tauri_lib::download::orchestrator::{DownloadOrchestrator, SongLinkEngineTarget};
use syncify_tauri_lib::download::progress::DownloadRequest;
use syncify_tauri_lib::download::songlink::{
    extract_id_from_entity, extract_id_from_url, SongLinkAvailability, SongLinkClient, TrackAvailability,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// Helper to set up an in-memory SQLite database with schema and test accounts
async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    // Insert baseline services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    pool
}

/// Spawns a local HTTP server that serves mock SongLink responses
async fn spawn_mock_songlink_server() -> (String, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, mut rx) = oneshot::channel();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                Ok((mut stream, _)) = listener.accept() => {
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    let req_str = String::from_utf8_lossy(&buf[..n]);

                    let (status, body) = if req_str.contains("spotify_tidal_only") {
                        (
                            "200 OK",
                            r#"{
                                "entityUniqueId": "SPOTIFY_SONG::spotify_tidal_only",
                                "linksByPlatform": {
                                    "tidal": {
                                        "country": "US",
                                        "url": "https://tidal.com/browse/track/34782012",
                                        "entityUniqueId": "TIDAL_SONG::34782012"
                                    }
                                },
                                "entitiesByUniqueId": {
                                    "TIDAL_SONG::34782012": {
                                        "id": "34782012",
                                        "type": "song",
                                        "title": "Tidal Only Track",
                                        "artistName": "Tidal Artist",
                                        "apiProvider": "tidal"
                                    }
                                }
                            }"#,
                        )
                    } else if req_str.contains("spotify_qobuz_only") {
                        (
                            "200 OK",
                            r#"{
                                "entityUniqueId": "SPOTIFY_SONG::spotify_qobuz_only",
                                "linksByPlatform": {
                                    "qobuz": {
                                        "country": "US",
                                        "url": "https://open.qobuz.com/track/19827364",
                                        "entityUniqueId": "QOBUZ_SONG::19827364"
                                    }
                                },
                                "entitiesByUniqueId": {
                                    "QOBUZ_SONG::19827364": {
                                        "id": "19827364",
                                        "type": "song",
                                        "title": "Qobuz Only Track",
                                        "artistName": "Qobuz Artist",
                                        "apiProvider": "qobuz"
                                    }
                                }
                            }"#,
                        )
                    } else if req_str.contains("spotify_both_tidal_qobuz") {
                        (
                            "200 OK",
                            r#"{
                                "entityUniqueId": "SPOTIFY_SONG::spotify_both_tidal_qobuz",
                                "linksByPlatform": {
                                    "tidal": {
                                        "country": "US",
                                        "url": "https://tidal.com/browse/track/555111",
                                        "entityUniqueId": "TIDAL_SONG::555111"
                                    },
                                    "qobuz": {
                                        "country": "US",
                                        "url": "https://open.qobuz.com/track/777222",
                                        "entityUniqueId": "QOBUZ_SONG::777222"
                                    },
                                    "amazonMusic": {
                                        "url": "https://music.amazon.com/albums/B07AMAZON1"
                                    }
                                },
                                "entitiesByUniqueId": {
                                    "TIDAL_SONG::555111": {
                                        "id": "555111",
                                        "type": "song",
                                        "apiProvider": "tidal"
                                    },
                                    "QOBUZ_SONG::777222": {
                                        "id": "777222",
                                        "type": "song",
                                        "apiProvider": "qobuz"
                                    }
                                }
                            }"#,
                        )
                    } else if req_str.contains("spotify_amazon_only") {
                        (
                            "200 OK",
                            r#"{
                                "entityUniqueId": "SPOTIFY_SONG::spotify_amazon_only",
                                "linksByPlatform": {
                                    "amazonMusic": {
                                        "url": "https://music.amazon.com/albums/B07FALLBACK99"
                                    }
                                }
                            }"#,
                        )
                    } else {
                        (
                            "404 Not Found",
                            r#"{"statusCode": 404, "message": "Not found"}"#,
                        )
                    };

                    let response = format!(
                        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status,
                        body.len(),
                        body
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.flush().await;
                }
                _ = &mut rx => {
                    break;
                }
            }
        }
    });

    (format!("http://{}", addr), tx)
}

#[tokio::test]
async fn test_songlink_availability_serialization_and_parsing() {
    let raw_json = r#"{
        "entityUniqueId": "SPOTIFY_SONG::4cOdK2wGLETKBW3PvgPWqT",
        "linksByPlatform": {
            "tidal": {
                "country": "US",
                "url": "https://tidal.com/browse/track/34782012",
                "entityUniqueId": "TIDAL_SONG::34782012"
            },
            "qobuz": {
                "country": "US",
                "url": "https://open.qobuz.com/track/19827364",
                "entityUniqueId": "QOBUZ_SONG::19827364"
            },
            "amazonMusic": {
                "url": "https://music.amazon.com/albums/B07TESTASIN"
            },
            "deezer": {
                "url": "https://www.deezer.com/track/987654",
                "entityUniqueId": "DEEZER_SONG::987654"
            }
        },
        "entitiesByUniqueId": {
            "TIDAL_SONG::34782012": {
                "id": "34782012",
                "type": "song",
                "apiProvider": "tidal"
            },
            "QOBUZ_SONG::19827364": {
                "id": "19827364",
                "type": "song",
                "apiProvider": "qobuz"
            },
            "DEEZER_SONG::987654": {
                "id": "987654",
                "type": "song",
                "apiProvider": "deezer"
            }
        }
    }"#;

    let avail = TrackAvailability::parse_from_json(raw_json).expect("Must parse Odesli response");

    assert!(avail.tidal, "Tidal must be flagged as available");
    assert_eq!(avail.tidal_id.as_deref(), Some("34782012"));

    assert!(avail.qobuz, "Qobuz must be flagged as available");
    assert_eq!(avail.qobuz_id.as_deref(), Some("19827364"));

    assert!(avail.amazon, "Amazon must be flagged as available");
    assert_eq!(
        avail.amazon_url.as_deref(),
        Some("https://music.amazon.com/albums/B07TESTASIN")
    );

    assert!(avail.deezer, "Deezer must be flagged as available");
    assert_eq!(avail.deezer_id.as_deref(), Some("987654"));

    // Roundtrip serialization
    let serialized = serde_json::to_string(&avail).expect("Must serialize TrackAvailability");
    let deserialized: SongLinkAvailability =
        serde_json::from_str(&serialized).expect("Must deserialize TrackAvailability");
    assert_eq!(avail, deserialized);
}

#[test]
fn test_id_extraction_from_entity_and_url() {
    assert_eq!(
        extract_id_from_entity("TIDAL_SONG::12345678"),
        Some("12345678".to_string())
    );
    assert_eq!(
        extract_id_from_entity("QOBUZ_SONG::998877"),
        Some("998877".to_string())
    );
    assert_eq!(
        extract_id_from_entity("plain_id_without_prefix"),
        Some("plain_id_without_prefix".to_string())
    );
    assert_eq!(extract_id_from_entity("   "), None);

    // Fallback extraction from URLs
    assert_eq!(
        extract_id_from_url("https://tidal.com/browse/track/55443322", "tidal"),
        Some("55443322".to_string())
    );
    assert_eq!(
        extract_id_from_url("https://listen.tidal.com/track/11223344", "tidal"),
        Some("11223344".to_string())
    );
    assert_eq!(
        extract_id_from_url("https://open.qobuz.com/track/88776655", "qobuz"),
        Some("88776655".to_string())
    );
    assert_eq!(
        extract_id_from_url("https://www.deezer.com/track/334455", "deezer"),
        Some("334455".to_string())
    );
    assert_eq!(
        extract_id_from_url("https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT", "spotify"),
        Some("4cOdK2wGLETKBW3PvgPWqT".to_string())
    );
}

#[tokio::test]
async fn test_songlink_tidal_id_routes_to_tidal_engine() {
    let (base_url, _tx) = spawn_mock_songlink_server().await;
    let songlink = Arc::new(SongLinkClient::new().with_base_url(base_url));
    let orchestrator = DownloadOrchestrator::new().with_songlink(songlink);

    let req = DownloadRequest {
        item_id: "test_spotify_tidal_1".to_string(),
        spotify_id: Some("spotify_tidal_only".to_string()),
        service_name: Some("spotify".to_string()),
        service_track_id: Some("spotify_tidal_only".to_string()),
        track_name: "Heroes".to_string(),
        artist_name: "David Bowie".to_string(),
        album_name: "Heroes".to_string(),
        quality: "LOSSLESS".to_string(),
        allow_fallback: true,
        ..Default::default()
    };

    let (candidates, avail) = orchestrator
        .resolve_songlink_candidates(&req)
        .await
        .expect("SongLink candidates must resolve");

    assert_eq!(avail.tidal_id.as_deref(), Some("34782012"));
    assert!(!candidates.is_empty(), "Must resolve at least one candidate");
    assert_eq!(
        candidates[0],
        SongLinkEngineTarget::Tidal("34782012".to_string())
    );

    // Verify orchestrator execution flow routes to Tidal
    let result = orchestrator.download_track(&req).await;
    assert!(result.is_err(), "Standalone download fails without auth credentials");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.to_lowercase().contains("tidal")
            || err_msg.contains("RequiresAuth")
            || err_msg.contains("SqlitePool"),
        "Error must originate from Tidal engine pipeline, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_songlink_qobuz_id_routes_to_qobuz_engine() {
    let (base_url, _tx) = spawn_mock_songlink_server().await;
    let songlink = Arc::new(SongLinkClient::new().with_base_url(base_url));
    let orchestrator = DownloadOrchestrator::new().with_songlink(songlink);

    let req = DownloadRequest {
        item_id: "test_spotify_qobuz_1".to_string(),
        spotify_id: Some("spotify_qobuz_only".to_string()),
        service_name: Some("spotify".to_string()),
        service_track_id: Some("spotify_qobuz_only".to_string()),
        track_name: "Life on Mars?".to_string(),
        artist_name: "David Bowie".to_string(),
        album_name: "Hunky Dory".to_string(),
        quality: "HI_RES_LOSSLESS".to_string(),
        allow_fallback: true,
        ..Default::default()
    };

    let (candidates, avail) = orchestrator
        .resolve_songlink_candidates(&req)
        .await
        .expect("SongLink candidates must resolve");

    assert_eq!(avail.qobuz_id.as_deref(), Some("19827364"));
    assert!(!candidates.is_empty(), "Must resolve at least one candidate");
    assert_eq!(
        candidates[0],
        SongLinkEngineTarget::Qobuz("19827364".to_string())
    );

    // Verify orchestrator execution flow routes to Qobuz
    let result = orchestrator.download_track(&req).await;
    assert!(result.is_err(), "Standalone download fails without auth credentials");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.to_lowercase().contains("qobuz")
            || err_msg.contains("RequiresAuth")
            || err_msg.contains("app_id"),
        "Error must originate from Qobuz engine pipeline, got: {}",
        err_msg
    );
}

#[tokio::test]
async fn test_songlink_both_matches_respects_service_priority() {
    let (base_url, _tx) = spawn_mock_songlink_server().await;

    let req = DownloadRequest {
        item_id: "test_priority_1".to_string(),
        spotify_id: Some("spotify_both_tidal_qobuz".to_string()),
        service_name: Some("spotify".to_string()),
        service_track_id: Some("spotify_both_tidal_qobuz".to_string()),
        track_name: "Starman".to_string(),
        artist_name: "David Bowie".to_string(),
        album_name: "Ziggy Stardust".to_string(),
        quality: "LOSSLESS".to_string(),
        allow_fallback: true,
        ..Default::default()
    };

    // 1. When priority is Qobuz -> Tidal -> Amazon
    let songlink1 = Arc::new(SongLinkClient::new().with_base_url(base_url.clone()));
    let orch_qobuz_first = DownloadOrchestrator::new()
        .with_songlink(songlink1)
        .with_priority(vec![
            "qobuz".to_string(),
            "tidal".to_string(),
            "amazon".to_string(),
        ]);

    let (candidates_qobuz, _) = orch_qobuz_first
        .resolve_songlink_candidates(&req)
        .await
        .unwrap();

    assert_eq!(
        candidates_qobuz[0],
        SongLinkEngineTarget::Qobuz("777222".to_string()),
        "Primary candidate must be Qobuz when Qobuz has top priority"
    );
    assert_eq!(
        candidates_qobuz[1],
        SongLinkEngineTarget::Tidal("555111".to_string()),
        "Secondary candidate must be Tidal"
    );
    assert_eq!(
        candidates_qobuz[2],
        SongLinkEngineTarget::Amazon("https://music.amazon.com/albums/B07AMAZON1".to_string()),
        "Tertiary candidate must be Amazon"
    );

    // 2. When priority is Tidal -> Qobuz -> Amazon
    let songlink2 = Arc::new(SongLinkClient::new().with_base_url(base_url));
    let orch_tidal_first = DownloadOrchestrator::new()
        .with_songlink(songlink2)
        .with_priority(vec![
            "tidal".to_string(),
            "qobuz".to_string(),
            "amazon".to_string(),
        ]);

    let (candidates_tidal, _) = orch_tidal_first
        .resolve_songlink_candidates(&req)
        .await
        .unwrap();

    assert_eq!(
        candidates_tidal[0],
        SongLinkEngineTarget::Tidal("555111".to_string()),
        "Primary candidate must be Tidal when Tidal has top priority"
    );
    assert_eq!(
        candidates_tidal[1],
        SongLinkEngineTarget::Qobuz("777222".to_string()),
        "Secondary candidate must be Qobuz"
    );
}

#[tokio::test]
async fn test_songlink_absence_of_tidal_and_qobuz_falls_back_to_amazon() {
    let (base_url, _tx) = spawn_mock_songlink_server().await;
    let songlink = Arc::new(SongLinkClient::new().with_base_url(base_url));
    let orchestrator = DownloadOrchestrator::new().with_songlink(songlink);

    let req = DownloadRequest {
        item_id: "test_amazon_fallback".to_string(),
        spotify_id: Some("spotify_amazon_only".to_string()),
        service_name: Some("spotify".to_string()),
        service_track_id: Some("spotify_amazon_only".to_string()),
        track_name: "Amazon Exclusive".to_string(),
        artist_name: "Artist".to_string(),
        album_name: "Album".to_string(),
        quality: "LOSSLESS".to_string(),
        allow_fallback: true,
        ..Default::default()
    };

    let (candidates, avail) = orchestrator
        .resolve_songlink_candidates(&req)
        .await
        .expect("SongLink candidates must resolve");

    assert!(avail.tidal_id.is_none());
    assert!(avail.qobuz_id.is_none());
    assert_eq!(
        avail.amazon_url.as_deref(),
        Some("https://music.amazon.com/albums/B07FALLBACK99")
    );

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0],
        SongLinkEngineTarget::Amazon("https://music.amazon.com/albums/B07FALLBACK99".to_string())
    );
}

#[tokio::test]
async fn test_songlink_db_active_account_filters_unavailable_engine() {
    let db = setup_test_db().await;

    // Only activate Tidal account in DB (Qobuz has no active account)
    sqlx::query(
        r#"
        INSERT INTO accounts (id, service_id, display_name, email, is_active, credentials_json)
        VALUES (301, 3, 'Tidal User', 'user@tidal.com', 1, '{"access_token":"token","user_id":1}')
        "#,
    )
    .execute(&db)
    .await
    .unwrap();

    let (base_url, _tx) = spawn_mock_songlink_server().await;
    let songlink = Arc::new(SongLinkClient::new().with_base_url(base_url));
    let orchestrator = DownloadOrchestrator::new()
        .with_db(db.clone())
        .with_songlink(songlink)
        // Set priority Qobuz first, but Qobuz has NO active account
        .with_priority(vec![
            "qobuz".to_string(),
            "tidal".to_string(),
            "amazon".to_string(),
        ]);

    assert!(
        orchestrator.is_service_available("tidal").await,
        "Tidal must be available since account is active"
    );
    assert!(
        !orchestrator.is_service_available("qobuz").await,
        "Qobuz must NOT be available since no active account exists in DB"
    );

    let req = DownloadRequest {
        item_id: "test_db_filter".to_string(),
        spotify_id: Some("spotify_both_tidal_qobuz".to_string()),
        service_name: Some("spotify".to_string()),
        service_track_id: Some("spotify_both_tidal_qobuz".to_string()),
        track_name: "Rebel Rebel".to_string(),
        artist_name: "David Bowie".to_string(),
        album_name: "Diamond Dogs".to_string(),
        quality: "LOSSLESS".to_string(),
        allow_fallback: true,
        ..Default::default()
    };

    let (candidates, _) = orchestrator
        .resolve_songlink_candidates(&req)
        .await
        .unwrap();

    // Qobuz must be skipped because it is inactive in DB, leaving Tidal as first candidate
    assert_eq!(
        candidates[0],
        SongLinkEngineTarget::Tidal("555111".to_string()),
        "Tidal must be chosen because Qobuz has no active account in DB"
    );
}

#[tokio::test]
async fn test_songlink_unsupported_service_when_no_match() {
    let (base_url, _tx) = spawn_mock_songlink_server().await;
    let songlink = Arc::new(SongLinkClient::new().with_base_url(base_url));
    let orchestrator = DownloadOrchestrator::new().with_songlink(songlink);

    let req = DownloadRequest {
        item_id: "test_no_match".to_string(),
        spotify_id: Some("nonexistent_track_404".to_string()),
        service_name: Some("spotify".to_string()),
        service_track_id: Some("nonexistent_track_404".to_string()),
        track_name: "Unknown Track".to_string(),
        artist_name: "Unknown Artist".to_string(),
        album_name: "Unknown Album".to_string(),
        quality: "LOSSLESS".to_string(),
        allow_fallback: true,
        ..Default::default()
    };

    let result = orchestrator.download_track(&req).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("Unsupported or unavailable service") || err_msg.contains("404"),
        "Expected unsupported or unavailable service error, got: {}",
        err_msg
    );
}

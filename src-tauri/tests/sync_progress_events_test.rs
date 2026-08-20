use sqlx::sqlite::SqlitePoolOptions;
use std::sync::{Arc, Mutex};
use syncify_tauri_lib::commands::{
    mark_account_credentials_invalid, perform_get_service_auth_status, perform_sync_service,
    perform_sync_service_with_emitter, ImportPreferences, SyncProgressEmitter, SyncProgressEvent,
};
use syncify_tauri_lib::crypto;

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

/// Helper struct for collecting progress events in tests
#[derive(Clone, Default)]
struct TestEventCollector {
    events: Arc<Mutex<Vec<SyncProgressEvent>>>,
}

impl TestEventCollector {
    fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_events(&self) -> Vec<SyncProgressEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl SyncProgressEmitter for TestEventCollector {
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

#[tokio::test]
async fn test_started_event_emitted_immediately() {
    let pool = setup_test_db().await;
    let collector = TestEventCollector::new();

    // Trigger sync on missing account
    let _ = perform_sync_service_with_emitter(&pool, "qobuz", None, None, Some(&collector)).await;

    let events = collector.get_events();
    assert!(!events.is_empty(), "Events should not be empty");

    let first = &events[0];
    assert_eq!(first.service, "qobuz");
    assert_eq!(first.operation, "sync");
    assert_eq!(first.phase, "authenticating");
    assert_eq!(first.status, "running");
    assert_eq!(first.current, 0);
    assert_eq!(first.total, None);
    assert_eq!(first.imported_tracks_total, 0);
    assert_eq!(first.favorite_tracks_total, 0);
    assert_eq!(first.terminal, false);
}

#[tokio::test]
async fn test_requires_auth_on_missing_account_stops_immediately() {
    let pool = setup_test_db().await;
    let collector = TestEventCollector::new();

    let err = perform_sync_service_with_emitter(&pool, "qobuz", None, None, Some(&collector))
        .await
        .unwrap_err();

    assert!(err.starts_with("RequiresAuth:"));

    let events = collector.get_events();
    assert_eq!(events.len(), 2, "Must emit started and requires_auth only");

    let terminal = &events[1];
    assert_eq!(terminal.service, "qobuz");
    assert_eq!(terminal.phase, "requires_auth");
    assert_eq!(terminal.status, "requires_auth");
    assert_eq!(terminal.terminal, true);

    // Ensure NO fetching phases were emitted
    assert!(!events.iter().any(|e| e.phase == "fetching_favorite_tracks"));
    assert!(!events.iter().any(|e| e.phase == "fetching_playlists"));
    assert!(!events.iter().any(|e| e.phase == "persisting"));
    assert!(!events.iter().any(|e| e.phase == "completed"));
}

#[tokio::test]
async fn test_requires_auth_on_account_with_missing_token() {
    let pool = setup_test_db().await;
    let collector = TestEventCollector::new();

    let qobuz_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let empty_creds = serde_json::json!({ "user_id": "999" }).to_string();
    let encrypted = crypto::encrypt(&empty_creds).unwrap();

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active)
           VALUES (?, 'No Token Qobuz', ?, 1) RETURNING id"#
    )
    .bind(qobuz_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    let err = perform_sync_service_with_emitter(&pool, "qobuz", Some(account_id), None, Some(&collector))
        .await
        .unwrap_err();

    assert!(err.starts_with("RequiresAuth:"));

    let events = collector.get_events();
    let last = events.last().expect("must have events");
    assert_eq!(last.phase, "requires_auth");
    assert_eq!(last.status, "requires_auth");
    assert_eq!(last.terminal, true);
    assert_eq!(last.account_id, Some(account_id));
}

#[tokio::test]
async fn test_401_marks_credentials_invalid_and_emits_requires_auth() {
    let pool = setup_test_db().await;
    let collector = TestEventCollector::new();

    let qobuz_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let creds = serde_json::json!({
        "user_auth_token": "valid_token_string_1234567890abcdef",
        "user_id": "12345"
    })
    .to_string();
    let encrypted = crypto::encrypt(&creds).unwrap();

    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active)
           VALUES (?, 'Qobuz 401 Test', ?, 1) RETURNING id"#
    )
    .bind(qobuz_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Mark credentials invalid as happens on 401
    mark_account_credentials_invalid(&pool, "qobuz", "HTTP 401: User authentication required")
        .await
        .unwrap();

    let err = perform_sync_service_with_emitter(&pool, "qobuz", Some(account_id), None, Some(&collector))
        .await
        .unwrap_err();

    assert!(err.starts_with("RequiresAuth:"));

    let status = perform_get_service_auth_status(&pool, "qobuz", Some(account_id))
        .await
        .unwrap();
    assert_eq!(status.status, "requires_auth");
    assert!(!status.is_authenticated);

    let events = collector.get_events();
    let terminal = events.iter().find(|e| e.terminal).expect("must have terminal event");
    assert_eq!(terminal.phase, "requires_auth");
    assert_eq!(terminal.status, "requires_auth");
}

#[tokio::test]
async fn test_events_do_not_leak_secrets() {
    let pool = setup_test_db().await;
    let collector = TestEventCollector::new();

    let secret_token = "ultra_sensitive_secret_token_abcdef987654";
    let spotify_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'spotify'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let creds = serde_json::json!({
        "access_token": secret_token,
        "refresh_token": "refresh_secret_12345",
        "expires_at": 100 // expired to trigger failure/requires_auth
    })
    .to_string();
    let encrypted = crypto::encrypt(&creds).unwrap();

    let _aid: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active)
           VALUES (?, 'Spotify Secret Test', ?, 1) RETURNING id"#
    )
    .bind(spotify_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    let _ = perform_sync_service_with_emitter(&pool, "spotify", None, None, Some(&collector)).await;

    let events = collector.get_events();
    assert!(!events.is_empty());

    for event in &events {
        assert!(!event.message.contains(secret_token), "Secret token leaked in message: {}", event.message);
        assert!(!event.message.contains("refresh_secret"), "Refresh token leaked in message: {}", event.message);

        let json = serde_json::to_string(event).unwrap();
        assert!(!json.contains(secret_token), "Secret token leaked in event JSON: {}", json);
        assert!(!json.contains("refresh_secret"), "Refresh token leaked in event JSON: {}", json);
    }
}

#[tokio::test]
async fn test_etapas_activadas_por_preferencias_and_completion() {
    let pool = setup_test_db().await;
    let collector = TestEventCollector::new();

    let tidal_svc_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(&pool)
        .await
        .unwrap();

    let creds = serde_json::json!({
        "access_token": "tidal_test_token_1234567890abcdef12345678",
        "user_id": "12345",
        "country_code": "US"
    })
    .to_string();
    let encrypted = crypto::encrypt(&creds).unwrap();

    let _aid: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, credentials_json, is_active)
           VALUES (?, 'Tidal Preferences Test', ?, 1) RETURNING id"#
    )
    .bind(tidal_svc_id)
    .bind(&encrypted)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Preferences: all fetching phases disabled to verify phase suppression and clean completion
    let prefs = ImportPreferences {
        service_name: "tidal".to_string(),
        favorite_tracks: false,
        favorite_albums: false,
        favorite_artists: false,
        playlists: false,
        purchases: false,
        library_history: false,
        include_appearances: false,
        incremental_sync: true,
        ..Default::default()
    };

    let result = perform_sync_service_with_emitter(&pool, "tidal", None, Some(prefs), Some(&collector))
        .await
        .expect("Tidal sync should complete cleanly when phases are filtered");

    assert!(result.success);

    let events = collector.get_events();
    let phases: Vec<String> = events.iter().map(|e| e.phase.clone()).collect();

    // Must have authenticating, persisting, completed
    assert!(phases.contains(&"authenticating".to_string()));
    assert!(phases.contains(&"persisting".to_string()));
    assert!(phases.contains(&"completed".to_string()));

    // Must NOT have disabled phases
    assert!(!phases.contains(&"fetching_favorite_tracks".to_string()), "favorite_tracks was disabled");
    assert!(!phases.contains(&"fetching_favorite_albums".to_string()), "favorite_albums was disabled");
    assert!(!phases.contains(&"fetching_favorite_artists".to_string()), "favorite_artists was disabled");
    assert!(!phases.contains(&"fetching_playlists".to_string()), "playlists was disabled");
    assert!(!phases.contains(&"fetching_purchases".to_string()), "purchases was disabled");
    assert!(!phases.contains(&"fetching_history".to_string()), "library_history was disabled");

    // Last event must be completed and terminal
    let last = events.last().unwrap();
    assert_eq!(last.phase, "completed");
    assert_eq!(last.status, "completed");
    assert_eq!(last.terminal, true);
}

#[tokio::test]
async fn test_sync_service_backward_compatibility_delegates_to_perform_sync_service() {
    let pool = setup_test_db().await;

    // Direct call to perform_sync_service (no emitter) works seamlessly
    let err = perform_sync_service(&pool, "qobuz", None, None).await.unwrap_err();
    assert!(err.starts_with("RequiresAuth:"));
}

#[tokio::test]
async fn test_closure_emitter_receives_events() {
    let pool = setup_test_db().await;
    let events = Arc::new(Mutex::new(Vec::<SyncProgressEvent>::new()));
    let events_clone = events.clone();

    let closure_emitter = syncify_tauri_lib::commands::SyncCallback(move |evt: &SyncProgressEvent| {
        events_clone.lock().unwrap().push(evt.clone());
    });

    let _ = perform_sync_service_with_emitter(&pool, "qobuz", None, None, Some(&closure_emitter)).await;

    let collected = events.lock().unwrap().clone();
    assert_eq!(collected.len(), 2);
    assert_eq!(collected[0].phase, "authenticating");
    assert_eq!(collected[1].phase, "requires_auth");
}

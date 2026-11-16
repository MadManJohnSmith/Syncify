//! Manifest v2 (SB04, #200; ADR-0020): el export lleva biblioteca, ajustes
//! completos y metadatos de cuenta sin ningún secreto; la `schema_version`
//! se deriva del estado real de la BD; un manifest v1 sigue importándose.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::sync::Arc;
use syncify_tauri_lib::{
    commands::backup::{export_library_with_bases, import_library_with_bases},
    crypto,
    settings_defaults::SETTINGS_KV_KEYS,
    worker::DownloadWorkerState,
    AppState, EnrichmentWorkerState,
};
use tauri::Manager;

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations through current must apply cleanly");

    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    pool
}

fn create_test_app(pool: SqlitePool) -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    let state = AppState {
        db: pool,
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };
    app.manage(state);
    app
}

struct TestBackupPath {
    _temp_dir: tempfile::TempDir,
    path: String,
    allowed_bases: Vec<std::path::PathBuf>,
}

fn get_test_backup_path(prefix: &str) -> TestBackupPath {
    let temp_dir = tempfile::tempdir().expect("create isolated backup directory");
    let base = temp_dir
        .path()
        .canonicalize()
        .expect("canonicalize isolated backup directory");
    let path = base
        .join(format!("{}_{}.json", prefix, uuid::Uuid::new_v4()))
        .to_string_lossy()
        .to_string();
    TestBackupPath {
        _temp_dir: temp_dir,
        path,
        allowed_bases: vec![base],
    }
}

/// Recorre el JSON y devuelve cada (nombre de clave, valor de cadena) que
/// aparece en cualquier nivel, para el contrato sin secretos.
fn collect_strings(value: &serde_json::Value, out: &mut Vec<(String, String)>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                if let serde_json::Value::String(s) = val {
                    out.push((key.clone(), s.clone()));
                }
                collect_strings(val, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_strings(item, out);
            }
        }
        _ => {}
    }
}

async fn export_to(
    app: &tauri::App<tauri::test::MockRuntime>,
    backup: &TestBackupPath,
) -> serde_json::Value {
    let app_state = app.state::<AppState>();
    let res =
        export_library_with_bases(app_state, Some(backup.path.clone()), &backup.allowed_bases)
            .await
            .expect("export_library must succeed");
    assert_eq!(res.checksum.len(), 64, "checksum SHA-256 must survive");

    let raw = std::fs::read_to_string(&backup.path).expect("manifest file must exist");
    serde_json::from_str(&raw).expect("manifest file must parse as JSON")
}

#[tokio::test]
async fn manifest_v2_carries_library_settings_and_account_metadata() {
    let db = create_test_db().await;

    // Biblioteca v1 intacta.
    let _artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name, favorite_at) VALUES ('Talking Heads', '2026-10-01T00:00:00Z') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, favorite_at) VALUES ('Remain in Light', '2026-10-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    let _: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, isrc, favorite_at) VALUES ('Once in a Lifetime', ?, 'GBAAA8001234', '2026-10-03T00:00:00Z') RETURNING id",
    )
    .bind(album_id)
    .fetch_one(&db)
    .await
    .unwrap();

    // Ajustes: un valor no default en cada patrón de almacenamiento.
    sqlx::query(
        "UPDATE sync_settings SET auto_sync_enabled = 1, max_concurrent_downloads = 5 WHERE id = 1",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE quality_preferences SET max_quality = 'hires' WHERE service_name = 'spotify'",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query("UPDATE metadata_preferences SET multi_value_separator = ';' WHERE id = 1")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE folder_settings SET folder_template = '{artist}/{album}' WHERE id = 1")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('dl_concurrent_downloads', '7', datetime('now'))")
        .execute(&db)
        .await
        .unwrap();

    // Cuenta con identidad externa; el token jamás entra al manifest.
    sqlx::query(
        "INSERT INTO accounts (id, service_id, display_name, email, external_user_id, is_active, credentials_json)
         VALUES (7, 3, 'Tidal Main', 'main@example.com', 'ext-usr-4242', 1, '{\"access_token\":\"tok-should-not-travel\"}')",
    )
    .execute(&db)
    .await
    .unwrap();

    let app = create_test_app(db);
    let backup = get_test_backup_path("manifest_v2_full");
    let manifest = export_to(&app, &backup).await;

    assert_eq!(manifest["version"], "2.0.0");

    // Biblioteca.
    assert_eq!(manifest["tracks"].as_array().unwrap().len(), 1);
    assert_eq!(manifest["tracks"][0]["title"], "Once in a Lifetime");

    // Ajustes tipados: todas las superficies presentes y con el valor mutado.
    let settings = &manifest["settings"];
    assert!(settings.is_object(), "settings must be present in v2");
    for surface in [
        "service_preferences",
        "sync_settings",
        "service_sync_settings",
        "quality_preferences",
        "folder_settings",
        "duplicate_settings",
        "audio_processing_settings",
        "lyrics_provider_settings",
        "lyrics_config",
        "metadata_preferences",
        "advanced_settings",
    ] {
        assert!(
            settings[surface].is_array(),
            "settings surface '{}' must be exported",
            surface
        );
    }
    assert_eq!(settings["sync_settings"][0]["max_concurrent_downloads"], 5);
    assert_eq!(settings["sync_settings"][0]["auto_sync_enabled"], 1);
    assert_eq!(settings["quality_preferences"][0]["max_quality"], "hires");
    assert_eq!(
        settings["metadata_preferences"][0]["multi_value_separator"],
        ";"
    );
    assert_eq!(
        settings["folder_settings"][0]["folder_template"],
        "{artist}/{album}"
    );

    // KV de ajustes: dentro del catálogo de usuario.
    let kv = settings["kv"].as_object().expect("kv must be an object");
    assert_eq!(kv.get("dl_concurrent_downloads").unwrap(), "7");

    // Metadatos de cuenta: identidad sin credenciales.
    let accounts = manifest["accounts"]
        .as_array()
        .expect("accounts must be present in v2");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0]["service"], "tidal");
    assert_eq!(accounts[0]["external_user_id"], "ext-usr-4242");
    assert_eq!(accounts[0]["display_name"], "Tidal Main");
    assert_eq!(accounts[0]["email"], "main@example.com");
    assert_eq!(accounts[0]["is_active"], true);
    assert!(
        accounts[0].get("credentials_json").is_none(),
        "account metadata must never carry credentials"
    );
}

#[tokio::test]
async fn manifest_v2_excludes_every_secret_from_settings_kv() {
    // El secret de spotify se planta cifrado, como lo guarda el backend real.
    let _ = crypto::init_keychain_crypto().or_else(|_| crypto::init_crypto([42u8; 32]));

    let db = create_test_db().await;

    // Semillas de secretos: la KV de spotify_* como la guarda el backend
    // (secret cifrado) y la clave de Last.fm en texto.
    let encrypted_secret = crypto::encrypt("spotify-secret-PLAINTEXT-4242").unwrap();
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('spotify_client_id', 'sp-client-id-9999', datetime('now'))")
        .execute(&db).await.unwrap();
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('spotify_client_secret', ?, datetime('now'))")
        .bind(&encrypted_secret)
        .execute(&db).await.unwrap();
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('lastfm_api_key', 'lastfm-key-7777', datetime('now'))")
        .execute(&db).await.unwrap();
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('session_token', 'tok-session-1111', datetime('now'))")
        .execute(&db).await.unwrap();

    let app = create_test_app(db);
    let backup = get_test_backup_path("manifest_v2_secrets");
    let manifest = export_to(&app, &backup).await;

    let mut strings = Vec::new();
    collect_strings(&manifest, &mut strings);
    let secrets = [
        "spotify-secret-PLAINTEXT-4242",
        encrypted_secret.as_str(),
        "sp-client-id-9999",
        "lastfm-key-7777",
        "tok-session-1111",
    ];
    for (key, value) in &strings {
        for secret in &secrets {
            assert!(
                !value.contains(secret),
                "secret leaked through settings key '{}': manifest must carry no token, API key or credential",
                key
            );
        }
    }
    assert!(
        !strings
            .iter()
            .any(|(key, _)| key.contains("spotify_client")),
        "spotify credential keys must not travel under any name"
    );

    // La exclusión es selectiva: todo lo que viaja en la KV pertenece al
    // catálogo de usuario y ninguna clave sensible del catálogo se cuela.
    let kv = manifest["settings"]["kv"].as_object().unwrap();
    for key in kv.keys() {
        assert!(
            SETTINGS_KV_KEYS.contains(&key.as_str()),
            "exported KV key '{}' must be part of the user-settings catalog",
            key
        );
        assert!(
            !key.contains("api_key") && !key.contains("token") && !key.contains("secret"),
            "sensitive catalog key '{}' must never travel",
            key
        );
    }
}

#[tokio::test]
async fn schema_version_is_derived_from_the_applied_migrations() {
    let db = create_test_db().await;
    let applied: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
        .fetch_one(&db)
        .await
        .unwrap();

    let app = create_test_app(db);
    let backup = get_test_backup_path("manifest_v2_schema_version");
    let manifest = export_to(&app, &backup).await;

    assert_eq!(
        manifest["schema_version"], applied,
        "schema_version must reflect the real schema state, not a constant"
    );
    assert!(
        applied > 100,
        "the fixture must run against a schema newer than the old hardcoded 47"
    );
}

#[tokio::test]
async fn v1_manifest_without_settings_or_accounts_still_imports() {
    let db = create_test_db().await;
    let app = create_test_app(db);
    let backup = get_test_backup_path("manifest_v1_import");

    // Un manifest v1 auténtico: sin settings ni accounts, biblioteca sola.
    let v1 = serde_json::json!({
        "version": "1.0.0",
        "schema_version": 47,
        "exported_at": "2026-01-01T00:00:00Z",
        "app_version": "0.1.0",
        "checksum_sha256": null,
        "tracks": [{
            "isrc": "GBAYE7700037",
            "title": "Heroes",
            "artist": "David Bowie",
            "album": "Heroes",
            "track_number": 3,
            "disc_number": 1,
            "duration_ms": 368000,
            "explicit": 0,
            "favorite_at": "2026-08-15T12:10:00Z",
            "service": "tidal",
            "service_track_id": "12345",
            "downloaded_format": "flac"
        }],
        "albums": [{
            "title": "Heroes",
            "artist": "David Bowie",
            "upc": null,
            "release_date": "1977-10-14",
            "favorite_at": "2026-08-15T12:05:00Z"
        }],
        "artists": [{"name": "David Bowie", "favorite_at": "2026-08-15T12:00:00Z"}],
        "playlists": []
    });
    std::fs::write(&backup.path, serde_json::to_string_pretty(&v1).unwrap()).unwrap();

    let app_state = app.state::<AppState>();
    let res =
        import_library_with_bases(app_state, backup.path.clone(), None, &backup.allowed_bases)
            .await
            .expect("a v1 manifest must keep importing");

    assert_eq!(res.tracks_imported, 1);
    let track_exists: i64 = {
        let state = app.state::<AppState>();
        sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE isrc = 'GBAYE7700037'")
            .fetch_one(&state.db)
            .await
            .unwrap()
    };
    assert_eq!(track_exists, 1, "the v1 track must land in the library");
}

#[tokio::test]
async fn v2_export_is_idempotent_across_runs() {
    let db = create_test_db().await;
    sqlx::query("UPDATE sync_settings SET auto_sync_enabled = 1 WHERE id = 1")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO accounts (id, service_id, display_name, external_user_id, is_active)
         VALUES (9, 1, 'Spotify Primary', 'sp-user-1', 1)",
    )
    .execute(&db)
    .await
    .unwrap();

    let app = create_test_app(db);
    let backup = get_test_backup_path("manifest_v2_idempotent");
    let first = export_to(&app, &backup).await;
    let second = export_to(&app, &backup).await;

    // Los campos volátiles cambian; el contenido semántico no.
    for field in [
        "version",
        "schema_version",
        "tracks",
        "albums",
        "artists",
        "playlists",
        "settings",
        "accounts",
    ] {
        assert_eq!(
            first[field], second[field],
            "field '{}' must be stable",
            field
        );
    }
}

/// Semilla compartida de los tests de import v2: biblioteca mínima, ajustes
/// no default en cada patrón de almacenamiento y una cuenta con credenciales
/// que jamás deben viajar ni sobrevivir a la restauración.
async fn seed_source_for_v2_import(db: &SqlitePool) {
    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, favorite_at) VALUES ('Remain in Light', '2026-10-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(db)
    .await
    .unwrap();
    let artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name, favorite_at) VALUES ('Talking Heads', '2026-10-01T00:00:00Z') RETURNING id",
    )
    .fetch_one(db)
    .await
    .unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, isrc, favorite_at) VALUES ('Once in a Lifetime', ?, 'GBAAA8001234', '2026-10-03T00:00:00Z') RETURNING id",
    )
    .bind(album_id)
    .fetch_one(db)
    .await
    .unwrap();
    let _ = sqlx::query(
        "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
    )
    .bind(track_id)
    .bind(artist_id)
    .execute(db)
    .await
    .unwrap();

    sqlx::query(
        "UPDATE sync_settings SET auto_sync_enabled = 1, max_concurrent_downloads = 5 WHERE id = 1",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE quality_preferences SET max_quality = 'hires' WHERE service_name = 'spotify'",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query("UPDATE metadata_preferences SET multi_value_separator = ';' WHERE id = 1")
        .execute(db)
        .await
        .unwrap();
    sqlx::query("UPDATE folder_settings SET folder_template = '{artist}/{album}' WHERE id = 1")
        .execute(db)
        .await
        .unwrap();
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('dl_concurrent_downloads', '7', datetime('now'))")
        .execute(db)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO accounts (id, service_id, display_name, email, external_user_id, is_active, credentials_json)
         VALUES (7, 3, 'Tidal Main', 'main@example.com', 'ext-usr-4242', 1, '{\"access_token\":\"tok-should-not-survive\"}')",
    )
    .execute(db)
    .await
    .unwrap();
}

#[tokio::test]
async fn v2_import_restores_settings_and_accounts_await_reauth() {
    let source_db = create_test_db().await;
    seed_source_for_v2_import(&source_db).await;

    let source_app = create_test_app(source_db);
    let backup = get_test_backup_path("manifest_v2_import");
    let manifest = export_to(&source_app, &backup).await;
    assert_eq!(manifest["version"], "2.0.0");

    let target_db = create_test_db().await;
    let target_app = create_test_app(target_db.clone());
    // export_to ya volcó el manifest en disco: import_library_with_bases
    // consume la ruta del fichero.
    let res = {
        let state = target_app.state::<AppState>();
        import_library_with_bases(state, backup.path.clone(), None, &backup.allowed_bases)
            .await
            .expect("a v2 manifest must import into a clean database")
    };

    assert_eq!(res.tracks_imported, 1);
    assert!(
        res.settings_rows_restored > 0,
        "v2 import must restore settings"
    );
    assert_eq!(res.accounts_restored, 1);
    assert_eq!(
        res.accounts_awaiting_reauth,
        vec!["tidal".to_string()],
        "a fresh account without credentials must await re-authentication"
    );

    // Ajustes restaurados por superficie, en cada patrón de almacenamiento.
    let (auto_sync, max_conc): (i64, i64) = sqlx::query_as(
        "SELECT auto_sync_enabled, max_concurrent_downloads FROM sync_settings WHERE id = 1",
    )
    .fetch_one(&target_db)
    .await
    .unwrap();
    assert_eq!((auto_sync, max_conc), (1, 5));
    let max_quality: String = sqlx::query_scalar(
        "SELECT max_quality FROM quality_preferences WHERE service_name = 'spotify'",
    )
    .fetch_one(&target_db)
    .await
    .unwrap();
    assert_eq!(max_quality, "hires");
    let separator: String =
        sqlx::query_scalar("SELECT multi_value_separator FROM metadata_preferences WHERE id = 1")
            .fetch_one(&target_db)
            .await
            .unwrap();
    assert_eq!(separator, ";");
    let template: String =
        sqlx::query_scalar("SELECT folder_template FROM folder_settings WHERE id = 1")
            .fetch_one(&target_db)
            .await
            .unwrap();
    assert_eq!(template, "{artist}/{album}");
    let kv: String =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'dl_concurrent_downloads'")
            .fetch_one(&target_db)
            .await
            .unwrap();
    assert_eq!(kv, "7");

    // Cuenta: metadatos aplicados, sesión jamás fabricada.
    let (credentials, display, email, is_active): (
        Option<String>,
        Option<String>,
        Option<String>,
        i64,
    ) = sqlx::query_as(
        "SELECT credentials_json, display_name, email, is_active FROM accounts WHERE external_user_id = 'ext-usr-4242'",
    )
    .fetch_one(&target_db)
    .await
    .unwrap();
    assert_eq!(
        credentials, None,
        "restoration must never fabricate credentials"
    );
    assert_eq!(display, Some("Tidal Main".to_string()));
    assert_eq!(email, Some("main@example.com".to_string()));
    assert_eq!(is_active, 1);

    // Idempotencia: reimportar actualiza en el sitio y no duplica nada.
    let res2 = {
        let state = target_app.state::<AppState>();
        import_library_with_bases(state, backup.path.clone(), None, &backup.allowed_bases)
            .await
            .expect("reimporting a v2 manifest must be idempotent")
    };
    assert_eq!(
        res2.accounts_awaiting_reauth,
        Vec::<String>::new(),
        "second import finds the account and updates it in place"
    );
    let account_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE external_user_id = 'ext-usr-4242'")
            .fetch_one(&target_db)
            .await
            .unwrap();
    assert_eq!(account_count, 1);
    let track_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE isrc = 'GBAAA8001234'")
            .fetch_one(&target_db)
            .await
            .unwrap();
    assert_eq!(track_count, 1);
    let max_conc_after: i64 =
        sqlx::query_scalar("SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1")
            .fetch_one(&target_db)
            .await
            .unwrap();
    assert_eq!(
        max_conc_after, 5,
        "settings must be stable across reimports"
    );
}

#[tokio::test]
async fn future_manifest_fails_safely_without_partial_writes() {
    let source_db = create_test_db().await;
    seed_source_for_v2_import(&source_db).await;

    let source_app = create_test_app(source_db);
    let backup = get_test_backup_path("manifest_v2_future");
    let mut manifest = export_to(&source_app, &backup).await;
    manifest["checksum_sha256"] = serde_json::Value::Null;

    // Destino con un valor centinela: si el import falla seguro, nada cambia.
    let target_db = create_test_db().await;
    sqlx::query("UPDATE sync_settings SET max_concurrent_downloads = 9 WHERE id = 1")
        .execute(&target_db)
        .await
        .unwrap();
    let target_app = create_test_app(target_db.clone());

    // Caso 1: major futuro (3.0.0).
    manifest["version"] = "3.0.0".into();
    let future_path = backup.path.replace(".json", "_v3.json");
    std::fs::write(
        &future_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let err = {
        let state = target_app.state::<AppState>();
        import_library_with_bases(state, future_path.clone(), None, &backup.allowed_bases)
            .await
            .expect_err("a future-major manifest must be rejected")
    };
    assert!(
        err.contains("versión futura") && err.contains("3.0.0"),
        "diagnostic must name the future version: {err}"
    );

    // Caso 2: manifest de una versión soportada pero derivado de un esquema
    // más nuevo que la BD destino.
    let applied: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
        .fetch_one(&target_db)
        .await
        .unwrap();
    manifest["version"] = "2.0.0".into();
    manifest["schema_version"] = (applied + 1).into();
    let future_schema_path = backup.path.replace(".json", "_future_schema.json");
    std::fs::write(
        &future_schema_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let err = {
        let state = target_app.state::<AppState>();
        import_library_with_bases(
            state,
            future_schema_path.clone(),
            None,
            &backup.allowed_bases,
        )
        .await
        .expect_err("a manifest from a newer schema must be rejected")
    };
    assert!(
        err.contains("esquema más nuevo"),
        "diagnostic must name the schema gap: {err}"
    );

    // Sin escrituras a medias: ni biblioteca, ni ajustes, ni cuentas.
    let track_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&target_db)
        .await
        .unwrap();
    assert_eq!(track_count, 0);
    let account_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&target_db)
        .await
        .unwrap();
    assert_eq!(account_count, 0);
    let sentinel: i64 =
        sqlx::query_scalar("SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1")
            .fetch_one(&target_db)
            .await
            .unwrap();
    assert_eq!(
        sentinel, 9,
        "settings must remain untouched after a rejected manifest"
    );
}

/// Contrato sin secretos, lado restauración (ADR-0020): ni los secretos de la
/// BD de origen ni los que un manifest hostil intenta colar por la KV, por
/// una superficie tipada o por `accounts` sobreviven a la restauración.
#[tokio::test]
async fn restored_database_never_contains_secrets() {
    let source_db = create_test_db().await;
    seed_source_for_v2_import(&source_db).await;
    // Secretos plantados en la KV de origen, como en el test de export.
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('session_token', 'tok-source-1234', datetime('now'))")
        .execute(&source_db).await.unwrap();
    sqlx::query("INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('lastfm_api_key', 'lastfm-key-7777', datetime('now'))")
        .execute(&source_db).await.unwrap();

    let source_app = create_test_app(source_db);
    let backup = get_test_backup_path("manifest_v2_secrets_import");
    let mut manifest = export_to(&source_app, &backup).await;
    manifest["checksum_sha256"] = serde_json::Value::Null;

    // Inyección hostil por las tres puertas.
    let kv = manifest["settings"]["kv"].as_object_mut().unwrap();
    kv.insert(
        "session_token".to_string(),
        serde_json::Value::String("tok-evil-1".to_string()),
    );
    kv.insert(
        "spotify_client_secret".to_string(),
        serde_json::Value::String("sec-evil-2".to_string()),
    );
    manifest["settings"]["sync_settings"][0]["credentials_json"] = "evil-3".into();
    manifest["accounts"][0]["credentials_json"] = "evil-4".into();
    manifest["accounts"][0]["access_token"] = "evil-5".into();

    let hostile_path = backup.path.replace(".json", "_hostile.json");
    std::fs::write(
        &hostile_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let target_db = create_test_db().await;
    let target_app = create_test_app(target_db.clone());
    let res = {
        let state = target_app.state::<AppState>();
        import_library_with_bases(state, hostile_path.clone(), None, &backup.allowed_bases)
            .await
            .expect("the hostile manifest still imports: its secrets are simply dropped")
    };

    // Ningún secreto en la KV restaurada, por clave ni por valor.
    let leaked_kv: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM settings WHERE key LIKE '%token%' OR key LIKE '%secret%' \
         OR key LIKE '%api_key%' OR value LIKE '%evil%' OR value LIKE '%tok-source-1234%' \
         OR value LIKE '%lastfm-key-7777%'",
    )
    .fetch_one(&target_db)
    .await
    .unwrap();
    assert_eq!(
        leaked_kv, 0,
        "no token, api key or secret may survive restoration"
    );

    // Ninguna credencial en las cuentas restauradas.
    let accounts_with_credentials: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE credentials_json IS NOT NULL")
            .fetch_one(&target_db)
            .await
            .unwrap();
    assert_eq!(
        accounts_with_credentials, 0,
        "credentials_json must never be fabricated by restoration"
    );

    // Y la restauración no fue un no-op: lo legítimo sí aterrizó.
    assert!(res.settings_rows_restored > 0);
    assert_eq!(res.accounts_restored, 1);
    let max_conc: i64 =
        sqlx::query_scalar("SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1")
            .fetch_one(&target_db)
            .await
            .unwrap();
    assert_eq!(max_conc, 5);
    let display: Option<String> = sqlx::query_scalar(
        "SELECT display_name FROM accounts WHERE external_user_id = 'ext-usr-4242'",
    )
    .fetch_one(&target_db)
    .await
    .unwrap();
    assert_eq!(display, Some("Tidal Main".to_string()));
}

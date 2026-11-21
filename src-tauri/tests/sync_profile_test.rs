//! Issue #69 (T05, spec #64) — perfil de sincronización resuelto.
//!
//! **Hito 1 (caracterización, antes de tocar código).** Fija de dónde salen hoy
//! los defaults que el perfil hereda —`service_sync_settings` recién migrada,
//! leída por el camino existente de preferencias de importación— y deja
//! constancia de que una cuenta nueva no tiene todavía una capa de override
//! propia. El hito 2 extiende este mismo fichero con la resolución pura y la
//! persistencia del override.
//!
//! BD `:memory:` y fixtures desechables: nunca la BD personal.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::settings::{
    perform_get_service_import_preferences, perform_update_service_sync_settings,
};
use syncify_tauri_lib::sync::profile::{
    self, AccountSyncOverride, ProfileSource, ServiceSyncDefaults, SyncCollections, SyncMode,
    SyncProfileUpdate, BIDIRECTIONAL_UNAVAILABLE_REASON, DEFAULT_INTERVAL_HOURS,
    MIRROR_ENGINE_UNAVAILABLE_REASON,
};

async fn setup_test_db() -> sqlx::SqlitePool {
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

/// Cuenta de prueba sobre el esquema migrado; devuelve (account_id, service_id).
async fn seed_account(pool: &sqlx::SqlitePool, service: &str) -> (i64, i64) {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service)
        .fetch_one(pool)
        .await
        .expect("servicio sembrado por migraciones");

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'Perfil', 1) RETURNING id",
    )
    .bind(service_id)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada");

    (account_id, service_id)
}

// ============================================================
// Hito 1 — caracterización del default por servicio
// ============================================================

/// El default por servicio que el perfil va a heredar es el que ya sirve el
/// camino de preferencias de importación: favoritos, playlists, álbumes y
/// artistas activos; compras, historial y apariciones apagados.
#[tokio::test]
async fn characterization_service_defaults_match_the_import_preferences() {
    let pool = setup_test_db().await;

    let prefs = perform_get_service_import_preferences(&pool, "qobuz")
        .await
        .expect("preferencias del servicio");

    assert!(prefs.favorite_tracks, "favoritos activos por defecto");
    assert!(prefs.playlists, "playlists activas por defecto");
    assert!(prefs.favorite_albums, "álbumes activos por defecto");
    assert!(prefs.favorite_artists, "artistas activos por defecto");
    assert!(!prefs.purchases);
    assert!(!prefs.library_history);
    assert!(!prefs.include_appearances);
}

/// Un servicio sin fila sembrada (Apple Music) hereda el `Default` del
/// contrato de importación: las cuatro colecciones del perfil, activas.
#[tokio::test]
async fn characterization_service_without_row_falls_back_to_import_defaults() {
    let pool = setup_test_db().await;

    let prefs = perform_get_service_import_preferences(&pool, "apple_music")
        .await
        .expect("fallback del servicio sin fila");

    assert!(prefs.favorite_tracks);
    assert!(prefs.playlists);
    assert!(prefs.favorite_albums);
    assert!(prefs.favorite_artists);
}

/// Cambiar las colecciones de un servicio no toca las demás banderas: la capa
/// por servicio es la única fuente de los defaults de colecciones.
#[tokio::test]
async fn characterization_updating_collections_keeps_the_other_flags() {
    let pool = setup_test_db().await;

    perform_update_service_sync_settings(&pool, "qobuz", true, false, false)
        .await
        .expect("actualización de colecciones");

    let prefs = perform_get_service_import_preferences(&pool, "qobuz")
        .await
        .expect("preferencias tras actualizar");

    assert!(prefs.favorite_tracks);
    assert!(!prefs.playlists);
    assert!(!prefs.favorite_albums);
    // No tocadas por la actualización de colecciones.
    assert!(prefs.favorite_artists);
    assert!(!prefs.purchases);
}

/// Una cuenta nueva no tiene capa de override: hoy no existe ninguna fila
/// account-scoped que el perfil pueda leer, así que todo se hereda del
/// servicio. El test se mantiene después del hito 2: la ausencia de fila es el
/// estado de herencia limpia.
#[tokio::test]
async fn characterization_a_fresh_account_has_no_account_scoped_profile_row() {
    let pool = setup_test_db().await;
    let (account_id, _) = seed_account(&pool, "qobuz").await;

    let key_prefix = format!("sync_profile:account:{}", account_id);
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = ?")
        .bind(&key_prefix)
        .fetch_one(&pool)
        .await
        .expect("consulta de la KV de ajustes");

    assert_eq!(rows, 0, "una cuenta nueva no tiene override persistido");
}

// ============================================================
// Hito 2 — resolución pura y persistencia del override
// ============================================================

fn service_defaults(
    mode: SyncMode,
    interval_hours: i64,
    collections: SyncCollections,
) -> ServiceSyncDefaults {
    ServiceSyncDefaults {
        service_name: "qobuz".to_string(),
        mode,
        interval_hours,
        collections,
    }
}

/// Sin override: modo solo importar, 24 h y colecciones del servicio.
#[test]
fn resolve_without_override_inherits_the_service_defaults() {
    let service = service_defaults(
        SyncMode::ImportOnly,
        DEFAULT_INTERVAL_HOURS,
        SyncCollections {
            favorites: true,
            playlists: false,
            albums: true,
            artists: false,
        },
    );

    let profile = profile::resolve(7, &service, None);

    assert_eq!(profile.account_id, 7);
    assert!(profile.enabled, "una cuenta participa salvo que se apague");
    assert_eq!(profile.mode, SyncMode::ImportOnly);
    assert_eq!(profile.mode_source, ProfileSource::Service);
    assert_eq!(profile.interval_hours, 24);
    assert_eq!(profile.interval_source, ProfileSource::Service);
    assert_eq!(
        profile.collections,
        SyncCollections {
            favorites: true,
            playlists: false,
            albums: true,
            artists: false,
        }
    );
    assert_eq!(profile.collections_source, ProfileSource::Service);
}

/// Criterio 1: el override de cuenta gana al default del servicio en modo,
/// colecciones e intervalo, y la ficha puede decir de dónde viene cada valor.
#[test]
fn resolve_account_override_wins_on_mode_collections_and_interval() {
    let service = service_defaults(
        SyncMode::ImportOnly,
        DEFAULT_INTERVAL_HOURS,
        SyncCollections::all(),
    );
    let account = AccountSyncOverride {
        enabled: true,
        mode: Some(SyncMode::MirrorQuarantine),
        interval_hours: Some(6),
        collections: Some(SyncCollections {
            favorites: true,
            playlists: true,
            albums: false,
            artists: false,
        }),
    };

    let profile = profile::resolve(7, &service, Some(&account));

    assert_eq!(profile.mode, SyncMode::MirrorQuarantine);
    assert_eq!(profile.mode_source, ProfileSource::Account);
    assert_eq!(profile.interval_hours, 6);
    assert_eq!(profile.interval_source, ProfileSource::Account);
    assert!(!profile.collections.albums);
    assert!(!profile.collections.artists);
    assert_eq!(profile.collections_source, ProfileSource::Account);
}

/// Criterio 3: el bidireccional existe, está deshabilitado y lleva su motivo.
/// Los modos espejo tampoco son seleccionables hasta que exista su motor: la
/// UI no debe ofrecer un modo que nadie ejecuta.
#[test]
fn resolve_exposes_only_import_only_as_selectable() {
    let service = service_defaults(
        SyncMode::ImportOnly,
        DEFAULT_INTERVAL_HOURS,
        SyncCollections::all(),
    );

    let profile = profile::resolve(7, &service, None);

    let option = |mode: SyncMode| {
        profile
            .modes
            .iter()
            .find(|option| option.value == mode)
            .unwrap_or_else(|| panic!("{:?} must be modelled", mode))
    };

    assert!(option(SyncMode::ImportOnly).enabled);
    assert_eq!(option(SyncMode::ImportOnly).reason, None);
    for mode in [SyncMode::MirrorQuarantine, SyncMode::StrictMirror] {
        assert!(!option(mode).enabled, "{mode:?} has no engine yet");
        assert_eq!(
            option(mode).reason,
            Some(MIRROR_ENGINE_UNAVAILABLE_REASON),
            "{mode:?} must name why it is unavailable"
        );
    }
    assert!(!option(SyncMode::Bidirectional).enabled);
    assert_eq!(
        option(SyncMode::Bidirectional).reason,
        Some(BIDIRECTIONAL_UNAVAILABLE_REASON)
    );
}

/// Persistencia: lo guardado por la ficha se lee de vuelta y gana al servicio.
#[tokio::test]
async fn persisted_override_roundtrips_and_wins_over_service_defaults() {
    let pool = setup_test_db().await;
    let (account_id, _) = seed_account(&pool, "qobuz").await;
    perform_update_service_sync_settings(&pool, "qobuz", true, true, false)
        .await
        .expect("defaults del servicio");

    let update = SyncProfileUpdate {
        enabled: true,
        mode: None,
        interval_hours: Some(12),
        collections: Some(SyncCollections {
            favorites: true,
            playlists: true,
            albums: true,
            artists: false,
        }),
    };
    let saved = profile::save_account_profile(&pool, account_id, &update)
        .await
        .expect("guardar el perfil");

    // El modo sigue siendo el único ejecutable; el override es de intervalo y
    // colecciones.
    assert_eq!(saved.mode, SyncMode::ImportOnly);
    assert_eq!(saved.mode_source, ProfileSource::Service);
    assert_eq!(saved.interval_hours, 12);
    assert_eq!(saved.interval_source, ProfileSource::Account);
    assert!(saved.collections.albums);
    assert!(!saved.collections.artists);
    assert_eq!(saved.collections_source, ProfileSource::Account);

    let loaded = profile::load_sync_profile(&pool, account_id)
        .await
        .expect("leer el perfil");
    assert_eq!(loaded, saved);
}

/// Guardar exactamente los defaults del servicio no crea override: la herencia
/// limpia es la ausencia de fila y la fuente sigue siendo el servicio.
#[tokio::test]
async fn saving_service_default_values_keeps_inheritance_clean() {
    let pool = setup_test_db().await;
    let (account_id, _) = seed_account(&pool, "qobuz").await;

    let update = SyncProfileUpdate {
        enabled: true,
        mode: Some("import_only".to_string()),
        interval_hours: Some(24),
        collections: Some(SyncCollections::all()),
    };
    let saved = profile::save_account_profile(&pool, account_id, &update)
        .await
        .expect("guardar valores heredados");

    assert_eq!(saved.mode_source, ProfileSource::Service);
    assert_eq!(saved.interval_source, ProfileSource::Service);
    assert_eq!(saved.collections_source, ProfileSource::Service);

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = ?")
        .bind(profile::account_profile_key(account_id))
        .fetch_one(&pool)
        .await
        .expect("consulta de la KV");
    assert_eq!(rows, 0, "sin override no hay fila persistida");
}

/// Criterio 2: apagar el toggle excluye la cuenta del universo de
/// «sincronizar todo» (el mismo que usa el auto-sync) sin borrar su perfil.
#[tokio::test]
async fn disabled_toggle_excludes_the_account_from_bulk_services() {
    let pool = setup_test_db().await;
    let (qobuz_id, _) = seed_account(&pool, "qobuz").await;
    let (_tidal_id, _) = seed_account(&pool, "tidal").await;

    let update = SyncProfileUpdate {
        enabled: false,
        mode: None,
        interval_hours: None,
        collections: None,
    };
    let saved = profile::save_account_profile(&pool, qobuz_id, &update)
        .await
        .expect("pausar la cuenta");

    assert!(!saved.enabled);
    assert!(!profile::is_account_participating(&pool, qobuz_id)
        .await
        .expect("participación"));

    let services = profile::active_participating_services(&pool)
        .await
        .expect("servicios del sync masivo");
    assert_eq!(services, vec!["tidal".to_string()]);

    // «Sync ahora» resuelve el perfil aunque esté pausado: el toggle no lo
    // bloquea (ADR-0006, decisión 4).
    let loaded = profile::load_sync_profile(&pool, qobuz_id)
        .await
        .expect("perfil de la cuenta pausada");
    assert!(!loaded.enabled);
}

/// Validación: ningún modo sin motor se puede seleccionar, y cada uno nombra
/// su motivo en el error.
#[tokio::test]
async fn update_rejects_modes_without_an_engine() {
    let pool = setup_test_db().await;
    let (account_id, _) = seed_account(&pool, "qobuz").await;

    for (raw, reason) in [
        ("bidirectional", BIDIRECTIONAL_UNAVAILABLE_REASON),
        ("mirror_quarantine", MIRROR_ENGINE_UNAVAILABLE_REASON),
        ("strict_mirror", MIRROR_ENGINE_UNAVAILABLE_REASON),
    ] {
        let update = SyncProfileUpdate {
            enabled: true,
            mode: Some(raw.to_string()),
            interval_hours: None,
            collections: None,
        };
        let error = profile::save_account_profile(&pool, account_id, &update)
            .await
            .expect_err("un modo sin motor no es seleccionable");
        assert!(error.contains(reason), "mode '{}': {error}", raw);
    }
}

/// Validación: modo desconocido e intervalo fuera de rango.
#[tokio::test]
async fn update_rejects_unknown_mode_and_out_of_range_interval() {
    let pool = setup_test_db().await;
    let (account_id, _) = seed_account(&pool, "qobuz").await;

    let unknown_mode = SyncProfileUpdate {
        enabled: true,
        mode: Some("mirror_forever".to_string()),
        interval_hours: None,
        collections: None,
    };
    let error = profile::save_account_profile(&pool, account_id, &unknown_mode)
        .await
        .expect_err("modo desconocido");
    assert!(error.contains("unknown sync mode"), "{error}");

    let bad_interval = SyncProfileUpdate {
        enabled: true,
        mode: None,
        interval_hours: Some(0),
        collections: None,
    };
    let error = profile::save_account_profile(&pool, account_id, &bad_interval)
        .await
        .expect_err("intervalo fuera de rango");
    assert!(error.contains("interval_hours 0"), "{error}");
}

/// Un override corrupto se reporta; no se sustituye en silencio por defaults.
#[tokio::test]
async fn corrupt_override_is_reported_not_silently_defaulted() {
    let pool = setup_test_db().await;
    let (account_id, _) = seed_account(&pool, "qobuz").await;

    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?)")
        .bind(profile::account_profile_key(account_id))
        .bind("{not json")
        .execute(&pool)
        .await
        .expect("sembrar el override corrupto");

    let error = profile::load_sync_profile(&pool, account_id)
        .await
        .expect_err("el JSON corrupto se propaga");
    assert!(error.contains("SyncProfileInvalid"), "{error}");
}

/// El borrado de la cuenta limpia el override (la KV no tiene FK).
#[tokio::test]
async fn deleting_the_profile_clears_the_override() {
    let pool = setup_test_db().await;
    let (account_id, _) = seed_account(&pool, "qobuz").await;

    let update = SyncProfileUpdate {
        enabled: false,
        mode: None,
        interval_hours: Some(48),
        collections: None,
    };
    profile::save_account_profile(&pool, account_id, &update)
        .await
        .expect("guardar el override");

    profile::delete_account_profile(&pool, account_id)
        .await
        .expect("borrar el override");

    let loaded = profile::load_sync_profile(&pool, account_id)
        .await
        .expect("perfil tras el borrado");
    assert!(loaded.enabled);
    assert_eq!(loaded.mode, SyncMode::ImportOnly);
    assert_eq!(loaded.interval_hours, DEFAULT_INTERVAL_HOURS);
    assert_eq!(loaded.mode_source, ProfileSource::Service);
}

/// Las DOS vías de borrado de cuenta limpian el override, no solo la del
/// borrado completo: `remove_account` (Settings → Services) borra la fila de
/// `accounts` sin pasar por `perform_delete_account`, y la clave de la KV no
/// tiene FK que la arrastre.
#[tokio::test]
async fn both_account_deletion_routes_clear_the_override() {
    use syncify_tauri_lib::commands::accounts::{perform_delete_account, perform_remove_account};

    let pool = setup_test_db().await;

    async fn override_rows(pool: &sqlx::SqlitePool, account_id: i64) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM settings WHERE key = ?")
            .bind(profile::account_profile_key(account_id))
            .fetch_one(pool)
            .await
            .expect("consulta de la KV")
    }

    let update = SyncProfileUpdate {
        enabled: false,
        mode: None,
        interval_hours: Some(6),
        collections: None,
    };

    // Vía 1: borrado completo de la cuenta (issue #85).
    let (full_delete_id, _) = seed_account(&pool, "qobuz").await;
    profile::save_account_profile(&pool, full_delete_id, &update)
        .await
        .expect("override de la vía de borrado completo");
    perform_delete_account(&pool, full_delete_id, false)
        .await
        .expect("borrado completo");
    assert_eq!(
        override_rows(&pool, full_delete_id).await,
        0,
        "el borrado completo no debe dejar la KV huérfana"
    );

    // Vía 2: `remove_account` de Settings → Services.
    let (remove_id, _) = seed_account(&pool, "tidal").await;
    profile::save_account_profile(&pool, remove_id, &update)
        .await
        .expect("override de la vía de Settings");
    perform_remove_account(&pool, remove_id)
        .await
        .expect("remove_account");
    assert_eq!(
        override_rows(&pool, remove_id).await,
        0,
        "remove_account no debe dejar la KV huérfana"
    );

    let remaining_accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .expect("consulta de cuentas");
    assert_eq!(remaining_accounts, 0, "ambas vías borran la fila de cuenta");
}

/// Una cuenta inexistente no inventa un perfil.
#[tokio::test]
async fn unknown_account_is_reported() {
    let pool = setup_test_db().await;

    let error = profile::load_sync_profile(&pool, 9_999)
        .await
        .expect_err("cuenta inexistente");
    assert!(error.contains("SyncProfileNotFound"), "{error}");
}

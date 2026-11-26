//! Issue #134 — identidad y promoción sin red.
//!
//! Las pruebas usan el núcleo real de cola + motor unificado + crédito sobre una
//! BD migrada. El proveedor se sustituye por una identidad ya resuelta; no hay
//! peticiones de red ni temporizadores.

use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use syncify_core_domain::metadata::ProviderTrackIdentity;
use syncify_tauri_lib::commands::playback::perform_apply_listen_credit;
use syncify_tauri_lib::commands::playback_queue::{
    perform_enqueue_provider_item, perform_get_playback_queue, perform_get_queue_entry,
    perform_set_current_entry, PlaybackQueueEntryDto,
};
use syncify_tauri_lib::services::playback_advance::PlayerAdvancedPayload;
use syncify_tauri_lib::services::playback_promotion::{
    freeze_queue_listen_session, promote_queue_entry, reconcile_queue_promotions, PromotionOutcome,
    ResolvedProviderIdentity,
};
use syncify_tauri_lib::services::playback_session::ListenCredit;
use syncify_tauri_lib::services::streaming::{ProviderStreamItem, StreamTarget};

const SESSION_A: &str = "0b8df3ea-1c2d-4e5f-8a9b-0c1d2e3f4a5b";
const SESSION_B: &str = "1b8df3ea-1c2d-4e5f-8a9b-0c1d2e3f4a5c";
const SESSION_C: &str = "2b8df3ea-1c2d-4e5f-8a9b-0c1d2e3f4a5d";

async fn pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("FK activas");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migraciones");
    pool
}

async fn concurrent_pool() -> sqlx::SqlitePool {
    let name = uuid::Uuid::new_v4().simple();
    let options =
        SqliteConnectOptions::from_str(&format!("sqlite:file:{name}?mode=memory&cache=shared"))
            .expect("opciones SQLite compartida")
            .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .min_connections(1)
        .connect_with(options)
        .await
        .expect("BD concurrente en memoria");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migraciones");
    pool
}

async fn service_id(db: &sqlx::SqlitePool, name: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(name)
        .fetch_one(db)
        .await
        .expect("servicio sembrado")
}

async fn account_id(db: &sqlx::SqlitePool, service: &str) -> i64 {
    let service_id = service_id(db, service).await;
    sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) \
         VALUES (?, ?, 1) RETURNING id",
    )
    .bind(service_id)
    .bind(format!("Cuenta {service}"))
    .fetch_one(db)
    .await
    .expect("cuenta sintética")
}

fn identity(service_id: i64, service: &str, item: &str, isrc: &str) -> ResolvedProviderIdentity {
    ResolvedProviderIdentity {
        identity: ProviderTrackIdentity {
            service_id,
            service_name: service.into(),
            service_track_id: item.into(),
            isrc: Some(isrc.into()),
            provider_album_id: Some("album-fixture".into()),
            provider_artist_id: Some("artist-fixture".into()),
            title: Some("Canción de prueba".into()),
            artist: Some("Artista de prueba".into()),
            album: Some("Álbum de prueba".into()),
            duration_ms: Some(180_000),
            track_number: Some(1),
            disc_number: Some(1),
            explicit: Some(false),
        },
        account_id: 1,
    }
}

#[tokio::test]
async fn encolar_provider_item_no_crea_track_ni_source() {
    let db = pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-1", false)
        .await
        .expect("encolar efímera");

    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&db)
        .await
        .expect("conteo de tracks");
    let sources: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources")
        .fetch_one(&db)
        .await
        .expect("conteo de fuentes");
    assert_eq!(tracks, 0, "encolar no materializa biblioteca");
    assert_eq!(sources, 0, "encolar no crea TrackSource");

    let snapshot = perform_get_playback_queue(&db).await.expect("snapshot");
    let entry = &snapshot.pending[0];
    assert_eq!(entry.entry_id, entry_id);
    assert_eq!(entry.track_id, 0);
    assert_eq!(entry.service_id, Some(qobuz));
    assert_eq!(entry.service_track_id.as_deref(), Some("qobuz-1"));
    assert!(entry
        .promotion_error
        .as_deref()
        .is_some_and(|reason| reason.starts_with("reconnect:")));
}

#[tokio::test]
async fn promocion_crea_track_source_y_credita_una_sola_vez() {
    let db = pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let account = account_id(&db, "qobuz").await;
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-2", false)
        .await
        .expect("encolar efímera");
    let mut resolved = identity(qobuz, "qobuz", "qobuz-2", "GBAYE0601477");
    resolved.account_id = account;

    let first = promote_queue_entry(&db, &entry_id, SESSION_A, Ok(resolved.clone()))
        .await
        .expect("primera promoción");
    assert_eq!(
        first,
        PromotionOutcome::Promoted {
            track_id: 1,
            counted: true,
        }
    );
    let replay = promote_queue_entry(&db, &entry_id, SESSION_B, Ok(resolved))
        .await
        .expect("reintento idempotente");
    assert_eq!(
        replay,
        PromotionOutcome::AlreadyLibrary {
            track_id: 1,
            counted: false,
        }
    );

    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&db)
        .await
        .expect("tracks");
    let sources: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM track_sources \
         WHERE service_id = ? AND service_track_id = 'qobuz-2'",
    )
    .bind(qobuz)
    .fetch_one(&db)
    .await
    .expect("fuentes");
    let listens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_listens")
        .fetch_one(&db)
        .await
        .expect("escuchas");
    let play_count: i64 = sqlx::query_scalar("SELECT play_count FROM tracks WHERE id = 1")
        .fetch_one(&db)
        .await
        .expect("play_count");
    assert_eq!(tracks, 1, "reintentar no duplica Track");
    assert_eq!(sources, 1, "reintentar no duplica TrackSource");
    let provider_album_id: Option<String> = sqlx::query_scalar(
        "SELECT r.provider_release_id \
         FROM edition_provider_items i \
         JOIN edition_releases r ON r.id = i.release_id \
         WHERE i.service_id = ? AND i.service_track_id = 'qobuz-2'",
    )
    .bind(qobuz)
    .fetch_optional(&db)
    .await
    .expect("álbum del proveedor");
    assert_eq!(provider_album_id.as_deref(), Some("album-fixture"));
    assert_eq!(listens, 1, "el token congelado deduplica la escucha");
    assert_eq!(play_count, 1, "play_count sube una sola vez");

    let snapshot = perform_get_playback_queue(&db).await.expect("snapshot");
    let entry = &snapshot.pending[0];
    assert_eq!(entry.track_id, 1);
    assert_eq!(entry.service_id, None);
    assert_eq!(entry.service_track_id, None);
    assert_eq!(entry.promotion_error, None);
    let frozen_token: String =
        sqlx::query_scalar("SELECT listen_session_id FROM player_queue_entries WHERE entry_id = ?")
            .bind(&entry_id)
            .fetch_one(&db)
            .await
            .expect("token congelado");
    assert_eq!(frozen_token, SESSION_A);
}

#[tokio::test]
async fn una_reproduccion_posterior_de_la_promovida_cuenta_como_biblioteca() {
    let db = pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let account = account_id(&db, "qobuz").await;
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-repeat", false)
        .await
        .expect("encolar efímera");
    let mut resolved = identity(qobuz, "qobuz", "qobuz-repeat", "GBAYE0601481");
    resolved.account_id = account;
    promote_queue_entry(&db, &entry_id, SESSION_A, Ok(resolved))
        .await
        .expect("promocionar");

    let credit = ListenCredit {
        target: StreamTarget::Provider(ProviderStreamItem {
            entry_id,
            service_id: qobuz,
            service_track_id: "qobuz-repeat".into(),
        }),
        listen_session_id: SESSION_C.into(),
    };
    let outcome = perform_apply_listen_credit(&db, &credit)
        .await
        .expect("segunda escucha");
    assert_eq!(
        outcome,
        PromotionOutcome::AlreadyLibrary {
            track_id: 1,
            counted: true,
        }
    );
    let play_count: i64 = sqlx::query_scalar("SELECT play_count FROM tracks WHERE id = 1")
        .fetch_one(&db)
        .await
        .expect("play_count");
    assert_eq!(
        play_count, 2,
        "el token congelado sólo deduplica la promoción"
    );
}

#[tokio::test]
async fn identidad_no_resuelta_sigue_efimera_y_reintenta_con_el_primer_token() {
    let db = pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let account = account_id(&db, "qobuz").await;
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-3", false)
        .await
        .expect("encolar efímera");

    let failed = promote_queue_entry(
        &db,
        &entry_id,
        SESSION_A,
        Err("catálogo no disponible".into()),
    )
    .await
    .expect("fallo de identidad registrado");
    assert_eq!(
        failed,
        PromotionOutcome::StillEphemeral {
            reason: "catálogo no disponible".into(),
        }
    );
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&db)
        .await
        .expect("tracks");
    assert_eq!(tracks, 0, "un fallo de identidad no crea Track");

    let snapshot = perform_get_playback_queue(&db).await.expect("snapshot");
    assert_eq!(snapshot.pending[0].track_id, 0);
    assert_eq!(
        snapshot.pending[0].promotion_error.as_deref(),
        Some("catálogo no disponible")
    );

    let mut resolved = identity(qobuz, "qobuz", "qobuz-3", "GBAYE0601478");
    resolved.account_id = account;
    let retried = promote_queue_entry(&db, &entry_id, SESSION_B, Ok(resolved))
        .await
        .expect("reintento");
    assert_eq!(
        retried,
        PromotionOutcome::Promoted {
            track_id: 1,
            counted: true,
        }
    );
    let listens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_listens")
        .fetch_one(&db)
        .await
        .expect("escuchas");
    assert_eq!(listens, 1);
    let counted_track: i64 =
        sqlx::query_scalar("SELECT track_id FROM playback_listens WHERE listen_session_id = ?")
            .bind(SESSION_A)
            .fetch_one(&db)
            .await
            .expect("token usado");
    assert_eq!(counted_track, 1, "el reintento conserva la primera sesión");
}

#[tokio::test]
async fn isrc_compartido_enlaza_el_track_existente_y_anade_la_fuente() {
    let db = pool().await;
    let spotify = service_id(&db, "spotify").await;
    let qobuz = service_id(&db, "qobuz").await;
    let account = account_id(&db, "qobuz").await;
    sqlx::query("INSERT INTO artists (name) VALUES ('Artista de prueba')")
        .execute(&db)
        .await
        .expect("artista");
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) \
         VALUES ('Canción de prueba', 'GBAYE0601479', 180000) RETURNING id",
    )
    .fetch_one(&db)
    .await
    .expect("track canónico");
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')")
        .bind(track_id)
        .execute(&db)
        .await
        .expect("relación artista");
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) \
         VALUES (?, ?, 'spotify-1')",
    )
    .bind(track_id)
    .bind(spotify)
    .execute(&db)
    .await
    .expect("fuente Spotify");

    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-4", false)
        .await
        .expect("encolar efímera");
    let mut resolved = identity(qobuz, "qobuz", "qobuz-4", "GBAYE0601479");
    resolved.account_id = account;
    let outcome = promote_queue_entry(&db, &entry_id, SESSION_A, Ok(resolved))
        .await
        .expect("promoción cross-service");
    assert_eq!(
        outcome,
        PromotionOutcome::Promoted {
            track_id,
            counted: true,
        }
    );
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&db)
        .await
        .expect("tracks");
    let sources: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources WHERE track_id = ?")
        .bind(track_id)
        .fetch_one(&db)
        .await
        .expect("fuentes");
    assert_eq!(tracks, 1, "el ISRC no duplica la grabación");
    assert_eq!(sources, 2, "Qobuz queda enlazado al Track canónico");
}

#[tokio::test]
async fn huella_ambigua_deja_la_entrada_efimera_sin_crear_track() {
    let db = pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let account = account_id(&db, "qobuz").await;
    sqlx::query("INSERT INTO artists (name) VALUES ('Artista de prueba')")
        .execute(&db)
        .await
        .expect("artista");
    for title in ["Candidato A", "Candidato B"] {
        let track_id: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, duration_ms) VALUES (?, 180000) RETURNING id",
        )
        .bind(title)
        .fetch_one(&db)
        .await
        .expect("candidato");
        sqlx::query(
            "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')",
        )
        .bind(track_id)
        .execute(&db)
        .await
        .expect("relación artista");
    }
    // Mismo título que los candidatos: la huella encuentra dos tracks.
    sqlx::query("UPDATE tracks SET title = 'Canción de prueba'")
        .execute(&db)
        .await
        .expect("títulos coincidentes");
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-ambiguous", false)
        .await
        .expect("encolar efímera");
    let mut resolved = identity(qobuz, "qobuz", "qobuz-ambiguous", "GBAYE0601482");
    resolved.account_id = account;
    resolved.identity.isrc = None;
    let outcome = promote_queue_entry(&db, &entry_id, SESSION_A, Ok(resolved))
        .await
        .expect("ambigüedad registrada");
    assert!(matches!(
        outcome,
        PromotionOutcome::StillEphemeral { reason } if reason.contains("varios candidatos")
    ));
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&db)
        .await
        .expect("tracks");
    let sources: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources")
        .fetch_one(&db)
        .await
        .expect("fuentes");
    assert_eq!(tracks, 2, "la promoción ambigua no crea un tercer Track");
    assert_eq!(sources, 0);
}

#[tokio::test]
async fn reconciliador_recupera_un_credito_pendiente_sin_duplicarlo() {
    let db = pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-6", false)
        .await
        .expect("encolar efímera");
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title) VALUES ('Promoción recuperable') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .expect("track");
    // Estado dejado por una caída: la entrada ya convirtió, pero el crédito no.
    sqlx::query(
        "UPDATE player_queue_entries \
         SET track_id = ?, service_id = NULL, service_track_id = NULL, \
             listen_session_id = ? WHERE entry_id = ?",
    )
    .bind(track_id)
    .bind(SESSION_A)
    .bind(&entry_id)
    .execute(&db)
    .await
    .expect("simular conversión sin crédito");

    assert_eq!(
        reconcile_queue_promotions(&db).await.expect("reconciliar"),
        1
    );
    assert_eq!(reconcile_queue_promotions(&db).await.expect("repetir"), 0);
    let play_count: i64 = sqlx::query_scalar("SELECT play_count FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&db)
        .await
        .expect("play_count");
    assert_eq!(play_count, 1);
}

#[tokio::test]
async fn proveedor_sin_capacidad_de_reproduccion_se_rechaza_al_encolar() {
    let db = pool().await;
    let spotify = service_id(&db, "spotify").await;
    let error = perform_enqueue_provider_item(&db, spotify, "spotify-1", false)
        .await
        .expect_err("Spotify no reproduce entradas efímeras");
    assert!(error.contains("sin reproducción"));
    let snapshot = perform_get_playback_queue(&db).await.expect("snapshot");
    assert!(snapshot.pending.is_empty());
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&db)
        .await
        .expect("tracks");
    assert_eq!(tracks, 0);
}

#[tokio::test]
async fn identidad_de_otro_item_no_convierte_la_entrada() {
    let db = pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let account = account_id(&db, "qobuz").await;
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-item", false)
        .await
        .expect("encolar");
    let mut resolved = identity(qobuz, "qobuz", "otro-item", "GBAYE0601483");
    resolved.account_id = account;
    let outcome = promote_queue_entry(&db, &entry_id, SESSION_A, Ok(resolved))
        .await
        .expect("validación de identidad");
    assert!(matches!(
        outcome,
        PromotionOutcome::StillEphemeral { reason } if reason.contains("no corresponde")
    ));
    let entry = perform_get_queue_entry(&db, &entry_id)
        .await
        .expect("entrada");
    assert_eq!(entry.track_id, 0);
}

#[tokio::test]
async fn cuenta_deshabilitada_no_autoriza_la_promocion() {
    let db = pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let account = account_id(&db, "qobuz").await;
    sqlx::query("UPDATE accounts SET state = 'disabled' WHERE id = ?")
        .bind(account)
        .execute(&db)
        .await
        .expect("deshabilitar cuenta");
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-disabled", false)
        .await
        .expect("encolar");
    assert!(perform_get_queue_entry(&db, &entry_id)
        .await
        .expect("entrada")
        .promotion_error
        .as_deref()
        .is_some_and(|reason| reason.starts_with("reconnect:")));
    let mut resolved = identity(qobuz, "qobuz", "qobuz-disabled", "GBAYE0601484");
    resolved.account_id = account;
    let outcome = promote_queue_entry(&db, &entry_id, SESSION_A, Ok(resolved))
        .await
        .expect("validación de cuenta");
    assert!(matches!(
        outcome,
        PromotionOutcome::StillEphemeral { reason } if reason.contains("cuenta resuelta")
    ));
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&db)
        .await
        .expect("tracks");
    assert_eq!(tracks, 0);
}

#[tokio::test]
async fn edicion_en_revision_deja_la_entrada_efimera() {
    let db = pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let account = account_id(&db, "qobuz").await;
    sqlx::query("INSERT INTO artists (name) VALUES ('Artista de prueba')")
        .execute(&db)
        .await
        .expect("artista");
    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Álbum original') RETURNING id")
            .fetch_one(&db)
            .await
            .expect("álbum");
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms, album_id) \
         VALUES ('Canción de prueba', 'GBAYE0601485', 180000, ?) RETURNING id",
    )
    .bind(album_id)
    .fetch_one(&db)
    .await
    .expect("track existente");
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, 1, 'primary')")
        .bind(track_id)
        .execute(&db)
        .await
        .expect("relación artista");
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-edition", false)
        .await
        .expect("encolar");
    let mut resolved = identity(qobuz, "qobuz", "qobuz-edition", "GBAYE0601485");
    resolved.account_id = account;
    resolved.identity.album = Some("Álbum distinto".into());
    resolved.identity.provider_album_id = None;
    let outcome = promote_queue_entry(&db, &entry_id, SESSION_A, Ok(resolved))
        .await
        .expect("edición en revisión");
    assert!(matches!(
        outcome,
        PromotionOutcome::StillEphemeral { reason } if reason.contains("needs_review")
    ));
    let entry = perform_get_queue_entry(&db, &entry_id)
        .await
        .expect("entrada");
    assert_eq!(entry.track_id, 0);
    let sources: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources")
        .fetch_one(&db)
        .await
        .expect("fuentes");
    assert_eq!(
        sources, 0,
        "la revisión de edición no enlaza una fuente nueva"
    );
}

#[tokio::test]
async fn reconciliador_recupera_fuente_persistida_antes_de_convertir() {
    let db = pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-crash-source", false)
        .await
        .expect("encolar");
    let track_id: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Fuente recuperable') RETURNING id")
            .fetch_one(&db)
            .await
            .expect("track");
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) \
         VALUES (?, ?, 'qobuz-crash-source')",
    )
    .bind(track_id)
    .bind(qobuz)
    .execute(&db)
    .await
    .expect("fuente persistida");
    freeze_queue_listen_session(&db, &entry_id, SESSION_A)
        .await
        .expect("congelar token");

    assert_eq!(reconcile_queue_promotions(&db).await.expect("recuperar"), 1);
    assert_eq!(reconcile_queue_promotions(&db).await.expect("repetir"), 0);
    let entry = perform_get_queue_entry(&db, &entry_id)
        .await
        .expect("entrada");
    assert_eq!(entry.track_id, track_id);
    let play_count: i64 = sqlx::query_scalar("SELECT play_count FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&db)
        .await
        .expect("play_count");
    assert_eq!(play_count, 1);
}

#[tokio::test]
async fn promociones_concurrentes_consumen_el_mismo_token_congelado() {
    let db = concurrent_pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let account = account_id(&db, "qobuz").await;
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-concurrent", false)
        .await
        .expect("encolar");
    assert_eq!(
        freeze_queue_listen_session(&db, &entry_id, SESSION_A)
            .await
            .expect("primera sesión"),
        SESSION_A
    );
    assert_eq!(
        freeze_queue_listen_session(&db, &entry_id, SESSION_B)
            .await
            .expect("segunda sesión"),
        SESSION_A
    );
    let mut resolved = identity(qobuz, "qobuz", "qobuz-concurrent", "GBAYE0601486");
    resolved.account_id = account;
    let (first, second) = tokio::join!(
        promote_queue_entry(&db, &entry_id, SESSION_A, Ok(resolved.clone())),
        promote_queue_entry(&db, &entry_id, SESSION_B, Ok(resolved)),
    );
    let outcomes = [
        first.expect("primera promoción"),
        second.expect("segunda promoción"),
    ];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| matches!(
                outcome,
                PromotionOutcome::Promoted { counted: true, .. }
                    | PromotionOutcome::AlreadyLibrary { counted: true, .. }
            ))
            .count(),
        1
    );
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&db)
        .await
        .expect("tracks");
    let sources: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources")
        .fetch_one(&db)
        .await
        .expect("fuentes");
    let listens: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playback_listens")
        .fetch_one(&db)
        .await
        .expect("escuchas");
    let play_count: i64 = sqlx::query_scalar("SELECT play_count FROM tracks WHERE id = 1")
        .fetch_one(&db)
        .await
        .expect("play_count");
    assert_eq!((tracks, sources, listens, play_count), (1, 1, 1, 1));
    let frozen: String =
        sqlx::query_scalar("SELECT listen_session_id FROM player_queue_entries WHERE entry_id = ?")
            .bind(&entry_id)
            .fetch_one(&db)
            .await
            .expect("token congelado");
    assert_eq!(frozen, SESSION_A);
}

#[tokio::test]
async fn activar_y_promocionar_concurrentemente_conserva_la_entrada_actual() {
    let db = concurrent_pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-activate-race", false)
        .await
        .expect("encolar");
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title) VALUES ('Activación concurrente') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .expect("track");
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) \
         VALUES (?, ?, 'qobuz-activate-race')",
    )
    .bind(track_id)
    .bind(qobuz)
    .execute(&db)
    .await
    .expect("fuente");
    let (activated, promoted) = tokio::join!(
        perform_set_current_entry(&db, &entry_id),
        promote_queue_entry(
            &db,
            &entry_id,
            SESSION_A,
            Err("recuperación concurrente".into()),
        ),
    );
    activated.expect("activar entrada");
    promoted.expect("promocionar entrada");
    let snapshot = perform_get_playback_queue(&db).await.expect("snapshot");
    let current = snapshot.current.expect("current");
    assert_eq!(current.entry_id, entry_id);
    assert_eq!(current.track_id, track_id);
    assert!(snapshot.pending.is_empty());
}

#[test]
fn el_contrato_ipc_expone_cero_y_la_identidad_del_proveedor() {
    let entry = PlaybackQueueEntryDto {
        entry_id: "entry-ephemera".into(),
        track_id: 0,
        service_id: Some(2),
        service_track_id: Some("qobuz-1".into()),
        kind: "pending".into(),
        position: 0,
        promotion_error: Some("reconnect".into()),
    };
    let value = serde_json::to_value(entry).expect("serializar entrada");
    assert_eq!(value["track_id"], 0);
    assert_eq!(value["service_id"], 2);
    assert_eq!(value["service_track_id"], "qobuz-1");
    let advanced = PlayerAdvancedPayload {
        seq: 3,
        track_id: 0,
        service_id: Some(2),
        service_track_id: Some("qobuz-1".into()),
        entry_id: "entry-ephemera".into(),
        previous_entry_id: None,
    };
    let value = serde_json::to_value(advanced).expect("serializar avance");
    assert_eq!(value["track_id"], 0);
    assert_eq!(value["service_id"], 2);
    assert_eq!(value["service_track_id"], "qobuz-1");
    assert!(value.get("previous_entry_id").is_none());
}

#[tokio::test]
async fn metadatos_incompletos_no_materializan_biblioteca() {
    let db = pool().await;
    let qobuz = service_id(&db, "qobuz").await;
    let account = account_id(&db, "qobuz").await;
    let entry_id = perform_enqueue_provider_item(&db, qobuz, "qobuz-5", false)
        .await
        .expect("encolar efímera");
    let mut resolved = identity(qobuz, "qobuz", "qobuz-5", "GBAYE0601480");
    resolved.account_id = account;
    resolved.identity.title = None;
    let outcome = promote_queue_entry(&db, &entry_id, SESSION_A, Ok(resolved))
        .await
        .expect("resultado de identidad incompleta");
    assert!(matches!(
        outcome,
        PromotionOutcome::StillEphemeral { reason } if reason.contains("metadatos incompletos")
    ));
    let tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&db)
        .await
        .expect("tracks");
    assert_eq!(tracks, 0);
}

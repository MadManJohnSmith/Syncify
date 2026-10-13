//! Tests del resolutor único de descargas (WF-RES-01).
//!
//! Todos usan SQLite en memoria y proveedores simulados: no hay red. Comprueban
//! que una sola decisión (pista/edición, servicio, cuenta concreta,
//! `service_track_id`, calidad permitida y secuencia de alternativas) es la que
//! consumen preflight, worker y adaptadores, y que la re-resolución excluye
//! exactamente el intento que falló.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::download::{
    classify_attempt_failure, exclusions_for_failure, expand_attempt_sequence,
    resolve_download_plan, DownloadIntent, ReResolutionTrigger, ResolutionError,
    ResolutionExclusions,
};

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");

    // Baselines: spotify (no descargable), qobuz + tidal (descargables). Las
    // prioridades vienen de 0007 (spotify 1, qobuz 2, tidal 3).
    for (id, name, supports_download) in [(1, "spotify", 0), (2, "qobuz", 1), (3, "tidal", 1)] {
        sqlx::query(
            "INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (?, ?, ?, 'hires')",
        )
        .bind(id)
        .bind(name)
        .bind(supports_download)
        .execute(&pool)
        .await
        .unwrap();
    }
    pool
}

/// Inserta una cuenta. `streaming_entitled`: 1 sí / 0 no / -1 desconocido.
#[allow(clippy::too_many_arguments)]
async fn add_account(
    db: &SqlitePool,
    account_id: i64,
    service_id: i64,
    email: &str,
    is_active: bool,
    credentials_invalid: bool,
    streaming_entitled: Option<i64>,
) {
    sqlx::query(
        "INSERT INTO accounts (id, service_id, display_name, email, is_active, credentials_json, credentials_invalid, streaming_entitled)
         VALUES (?, ?, ?, ?, ?, 'encrypted-test-blob', ?, ?)",
    )
    .bind(account_id)
    .bind(service_id)
    .bind(email)
    .bind(email)
    .bind(i32::from(is_active))
    .bind(i32::from(credentials_invalid))
    // La columna es NOT NULL (migración 0095): «desconocido» se guarda -1.
    .bind(streaming_entitled.unwrap_or(-1))
    .execute(db)
    .await
    .unwrap();
}

#[allow(clippy::too_many_arguments)]
async fn add_source(
    db: &SqlitePool,
    track_id: i64,
    service_id: i64,
    service_track_id: &str,
    format: &str,
    bit_depth: i64,
    sample_rate: i64,
) {
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available)
         VALUES (?, ?, ?, ?, ?, ?, 100, 1)",
    )
    .bind(track_id)
    .bind(service_id)
    .bind(service_track_id)
    .bind(format)
    .bind(bit_depth)
    .bind(sample_rate)
    .execute(db)
    .await
    .unwrap();
}

/// 1. Dos cuentas Qobuz: la primera da preview y la segunda el FLAC completo.
/// El plan debe ordenar primero la que SÍ tiene streaming, y la secuencia debe
/// poner la hermana justo detrás de la primaria.
#[tokio::test]
async fn two_qobuz_accounts_sibling_after_preview_primary() {
    let db = create_test_db().await;
    add_account(&db, 11, 2, "no-stream@qobuz.com", true, false, Some(0)).await;
    add_account(&db, 12, 2, "full@qobuz.com", true, false, Some(1)).await;

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('T', 'GBAAA0000001') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    add_source(&db, track_id, 2, "qobuz_777", "FLAC", 24, 96000).await;

    let intent = DownloadIntent {
        canonical_track_id: track_id,
        requested_quality: Some("hires".to_string()),
        ..Default::default()
    };
    let plan = resolve_download_plan(&db, &intent, &ResolutionExclusions::default())
        .await
        .expect("plan resolves");

    let primary = plan.primary().expect("primary candidate");
    assert_eq!(primary.service_name, "qobuz");
    assert_eq!(primary.service_track_id, "qobuz_777");
    // La cuenta SIN streaming nunca va primera (accounts.rs:488-493).
    assert_eq!(
        primary.account.as_ref().unwrap().account_id,
        12,
        "the streaming account must run first"
    );
    assert_eq!(
        primary.sibling_accounts.len(),
        1,
        "the sibling account must be carried, not dropped"
    );
    assert_eq!(primary.sibling_accounts[0].account_id, 11);

    // La secuencia pone la hermana INMEDATAMENTE después de la primaria, para el
    // MISMO service_track_id: eso es lo que exige el fallo de preview.
    let steps = expand_attempt_sequence(&plan);
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].candidate_index, 0);
    assert_eq!(steps[0].account_index, 0);
    assert_eq!(
        steps[1].candidate_index, 0,
        "sibling before any other edition"
    );
    assert_eq!(steps[1].account_index, 1);
    assert_eq!(
        steps[1].identity(&plan).unwrap().service_track_id,
        "qobuz_777"
    );
    assert_eq!(steps[1].account(&plan).unwrap().account_id, 11);
}

/// 2. Cuenta desconocida (`streaming_entitled = -1`): sigue siendo candidata,
/// pero por detrás de la que sí tiene streaming. No se excluye.
#[tokio::test]
async fn unknown_entitlement_account_is_candidate_not_dropped() {
    let db = create_test_db().await;
    add_account(&db, 21, 2, "unknown@qobuz.com", true, false, Some(-1)).await;
    add_account(&db, 22, 2, "yes@qobuz.com", true, false, Some(1)).await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    add_source(&db, track_id, 2, "q_1", "FLAC", 16, 44100).await;

    let plan = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .unwrap();

    assert_eq!(
        plan.primary().unwrap().account.as_ref().unwrap().account_id,
        22
    );
    let sibling = &plan.primary().unwrap().sibling_accounts;
    assert_eq!(sibling.len(), 1);
    assert_eq!(sibling[0].account_id, 21, "unknown stays a candidate");
    assert_eq!(sibling[0].streaming_entitled, Some(-1));
}

/// 3. Todas las cuentas sin streaming: el plan sigue eligiendo (el plan no es
/// prueba universal de la cuenta), pero lo documenta con el estado.
#[tokio::test]
async fn all_accounts_without_streaming_still_resolves() {
    let db = create_test_db().await;
    add_account(&db, 31, 2, "a@qobuz.com", true, false, Some(0)).await;
    add_account(&db, 32, 2, "b@qobuz.com", true, false, Some(0)).await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    add_source(&db, track_id, 2, "q_1", "FLAC", 16, 44100).await;

    let plan = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .expect("a preview-possible account is still an attempt");

    assert_eq!(plan.candidates.len(), 1);
    assert_eq!(plan.primary().unwrap().sibling_accounts.len(), 1);
    assert_eq!(
        plan.primary()
            .unwrap()
            .account
            .as_ref()
            .unwrap()
            .streaming_entitled,
        Some(0),
        "the plan keeps the entitlement state visible for the log"
    );
}

/// 4. Fuente obsoleta (`stale_404`): no es candidata.
#[tokio::test]
async fn stale_source_is_not_a_candidate() {
    let db = create_test_db().await;
    add_account(&db, 41, 2, "a@qobuz.com", true, false, Some(1)).await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, available, availability_status)
         VALUES (?, 2, 'q_gone', 'FLAC', 1, 'stale_404')",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    let err = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .expect_err("stale source must not be offered");
    assert!(
        matches!(err, ResolutionError::NoDownloadableSource { .. }),
        "got {:?}",
        err
    );
}

/// 5. Ambigüedad: `tracks.isrc` es UNIQUE (`0002_normalize_schema.sql:91`,
/// reforzado por `0064:186`) y `track_sources` es UNIQUE en
/// `(track_id, service_id)`. Por tanto dos pistas NO pueden compartir ISRC, y un
/// mismo par pista+servicio solo tiene UNA fila.
///
/// La ambigüedad real que el plan describe es la de `ambiguous_source_dedupe_test.rs`:
/// varias pistas candidatas SIN ISRC que compiten por título, y el ISRC que las
/// separaría no existe. El resolutor trabaja SIEMPRE sobre una pista canónica
/// concreta (`canonical_track_id`), así que una ambigüedad de título no puede
/// colarse: no se resuelve «eligiendo» entre pistas, se exige la identidad.
#[tokio::test]
async fn title_level_ambiguity_cannot_leak_into_a_plan() {
    let db = create_test_db().await;
    add_account(&db, 51, 3, "a@tidal.com", true, false, Some(-1)).await;

    // Dos pistas con el mismo título y SIN ISRC: compiten por título.
    let t1: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Same Title') RETURNING id")
            .fetch_one(&db)
            .await
            .unwrap();
    let t2: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Same Title') RETURNING id")
            .fetch_one(&db)
            .await
            .unwrap();

    add_source(&db, t1, 3, "tidal_first", "FLAC", 16, 44100).await;
    add_source(&db, t2, 3, "tidal_second", "FLAC", 16, 44100).await;

    // El plan solo conoce la pista pedida.
    let plan = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: t1,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .expect("a concrete canonical track always resolves to its own edition");

    assert_eq!(plan.canonical_track_id, t1);
    assert_eq!(
        plan.primary().unwrap().service_track_id,
        "tidal_first",
        "the other same-title track must not leak into this plan"
    );
    assert_eq!(
        plan.primary().unwrap().identity.canonical_track_id,
        t1,
        "the identity is the canonical track, not a title match"
    );

    // Excluida esa edición, el plan falla cerrado en vez de saltar a la otra
    // pista: cruzar pistas requiere la jerarquía de equivalencia del orquestador,
    // que no es una decisión que el resolutor tome por su cuenta.
    let err = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: t1,
            ..Default::default()
        },
        &exclusions_for_failure(
            ReResolutionTrigger::NotFound,
            t1,
            3,
            "tidal_first",
            Some(51),
        ),
    )
    .await
    .expect_err("no silent edition substitution across canonical tracks");
    assert!(matches!(err, ResolutionError::NoDownloadableSource { .. }));
}

/// 6. Qobuz → Tidal: solo si la política lo permite, y manteniendo el orden de
/// servicios de `service_preferences`.
#[tokio::test]
async fn cross_service_alternative_respects_service_priority_order() {
    let db = create_test_db().await;
    add_account(&db, 61, 2, "a@qobuz.com", true, false, Some(1)).await;
    add_account(&db, 62, 3, "b@tidal.com", true, false, None).await;

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('T', 'GBAAA0000009') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    add_source(&db, track_id, 2, "q_1", "FLAC", 24, 96000).await;
    add_source(&db, track_id, 3, "t_1", "FLAC", 24, 96000).await;

    let plan = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            requested_quality: Some("hires".to_string()),
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .unwrap();

    assert_eq!(plan.candidates.len(), 2);
    assert_eq!(
        plan.candidates[0].service_name, "qobuz",
        "qobuz priority 2 < tidal 3"
    );
    assert_eq!(plan.candidates[1].service_name, "tidal");

    // Sin permiso de fallback, el plan sigue ofreciendo ambas pero la ejecución
    // respeta `allow_fallback=false`: el plan lo declara.
    let strict = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            allow_fallback: false,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .unwrap();
    assert!(
        !strict.allow_fallback,
        "strict plan must declare no fallback"
    );

    // Con Qobuz excluido y permiso, Tidal es la alternativa.
    let tidal = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            ..Default::default()
        },
        &exclusions_for_failure(
            ReResolutionTrigger::QobuzPreview,
            track_id,
            2,
            "q_1",
            Some(61),
        ),
    )
    .await
    .unwrap();
    assert_eq!(tidal.primary().unwrap().service_name, "tidal");
    assert_eq!(tidal.primary().unwrap().service_track_id, "t_1");
}

/// 7. Región: un 403 por región clasifica como región y excluye la cuenta.
#[tokio::test]
async fn region_restriction_is_classified_and_excludes_account() {
    let trigger = classify_attempt_failure("RegionRestricted: CountryNotAvailable for country US");
    assert_eq!(trigger, Some(ReResolutionTrigger::RegionRestricted));

    let ex = exclusions_for_failure(ReResolutionTrigger::RegionRestricted, 1, 2, "q_1", Some(71));
    assert!(ex.excludes_identity(2, "q_1"));
    assert!(ex.excludes_account(1, 2, 71));
}

/// 8. Modo estricto: no se degrada la calidad permitida.
#[tokio::test]
async fn strict_mode_never_downgrades_allowed_quality() {
    let db = create_test_db().await;
    add_account(&db, 81, 2, "a@qobuz.com", true, false, Some(1)).await;

    // Techo global en 'lossless' y solicitud 'hires': se recorta a lossless.
    sqlx::query("INSERT INTO settings (key, value) VALUES ('global_max_quality', 'lossless')")
        .execute(&db)
        .await
        .unwrap();

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    add_source(&db, track_id, 2, "q_1", "FLAC", 24, 96000).await;

    let plan = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            requested_quality: Some("hires".to_string()),
            strict_quality: true,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .unwrap();

    assert_eq!(
        plan.quality.effective, "LOSSLESS",
        "hires must be clamped under the global ceiling"
    );
    assert!(plan.quality.clamped);
    assert!(plan.strict_quality);
}

/// 9. Degradación consentida: con `allow_fallback` el plan lo declara y permite
/// cambiar de edición/servicio.
#[tokio::test]
async fn consented_degradation_is_declared() {
    let db = create_test_db().await;
    add_account(&db, 91, 2, "a@qobuz.com", true, false, Some(1)).await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    // Fuente anuncia MP3 320 (lossy).
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bitrate, available)
         VALUES (?, 2, 'q_lossy', 'MP3', 320, 1)",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    let plan = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            requested_quality: Some("lossless".to_string()),
            allow_fallback: true,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .unwrap();

    assert!(plan.allow_fallback, "consent must be visible to adapters");
    assert!(
        !plan.strict_quality,
        "allowing fallback is not strict quality"
    );
    assert_eq!(
        plan.quality.candidate, "lossy",
        "the advertised tier stays honest even when the request was lossless"
    );
}

/// 10. Preferencias que cambian entre cola y ejecución: el mismo ítem resuelto
/// con intents distintos da calidades distintas, y el worker vuelve a calcular
/// justo antes de la red (aquí se comprueba que el resolutor es puro respecto a
/// la preferencia efectiva en el momento de la llamada).
#[tokio::test]
async fn preferences_changing_between_queue_and_execution_are_picked_up() {
    let db = create_test_db().await;
    add_account(&db, 101, 2, "a@qobuz.com", true, false, Some(1)).await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    add_source(&db, track_id, 2, "q_1", "FLAC", 24, 96000).await;

    // Encolado con techo sin límite: hires.
    let queued = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .unwrap();
    assert_eq!(queued.quality.effective, "HI_RES_LOSSLESS");

    // Las preferencias cambian: ahora el techo global es 'high'.
    sqlx::query(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('global_max_quality', 'high')",
    )
    .execute(&db)
    .await
    .unwrap();

    // El worker recalcula: la nueva decisión respeta el techo nuevo.
    let executed = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .unwrap();
    assert_eq!(
        executed.quality.effective, "HIGH",
        "execution must see the new ceiling"
    );
    assert_ne!(
        queued.quality.effective, executed.quality.effective,
        "the plan is recomputed at execution, not frozen at queue time"
    );
}

/// 11. Reintentos acotados: el mismo paquete de exclusiones nunca produce la
/// misma combinación y la secuencia es finita.
#[tokio::test]
async fn retries_are_bounded_and_never_repeat_a_combination() {
    let db = create_test_db().await;
    add_account(&db, 111, 2, "a@qobuz.com", true, false, Some(0)).await;
    add_account(&db, 112, 2, "b@qobuz.com", true, false, Some(0)).await;
    add_account(&db, 113, 2, "c@qobuz.com", true, false, Some(0)).await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    add_source(&db, track_id, 2, "q_1", "FLAC", 16, 44100).await;

    let plan = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .unwrap();

    let steps = expand_attempt_sequence(&plan);
    assert_eq!(steps.len(), 3, "one edition, three accounts");

    // Ningún par (edición, cuenta) se repite.
    let mut seen: Vec<(String, i64)> = Vec::new();
    for step in &steps {
        let identity = step.identity(&plan).unwrap().service_track_id.clone();
        let account = step.account(&plan).unwrap().account_id;
        assert!(
            !seen.contains(&(identity.clone(), account)),
            "combination {} / {} repeated",
            identity,
            account
        );
        seen.push((identity, account));
    }

    // Y está acotado por el tope del módulo.
    assert!(
        syncify_tauri_lib::download::MAX_PLAN_CANDIDATES >= steps.len(),
        "the attempt sequence must be bounded"
    );
    let _ = &steps;
}

/// 12. Concurrencia: dos resoluciones concurrentes del mismo ítem devuelven
/// planes equivalentes (el resolutor no tiene estado compartido).
#[tokio::test]
async fn concurrent_resolutions_of_the_same_item_agree() {
    let db = create_test_db().await;
    add_account(&db, 121, 2, "a@qobuz.com", true, false, Some(1)).await;
    add_account(&db, 122, 3, "b@tidal.com", true, false, None).await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    add_source(&db, track_id, 2, "q_1", "FLAC", 24, 96000).await;
    add_source(&db, track_id, 3, "t_1", "FLAC", 24, 96000).await;

    let intent = DownloadIntent {
        canonical_track_id: track_id,
        requested_quality: Some("lossless".to_string()),
        ..Default::default()
    };
    let exclusions = ResolutionExclusions::default();
    let (a, b) = tokio::join!(
        resolve_download_plan(&db, &intent, &exclusions),
        resolve_download_plan(&db, &intent, &exclusions)
    );
    let plan_a = a.expect("first resolution");
    let plan_b = b.expect("second resolution");
    assert_eq!(
        plan_a, plan_b,
        "the resolver is stateless and deterministic"
    );
}

/// 13. Cancelación: un fallo de cancelación NO dispara re-resolución.
#[tokio::test]
async fn cancellation_never_triggers_re_resolution() {
    assert_eq!(classify_attempt_failure("Download cancelled by user"), None);
    assert_eq!(classify_attempt_failure("Download cancelled"), None);
}

/// 14. Registro de la decisión sin credenciales.
#[tokio::test]
async fn decision_log_carries_no_credentials() {
    let db = create_test_db().await;
    add_account(&db, 141, 2, "a@qobuz.com", true, false, Some(1)).await;

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('T', 'GBAAA0000014') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    add_source(&db, track_id, 2, "q_1", "FLAC", 24, 96000).await;

    let plan = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .unwrap();

    let log = plan.log(0).expect("a log for the primary attempt");
    assert_eq!(log.service_name, "qobuz");
    assert_eq!(log.service_track_id, "q_1");
    assert_eq!(log.account_id, Some(141));
    let serialized = serde_json::to_string(&log).unwrap();
    for forbidden in ["token", "credential", "password", "app_secret"] {
        assert!(
            !serialized.to_lowercase().contains(forbidden),
            "the log leaked '{}'",
            forbidden
        );
    }
}

/// 15. Sin cuenta con credenciales resolubles: el error lo dice explícitamente.
#[tokio::test]
async fn no_account_is_reported_not_invented() {
    let db = create_test_db().await;
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    add_source(&db, track_id, 2, "q_1", "FLAC", 16, 44100).await;

    let err = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .expect_err("no account means no candidate");
    assert!(matches!(err, ResolutionError::NoDownloadableSource { .. }));
}

/// 16. Las exclusiones se ACUMULAN entre rondas de re-resolución: una segunda
/// ronda no puede reintentar una combinación que la primera ya descartó.
#[tokio::test]
async fn exclusions_accumulate_across_re_resolution_rounds() {
    let db = create_test_db().await;
    add_account(&db, 161, 2, "a@qobuz.com", true, false, Some(1)).await;
    add_account(&db, 162, 2, "b@qobuz.com", true, false, Some(1)).await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    add_source(&db, track_id, 2, "q_1", "FLAC", 16, 44100).await;

    // Ronda 1: falla la cuenta 161 con preview.
    let mut round1 = exclusions_for_failure(
        ReResolutionTrigger::QobuzPreview,
        track_id,
        2,
        "q_1",
        Some(161),
    );
    // Ronda 2: falla la cuenta 162 con RequireAuth. Las exclusiones de la ronda 1
    // deben seguir vigentes.
    let round2 = exclusions_for_failure(
        ReResolutionTrigger::RequiresAuth,
        track_id,
        2,
        "q_1",
        Some(162),
    );
    round1.merge(&round2);

    assert!(
        round1.excludes_account(track_id, 2, 161) && round1.excludes_account(track_id, 2, 162),
        "both failed accounts must stay excluded for this track"
    );

    // Sin ninguna cuenta disponible, el plan falla cerrado en vez de reintentar.
    let err = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            ..Default::default()
        },
        &round1,
    )
    .await
    .expect_err("both accounts excluded and the only edition excluded too");
    assert!(matches!(err, ResolutionError::NoDownloadableSource { .. }));
}

/// 17. Qué excluye cada motivo. La unidad que nunca se repite es la
/// combinación (edición, cuenta); qué se marca depende de dónde estuvo el
/// problema. Un preview NO excluye la edición: si lo hiciera, sería imposible
/// probar el MISMO `service_track_id` con la cuenta hermana, que es
/// exactamente lo que la política exige tras un preview.
#[tokio::test]
async fn preview_excludes_account_transport_failure_does_not() {
    let preview = exclusions_for_failure(ReResolutionTrigger::QobuzPreview, 1, 2, "q_1", Some(171));
    assert!(
        preview.excludes_account(1, 2, 171),
        "a preview is a per-account observation for this track"
    );
    assert!(
        !preview.excludes_identity(2, "q_1"),
        "the edition stays available for the sibling account"
    );

    let transport = exclusions_for_failure(
        ReResolutionTrigger::TransientFailure,
        1,
        2,
        "q_1",
        Some(171),
    );
    assert!(
        transport.excludes_account(1, 2, 171),
        "the concrete combination must not be retried"
    );
    assert!(
        !transport.excludes_identity(2, "q_1"),
        "a transport failure does not implicate the edition"
    );

    let not_found = exclusions_for_failure(ReResolutionTrigger::NotFound, 1, 2, "q_1", Some(171));
    assert!(
        not_found.excludes_identity(2, "q_1"),
        "a 404 is a fact about the edition"
    );
    assert!(
        !not_found.excludes_account(1, 2, 171),
        "a plain 404 does not implicate the account"
    );

    let region = exclusions_for_failure(
        ReResolutionTrigger::RegionRestricted,
        1,
        2,
        "q_1",
        Some(171),
    );
    assert!(region.excludes_identity(2, "q_1"));
    assert!(
        region.excludes_account(1, 2, 171),
        "a region refusal implicates the account too"
    );
}

/// 18. Un preview aislado NUNCA se convierte en «la cuenta no tiene plan»: con
/// la cuenta excluida solo para esta pista, la misma cuenta sigue siendo
/// candidata para otra.
#[tokio::test]
async fn an_isolated_preview_never_verdicts_the_account_globally() {
    let db = create_test_db().await;
    add_account(&db, 181, 2, "a@qobuz.com", true, false, Some(1)).await;
    add_account(&db, 182, 3, "b@tidal.com", true, false, Some(-1)).await;

    let this_track: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('This') RETURNING id")
            .fetch_one(&db)
            .await
            .unwrap();
    let other_track: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Other') RETURNING id")
            .fetch_one(&db)
            .await
            .unwrap();
    add_source(&db, this_track, 2, "q_this", "FLAC", 16, 44100).await;
    add_source(&db, other_track, 2, "q_other", "FLAC", 16, 44100).await;

    // La cuenta 181 falló con preview en ESTA pista.
    let exclusions_this_track = exclusions_for_failure(
        ReResolutionTrigger::QobuzPreview,
        this_track,
        2,
        "q_this",
        Some(181),
    );

    // Para OTRA pista, la misma cuenta sigue siendo la primaria.
    let other_plan = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: other_track,
            ..Default::default()
        },
        &exclusions_this_track,
    )
    .await
    .expect("the other track is unaffected");
    assert_eq!(
        other_plan
            .primary()
            .unwrap()
            .account
            .as_ref()
            .unwrap()
            .account_id,
        181,
        "a preview on one track must not demote the account for another"
    );
}

/// 19. La prueba central del carril: lo que el preflight presenta como PROPUESTA
/// es exactamente lo que el worker vuelve a calcular antes de la red. Se resuelve
/// dos veces (una para el preflight y otra para la ejecución) con la misma
/// intención y se comprueba que coinciden en edición, cuenta y calidad.
#[tokio::test]
async fn preflight_proposal_matches_execution_decision() {
    let db = create_test_db().await;
    add_account(&db, 191, 2, "no-stream@qobuz.com", true, false, Some(0)).await;
    add_account(&db, 192, 2, "full@qobuz.com", true, false, Some(1)).await;
    add_account(&db, 193, 3, "tidal@tidal.com", true, false, Some(-1)).await;

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('T', 'GBAAA0000019') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    add_source(&db, track_id, 2, "q_19", "FLAC", 24, 96000).await;
    add_source(&db, track_id, 3, "t_19", "FLAC", 24, 96000).await;

    // La intención es la que usa el preflight y la que reutiliza el worker.
    let intent = DownloadIntent {
        canonical_track_id: track_id,
        requested_service: None,
        requested_quality: Some("hires".to_string()),
        strict_quality: false,
        allow_fallback: true,
        locked_track_source_id: None,
        preferred_account_id: None,
    };

    let proposal = resolve_download_plan(&db, &intent, &ResolutionExclusions::default())
        .await
        .expect("preflight proposal");
    let execution = resolve_download_plan(&db, &intent, &ResolutionExclusions::default())
        .await
        .expect("worker recalculation before the network");

    assert_eq!(
        proposal.primary().unwrap().service_track_id,
        execution.primary().unwrap().service_track_id,
        "the same edition must be proposed and executed"
    );
    assert_eq!(
        proposal
            .primary()
            .unwrap()
            .account
            .as_ref()
            .unwrap()
            .account_id,
        execution
            .primary()
            .unwrap()
            .account
            .as_ref()
            .unwrap()
            .account_id,
        "the same concrete account must be proposed and executed"
    );
    assert_eq!(
        proposal.quality.effective, execution.quality.effective,
        "the same allowed quality must be proposed and executed"
    );
    // Y el candidato propuesto es el que tiene streaming, no el primero por id.
    assert_eq!(
        proposal
            .primary()
            .unwrap()
            .account
            .as_ref()
            .unwrap()
            .account_id,
        192
    );
}

/// 20. Reinicio/cancelación: tras un fallo clasificado el plan excluye el intento
/// y produce una decisión DISTINTA (no reintenta lo mismo); la cancelación, en
/// cambio, nunca produce nueva decisión.
#[tokio::test]
async fn restart_and_cancellation_paths_diverge() {
    // Fallo clasificado -> nueva decisión, combinación distinta.
    let db = create_test_db().await;
    add_account(&db, 201, 2, "a@qobuz.com", true, false, Some(1)).await;
    add_account(&db, 202, 2, "b@qobuz.com", true, false, Some(1)).await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T') RETURNING id")
        .fetch_one(&db)
        .await
        .unwrap();
    add_source(&db, track_id, 2, "q_20", "FLAC", 16, 44100).await;

    let intent = DownloadIntent {
        canonical_track_id: track_id,
        ..Default::default()
    };
    let first = resolve_download_plan(&db, &intent, &ResolutionExclusions::default())
        .await
        .unwrap();

    let failed_account = first
        .primary()
        .unwrap()
        .account
        .as_ref()
        .unwrap()
        .account_id;
    let retry_exclusions = exclusions_for_failure(
        ReResolutionTrigger::QobuzPreview,
        track_id,
        2,
        "q_20",
        Some(failed_account),
    );
    let second = resolve_download_plan(&db, &intent, &retry_exclusions)
        .await
        .unwrap();

    // El orden de cuentas termina en «más nueva primero» (`resolver.rs`), así que
    // 202 corre antes que 201. Excluir 202 deja 201 como intento siguiente: no se
    // repite la combinación que falló y se mantiene la MISMA edición.
    assert_eq!(
        first
            .primary()
            .unwrap()
            .account
            .as_ref()
            .unwrap()
            .account_id,
        202,
        "the newest account runs first (same policy as get_service_fallback_account_ids)"
    );
    assert_eq!(
        second
            .primary()
            .unwrap()
            .account
            .as_ref()
            .unwrap()
            .account_id,
        201,
        "the retry must move to the sibling account, never repeat the same one"
    );
    assert_eq!(
        first.primary().unwrap().service_track_id,
        second.primary().unwrap().service_track_id,
        "the sibling retry keeps the SAME service_track_id"
    );

    // Cancelación -> sin nueva decisión.
    assert_eq!(classify_attempt_failure("Download cancelled by user"), None);
}

/// 21. El caso del plan, de principio a fin: la PRIMERA cuenta Qobuz entrega un
/// preview y la SEGUNDA el FLAC completo. La secuencia del plan debe resolver la
/// cuenta hermana para el MISMO `service_track_id`, con el mismo techo de
/// calidad y sin reintentar la cuenta que falló.
#[tokio::test]
async fn preview_then_sibling_full_flac_keeps_same_edition() {
    let db = create_test_db().await;
    // La primera da preview: entitlement desconocido (se cree) y activa.
    add_account(&db, 211, 2, "preview@qobuz.com", true, false, Some(-1)).await;
    // La segunda da el FLAC: entitlement confirmado.
    add_account(&db, 212, 2, "full@qobuz.com", true, false, Some(1)).await;

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('T', 'GBAAA0000021') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    add_source(&db, track_id, 2, "qobuz_same_edition", "FLAC", 24, 96000).await;

    let intent = DownloadIntent {
        canonical_track_id: track_id,
        requested_quality: Some("hires".to_string()),
        ..Default::default()
    };

    // Intento 1: la cuenta 212 (la que tiene streaming confirmado).
    let first = resolve_download_plan(&db, &intent, &ResolutionExclusions::default())
        .await
        .unwrap();
    let first_account = first
        .primary()
        .unwrap()
        .account
        .as_ref()
        .unwrap()
        .account_id;
    assert_eq!(first_account, 212);

    // La cuenta 212 devuelve un preview: se clasifica y se construyen las
    // exclusiones. El preview es de pista + cuenta + calidad, así que la cuenta
    // queda fuera pero la EDICIÓN sigue viva para la hermana.
    let trigger = classify_attempt_failure(
        "QobuzPreviewPayload: requested Qobuz FLAC but obtained MP3 30s preview",
    )
    .expect("a preview must be classified");
    assert_eq!(trigger, ReResolutionTrigger::QobuzPreview);

    let exclusions = exclusions_for_failure(
        trigger,
        track_id,
        2,
        first.primary().unwrap().service_track_id.as_str(),
        Some(first_account),
    );

    // Intento 2: la misma edición, la cuenta hermana.
    let second = resolve_download_plan(&db, &intent, &exclusions)
        .await
        .expect("the sibling account keeps the edition available");
    assert_eq!(
        second.primary().unwrap().service_track_id,
        "qobuz_same_edition",
        "the sibling retry must use the SAME service_track_id"
    );
    assert_eq!(
        second
            .primary()
            .unwrap()
            .account
            .as_ref()
            .unwrap()
            .account_id,
        211,
        "the retry moves to the sibling account"
    );
    assert_ne!(
        second
            .primary()
            .unwrap()
            .account
            .as_ref()
            .unwrap()
            .account_id,
        first_account,
        "the same combination must never be retried"
    );
    assert_eq!(
        second.quality.effective, first.quality.effective,
        "the allowed quality is identical across the sibling retry"
    );

    // Y la secuencia completa ya venía preparada desde el primer plan: hermana
    // inmediatamente después de la primaria, misma edición.
    let steps = expand_attempt_sequence(&first);
    let editions: Vec<&str> = steps
        .iter()
        .filter_map(|step| step.identity(&first))
        .map(|candidate| candidate.service_track_id.as_str())
        .collect();
    assert_eq!(
        editions,
        vec!["qobuz_same_edition", "qobuz_same_edition"],
        "both attempts are the SAME edition; only the account changes"
    );
}

/// 22. Después de un preview, si la política lo permite, la siguiente edición es
/// otra (Tidal confirmada); en modo estricto, no se sale del servicio.
#[tokio::test]
async fn after_preview_tidal_only_when_policy_permits() {
    let db = create_test_db().await;
    add_account(&db, 221, 2, "preview@qobuz.com", true, false, Some(1)).await;
    add_account(&db, 222, 3, "tidal@tidal.com", true, false, Some(-1)).await;

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('T', 'GBAAA0000022') RETURNING id",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    add_source(&db, track_id, 2, "q_22", "FLAC", 24, 96000).await;
    add_source(&db, track_id, 3, "t_22", "FLAC", 24, 96000).await;

    let intent = DownloadIntent {
        canonical_track_id: track_id,
        requested_quality: Some("hires".to_string()),
        ..Default::default()
    };

    // Con permiso de fallback: tras el preview y sin más hermanas Qobuz, Tidal es
    // la alternativa del plan.
    let with_permission = resolve_download_plan(
        &db,
        &intent,
        &exclusions_for_failure(
            ReResolutionTrigger::QobuzPreview,
            track_id,
            2,
            "q_22",
            Some(221),
        ),
    )
    .await
    .expect("Tidal is the next alternative when the policy permits");
    assert_eq!(with_permission.primary().unwrap().service_name, "tidal");
    assert_eq!(with_permission.primary().unwrap().service_track_id, "t_22");
    assert_eq!(
        with_permission.primary().unwrap().identity.isrc.as_deref(),
        Some("GBAAA0000022"),
        "the alternative is reached through the ISRC equivalence, never through SongLink"
    );

    // Sin permiso de fallback, el plan declara que no se sale del servicio.
    let strict = resolve_download_plan(
        &db,
        &DownloadIntent {
            canonical_track_id: track_id,
            requested_quality: Some("hires".to_string()),
            strict_quality: true,
            allow_fallback: false,
            ..Default::default()
        },
        &ResolutionExclusions::default(),
    )
    .await
    .unwrap();
    assert!(!strict.allow_fallback);
    assert!(strict.strict_quality);
    assert_eq!(
        strict.primary().unwrap().service_name,
        "qobuz",
        "strict mode keeps the resolved service"
    );
}

//! D05 (#117): verificación de entrega y fallback de calidad.
//!
//! Cubre los criterios de aceptación del ticket sobre la base real de
//! migraciones y sin red:
//!
//! 1. Un fichero truncado (duración fuera de la tolerancia de ±2 s) no pasa
//!    el gate ni se marca `complete`.
//! 2. Tags o portada inválidos no pasan el gate.
//! 3. La calidad pedida no disponible bajo política estricta
//!    (`allow_fallback=false` o `fallback_action='skip'`) rechaza la entrega;
//!    con fallback permitido `mark_complete` completa y declara la caída.
//! 4. El ledger guarda la calidad FÍSICA (inspección del fichero), no la
//!    anunciada por el proveedor, y atribuye la entrega a la cuenta del
//!    `ProviderDelivery` (`downloads.account_id`).
//! 5. Una decisión `RejectedQuality` de la política compartida no se
//!    promociona a `complete` (defensa en `mark_complete`).
//! 6. SoundCloud 128 está en el piso por proveedor: se acepta, se declara y
//!    nunca se etiqueta lossless.
//! 7. El gate corre ANTES de `mark_complete`: `verify_then_complete` es la
//!    costura que usa `process_download_internal` y el test la ejerce por
//!    comportamiento (#338; antes era una guardia de cadenas sobre el fuente).
//! 8. #338: un FLAC/M4A no vacío que el lector no puede parsear falla cerrado
//!    (lector ausente ≠ lectura fallida) y un rechazo no deja el fichero
//!    promovido sin dueño ni la fila que la propia entrega (Tidal,
//!    `tidal_pipeline` §9) escribió antes del gate.

use std::path::Path;

use sqlx::sqlite::SqlitePoolOptions;
use syncify_core_domain::quality::{
    classify_audio_tier, is_below_provider_floor, provider_lossy_floor_kbps, AudioTier,
};
use syncify_tauri_lib::download::progress::DownloadResult;
use syncify_tauri_lib::download::verification::ExpectedDelivery;
use syncify_tauri_lib::worker::{DownloadWorker, DownloadWorkerState};
use tempfile::TempDir;

async fn migrated_pool(dir: &TempDir, name: &str) -> sqlx::SqlitePool {
    let db_path = dir.path().join(name);
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("connect test db");
    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("migrations");
    pool
}

/// FLAC sintético completo: STREAMINFO con duración exacta, tags correctos y
/// portada PNG frontal. Es la entrega que pasa el gate entero.
fn write_verified_flac(path: &Path, total_samples: u64) {
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]);
    let mut streaminfo = [0u8; 34];
    let block_size = (total_samples.min(65535)) as u16;
    streaminfo[0..2].copy_from_slice(&block_size.to_be_bytes());
    streaminfo[2..4].copy_from_slice(&block_size.to_be_bytes());
    // sample_rate (20 bits) = 44100, channels-1 (3 bits) = 1, bps-1 (5 bits) = 15
    streaminfo[10] = 0x0A;
    streaminfo[11] = 0xC4;
    streaminfo[12] = 0x42;
    streaminfo[13] = 0xF0;
    // total_samples: 36 bits; el nibble bajo del byte 13 ya vale 0.
    streaminfo[14..18]
        .copy_from_slice(&(u32::try_from(total_samples).unwrap_or(u32::MAX)).to_be_bytes());
    flac_bytes.extend_from_slice(&streaminfo);
    flac_bytes.extend_from_slice(&[0xFF, 0xF8, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00]);
    std::fs::write(path, &flac_bytes).unwrap();

    // Se lee el tag del fichero sintético (que trae el STREAMINFO) para que
    // la reescritura de metaflac lo conserve: sin STREAMINFO el inspector no
    // puede leer la duración y el gate pierde su evidencia.
    let mut tag = metaflac::Tag::read_from_path(path).unwrap();
    assert!(
        tag.get_streaminfo().is_some(),
        "el fichero sintético trae STREAMINFO"
    );
    let vc = tag.vorbis_comments_mut();
    vc.set("TITLE", vec!["Verified Track"]);
    vc.set("ARTIST", vec!["Verified Artist"]);
    vc.set("ALBUM", vec!["Verified Album"]);
    vc.set("TRACKNUMBER", vec!["3"]);
    vc.set("DISCNUMBER", vec!["1"]);
    tag.add_picture(
        "image/png",
        metaflac::block::PictureType::CoverFront,
        vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13],
    );
    tag.write_to_path(path).unwrap();
}

fn expected_delivery(catalog_duration_ms: i64) -> ExpectedDelivery {
    ExpectedDelivery {
        title: "Verified Track".to_string(),
        artist: "Verified Artist".to_string(),
        album: Some("Verified Album".to_string()),
        track_number: Some(3),
        disc_number: Some(1),
        catalog_duration_ms: Some(catalog_duration_ms),
    }
}

/// Fila de cola `downloading` con calidad pedida, cuenta resuelta y política
/// de fallback de la seed. Devuelve el queue_id.
async fn seed_downloading_row(
    pool: &sqlx::SqlitePool,
    dir: &TempDir,
    allow_fallback: i64,
    requested_quality: &str,
) -> (i64, i64) {
    sqlx::query("INSERT INTO services (id, name) VALUES (1, 'qobuz') ON CONFLICT(id) DO NOTHING")
        .execute(pool)
        .await
        .unwrap();
    // Dos cuentas reales del servicio: la 9 es la resuelta en la seed y la 7
    // es la que el ProviderDelivery declara como entregadora.
    for (id, email) in [(7, "delivered@example.test"), (9, "resolved@example.test")] {
        sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (?, 1, ?, ?, 1)")
            .bind(id)
            .bind(email)
            .bind(email)
            .execute(pool)
            .await
            .unwrap();
    }
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms, isrc) VALUES ('Verified Track', ?, 'USAAA2000003') RETURNING id",
    )
    .bind(200_000i64)
    .fetch_one(pool)
    .await
    .unwrap();
    let queue_id: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, progress_percent, allow_fallback, requested_quality, resolved_account_id) VALUES (?, 'downloading', 90.0, ?, ?, 9) RETURNING id",
    )
    .bind(track_id)
    .bind(allow_fallback)
    .bind(requested_quality)
    .fetch_one(pool)
    .await
    .unwrap();
    let file = dir.path().join("delivered.flac");
    write_verified_flac(&file, 200 * 44_100);
    (queue_id, track_id)
}

async fn queue_status(pool: &sqlx::SqlitePool, queue_id: i64) -> String {
    sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(queue_id)
        .fetch_one(pool)
        .await
        .expect("queue row exists")
}

async fn ledger_row(
    pool: &sqlx::SqlitePool,
    track_id: i64,
) -> Option<(String, i32, i32, Option<i64>)> {
    sqlx::query_as(
        "SELECT effective_quality, bit_depth, sample_rate, account_id FROM downloads WHERE track_id = ?",
    )
    .bind(track_id)
    .fetch_optional(pool)
    .await
    .unwrap()
}

/// Nombres de los ficheros que la limpieza de un rechazo conserva en la
/// cuarentena hermana del directorio de la entrega.
fn quarantined_files(parent: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(parent.join(".syncify-quarantine")) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect()
}

fn delivered_result(file: &Path) -> DownloadResult {
    DownloadResult {
        file_path: file.to_string_lossy().to_string(),
        bit_depth: 24,
        sample_rate: 96_000,
        title: "Verified Track".to_string(),
        artist: "Verified Artist".to_string(),
        album: "Verified Album".to_string(),
        service: "qobuz".to_string(),
        effective_service: Some("qobuz".to_string()),
        account_id: Some(7),
        ..Default::default()
    }
}

/// AC «Un fichero truncado (duración fuera de tolerancia) no se marca
/// `complete`»: el gate rechaza y la fila no se promociona.
#[tokio::test]
async fn truncated_duration_fails_the_gate_and_never_completes() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "truncated.db").await;
    let (queue_id, track_id) = seed_downloading_row(&pool, &dir, 1, "hires").await;
    let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));

    // Físico 197 s frente a catálogo 200 s: fuera de la tolerancia ±2 s.
    let truncated = dir.path().join("truncated.flac");
    write_verified_flac(&truncated, 197 * 44_100);
    let res = delivered_result(&truncated);

    let verdict = worker
        .verify_delivery_before_complete(
            queue_id,
            &res,
            expected_delivery(200_000),
            true,
            false,
            0.0,
        )
        .await;
    let reason = verdict.expect_err("el truncado debe fallar el gate");
    assert!(
        reason.contains("duration out of tolerance"),
        "motivo: {reason}"
    );

    // La fila sigue `downloading` y el ledger no se ha escrito: el worker no
    // marca `complete` tras un gate en fallo.
    assert_eq!(queue_status(&pool, queue_id).await, "downloading");
    assert!(ledger_row(&pool, track_id).await.is_none(), "sin ledger");
}

/// AC «Tags o portada inválidos no se marcan `complete`».
#[tokio::test]
async fn invalid_tags_or_cover_fail_the_gate() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "tags.db").await;
    let (queue_id, _track_id) = seed_downloading_row(&pool, &dir, 1, "hires").await;
    let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));

    // Fichero sin tags: título, artista y álbum faltan; portada sin pedir.
    let bare = dir.path().join("bare.flac");
    write_verified_flac(&bare, 200 * 44_100);
    let mut tag = metaflac::Tag::read_from_path(&bare).unwrap();
    for key in ["TITLE", "ARTIST", "ALBUM", "TRACKNUMBER", "DISCNUMBER"] {
        tag.vorbis_comments_mut().remove(key);
    }
    tag.remove_picture_type(metaflac::block::PictureType::CoverFront);
    tag.write_to_path(&bare).unwrap();
    let res = delivered_result(&bare);

    let reason = worker
        .verify_delivery_before_complete(
            queue_id,
            &res,
            expected_delivery(200_000),
            false,
            false,
            0.0,
        )
        .await
        .expect_err("sin tags el gate debe fallar");
    assert!(reason.contains("tag 'title' missing"), "motivo: {reason}");
    assert!(reason.contains("tag 'artist' missing"), "motivo: {reason}");

    // Portada corrupta (sin magia de imagen) con incrustación pedida: fallo.
    let bad_cover = dir.path().join("bad_cover.flac");
    std::fs::copy(&bare, &bad_cover).unwrap();
    let mut tag = metaflac::Tag::read_from_path(&bad_cover).unwrap();
    let vc = tag.vorbis_comments_mut();
    vc.set("TITLE", vec!["Verified Track"]);
    vc.set("ARTIST", vec!["Verified Artist"]);
    vc.set("ALBUM", vec!["Verified Album"]);
    vc.set("TRACKNUMBER", vec!["3"]);
    vc.set("DISCNUMBER", vec!["1"]);
    tag.remove_picture_type(metaflac::block::PictureType::CoverFront);
    tag.add_picture(
        "image/jpeg",
        metaflac::block::PictureType::CoverFront,
        b"GIF89a-not-an-image".to_vec(),
    );
    tag.write_to_path(&bad_cover).unwrap();

    let reason = worker
        .verify_delivery_before_complete(
            queue_id,
            &delivered_result(&bad_cover),
            expected_delivery(200_000),
            true,
            false,
            0.0,
        )
        .await
        .expect_err("portada inválida con embed_artwork debe fallar");
    assert!(reason.contains("cover art missing"), "motivo: {reason}");

    // La misma portada inválida sin pedir incrustación no falla por portada.
    let verdict = worker
        .verify_delivery_before_complete(
            queue_id,
            &delivered_result(&bad_cover),
            expected_delivery(200_000),
            false,
            false,
            0.0,
        )
        .await;
    assert!(
        verdict.is_ok(),
        "sin embed_artwork la portada no se exige: {:?}",
        verdict.err()
    );
}

/// AC «Calidad pedida no disponible con `allow_fallback=false` falla»: el
/// gate rechaza la entrega física por debajo de lo pedido bajo política
/// estricta. El ajuste `fallback_action='skip'` alimenta el mismo flag.
#[tokio::test]
async fn strict_policy_rejects_shortfall_before_complete() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "strict.db").await;
    // allow_fallback=0: la fila bloqueada es política estricta por sí misma.
    let (queue_id, track_id) = seed_downloading_row(&pool, &dir, 0, "hires").await;
    let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));

    let file = dir.path().join("delivered.flac");
    let res = delivered_result(&file);
    // El fichero entregado es FLAC CD (16/44.1) y se pidió hires.
    let reason = worker
        .verify_delivery_before_complete(
            queue_id,
            &res,
            expected_delivery(200_000),
            true,
            true, // política estricta: fallback_action='skip' o job sin fallback
            0.0,
        )
        .await
        .expect_err("shortfall bajo política estricta debe rechazar");
    assert!(
        reason.contains("RejectedQuality"),
        "el rechazo es de calidad: {reason}"
    );
    assert!(reason.contains("hires"), "declara lo pedido: {reason}");
    assert!(queue_status(&pool, queue_id).await == "downloading");
    assert!(ledger_row(&pool, track_id).await.is_none(), "sin ledger");

    // El mismo gate sin política estricta deja pasar la entrega: mark_complete
    // completará y declarará la caída (ver test siguiente).
    let verdict = worker
        .verify_delivery_before_complete(
            queue_id,
            &res,
            expected_delivery(200_000),
            true,
            false,
            0.0,
        )
        .await;
    assert!(verdict.is_ok(), "{:?}", verdict.err());
}

/// AC «con `true` completa y declara la caída» y «El ledger guarda la calidad
/// física, no la anunciada»: shortfall declarado con `quality_fallback_used`
/// y métricas del fichero, no del proveedor.
#[tokio::test]
async fn shortfall_completes_declared_with_physical_quality_in_ledger() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "shortfall.db").await;
    let (queue_id, track_id) = seed_downloading_row(&pool, &dir, 1, "hires").await;
    let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));

    let file = dir.path().join("delivered.flac");
    // El proveedor ANUNCIA 24/96 pero el fichero es FLAC CD 16/44.1.
    let res = delivered_result(&file);
    worker.mark_complete(queue_id, &res).await;

    assert_eq!(queue_status(&pool, queue_id).await, "complete");
    let decision: (String, Option<i64>, Option<String>) = sqlx::query_as(
        "SELECT quality_decision, quality_fallback_used, decision_reason FROM download_queue WHERE id = ?",
    )
    .bind(queue_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        decision.0, "CompletedWithQualityShortfall",
        "la caída hires→CD se declara"
    );
    assert_eq!(decision.1, Some(1), "quality_fallback_used = 1");
    let reason = decision.2.expect("el shortfall declara el motivo");
    assert!(reason.contains("Quality shortfall"), "motivo: {reason}");

    // El ledger guarda la calidad FÍSICA (16-bit/44.1kHz), no la anunciada.
    let ledger = ledger_row(&pool, track_id).await.expect("ledger escrito");
    assert!(
        ledger.0.contains("16-bit"),
        "calidad física, no anunciada: {}",
        ledger.0
    );
    assert_eq!(ledger.1, 16, "bit_depth físico");
    assert_eq!(ledger.2, 44_100, "sample_rate físico");
}

/// AC «`downloads.account_id` coincide con la cuenta del `ProviderDelivery`».
#[tokio::test]
async fn ledger_accounts_the_delivering_account() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "account.db").await;
    let (queue_id, track_id) = seed_downloading_row(&pool, &dir, 1, "lossless").await;
    let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));

    // La entrega declara la cuenta 7 (la cuenta que SIRVIÓ, puede diferir de la
    // resuelta 9 de la seed).
    let file = dir.path().join("delivered.flac");
    let res = delivered_result(&file);
    assert_eq!(res.account_id, Some(7));
    worker.mark_complete(queue_id, &res).await;

    let ledger = ledger_row(&pool, track_id).await.expect("ledger escrito");
    assert_eq!(ledger.3, Some(7), "la cuenta de la entrega manda");

    // Sin cuenta en la entrega, el `resolved_account_id` de la fila es el
    // fallback (rutas que no pasan por el gateway). La fila vuelve a
    // `downloading` porque `mark_complete` solo promociona filas que posee.
    sqlx::query("DELETE FROM downloads WHERE track_id = ?")
        .bind(track_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE download_queue SET status = 'downloading' WHERE id = ?")
        .bind(queue_id)
        .execute(&pool)
        .await
        .unwrap();
    let mut res_without_account = delivered_result(&file);
    res_without_account.account_id = None;
    worker.mark_complete(queue_id, &res_without_account).await;
    let ledger = ledger_row(&pool, track_id).await.expect("ledger escrito");
    assert_eq!(ledger.3, Some(9), "fallback al resolved_account_id");
}

/// Una decisión `RejectedQuality` de la política compartida no se promociona
/// a `complete` ni escribe el ledger (defensa en `mark_complete`).
#[tokio::test]
async fn rejected_quality_decision_never_completes() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "rejected.db").await;
    let (queue_id, track_id) = seed_downloading_row(&pool, &dir, 1, "lossless").await;
    let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));

    let file = dir.path().join("delivered.flac");
    let mut res = delivered_result(&file);
    res.quality_decision = Some(
        syncify_core_domain::quality::QualityPolicy::evaluate_stream_resolution(
            "lossless",
            "MP3 128kbps",
            "MP3",
            16,
            44_100.0,
            "qobuz",
            "qobuz",
            true,
            false,
            320,
        ),
    );
    assert!(res.quality_decision.as_ref().unwrap().reason.is_some());
    worker.mark_complete(queue_id, &res).await;

    assert_eq!(queue_status(&pool, queue_id).await, "failed");
    let error: String = sqlx::query_scalar("SELECT error_message FROM download_queue WHERE id = ?")
        .bind(queue_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        error.contains("RejectedQuality"),
        "el fallo declara el rechazo de calidad: {error}"
    );
    assert!(ledger_row(&pool, track_id).await.is_none(), "sin ledger");
}

/// AC «SoundCloud 128 se acepta y se declara; nunca se etiqueta lossless»:
/// el piso por proveedor consume la matriz, y un MP3 128 es lossy.
#[tokio::test]
async fn soundcloud_128_is_accepted_and_never_lossless() {
    // La matriz del piso por proveedor.
    assert_eq!(provider_lossy_floor_kbps("soundcloud", false), 128);
    // 128 kbps está EN el piso de SoundCloud, no por debajo.
    assert!(!is_below_provider_floor(
        "soundcloud",
        false,
        Some(128),
        None
    ));
    assert!(is_below_provider_floor("soundcloud", false, Some(96), None));
    // Un MP3 128 jamás se etiqueta lossless, ni en la clasificación física
    // ni en el tier canónico.
    assert_eq!(
        syncify_tauri_lib::download::audio_inspector::classify_physical_audio_quality(
            16, 44_100, "MP3"
        ),
        "lossy"
    );
    assert_eq!(
        classify_audio_tier(None, None, Some(128), Some("MP3")),
        AudioTier::Lossy
    );

    // La política compartida acepta el MP3 128 de SoundCloud como fallback
    // declarado cuando el piso del proveedor manda (el test de dominio cubre
    // el rechazo con el piso 320 legacy).
    let decision = syncify_core_domain::quality::QualityPolicy::evaluate_stream_resolution(
        "lossless",
        "MP3 128kbps",
        "MP3",
        16,
        44_100.0,
        "soundcloud",
        "soundcloud",
        false,
        true,
        provider_lossy_floor_kbps("soundcloud", false),
    );
    assert_eq!(
        decision.decision,
        syncify_core_domain::quality::QualityDecisionKind::CompletedWithQualityFallback
    );
    assert!(decision.quality_fallback_used);
}

/// La cuenta del `ProviderDelivery` viaja dentro del `DownloadResult` que
/// conservan los caminos del orquestador que solo se quedan `.result`.
#[test]
fn provider_delivery_account_propagates_into_result() {
    use syncify_tauri_lib::download::provider_gateway::ProviderDelivery;

    let dir = TempDir::new().unwrap();
    let file = dir.path().join("delivered.flac");
    write_verified_flac(&file, 200 * 44_100);
    let result = DownloadResult {
        file_path: file.to_string_lossy().to_string(),
        service: "qobuz".to_string(),
        ..Default::default()
    };
    let delivery = ProviderDelivery::from_result(result, Some(7));
    assert_eq!(delivery.account_id, Some(7));
    assert_eq!(
        delivery.result.account_id,
        Some(7),
        "el fallback que solo conserva .result no pierde la cuenta"
    );
    // Una cuenta ya declarada por el adaptador nunca se pisa.
    let owned = DownloadResult {
        file_path: file.to_string_lossy().to_string(),
        service: "qobuz".to_string(),
        account_id: Some(3),
        ..Default::default()
    };
    let delivery = ProviderDelivery::from_result(owned, Some(7));
    assert_eq!(delivery.result.account_id, Some(3));
}

/// #338: el orden gate → `mark_complete` se verifica por comportamiento, no
/// buscando cadenas en el fuente. `verify_then_complete` es la misma costura
/// que usa `process_download_internal`: un rechazo del gate deja la fila en
/// `downloading`, sin ledger, y borra el fichero promovido sin dueño; una
/// entrega válida sí promociona.
#[tokio::test]
async fn gate_runs_before_mark_complete_and_cleans_rejected_promotion() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "order.db").await;
    let (queue_id, track_id) = seed_downloading_row(&pool, &dir, 1, "hires").await;
    let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));

    // Físico 197 s frente a catálogo 200 s: el gate rechaza.
    let rejected = dir.path().join("rejected.flac");
    write_verified_flac(&rejected, 197 * 44_100);
    let reason = worker
        .verify_then_complete(
            queue_id,
            &delivered_result(&rejected),
            expected_delivery(200_000),
            true,
            false,
            0.0,
        )
        .await
        .expect_err("el gate debe rechazar antes de mark_complete");
    assert!(
        reason.contains("duration out of tolerance"),
        "motivo: {reason}"
    );
    assert_eq!(
        queue_status(&pool, queue_id).await,
        "downloading",
        "un rechazo no promociona la fila"
    );
    assert!(ledger_row(&pool, track_id).await.is_none(), "sin ledger");
    assert!(
        !rejected.exists(),
        "el fichero promovido rechazado no puede quedar sin dueño"
    );

    // La misma costura con una entrega válida promociona y escribe el ledger.
    let accepted = dir.path().join("accepted.flac");
    write_verified_flac(&accepted, 200 * 44_100);
    worker
        .verify_then_complete(
            queue_id,
            &delivered_result(&accepted),
            expected_delivery(200_000),
            true,
            false,
            0.0,
        )
        .await
        .expect("una entrega verificada completa");
    assert_eq!(queue_status(&pool, queue_id).await, "complete");
    assert!(
        ledger_row(&pool, track_id).await.is_some(),
        "ledger escrito"
    );
}

/// #338 AC: un FLAC o M4A no vacío que el lector no puede parsear no se marca
/// `complete` (antes el gate lo trataba como «formato sin lector»).
#[tokio::test]
async fn unreadable_flac_and_m4a_never_complete() {
    let dir = TempDir::new().unwrap();

    for (name, bytes) in [
        ("corrupt.flac", b"fLaC-not-a-parseable-tag-chain".as_slice()),
        ("corrupt.m4a", b"not-an-mp4-container".as_slice()),
    ] {
        let pool = migrated_pool(&dir, &format!("unreadable-{name}.db")).await;
        let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));
        let (queue_id, track_id) = seed_downloading_row(&pool, &dir, 1, "lossless").await;
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();

        let reason = worker
            .verify_then_complete(
                queue_id,
                &delivered_result(&path),
                expected_delivery(200_000),
                false,
                false,
                0.0,
            )
            .await
            .expect_err("un fichero ilegible con lector debe fallar cerrado");
        assert!(
            reason.contains("tags unreadable in a format that declares a reader"),
            "{name}: motivo: {reason}"
        );
        assert_eq!(queue_status(&pool, queue_id).await, "downloading");
        assert!(
            ledger_row(&pool, track_id).await.is_none(),
            "{name}: sin ledger"
        );
        assert!(!path.exists(), "{name}: sin fichero huérfano");
    }
}

/// FLAC con bloque de tags correcto pero sin STREAMINFO: los tags se dejan
/// leer, la cabecera de audio no. `inspect_physical_audio_file` no puede dar
/// métricas de este fichero.
fn write_tagged_flac_without_streaminfo(path: &Path) {
    let mut tag = metaflac::Tag::new();
    let vc = tag.vorbis_comments_mut();
    vc.set("TITLE", vec!["Verified Track"]);
    vc.set("ARTIST", vec!["Verified Artist"]);
    vc.set("ALBUM", vec!["Verified Album"]);
    vc.set("TRACKNUMBER", vec!["3"]);
    vc.set("DISCNUMBER", vec!["1"]);
    tag.write_to_path(path).unwrap();
    assert!(
        metaflac::Tag::read_from_path(path)
            .unwrap()
            .get_streaminfo()
            .is_none(),
        "el fixture no debe traer STREAMINFO"
    );
}

/// #338 AC: bajo política estricta, una reinspección física fallida NO se
/// sustituye por la calidad anunciada por el proveedor. Antes del cierre, el
/// gate caía a `bit_depth`/`sample_rate` declarados (24-bit/96 kHz → hires) y
/// un fichero sin métricas físicas se promocionaba como pérdida.
#[tokio::test]
async fn strict_policy_never_trusts_declared_values_when_inspection_fails() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "strict-inspection.db").await;
    let (queue_id, track_id) = seed_downloading_row(&pool, &dir, 0, "lossless").await;
    let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));

    let path = dir.path().join("no-streaminfo.flac");
    write_tagged_flac_without_streaminfo(&path);
    let res = delivered_result(&path);

    // Sin política estricta, la falta de métricas físicas sigue siendo falta
    // de evidencia: el gate físico no inventa un fallo.
    worker
        .verify_delivery_before_complete(
            queue_id,
            &res,
            expected_delivery(200_000),
            false,
            false,
            0.0,
        )
        .await
        .expect("sin strict, la falta de métricas no es un fallo");

    let reason = worker
        .verify_delivery_before_complete(
            queue_id,
            &res,
            expected_delivery(200_000),
            false,
            true,
            0.0,
        )
        .await
        .expect_err("la política estricta no puede confiar en lo anunciado");
    assert!(
        reason.contains("strict quality policy does not trust declared values"),
        "motivo: {reason}"
    );
    assert_eq!(queue_status(&pool, queue_id).await, "downloading");
    assert!(ledger_row(&pool, track_id).await.is_none(), "sin ledger");
}

/// #338: la limpieza no destruye una entrega ajena. Si el ledger ya posee el
/// path (otra pista lo ocupa), el rechazo conserva el fichero; solo se borra
/// cuando ninguna fila lo referencia.
#[tokio::test]
async fn rejected_delivery_keeps_a_path_the_ledger_already_owns() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "owned-path.db").await;
    let (queue_id, track_id) = seed_downloading_row(&pool, &dir, 1, "hires").await;
    let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));

    let path = dir.path().join("owned.flac");
    write_verified_flac(&path, 197 * 44_100);
    let other_track: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Other Track') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_format, bit_depth, sample_rate) \
         VALUES (?, ?, 'FLAC', 16, 44100)",
    )
    .bind(other_track)
    .bind(path.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    let reason = worker
        .verify_then_complete(
            queue_id,
            &delivered_result(&path),
            expected_delivery(200_000),
            true,
            false,
            0.0,
        )
        .await
        .expect_err("el gate rechaza la duración fuera de tolerancia");
    assert!(
        reason.contains("duration out of tolerance"),
        "motivo: {reason}"
    );
    assert!(
        path.exists(),
        "el path ya tiene dueño en el ledger: la limpieza no debe borrarlo"
    );
    assert!(
        ledger_row(&pool, track_id).await.is_none(),
        "sin ledger propio"
    );
}

/// #338 (revisión de #356): la ruta real de Tidal escribe la fila del ledger
/// en `tidal_pipeline` §9 y hace commit ANTES de que el worker ejecute el
/// gate. Esa fila es de la propia entrega (mismo `track_id` que la cola), no
/// de otra pista: si el gate la rechaza, la fila y el fichero no pueden quedar
/// en la biblioteca mientras la cola se marca `failed`.
#[tokio::test]
async fn rejected_delivery_removes_the_pipeline_row_of_the_same_delivery() {
    let dir = TempDir::new().unwrap();
    let pool = migrated_pool(&dir, "pipeline-row.db").await;
    let (queue_id, track_id) = seed_downloading_row(&pool, &dir, 1, "hires").await;
    let worker = DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1));

    let path = dir.path().join("pipeline-owned.flac");
    write_verified_flac(&path, 197 * 44_100);

    // Fila tal como la escribe `tidal_pipeline` §9 (ON CONFLICT(track_id)), ya
    // confirmada antes del gate.
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, file_format, \
         bit_depth, sample_rate, file_size_bytes, metadata_completeness, requested_quality, \
         effective_quality, requested_format, effective_format, quality_decision, \
         provider_fallback_used, quality_fallback_used) \
         VALUES (?, 1, ?, 'FLAC', 24, 96000, 4096, 100, 'hires', \
         'FLAC 24-bit / 96.0kHz', 'FLAC', 'FLAC', 'CompletedExactQuality', 0, 0)",
    )
    .bind(track_id)
    .bind(path.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        ledger_row(&pool, track_id).await.is_some(),
        "fixture: la fila de la propia entrega ya existe cuando corre el gate"
    );

    let reason = worker
        .verify_then_complete(
            queue_id,
            &delivered_result(&path),
            expected_delivery(200_000),
            true,
            false,
            0.0,
        )
        .await
        .expect_err("el gate rechaza la duración fuera de tolerancia");
    assert!(
        reason.contains("duration out of tolerance"),
        "motivo: {reason}"
    );

    assert!(
        ledger_row(&pool, track_id).await.is_none(),
        "la fila escrita por la propia entrega no puede sobrevivir al rechazo"
    );
    assert!(
        !path.exists(),
        "el fichero rechazado no puede quedar en la ruta de la biblioteca"
    );
    assert_eq!(
        quarantined_files(dir.path()).len(),
        1,
        "el audio rechazado se conserva en cuarentena explícita, no se destruye"
    );
    assert_eq!(
        queue_status(&pool, queue_id).await,
        "downloading",
        "el gate no promociona la fila; el llamador la marca failed"
    );
}

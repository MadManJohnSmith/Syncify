//! Issue #87 — integración del bloqueo por sesión con el sync y las descargas
//! (T15 del spec #64). Contrato: ADR-0007, decisiones 4 y 12.
//!
//! Un 401/403 marca SOLO la cuenta afectada (`mark_account_credentials_invalid`)
//! y la pausa se aplica en producción por dos puertas: el gate de sync
//! (`ensure_account_can_sync`) y el claim de descargas (`DownloadWorker`).
//! La reautenticación — el camino real de re-login `upsert_service_account` —
//! limpia la marca y el trabajo se reanuda solo, sin tocar biblioteca ni
//! fuentes. Contra SQLite `:memory:` con las migraciones del árbol; sin red.
//!
//! El mismo fichero ejercita la restauración del perfil cifrado del re-login
//! (`restore_profile_for_reauth`, API dejada por #86): el mismo webview al
//! reautenticar, y un archivo ilegible deja la cuenta «sin sesión» sin
//! bloquear el login.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::commands::account_profiles::{
    archive_profile_dir_to_file, restore_profile_for_reauth, PROFILE_STATUS_UNRECOVERABLE,
};
use syncify_tauri_lib::commands::accounts::{
    ensure_account_can_sync, mark_account_credentials_invalid,
};
use syncify_tauri_lib::commands::auth::upsert_service_account;
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::worker::{DownloadWorker, DownloadWorkerState};

async fn migrated_pool() -> SqlitePool {
    let _ = crypto::init_crypto([42u8; 32]);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations apply");
    pool
}

/// Cuenta qobuz sobre el esquema migrado; `email` la hace direccionable por
/// el upsert de re-login. Devuelve (account_id, service_id).
async fn seed_account(pool: &SqlitePool, label: &str, email: &str) -> (i64, i64) {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(pool)
        .await
        .expect("servicio qobuz sembrado por migraciones");
    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, email) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(service_id)
    .bind(label)
    .bind(email)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada");
    (account_id, service_id)
}

async fn credential_flags(pool: &SqlitePool, account_id: i64) -> (i64, Option<String>) {
    sqlx::query_as("SELECT credentials_invalid, invalid_reason FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(pool)
        .await
        .expect("cuenta existe")
}

async fn seed_queue_item(pool: &SqlitePool, service_name: &str) -> i64 {
    let track_id: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T87') RETURNING id")
            .fetch_one(pool)
            .await
            .expect("track sembrado");
    sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, service_name, status) VALUES (?, ?, 'queued') RETURNING id",
    )
    .bind(track_id)
    .bind(service_name)
    .fetch_one(pool)
    .await
    .expect("fila de cola sembrada")
}

fn worker(pool: &SqlitePool) -> DownloadWorker {
    DownloadWorker::new(pool.clone(), DownloadWorkerState::new(1))
}

async fn queue_status(pool: &SqlitePool, queue_id: i64) -> String {
    sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(queue_id)
        .fetch_one(pool)
        .await
        .expect("fila sigue ahí")
}

/// Criterio 1 y 3: el 401/403 marca solo la cuenta afectada, su sync queda
/// pausado (gate) y sus descargas dejan de reclamarse (claim) mientras la
/// otra cuenta sana del servicio sigue trabajando. El re-login por el camino
/// real de producción limpia la marca y el trabajo se reanuda solo.
#[tokio::test]
async fn credencial_marcada_pausa_sync_y_descargas_y_el_relogin_reanuda() {
    let pool = migrated_pool().await;
    let (account_a, service_id) = seed_account(&pool, "Cuenta A", "a@test.example").await;
    let (account_b, _) = seed_account(&pool, "Cuenta B", "b@test.example").await;

    // Antes del 401: ambas cuentas sincronizan.
    ensure_account_can_sync(&pool, account_a)
        .await
        .expect("cuenta sana sincroniza");

    // El 401 de A marca SOLO a A (historia 21 del spec #83).
    let marked =
        mark_account_credentials_invalid(&pool, "qobuz", "HTTP 401 Unauthorized", Some(account_a))
            .await
            .expect("marca la cuenta afectada");
    assert_eq!(marked, 1, "solo la cuenta del account_id dado se marca");
    let (invalid_a, reason_a) = credential_flags(&pool, account_a).await;
    assert_eq!(invalid_a, 1);
    // `invalid_reason` es el texto saneado que ve el usuario (contrato de
    // mark_account_credentials_invalid); los códigos canónicos como
    // `token_expired` los escriben los caminos que ya conocen la taxonomía.
    assert_eq!(reason_a.as_deref(), Some("HTTP 401 Unauthorized"));
    let (invalid_b, _) = credential_flags(&pool, account_b).await;
    assert_eq!(invalid_b, 0, "la cuenta hermana no se toca");

    // Gate de sync: A queda pausada, B sigue.
    let err = ensure_account_can_sync(&pool, account_a)
        .await
        .expect_err("la cuenta marcada no sincroniza");
    assert!(
        err.contains("AccountAuthExpired") && err.contains("reauthenticate"),
        "el error pide reautenticar, no un alta nueva: {}",
        err
    );
    ensure_account_can_sync(&pool, account_b)
        .await
        .expect("la hermana sana sigue sincronizando");

    // Descargas: con B sana, una fila del servicio se reclama con normalidad.
    seed_queue_item(&pool, "qobuz").await;
    let claimed = worker(&pool).claim_next_item().await;
    assert!(claimed.is_some(), "una cuenta sana del servicio descarga");

    // Otra fila aún encolada: marcando también B, el servicio entero queda en
    // pausa y el claim NO quema un intento por fila (criterio 4).
    let queue_id = seed_queue_item(&pool, "qobuz").await;
    let marked_b =
        mark_account_credentials_invalid(&pool, "qobuz", "HTTP 403 Forbidden", Some(account_b))
            .await
            .expect("marca la segunda cuenta");
    assert_eq!(marked_b, 1, "B marcada");
    assert_eq!(credential_flags(&pool, account_b).await.0, 1, "B inválida");
    let claimed = worker(&pool).claim_next_item().await;
    assert!(
        claimed.is_none(),
        "sin cuentas válidas el servicio no se reclama"
    );
    assert_eq!(queue_status(&pool, queue_id).await, "queued");

    // Re-login de A por el camino real de producción (upsert por identidad):
    // limpia la marca y, con una cuenta válida de vuelta, el claim reabre.
    let creds_a = crypto::encrypt(r#"{"user_auth_token":"fresh-a"}"#).expect("cifra credenciales");
    let relogged = upsert_service_account(
        &pool,
        service_id,
        "Cuenta A",
        Some("a@test.example"),
        None,
        &creds_a,
        None,
    )
    .await
    .expect("el re-login actualiza la cuenta existente");
    assert_eq!(relogged, account_a, "el re-login no duplica la cuenta");
    let (invalid_a, reason_a) = credential_flags(&pool, account_a).await;
    assert_eq!(invalid_a, 0, "el re-login limpia la marca");
    assert!(reason_a.is_none());
    ensure_account_can_sync(&pool, account_a)
        .await
        .expect("reautenticar reabre el sync");
    let claimed = worker(&pool)
        .claim_next_item()
        .await
        .expect("reautenticar reanuda las descargas");
    assert_eq!(claimed.0, queue_id, "la fila pausada es la reclamada");
}

/// 5xx o fallo de red NO es credencial inválida (decisión 12): sin marca en
/// la cuenta, ninguna puerta de sesión se cierra. La clasificación Retryable
/// del sync (hueco registrado, corrida parcial) vive en
/// `sync_run_job_fake_source_test`; aquí se fija el efecto en las puertas.
#[tokio::test]
async fn fallo_de_red_no_marca_ni_pausa() {
    let pool = migrated_pool().await;
    let (account_id, _) = seed_account(&pool, "Cuenta A", "a@test.example").await;

    let (invalid, reason) = credential_flags(&pool, account_id).await;
    assert_eq!(invalid, 0);
    assert!(reason.is_none());
    ensure_account_can_sync(&pool, account_id)
        .await
        .expect("un fallo de red no pausa el sync");
    seed_queue_item(&pool, "qobuz").await;
    assert!(
        worker(&pool).claim_next_item().await.is_some(),
        "un fallo de red no pausa las descargas"
    );
}

/// Criterio 3, re-login con el mismo webview: el perfil cifrado de la cuenta
/// (API de #86) se restaura en el directorio provisional del intento.
#[tokio::test]
async fn el_relogin_restaura_el_perfil_cifrado_de_la_cuenta() {
    let pool = migrated_pool().await;
    let (account_id, service_id) = seed_account(&pool, "Con perfil", "p@test.example").await;

    // Perfil cifrado en reposo: un directorio con un fichero se archiva al
    // contenedor (camino real de producción, archive_attempt_profile).
    let tmp = tempfile::tempdir().expect("temp dir");
    let profile_dir = tmp.path().join("profile");
    std::fs::create_dir_all(&profile_dir).expect("profile dir");
    std::fs::write(profile_dir.join("Cookies"), "cookie-jar-v1").expect("cookie jar");
    let archive_path = tmp.path().join("perfil.syncprof");
    archive_profile_dir_to_file(&profile_dir, &archive_path, &crypto::key().unwrap())
        .expect("archiva el perfil");
    sqlx::query(
        "INSERT INTO account_profiles (account_id, service_id, profile_key, archive_path)
         VALUES (?, ?, 'profkey87', ?)",
    )
    .bind(account_id)
    .bind(service_id)
    .bind(archive_path.to_string_lossy().as_ref())
    .execute(&pool)
    .await
    .expect("fila de perfil");

    // Restauración al directorio provisional de un intento: el mismo webview.
    let dest = tmp.path().join("attempt");
    let restored = restore_profile_for_reauth(&pool, account_id, &dest)
        .await
        .expect("restaura el perfil archived");
    assert!(restored.is_some());
    assert_eq!(
        std::fs::read_to_string(dest.join("Cookies")).expect("cookie restaurada"),
        "cookie-jar-v1"
    );

    // Una cuenta sin perfil 'archived' (previa a 0111) no restaura nada:
    // el intento sigue con un webview vacío.
    let (no_profile, _) = seed_account(&pool, "Sin perfil", "np@test.example").await;
    let dest_empty = tmp.path().join("attempt-empty");
    let restored = restore_profile_for_reauth(&pool, no_profile, &dest_empty)
        .await
        .expect("cuenta sin perfil no es error");
    assert!(restored.is_none());
    assert!(!dest_empty.join("Cookies").exists());
}

/// Un archivo que no descifra (clave perdida, fichero dañado — decisión 10)
/// NO bloquea el login: el helper marca el perfil 'unrecoverable', deja la
/// cuenta «sin sesión» y el error sube para que el intento continúe vacío.
#[tokio::test]
async fn perfil_ilegible_marca_sin_sesion_y_no_bloquea_el_login() {
    let pool = migrated_pool().await;
    let (account_id, service_id) = seed_account(&pool, "Dañado", "d@test.example").await;

    let tmp = tempfile::tempdir().expect("temp dir");
    let archive_path = tmp.path().join("roto.syncprof");
    std::fs::write(&archive_path, b"SYNCPROF1-corrupted-payload").expect("escribe archivo roto");
    let profile_id: i64 = sqlx::query_scalar(
        "INSERT INTO account_profiles (account_id, service_id, profile_key, archive_path)
         VALUES (?, ?, 'roto87', ?) RETURNING id",
    )
    .bind(account_id)
    .bind(service_id)
    .bind(archive_path.to_string_lossy().as_ref())
    .fetch_one(&pool)
    .await
    .expect("fila de perfil");

    let dest = tmp.path().join("attempt");
    let err = restore_profile_for_reauth(&pool, account_id, &dest)
        .await
        .expect_err("un archivo que no descifra es error");
    assert!(!err.is_empty());

    // El perfil queda 'unrecoverable' y la cuenta «sin sesión» (ficha que
    // ofrece reautenticar), pero la fila y la cuenta siguen existiendo.
    let (status,): (String,) = sqlx::query_as("SELECT status FROM account_profiles WHERE id = ?")
        .bind(profile_id)
        .fetch_one(&pool)
        .await
        .expect("fila de perfil");
    assert_eq!(status, PROFILE_STATUS_UNRECOVERABLE);
    let (invalid, reason) = credential_flags(&pool, account_id).await;
    assert_eq!(invalid, 1);
    assert_eq!(reason.as_deref(), Some("profile_unreadable"));

    // Reintento tras la marca: ya no hay fila 'archived' → Ok(None), el
    // intento de login continúa con un webview vacío.
    let again = restore_profile_for_reauth(&pool, account_id, &dest)
        .await
        .expect("el reintento no es error");
    assert!(again.is_none());
}

/// La reautenticación nunca toca biblioteca ni fuentes (criterio 3): el
/// re-login actualiza la fila de la cuenta y las entradas siguen ahí.
#[tokio::test]
async fn el_relogin_conserva_la_biblioteca() {
    let pool = migrated_pool().await;
    let (account_id, service_id) = seed_account(&pool, "Cuenta A", "a@test.example").await;

    let track_id: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('T', 'ISRC87') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("track");
    sqlx::query("INSERT INTO library_entries (account_id, track_id) VALUES (?, ?)")
        .bind(account_id)
        .bind(track_id)
        .execute(&pool)
        .await
        .expect("entrada de biblioteca");

    // Ciclo completo: 401 → marca → re-login.
    mark_account_credentials_invalid(&pool, "qobuz", "HTTP 401 Unauthorized", Some(account_id))
        .await
        .expect("marca");
    let creds = crypto::encrypt(r#"{"user_auth_token":"fresh"}"#).expect("cifra");
    let relogged = upsert_service_account(
        &pool,
        service_id,
        "Cuenta A",
        Some("a@test.example"),
        None,
        &creds,
        None,
    )
    .await
    .expect("re-login");
    assert_eq!(relogged, account_id);

    let entries: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("entradas");
    assert_eq!(entries, 1, "la biblioteca sobrevive al ciclo completo");
}

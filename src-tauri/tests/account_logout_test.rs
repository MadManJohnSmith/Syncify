//! Issue #91 — cerrar sesión: borrar credenciales y perfil, conservar cuenta y
//! biblioteca.
//!
//! Contrato: ADR-0007 decisión 9. Bases `:memory:` completamente migradas y
//! perfiles de webview falsos en directorios temporales. Cubre los criterios:
//! el logout borra credencial y perfil, conserva cuenta/pistas/pertenencias y
//! deja la cuenta «sin sesión» sin sync ni descargas; reautenticar reutiliza la
//! misma fila sin duplicar ni perder datos; desactivar y archivar conservan el
//! perfil y solo el logout lo borra.

use std::path::{Path, PathBuf};

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

use syncify_tauri_lib::commands::account_profiles::{
    archive_attempt_profile, profile_archive_path,
};
use syncify_tauri_lib::commands::accounts::{
    ensure_account_can_sync, get_service_fallback_account_ids, perform_logout_account,
    perform_set_account_state, ACCOUNT_STATE_ARCHIVED, ACCOUNT_STATE_DISABLED,
};
use syncify_tauri_lib::commands::auth::upsert_service_account;
use syncify_tauri_lib::crypto::{decrypt, encrypt, init_crypto};

const PASSWORD: &str = r#"{"user_auth_token":"token-de-prueba-a05"}"#;

async fn setup_db() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migraciones completas");
    pool
}

fn test_key() -> [u8; 32] {
    let key = [42u8; 32];
    let _ = init_crypto(key);
    key
}

async fn service_id(pool: &Pool<Sqlite>, service: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service)
        .fetch_one(pool)
        .await
        .expect("servicio sembrado por las migraciones")
}

/// Cuenta con credenciales cifradas reales y una identidad estable, para que
/// el re-login de `upsert_service_account` la reconozca.
async fn insert_account(pool: &Pool<Sqlite>, service_id: i64, tag: &str) -> i64 {
    let encrypted = encrypt(PASSWORD).expect("credenciales cifradas");
    sqlx::query_scalar(
        "INSERT INTO accounts
            (service_id, display_name, email, external_user_id, credentials_json, is_active)
         VALUES (?, ?, ?, ?, ?, 1) RETURNING id",
    )
    .bind(service_id)
    .bind(format!("Cuenta {}", tag))
    .bind(format!("{}@example.invalid", tag))
    .bind(format!("provider-user-{}", tag))
    .bind(encrypted)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada")
}

/// Filas que pertenecen a la cuenta y filas compartidas que NUNCA deben
/// desaparecer al cerrar sesión.
async fn insert_owned_rows(pool: &Pool<Sqlite>, account_id: i64, service_id: i64) -> i64 {
    let track_id: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Canon A05') RETURNING id")
            .fetch_one(pool)
            .await
            .expect("track sembrado");
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, 'src-a05')",
    )
    .bind(track_id)
    .bind(service_id)
    .execute(pool)
    .await
    .expect("track_source sembrado");
    sqlx::query("INSERT INTO library_entries (account_id, track_id) VALUES (?, ?)")
        .bind(account_id)
        .bind(track_id)
        .execute(pool)
        .await
        .expect("library_entry sembrado");
    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, name) VALUES (?, 'Mix A05') RETURNING id",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .expect("playlist sembrada");
    sqlx::query(
        "INSERT INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id)
         VALUES (?, ?, ?, 'pl-a05')",
    )
    .bind(playlist_id)
    .bind(account_id)
    .bind(service_id)
    .execute(pool)
    .await
    .expect("playlist_source sembrado");
    track_id
}

/// Perfil de webview falso cifrado al archivo real de `account_profiles`.
async fn attach_profile(
    pool: &Pool<Sqlite>,
    base: &Path,
    service_id: i64,
    account_id: i64,
    profile_key: &str,
) -> PathBuf {
    let profile_dir = base.join(format!("sess-{}", profile_key));
    std::fs::create_dir_all(&profile_dir).expect("directorio del webview falso");
    std::fs::write(
        profile_dir.join("Cookies"),
        format!("sqlite jar {}", profile_key),
    )
    .expect("cookie falsa");
    let archive_path = profile_archive_path(&base.join("profile-archives"), profile_key);
    archive_attempt_profile(
        pool,
        account_id,
        service_id,
        profile_key,
        &profile_dir,
        &archive_path,
    )
    .await
    .expect("perfil archivado y vinculado");
    archive_path
}

fn temp_base(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("syncify-a05-{}-{}", tag, uuid::Uuid::new_v4()))
}

#[tokio::test]
async fn cerrar_sesion_borra_credencial_y_perfil_y_conserva_cuenta_y_biblioteca() {
    let _ = test_key();
    let pool = setup_db().await;
    let service_id = service_id(&pool, "qobuz").await;
    let account_id = insert_account(&pool, service_id, "a05a").await;
    let track_id = insert_owned_rows(&pool, account_id, service_id).await;

    let base = temp_base("logout");
    let archive_path = attach_profile(&pool, &base, service_id, account_id, "key-a05a").await;
    assert!(archive_path.is_file(), "el perfil existe antes del logout");

    // Descarga en cola de la cuenta: deja de transferirse al cerrar sesión.
    sqlx::query(
        "INSERT INTO download_queue (track_id, status, resolved_account_id) VALUES (?, 'queued', ?)",
    )
    .bind(track_id)
    .bind(account_id)
    .execute(&pool)
    .await
    .expect("descarga encolada");

    let summary = perform_logout_account(&pool, account_id)
        .await
        .expect("cerrar sesión");

    assert_eq!(summary.account_id, account_id);
    assert_eq!(summary.service, "qobuz");
    assert_eq!(
        summary.state, "enabled",
        "cerrar sesión no cambia el ciclo de vida"
    );
    assert!(!summary.is_active);
    assert_eq!(
        summary.profiles_deleted, 1,
        "borra el perfil de ESTA cuenta"
    );
    assert!(!archive_path.is_file(), "el archivo cifrado desaparece");

    let profile_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM account_profiles WHERE account_id = ?")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("perfiles contados");
    assert_eq!(profile_rows, 0, "la fila del perfil también desaparece");

    // Credencial borrada y cuenta «sin sesión», pero la fila sigue.
    let row: (Option<String>, Option<String>, i64, Option<String>, i64, String) = sqlx::query_as(
        "SELECT credentials_json, credentials, credentials_invalid, invalid_reason, is_active, state
         FROM accounts WHERE id = ?",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .expect("la cuenta se conserva");
    assert!(row.0.is_none(), "credentials_json queda vacío");
    assert!(row.1.is_none(), "la columna legacy credentials queda vacía");
    assert_eq!(row.2, 1, "la cuenta queda marcada sin sesión");
    assert_eq!(row.3.as_deref(), Some("logged_out"));
    assert_eq!(row.4, 0, "deja de participar como cuenta activa");
    assert_eq!(row.5, "enabled", "el ciclo de vida no se toca");

    // Biblioteca y datos compartidos intactos.
    for (table, expected) in [
        ("library_entries", 1),
        ("playlists", 1),
        ("playlist_sources", 1),
        ("tracks", 1),
        ("track_sources", 1),
    ] {
        let rows: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {}", table))
            .fetch_one(&pool)
            .await
            .expect("tabla consultada");
        assert_eq!(rows, expected, "{} se conserva tras el logout", table);
    }

    // Descargas: la cola de la cuenta deja de estar viva.
    let queue_status: String =
        sqlx::query_scalar("SELECT status FROM download_queue WHERE resolved_account_id = ?")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("fila de cola consultada");
    assert_eq!(queue_status, "cancelled");

    // Sync y descargas: gate explícito y exclusión del fallback de descargas.
    let gate_error = ensure_account_can_sync(&pool, account_id)
        .await
        .expect_err("una cuenta sin sesión no sincroniza");
    assert!(
        gate_error.starts_with("AccountSessionClosed:"),
        "el error nombra el cierre de sesión: {}",
        gate_error
    );
    let fallback = get_service_fallback_account_ids(&pool, "qobuz", None)
        .await
        .expect("fallback de descargas");
    assert!(
        !fallback.contains(&account_id),
        "una cuenta sin sesión no recibe descargas: {:?}",
        fallback
    );

    std::fs::remove_dir_all(&base).ok();
}

#[tokio::test]
async fn reautenticar_tras_cerrar_sesion_reutiliza_la_cuenta_sin_perder_biblioteca() {
    let _ = test_key();
    let pool = setup_db().await;
    let service_id = service_id(&pool, "qobuz").await;
    let account_id = insert_account(&pool, service_id, "a05b").await;
    insert_owned_rows(&pool, account_id, service_id).await;

    perform_logout_account(&pool, account_id)
        .await
        .expect("cerrar sesión");
    assert!(
        ensure_account_can_sync(&pool, account_id).await.is_err(),
        "sin sesión no hay sync"
    );

    // Reautenticar: el login casa por identidad (email y user id) y actualiza
    // la MISMA fila, no inserta un duplicado.
    let encrypted = encrypt(PASSWORD).expect("credenciales cifradas");
    let reauthenticated = upsert_service_account(
        &pool,
        service_id,
        "Cuenta a05b",
        Some("A05B@example.invalid "),
        Some("provider-user-a05b"),
        &encrypted,
        None,
    )
    .await
    .expect("reautenticación");
    assert_eq!(
        reauthenticated, account_id,
        "la cuenta existente se recupera, no se duplica"
    );

    let accounts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ?")
        .bind(service_id)
        .fetch_one(&pool)
        .await
        .expect("cuentas contadas");
    assert_eq!(accounts, 1, "no hay cuenta duplicada");

    let row: (Option<String>, i64, Option<String>, i64) = sqlx::query_as(
        "SELECT credentials_json, credentials_invalid, invalid_reason, is_active
         FROM accounts WHERE id = ?",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .expect("cuenta consultada");
    assert!(row.0.is_some(), "la credencial nueva está guardada");
    assert_eq!(row.1, 0, "la marca de sin sesión desaparece");
    assert!(row.2.is_none(), "invalid_reason vuelve a NULL");
    assert_eq!(row.3, 1, "la cuenta vuelve a participar");
    assert_eq!(
        decrypt(&row.0.expect("credencial presente")).expect("descifra"),
        PASSWORD
    );

    // Biblioteca intacta y sync/descargas recuperados.
    let entries: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("entradas consultadas");
    assert_eq!(entries, 1, "la biblioteca sobrevive al ciclo logout/login");
    ensure_account_can_sync(&pool, account_id)
        .await
        .expect("tras reautenticar el gate vuelve a abrir");
    let fallback = get_service_fallback_account_ids(&pool, "qobuz", None)
        .await
        .expect("fallback de descargas");
    assert_eq!(
        fallback,
        vec![account_id],
        "la cuenta reautenticada vuelve a recibir descargas"
    );
}

#[tokio::test]
async fn desactivar_y_archivar_conservan_el_perfil_y_solo_el_logout_lo_borra() {
    let _ = test_key();
    let pool = setup_db().await;
    let service_id = service_id(&pool, "qobuz").await;
    let first = insert_account(&pool, service_id, "a05c").await;
    let second = insert_account(&pool, service_id, "a05d").await;

    let base = temp_base("ciclo");
    let first_archive = attach_profile(&pool, &base, service_id, first, "key-a05c").await;
    let second_archive = attach_profile(&pool, &base, service_id, second, "key-a05d").await;

    // Desactivar conserva el perfil.
    perform_set_account_state(&pool, first, ACCOUNT_STATE_DISABLED)
        .await
        .expect("desactivar");
    assert!(
        first_archive.is_file(),
        "desactivar NO borra el perfil cifrado"
    );
    // Archivar también.
    perform_set_account_state(&pool, first, ACCOUNT_STATE_ARCHIVED)
        .await
        .expect("archivar");
    assert!(
        first_archive.is_file(),
        "archivar NO borra el perfil cifrado"
    );
    let first_profiles: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM account_profiles WHERE account_id = ?")
            .bind(first)
            .fetch_one(&pool)
            .await
            .expect("perfil consultado");
    assert_eq!(first_profiles, 1, "la fila del perfil se conserva");

    // La otra cuenta, también archivada, conserva SU perfil.
    perform_set_account_state(&pool, second, ACCOUNT_STATE_ARCHIVED)
        .await
        .expect("archivar la segunda");
    assert!(second_archive.is_file());

    // Cerrar sesión es lo único que borra el perfil, y solo el de su cuenta.
    let summary = perform_logout_account(&pool, first)
        .await
        .expect("cerrar sesión");
    assert_eq!(summary.profiles_deleted, 1);
    assert!(!first_archive.is_file(), "el logout borra su perfil");
    assert!(
        second_archive.is_file(),
        "el perfil de la otra cuenta no se toca"
    );

    std::fs::remove_dir_all(&base).ok();
}

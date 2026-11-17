//! Issue #86 — perfiles de webview por cuenta: upgrade real de la migración
//! 0111 (`account_profiles`, `login_attempts`) y ciclo de vida del perfil
//! sobre BD real y directorios temporales.
//!
//! Mina de AGENTS: una BD `:memory:` recién migrada **no** demuestra que el
//! upgrade funcione. Molde de `account_lifecycle_migration_test` (#85): el
//! fixture ejecuta los ficheros REALES de migración 0001–0107 sobre una BD
//! limpia, siembra un perfil con su webview falso y después aplica 0111.
//!
//! Criterios cubiertos aquí (los de cifrado en reposo van en las pruebas de
//! unidad de `commands/account_profiles.rs`):
//! - Un perfil que no descifra pasa la cuenta a «sin sesión» y ofrece
//!   reautenticar (`mark_profile_unrecoverable`, higiene de arranque).
//! - Cerrar sesión y eliminar borran el archivo; desactivar y archivar lo
//!   conservan (`delete_profiles_for_account` solo es llamado por logout y
//!   borrado).
//! - Los temporales huérfanos de un cierre abrupto se limpian al arrancar.

use std::path::{Path, PathBuf};

use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};

use syncify_tauri_lib::commands::account_profiles::{
    archive_attempt_profile, delete_profiles_for_account, profile_archive_path,
    startup_profile_hygiene,
};
use syncify_tauri_lib::crypto::init_crypto;

const MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const NEW_VERSION: i64 = 111;

/// Aplica las migraciones reales hasta `max_version` (incluida), en orden.
async fn apply_migrations_through(pool: &Pool<Sqlite>, max_version: i64) {
    for migration in MIGRATOR.iter().filter(|m| m.version <= max_version) {
        sqlx::raw_sql(&migration.sql)
            .execute(pool)
            .await
            .unwrap_or_else(|e| panic!("migración {} falló: {}", migration.version, e));
    }
}

/// Clave del proceso de pruebas: `init_crypto` instancia una vez por proceso
/// (igual que en arranque) y absorbe la segunda llamada.
fn test_key() -> [u8; 32] {
    let key = [42u8; 32];
    let _ = init_crypto(key);
    key
}

/// Siembra el esquema vigente antes del upgrade: el servicio y una cuenta.
async fn seed_account(pool: &Pool<Sqlite>) -> (i64, i64) {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(pool)
        .await
        .expect("servicio qobuz sembrado por las migraciones");
    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'Con perfil', 1) RETURNING id",
    )
    .bind(service_id)
    .fetch_one(pool)
    .await
    .expect("cuenta sembrada");
    (service_id, account_id)
}

/// Webview falso: directorio de perfil con contenido inventado.
fn fake_webview_profile(base: &Path, name: &str, marker: &str) -> PathBuf {
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).expect("directorio del webview falso");
    std::fs::write(dir.join("Cookies"), format!("sqlite jar {}", marker))
        .expect("cookie falsa escrita");
    dir
}

fn temp_base(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("syncify-a86-{}-{}", tag, uuid::Uuid::new_v4()))
}

#[tokio::test]
async fn el_upgrade_a_0111_crea_las_tablas_de_perfiles_e_intentos() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");

    // 1. Base vigente: migraciones reales 0001–0107, sin tablas de perfil.
    apply_migrations_through(&pool, NEW_VERSION - 1).await;
    let tables_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('account_profiles', 'login_attempts')",
    )
    .fetch_one(&pool)
    .await
    .expect("tablas consultadas antes del upgrade");
    assert_eq!(tables_before, 0, "el esquema 0107 no conoce los perfiles");

    // 2. El upgrade: el fichero real de la migración nueva.
    let new_migration = MIGRATOR
        .iter()
        .find(|m| m.version == NEW_VERSION)
        .expect("la migración 0111 está en el árbol");
    assert_eq!(
        new_migration.description, "account profiles",
        "el número 0111 es nuevo y describe los perfiles por cuenta"
    );
    sqlx::raw_sql(&new_migration.sql)
        .execute(&pool)
        .await
        .expect("el upgrade a 0111 aplica sobre la base 0107");

    // 3. El esquema nuevo queda operativo: un perfil vinculado a la cuenta
    //    sembrada y un intento 'pending' que cierra a 'completed'.
    let (service_id, account_id) = seed_account(&pool).await;
    let inserted: i64 = sqlx::query_scalar(
        "INSERT INTO account_profiles (account_id, service_id, profile_key, archive_path, key_id, status)
         VALUES (?, ?, 'key-upgrade', '/tmp/x.syncprof', 'ab12', 'archived') RETURNING id",
    )
    .bind(account_id)
    .bind(service_id)
    .fetch_one(&pool)
    .await
    .expect("perfil insertado tras el upgrade");
    assert!(inserted > 0);

    let attempt: i64 = sqlx::query_scalar(
        "INSERT INTO login_attempts (service_id, profile_key) VALUES (?, 'key-upgrade') RETURNING id",
    )
    .bind(service_id)
    .fetch_one(&pool)
    .await
    .expect("intento insertado tras el upgrade");
    sqlx::query("UPDATE login_attempts SET status = 'completed', finished_at = '2026-01-01T00:00:00Z' WHERE id = ?")
        .bind(attempt)
        .execute(&pool)
        .await
        .expect("intento cerrado");
    let closed: (String, Option<String>) =
        sqlx::query_as("SELECT status, finished_at FROM login_attempts WHERE id = ?")
            .bind(attempt)
            .fetch_one(&pool)
            .await
            .expect("intento consultado");
    assert_eq!(closed.0, "completed");
    assert!(closed.1.is_some(), "el intento cerrado lleva finished_at");

    // 4. Los CHECK del contrato rechazan valores fuera de él.
    assert!(
        sqlx::query("UPDATE account_profiles SET status = 'plaintext' WHERE id = ?")
            .bind(inserted)
            .execute(&pool)
            .await
            .is_err(),
        "el CHECK de account_profiles.status rechaza valores fuera del contrato"
    );
    assert!(
        sqlx::query("UPDATE login_attempts SET status = 'aborted' WHERE id = ?")
            .bind(attempt)
            .execute(&pool)
            .await
            .is_err(),
        "el CHECK de login_attempts.status rechaza valores fuera del contrato"
    );

    // 5. El esquema completo con la migración nueva también migra desde cero.
    let fresh = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("segunda BD en memoria");
    MIGRATOR
        .run(&fresh)
        .await
        .expect("migración completa desde cero con 0111 incluida");
}

/// Criterio 3 del issue #86: un perfil que no descifra pasa la cuenta a «sin
/// sesión» y ofrece reautenticar; nada se borra y la app no se bloquea.
#[tokio::test]
async fn un_perfil_ilegible_deja_la_cuenta_sin_sesion_ofreciendo_reautenticar() {
    let _ = test_key();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");
    MIGRATOR.run(&pool).await.expect("BD migrada desde cero");

    let base = temp_base("ilegible");
    let archives = base.join("profile-archives");
    let (service_id, account_id) = seed_account(&pool).await;

    // Login completado: el perfil provisional se cifra al archivo y se
    // vincula a la cuenta (camino real de `finalize_attempt`).
    let profile_dir = fake_webview_profile(&base, "sess", "TOKEN_A86");
    let archive_path = profile_archive_path(&archives, "key-a");
    let profile_id = archive_attempt_profile(
        &pool,
        account_id,
        service_id,
        "key-a",
        &profile_dir,
        &archive_path,
    )
    .await
    .expect("perfil archivado y vinculado");

    // Higiene de arranque con el perfil íntegro: se verifica y la cuenta NO
    // se toca.
    let report = startup_profile_hygiene(&pool, &base.join("login-profiles"), &archives)
        .await
        .expect("higiene de arranque");
    assert_eq!(report.verified_archives, 1);
    assert_eq!(report.unrecoverable_profiles, 0);

    // El archivo deja de descifrar (byte corrupto: da lo mismo una clave
    // distinta por una base restaurada en otra máquina — la etiqueta GCM no
    // valida en ambos casos). La higiene lo detecta SIN materializar texto
    // plano y pasa la cuenta a «sin sesión», conservando cuenta, fila y
    // archivo.
    let mut corrupted = std::fs::read(&archive_path).expect("archivo leído");
    let last = corrupted.len() - 1;
    corrupted[last] ^= 0x01;
    std::fs::write(&archive_path, &corrupted).expect("archivo corrupto escrito");

    let report = startup_profile_hygiene(&pool, &base.join("login-profiles"), &archives)
        .await
        .expect("higiene de arranque con el archivo ilegible");
    assert_eq!(report.unrecoverable_profiles, 1);
    assert!(archive_path.is_file(), "el archivo NO se borra");

    let (credentials_invalid, reason, is_active): (i64, Option<String>, i64) = sqlx::query_as(
        "SELECT credentials_invalid, invalid_reason, is_active FROM accounts WHERE id = ?",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .expect("cuenta consultada tras la higiene");
    assert_eq!(credentials_invalid, 1, "la cuenta queda sin sesión");
    assert_eq!(
        reason.as_deref(),
        Some("profile_unreadable"),
        "la ficha sabe que debe ofrecer reautenticar"
    );
    assert_eq!(is_active, 0);

    let profile_status: String =
        sqlx::query_scalar("SELECT status FROM account_profiles WHERE id = ?")
            .bind(profile_id)
            .fetch_one(&pool)
            .await
            .expect("perfil consultado");
    assert_eq!(profile_status, "unrecoverable");

    std::fs::remove_dir_all(&base).ok();
}

/// Criterios 1 y 4 del issue #86: dos cuentas del mismo servicio con perfiles
/// separados; cerrar sesión y eliminar borran SU archivo; desactivar y
/// archivar lo conservan.
#[tokio::test]
async fn logout_y_eliminar_borran_el_archivo_y_desactivar_y_archivar_lo_conservan() {
    let _ = test_key();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("BD en memoria");
    MIGRATOR.run(&pool).await.expect("BD migrada desde cero");

    let base = temp_base("ciclo");
    let archives = base.join("profile-archives");
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .expect("servicio qobuz");

    // Dos cuentas del mismo servicio, cada una con SU perfil cifrado.
    let mut account_archive: Vec<(i64, PathBuf)> = Vec::new();
    for tag in ["A", "B"] {
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, ?, 1) RETURNING id",
        )
        .bind(service_id)
        .bind(format!("Cuenta {}", tag))
        .fetch_one(&pool)
        .await
        .expect("cuenta sembrada");
        let profile_dir = fake_webview_profile(&base, format!("sess-{}", tag).as_str(), tag);
        let archive_path = profile_archive_path(&archives, format!("key-{}", tag).as_str());
        archive_attempt_profile(
            &pool,
            account_id,
            service_id,
            format!("key-{}", tag).as_str(),
            &profile_dir,
            &archive_path,
        )
        .await
        .expect("perfil archivado");
        account_archive.push((account_id, archive_path));
    }

    let (id_a, path_a) = account_archive[0].clone();
    let (id_b, path_b) = account_archive[1].clone();
    assert_ne!(path_a, path_b, "cada cuenta tiene SU archivo");
    assert!(path_a.is_file() && path_b.is_file());

    // Desactivar y archivar NO tocan el perfil (no llaman a delete): solo
    // actualizan columnas de la cuenta.
    sqlx::query("UPDATE accounts SET is_active = 0, state = 'archived' WHERE id = ?")
        .bind(id_a)
        .execute(&pool)
        .await
        .expect("cuenta archivada");
    assert!(path_a.is_file(), "archivar conserva el archivo del perfil");

    // Cerrar sesión borra SU archivo (y solo el suyo).
    let deleted = delete_profiles_for_account(&pool, id_a)
        .await
        .expect("perfil borrado al cerrar sesión");
    assert_eq!(deleted, 1);
    assert!(!path_a.is_file(), "cerrar sesión borra el archivo");
    assert!(path_b.is_file(), "el perfil de la otra cuenta NO se toca");
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM account_profiles WHERE account_id = ?")
            .bind(id_a)
            .fetch_one(&pool)
            .await
            .expect("filas contadas");
    assert_eq!(remaining, 0, "cerrar sesión borra también la fila");

    // Eliminar la otra cuenta borra el resto (camino real: el comando llama
    // a delete_profiles_for_account ANTES de quitar la fila de cuenta).
    let deleted = delete_profiles_for_account(&pool, id_b)
        .await
        .expect("perfil borrado al eliminar");
    assert_eq!(deleted, 1);
    assert!(!path_b.is_file(), "eliminar borra el archivo");

    // La higiene de arranque limpia además los temporales huérfanos de un
    // cierre abrupto y los archivos sin fila.
    let orphan_temp = base.join("login-profiles").join("key-huerfano");
    std::fs::create_dir_all(&orphan_temp).expect("temporal huérfano creado");
    std::fs::write(orphan_temp.join("Cookies"), "sqlite jar huerfano").expect("cookie huérfana");
    let orphan_archive = archives.join("key-huerfano.syncprof");
    std::fs::write(&orphan_archive, b"basura sin fila").expect("archivo huérfano escrito");

    let report = startup_profile_hygiene(&pool, &base.join("login-profiles"), &archives)
        .await
        .expect("higiene de arranque");
    assert_eq!(report.removed_temp_dirs, 1, "temporal huérfano borrado");
    assert!(!orphan_temp.exists());
    assert_eq!(
        report.removed_orphan_archives, 1,
        "archivo sin fila borrado"
    );
    assert!(!orphan_archive.exists());

    std::fs::remove_dir_all(&base).ok();
}

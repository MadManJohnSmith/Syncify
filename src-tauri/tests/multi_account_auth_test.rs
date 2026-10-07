//! Multi-cuenta: identidad, exclusividad y estado POR CUENTA.
//!
//! El login histórico resolvía la fila destino del servicio por `email IS ?` y,
//! si no coincidía, tomaba la fila MÁS NUEVA y le sobrescribía email y
//! credenciales. Con dos cuentas del mismo servicio eso (a) destruía por CASCADE
//! la biblioteca de la cuenta anterior y (b) hacía indistinguibles dos sesiones
//! del mismo usuario.
//!
//! Estos tests fijan el contrato nuevo:
//!  1. La identidad se resuelve por email NORMALIZADO (LOWER/TRIM) y por
//!     `external_user_id`; ambos matchears usan `ORDER BY id DESC LIMIT 1`.
//!  2. Una identidad nueva INSERTA su cuenta y conserva las anteriores.
//!  3. `user_id` gana cuando email y user_id apuntan a filas distintas (email
//!     cambiado en el proveedor) — y por construcción no choca con el índice
//!     parcial de 0089.
//!  4. Sin identidad, Deezer conserva el fallback monocuenta; SoundCloud
//!     con perfil fallido inserta otra fila en vez de pisar la anterior.
//!  5. Exclusividad: activar, añadir o conectar deja EXACTAMENTE una activa por
//!     servicio.
//!  6. El estado y la invalidación son por cuenta: un `account_id` de otro
//!     servicio no responde, una cuenta desactivada elegida sí se evalúa, y un
//!     401 sobre la cuenta A no envenena a la B.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::sync::Arc;
use syncify_tauri_lib::commands::{
    logout_service, mark_account_credentials_invalid, perform_get_service_auth_status,
    perform_sync_favorites, set_active_account, toggle_account_active, upsert_service_account,
};
use syncify_tauri_lib::{crypto, worker::DownloadWorkerState, AppState, EnrichmentWorkerState};
use tauri::Manager;

async fn setup_test_db() -> SqlitePool {
    let _ = crypto::init_crypto([17u8; 32]);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations apply");
    pool
}

fn create_test_app(pool: SqlitePool) -> tauri::App<tauri::test::MockRuntime> {
    let app = tauri::test::mock_app();
    app.manage(AppState {
        db: pool,
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    });
    app
}

async fn service_id(pool: &SqlitePool, name: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = LOWER(?)")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| panic!("service {} seeded by migrations", name))
}

/// Inserta una cuenta directamente (sin pasar por el login) para preparar
/// escenarios de identidad.
async fn insert_account(
    pool: &SqlitePool,
    service: &str,
    email: Option<&str>,
    external_user_id: Option<&str>,
    is_active: bool,
) -> i64 {
    let svc = service_id(pool, service).await;
    sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, email, external_user_id, is_active, credentials_invalid)
           VALUES (?, 'Seeded', ?, ?, ?, 0) RETURNING id"#,
    )
    .bind(svc)
    .bind(email)
    .bind(external_user_id)
    .bind(if is_active { 1 } else { 0 })
    .fetch_one(pool)
    .await
    .expect("account insert")
}

async fn row_state(
    pool: &SqlitePool,
    account_id: i64,
) -> (Option<String>, Option<String>, i64, i64) {
    sqlx::query_as::<_, (Option<String>, Option<String>, i64, i64)>(
        "SELECT email, credentials_json, is_active, COALESCE(credentials_invalid, 0)
         FROM accounts WHERE id = ?",
    )
    .bind(account_id)
    .fetch_one(pool)
    .await
    .expect("row exists")
}

async fn active_ids(pool: &SqlitePool, service: &str) -> Vec<i64> {
    let svc = service_id(pool, service).await;
    sqlx::query_scalar("SELECT id FROM accounts WHERE service_id = ? AND is_active = 1 ORDER BY id")
        .bind(svc)
        .fetch_all(pool)
        .await
        .expect("actives readable")
}

// ── 1. Matching por email normalizado ──────────────────────────────────────────

#[tokio::test]
async fn test_normalized_email_revives_the_existing_row() {
    let pool = setup_test_db().await;
    let svc = service_id(&pool, "qobuz").await;
    let existing = insert_account(&pool, "qobuz", Some("user@Example.com"), None, false).await;

    let returned = upsert_service_account(
        &pool,
        svc,
        "Owner",
        Some("  USER@example.com "),
        None,
        "enc_fresh",
    )
    .await
    .expect("login con el mismo email en otra capitalización");

    assert_eq!(returned, existing, "revive SU fila, no crea otra");
    let (email, creds, active, invalid) = row_state(&pool, existing).await;
    assert_eq!(active, 1);
    assert_eq!(invalid, 0, "la fila revive con flags limpios");
    assert_eq!(creds.as_deref(), Some("enc_fresh"));
    assert_eq!(
        email.as_deref(),
        Some("user@example.com"),
        "el email se escribe normalizado"
    );

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ?")
        .bind(svc)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "no se crea una cuenta duplicada");
}

/// La pasada de normalización de 0089 no puede fusionar parejas case-variantes
/// que una BD trajera ya. Ambas casan por `LOWER(TRIM(email))`, así que sin un
/// `ORDER BY id DESC` el `fetch_optional` devolvería una fila arbitraria y
/// podría revivir la hermana equivocada. Este test fija que gana la de id MAYOR.
#[tokio::test]
async fn test_case_variant_residue_always_resolves_to_the_highest_id() {
    let pool = setup_test_db().await;
    let svc = service_id(&pool, "qobuz").await;

    let lower = insert_account(&pool, "qobuz", Some("dup@x.com"), None, false).await;
    let upper = insert_account(&pool, "qobuz", Some("DUP@x.com"), None, false).await;
    assert!(upper > lower, "la fila de id mayor se insertó después");

    let returned = upsert_service_account(&pool, svc, "Owner", Some("Dup@X.com"), None, "enc_high")
        .await
        .expect("el match case-insensitive resuelve");

    assert_eq!(
        returned, upper,
        "con residuos case-variantes el match es determinista: gana MAX(id)"
    );
    assert_eq!(
        row_state(&pool, upper).await.2,
        1,
        "la fila de id mayor queda activa"
    );
    assert_eq!(row_state(&pool, lower).await.2, 0);
    assert_eq!(row_state(&pool, upper).await.1.as_deref(), Some("enc_high"));
}

/// Una hermana con el email YA normalizado hace que escribir el email canónico
/// en la fila objetivo choque con la UNIQUE BINARY de 0002 (la identidad ya casó
/// case-insensitive). El UPDATE debe reintentarse SIN reescribir la columna
/// email, no fallar el login.
///
/// Verified en sqlite3 (pre-0089 schema + 0089 aplicada):
///   id=1 'a@x.com' (canónico) | id=2 ' A@X.COM ' (residuo)
///   match por LOWER(TRIM(email)) → id=2 (la de id mayor)
///   UPDATE ... email='a@x.com' → UNIQUE constraint failed: accounts.service_id, accounts.email
#[tokio::test]
async fn test_case_variant_sibling_does_not_break_the_update() {
    let pool = setup_test_db().await;
    let svc = service_id(&pool, "qobuz").await;

    // Residuos previos: el login normaliza a 'a@x.com', que es EXACTAMENTE lo que
    // ya guarda la hermana de id menor.
    let canonical = insert_account(&pool, "qobuz", Some("a@x.com"), None, false).await;
    let kept = insert_account(&pool, "qobuz", Some(" A@X.COM "), None, false).await;
    assert!(kept > canonical, "la fila objetivo es la de id mayor");

    let returned = upsert_service_account(
        &pool,
        svc,
        "Owner",
        Some("A@X.com"),
        None,
        "enc_no_email_rewrite",
    )
    .await
    .expect("el login no debe caer en el UNIQUE(email)");

    assert_eq!(returned, kept, "gana la fila de id mayor");
    let (email, creds, active, _) = row_state(&pool, kept).await;
    assert_eq!(creds.as_deref(), Some("enc_no_email_rewrite"));
    assert_eq!(active, 1);
    assert_eq!(
        email.as_deref(),
        Some(" A@X.COM "),
        "el email almacenado no se reescribe: la identidad ya casó case-insensitive"
    );
    assert_eq!(
        row_state(&pool, canonical).await.1,
        None,
        "la hermana queda intacta"
    );

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ?")
        .bind(svc)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total, 2, "ninguna fila se borra");
}

// ── 2. Matching por external_user_id ──────────────────────────────────────────

/// El usuario cambió su email en el proveedor: el email nuevo no casa con nada,
/// pero el `user_id` es estable y SÍ debe revivir su fila. Sin esta búsqueda el
/// flujo caería al fallback "fila más nueva" y le pisaría email y credenciales.
#[tokio::test]
async fn test_known_user_id_wins_over_an_unknown_email() {
    let pool = setup_test_db().await;
    let svc = service_id(&pool, "qobuz").await;

    let owner = insert_account(&pool, "qobuz", Some("old@x.com"), Some("user-42"), true).await;
    let newest = insert_account(&pool, "qobuz", Some("other@x.com"), Some("user-99"), false).await;
    assert!(newest > owner);

    let returned = upsert_service_account(
        &pool,
        svc,
        "Owner",
        Some("new@x.com"),
        Some("user-42"),
        "enc_email_changed",
    )
    .await
    .expect("login con email nuevo y user_id conocido");

    assert_eq!(
        returned, owner,
        "reviva la fila del user_id, NO la más reciente"
    );
    let (email, creds, active, _) = row_state(&pool, owner).await;
    assert_eq!(email.as_deref(), Some("new@x.com"), "su email se actualiza");
    assert_eq!(creds.as_deref(), Some("enc_email_changed"));
    assert_eq!(active, 1);
    assert_eq!(
        row_state(&pool, newest).await.2,
        0,
        "la otra cuenta queda intacta y desactivada"
    );
}

/// Email matchea A y user_id matchea B: gana B (identidad estable) y el login no
/// debe chocar con el índice parcial `idx_accounts_service_external_user`.
///
/// A guarda el email en otra capitalización a propósito: si guardara el MISMO
/// texto que el login, reescribirlo en B violaría UNIQUE(service_id, email) y
/// caería al camino "retry sin tocar email" (cubierto por
/// `test_case_variant_sibling_does_not_break_the_update`).
#[tokio::test]
async fn test_user_id_wins_when_email_and_user_id_point_to_different_rows() {
    let pool = setup_test_db().await;
    let svc = service_id(&pool, "spotify").await;

    let email_row = insert_account(&pool, "spotify", Some("TAKEN@x.com"), None, true).await;
    let user_row =
        insert_account(&pool, "spotify", Some("owner@x.com"), Some("sp-user"), true).await;
    assert!(user_row > email_row);

    let returned = upsert_service_account(
        &pool,
        svc,
        "Owner",
        Some("taken@x.com"),
        Some("sp-user"),
        "enc_user_row",
    )
    .await
    .expect("sin violación del índice parcial");

    assert_eq!(
        returned, user_row,
        "el user_id manda: su fila es la identidad real de esa cuenta"
    );
    let (email, creds, active, _) = row_state(&pool, user_row).await;
    assert_eq!(
        email.as_deref(),
        Some("taken@x.com"),
        "su email se refresca"
    );
    assert_eq!(creds.as_deref(), Some("enc_user_row"));
    assert_eq!(active, 1);
    assert_eq!(
        row_state(&pool, email_row).await.2,
        0,
        "la fila que solo casaba por email queda desactivada, no se borra"
    );
    assert_eq!(active_ids(&pool, "spotify").await.len(), 1);
}

/// Tidal no devuelve email: dos logins con `user_id` distinto deben producir dos
/// cuentas distinguibles en vez de filas indistinguibles con email NULL.
#[tokio::test]
async fn test_two_tidal_logins_with_distinct_user_ids_are_two_accounts() {
    let pool = setup_test_db().await;
    let svc = service_id(&pool, "tidal").await;

    let first = upsert_service_account(&pool, svc, "T1", None, Some("tid-1"), "enc_1")
        .await
        .expect("primer login tidal");
    let second = upsert_service_account(&pool, svc, "T2", None, Some("tid-2"), "enc_2")
        .await
        .expect("segundo login tidal");

    assert_ne!(first, second, "identidades distintas → cuentas distintas");
    assert_eq!(row_state(&pool, first).await.2, 0);
    assert_eq!(
        row_state(&pool, second).await.2,
        1,
        "la última conectada es la activa"
    );
    assert_eq!(active_ids(&pool, "tidal").await, vec![second]);

    // Y un re-login del primero revive SU fila sin tocar la segunda.
    let again = upsert_service_account(&pool, svc, "T1", None, Some("tid-1"), "enc_1b")
        .await
        .expect("re-login de la primera cuenta");
    assert_eq!(again, first);
    assert_eq!(row_state(&pool, second).await.2, 0);
    assert_eq!(row_state(&pool, first).await.1.as_deref(), Some("enc_1b"));
}

/// Deezer sin email NI user_id conserva el comportamiento histórico
/// de reutilizar la fila más nueva. Sin
/// este fallback, cada re-login INSERTARÍA una fila y el servicio crecería sin
/// límite.
#[tokio::test]
async fn test_login_without_identity_reuses_the_newest_row() {
    let pool = setup_test_db().await;
    let svc = service_id(&pool, "deezer").await;

    let older = insert_account(&pool, "deezer", None, None, true).await;
    let newer = insert_account(&pool, "deezer", None, None, false).await;
    assert!(newer > older);

    let returned = upsert_service_account(&pool, svc, "Deezer", None, None, "enc_arl")
        .await
        .expect("fallback monocuenta");

    assert_eq!(
        returned, newer,
        "sin identidad se reutiliza la fila más nueva"
    );
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ?")
        .bind(svc)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total, 2, "no se insertan filas sin fin");
}

#[tokio::test]
async fn soundcloud_placeholder_without_identity_does_not_overwrite_another_account() {
    let pool = setup_test_db().await;
    let svc = service_id(&pool, "soundcloud").await;
    let first = upsert_service_account(&pool, svc, "Unknown 1", None, None, "enc_first")
        .await
        .unwrap();
    let second = upsert_service_account(&pool, svc, "Unknown 2", None, None, "enc_second")
        .await
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(
        row_state(&pool, first).await.1.as_deref(),
        Some("enc_first")
    );
}

// ── 3. Exclusividad ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_new_identity_inserts_and_leaves_exactly_one_active() {
    let pool = setup_test_db().await;
    let svc = service_id(&pool, "qobuz").await;

    let first = upsert_service_account(&pool, svc, "A", Some("a@x.com"), Some("u-a"), "enc_a")
        .await
        .unwrap();
    let second = upsert_service_account(&pool, svc, "B", Some("b@x.com"), Some("u-b"), "enc_b")
        .await
        .unwrap();

    assert_ne!(first, second);
    assert_eq!(active_ids(&pool, "qobuz").await, vec![second]);
    assert_eq!(
        row_state(&pool, first).await.1.as_deref(),
        Some("enc_a"),
        "la cuenta anterior conserva SUS credenciales"
    );
}

#[tokio::test]
async fn test_set_active_account_switches_and_keeps_one_active() {
    let pool = setup_test_db().await;
    let app = create_test_app(pool.clone());

    let first = insert_account(&pool, "qobuz", Some("a@x.com"), None, true).await;
    let second = insert_account(&pool, "qobuz", Some("b@x.com"), None, false).await;

    set_active_account(app.handle().clone(), app.state::<AppState>(), second)
        .await
        .expect("set_active_account");

    assert_eq!(active_ids(&pool, "qobuz").await, vec![second]);
    assert_eq!(
        row_state(&pool, first).await.2,
        0,
        "la hermana queda desactivada"
    );

    // Volver atrás también funciona y no acumula activas.
    set_active_account(app.handle().clone(), app.state::<AppState>(), first)
        .await
        .expect("switch back");
    assert_eq!(active_ids(&pool, "qobuz").await, vec![first]);

    // Un id inexistente no se inventa.
    let err = set_active_account(app.handle().clone(), app.state::<AppState>(), 999_999)
        .await
        .expect_err("unknown account must fail");
    assert!(err.contains("999999"), "el error nombra la cuenta: {}", err);

    // Desactivar la activa sí deja el servicio sin cuenta activa (documentado:
    // es la forma de "silenciar" un servicio).
    toggle_account_active(app.handle().clone(), app.state::<AppState>(), first, false)
        .await
        .expect("toggle off");
    assert!(active_ids(&pool, "qobuz").await.is_empty());

    // Reactivar vuelve a imponer la exclusividad.
    toggle_account_active(app.handle().clone(), app.state::<AppState>(), second, true)
        .await
        .expect("toggle on");
    assert_eq!(active_ids(&pool, "qobuz").await, vec![second]);
}

// ── 4. Logout por cuenta ───────────────────────────────────────────────────────

/// "Desconectar" limpia las credenciales de ESA fila y conserva la fila con su
/// biblioteca. El bridge tiene un único archivo de sesión por servicio, así que
/// con más cuentas vivas su logout se salta (no cerraría las de las demás).
#[tokio::test]
async fn test_logout_by_account_clears_only_that_row() {
    let pool = setup_test_db().await;
    let app = create_test_app(pool.clone());

    let svc = service_id(&pool, "qobuz").await;
    let first = insert_account(&pool, "qobuz", Some("a@x.com"), Some("u-a"), true).await;
    let second = insert_account(&pool, "qobuz", Some("b@x.com"), Some("u-b"), false).await;
    sqlx::query("UPDATE accounts SET credentials_json = 'enc', credentials = 'legacy_ciphertext' WHERE service_id = ?")
        .bind(svc)
        .execute(&pool)
        .await
        .unwrap();

    let result = logout_service("qobuz".to_string(), Some(first), app.state::<AppState>())
        .await
        .expect("logout por cuenta");

    let data = result.data.as_ref().expect("logout returns data");
    assert_eq!(
        data["bridge_logout_skipped"].as_bool(),
        Some(true),
        "con otra cuenta viva no se toca la sesión compartida del bridge"
    );
    assert_eq!(
        data["account_id"].as_i64(),
        Some(first),
        "el logout identifica la fila afectada"
    );
    assert_eq!(
        data["new_active_account_id"].as_i64(),
        Some(second),
        "la cuenta que acaba de perder credenciales cede la activa a la más reciente"
    );

    let (_, creds, active, invalid) = row_state(&pool, first).await;
    assert!(creds.is_none(), "las credenciales de ESA fila se limpian");
    let legacy_credentials: Option<String> =
        sqlx::query_scalar("SELECT credentials FROM accounts WHERE id = ?")
            .bind(first)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        legacy_credentials.is_none(),
        "se borra también la columna heredada"
    );
    assert_eq!(invalid, 1, "y queda marcada como requiere_auth");
    assert_eq!(
        active, 0,
        "y deja de ser la activa: el servicio no puede quedarse apuntando a una fila \
         sin credenciales (todos los resolvedores leen `is_active = 1 ORDER BY id DESC`)"
    );

    let (_, other_creds, other_active, other_invalid) = row_state(&pool, second).await;
    assert_eq!(
        other_creds.as_deref(),
        Some("enc"),
        "la otra cuenta conserva sus credenciales"
    );
    assert_eq!(other_active, 1, "y hereda el hueco de activa");
    assert_eq!(other_invalid, 0);

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ?")
        .bind(svc)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        total, 2,
        "desconectar NO borra filas (ni su biblioteca por CASCADE)"
    );

    let active_now = active_ids(&pool, "qobuz").await;
    assert_eq!(
        active_now,
        vec![second],
        "el logout mantiene el invariante de exactamente una activa por servicio"
    );
}

/// Desconectar la única cuenta, incluso Spotify, conserva la fila y sus
/// library_entries/playlists pero la desactiva: la selección por defecto no
/// debe resolver credenciales recién invalidadas.
#[tokio::test]
async fn test_logout_of_the_only_account_keeps_its_data_but_deactivates_it() {
    let pool = setup_test_db().await;
    let app = create_test_app(pool.clone());

    let only = insert_account(&pool, "spotify", Some("a@x.com"), Some("u-a"), true).await;
    sqlx::query("UPDATE accounts SET credentials_json = 'enc' WHERE service_id = (SELECT id FROM services WHERE name = 'spotify')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tracks (title) VALUES ('Liked')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO library_entries (account_id, track_id) VALUES (?, (SELECT id FROM tracks WHERE title = 'Liked'))")
        .bind(only)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO playlists (account_id, name) VALUES (?, 'Saved playlist')")
        .bind(only)
        .execute(&pool)
        .await
        .unwrap();

    let result = logout_service("spotify".to_string(), Some(only), app.state::<AppState>())
        .await
        .expect("logout por cuenta");

    let data = result.data.as_ref().expect("logout returns data");
    assert_eq!(
        data["new_active_account_id"].as_i64(),
        None,
        "sin hermanas no hay a quién cederle la activa"
    );

    let (_, creds, active, invalid) = row_state(&pool, only).await;
    assert!(creds.is_none());
    assert_eq!(invalid, 1);
    assert_eq!(active, 0, "desconectar desactiva también la última cuenta");
    assert!(active_ids(&pool, "spotify").await.is_empty());

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM accounts WHERE service_id = (SELECT id FROM services WHERE name = 'spotify')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(total, 1, "tampoco se borra la fila");
    let saved: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM library_entries WHERE account_id = ?")
            .bind(only)
            .fetch_one(&pool)
            .await
            .unwrap();
    let playlists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playlists WHERE account_id = ?")
        .bind(only)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        (saved, playlists),
        (1, 1),
        "logout no dispara el CASCADE de eliminar cuenta"
    );
}

/// El camino legacy (None) solo desconecta a la activa. Tras quedarse sin
/// activas, repetirlo no debe limpiar el bridge ni tocar una hermana inactiva.
#[tokio::test]
async fn test_logout_without_id_never_targets_inactive_accounts() {
    let pool = setup_test_db().await;
    let app = create_test_app(pool.clone());
    let active = insert_account(&pool, "spotify", Some("one@x.com"), None, true).await;
    let inactive = insert_account(&pool, "spotify", Some("two@x.com"), None, false).await;
    sqlx::query("UPDATE accounts SET credentials_json = 'token' WHERE service_id = ?")
        .bind(service_id(&pool, "spotify").await)
        .execute(&pool)
        .await
        .unwrap();

    let first = logout_service("spotify".to_owned(), None, app.state::<AppState>())
        .await
        .unwrap();
    assert_eq!(
        first.data.as_ref().unwrap()["account_id"].as_i64(),
        Some(active)
    );
    assert_eq!(row_state(&pool, active).await.2, 0);
    assert_eq!(row_state(&pool, inactive).await.2, 1);

    let second = logout_service("spotify".to_owned(), None, app.state::<AppState>())
        .await
        .unwrap();
    assert_eq!(
        second.data.as_ref().unwrap()["account_id"].as_i64(),
        Some(inactive)
    );
    assert!(active_ids(&pool, "spotify").await.is_empty());
    let third = logout_service("spotify".to_owned(), None, app.state::<AppState>())
        .await
        .unwrap();
    assert!(third.success);
    assert!(third.data.as_ref().unwrap()["account_id"].is_null());
    assert_eq!(row_state(&pool, active).await.3, 1);
    assert_eq!(row_state(&pool, inactive).await.3, 1);
}

/// Un `account_id` de otro servicio no puede cerrar sesión en este.
#[tokio::test]
async fn test_logout_rejects_an_account_of_another_service() {
    let pool = setup_test_db().await;
    let app = create_test_app(pool.clone());

    let tidal_account = insert_account(&pool, "tidal", Some("t@x.com"), None, true).await;

    let err = logout_service(
        "qobuz".to_string(),
        Some(tidal_account),
        app.state::<AppState>(),
    )
    .await
    .expect_err("cuenta de otro servicio");
    assert!(
        err.contains("does not belong"),
        "el error explica la pertenencia: {}",
        err
    );
}

// ── 5. Estado e invalidación por cuenta ────────────────────────────────────────

#[tokio::test]
async fn test_auth_status_by_id_validates_service_membership() {
    let pool = setup_test_db().await;
    let tidal_account = insert_account(&pool, "tidal", Some("t@x.com"), None, true).await;

    let status = perform_get_service_auth_status(&pool, "qobuz", Some(tidal_account))
        .await
        .expect("la consulta por id resuelve");

    assert_eq!(
        status.status, "missing",
        "una cuenta de otro servicio no responde por este servicio"
    );
    assert_eq!(status.service, "qobuz");
    assert!(status.account_id.is_none());
    assert!(!status.is_active);
}

/// Una cuenta ELEGIDA y desactivada se evalúa con sus credenciales reales: el
/// corte `is_active == 0 → requires_auth` se aplica solo al camino sin id.
#[tokio::test]
async fn test_auth_status_by_id_does_not_short_circuit_on_inactive() {
    let pool = setup_test_db().await;
    let svc = service_id(&pool, "qobuz").await;

    let credentials =
        crypto::encrypt(r#"{"user_auth_token":"token_de_la_cuenta_elegida"}"#).expect("encrypt");
    let account_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, email, credentials_json, is_active, credentials_invalid)
           VALUES (?, 'Chosen', 'chosen@x.com', ?, 0, 0) RETURNING id"#,
    )
    .bind(svc)
    .bind(&credentials)
    .fetch_one(&pool)
    .await
    .unwrap();

    let status = perform_get_service_auth_status(&pool, "qobuz", Some(account_id))
        .await
        .expect("status by id");

    assert_eq!(
        status.status, "connected_valid",
        "la cuenta elegida desactivada conserva su diagnóstico real"
    );
    assert!(status.is_authenticated);
    assert!(
        !status.is_active,
        "el DTO dice explícitamente que NO es la activa del servicio"
    );
    assert_eq!(status.account_id, Some(account_id));
}

/// Un 401 durante el sync de la cuenta A no puede envenenar a la B.
#[tokio::test]
async fn test_mark_credentials_invalid_by_id_leaves_the_active_sibling_alone() {
    let pool = setup_test_db().await;
    let active = insert_account(&pool, "qobuz", Some("b@x.com"), None, true).await;
    let chosen = insert_account(&pool, "qobuz", Some("a@x.com"), None, false).await;

    let affected = mark_account_credentials_invalid(
        &pool,
        "qobuz",
        "HTTP 401: User authentication required",
        Some(chosen),
    )
    .await
    .expect("marking must not fail");

    assert_eq!(affected, 1, "solo la fila elegida se marca");
    assert_eq!(row_state(&pool, chosen).await.3, 1);
    assert_eq!(
        row_state(&pool, active).await.3,
        0,
        "la cuenta activa de otro usuario no se envenena"
    );

    // Y sin account_id se conserva el comportamiento histórico (activas).
    let affected = mark_account_credentials_invalid(&pool, "qobuz", "HTTP 401", None)
        .await
        .expect("legacy path");
    assert_eq!(affected, 1, "None sigue marcando las filas ACTIVAS");
    assert_eq!(row_state(&pool, active).await.3, 1);

    // Un id de otro servicio no envenena nada.
    let tidal_account = insert_account(&pool, "tidal", Some("t@x.com"), None, true).await;
    let affected =
        mark_account_credentials_invalid(&pool, "qobuz", "HTTP 401", Some(tidal_account))
            .await
            .expect("cross-service mark");
    assert_eq!(affected, 0, "una cuenta de otro servicio no se toca");
}

#[tokio::test]
async fn test_sync_favorites_rejects_an_account_of_another_service() {
    let pool = setup_test_db().await;
    let qobuz_account = insert_account(&pool, "qobuz", Some("a@x.com"), None, true).await;

    let err = perform_sync_favorites(&pool, "deezer", Some("all"), Some(qobuz_account))
        .await
        .expect_err("resolver credenciales de otra cuenta debe fallar ANTES de la red");

    assert!(
        err.contains("does not exist or belongs to another service"),
        "el error nombra la pertenencia, no el servicio genérico: {}",
        err
    );
}

//! Regresión de la migración 0089 (`accounts.external_user_id`).
//!
//! BD-8 (`src-tauri/src/db.rs:14-36`) hace INMUTABLE toda migración ya publicada:
//! sqlx valida el checksum SHA-384 del archivo contra `_sqlx_migrations` en cada
//! arranque. Editar 0089 a mano no es una opción — si se equivoca, solo esta red
//! de tests lo detecta antes de que la app deje de arrancar en las BDs que ya la
//! aplicaron.
//!
//! Qué fija este archivo:
//!   1. 0089 está registrada después de 0088 y el esquema completo sigue
//!      aplicando en una BD limpia (columna + índice presentes).
//!   2. El repair de la exclusividad deja EXACTAMENTE una cuenta activa por
//!      servicio y sobrevive la de MAX(id) — la misma regla determinista que
//!      usan los resolvedores (`is_active = 1 ORDER BY id DESC LIMIT 1`).
//!   3. El repair es idempotente (segunda ejecución → 0 filas).
//!   4. El índice PARCIAL admite N filas con `external_user_id` NULL y rechaza
//!      el duplicado no-NULL.
//!   5. La normalización de email NO viola la UNIQUE BINARY de 0002 ni fusiona
//!      (ni destruye) parejas case-variantes, y sí normaliza cuando no colisiona.
//!
//! El SQL de 0089 se ejecuta contra un esquema previo Equivalent al real
//! (0002 + 0031 + 0055) porque sus cláusulas (c) y (d) solo tocan `accounts`.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

/// El archivo tal cual se publica. Los tests lo ejecutan con `raw_sql` para que
/// lo que se prueba sea el SQL que sqlx embebió, no una copia.
const M0089: &str = include_str!("../migrations/0089_account_external_user_id.sql");

async fn in_memory() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory pool")
}

/// Esquema de `accounts` justo ANTES de 0089 (0002 + las columnas de 0031 y
/// 0055). Réplica declarada a mano: las cláusulas de datos de 0089 solo leen y
/// escriben estas columnas.
async fn pool_pre_0089() -> SqlitePool {
    let pool = in_memory().await;
    sqlx::raw_sql(
        r#"
        CREATE TABLE services (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            supports_download INTEGER DEFAULT 0,
            max_quality TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
            display_name TEXT,
            email TEXT,
            is_active INTEGER DEFAULT 1,
            credentials_json TEXT,
            last_synced TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            credentials_invalid INTEGER DEFAULT 0,
            invalid_reason TEXT,
            last_auth_error TEXT,
            last_auth_error_at TEXT,
            last_auth_checked_at TEXT,
            UNIQUE(service_id, email)
        );

        INSERT INTO services (id, name) VALUES
            (1, 'spotify'),
            (2, 'qobuz'),
            (3, 'tidal'),
            (4, 'deezer');
        "#,
    )
    .execute(&pool)
    .await
    .expect("pre-0089 schema");
    pool
}

/// Texto de la cláusula (c) de 0089 tal cual está en el archivo.
///
/// Se delimitan los marcadores DE LA SECCIÓN SQL (`-- (c) Repair:`), no los de la
/// cabecera del archivo: el archivo menciona "(c)" y "(d)" dos veces (cabecera
/// explicativa y sección) y un split por el primero devolvería solo comentarios.
/// Y se recorta desde el primer `UPDATE`: lo que queda entre el marcador y la
/// sentencia es el resto del comentario, que sin su `--` no sería SQL válido.
fn repair_clause() -> &'static str {
    let section = M0089
        .split("-- (c) Repair:")
        .nth(1)
        .expect("0089 keeps its repair section")
        .split("-- (d) Normalización")
        .next()
        .expect("repair section is delimited");

    let start = section
        .find("UPDATE accounts")
        .expect("the repair clause is an UPDATE on accounts");
    &section[start..]
}

#[tokio::test]
async fn test_0089_is_registered_right_after_0088_and_applies_cleanly() {
    let migrator = sqlx::migrate!("./migrations");

    let versions: Vec<i64> = migrator.iter().map(|m| m.version).collect();
    assert!(
        versions.windows(2).any(|pair| pair == [88, 89]),
        "0089 debe estar registrada inmediatamente después de 0088;          versiones posteriores (p. ej. 0090 de migración por cuenta) también son válidas"
    );

    let pool = in_memory().await;
    migrator
        .run(&pool)
        .await
        .expect("the full pipeline must apply on a clean database");

    let column: Option<String> = sqlx::query_scalar(
        "SELECT name FROM pragma_table_info('accounts') WHERE name = 'external_user_id'",
    )
    .fetch_optional(&pool)
    .await
    .expect("pragma_table_info readable");
    assert_eq!(
        column.as_deref(),
        Some("external_user_id"),
        "0089 adds external_user_id as a NULLABLE column"
    );

    let index: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_accounts_service_external_user'",
    )
    .fetch_optional(&pool)
    .await
    .expect("sqlite_master readable");
    assert!(
        index.is_some(),
        "0089 creates the partial unique index on (service_id, external_user_id)"
    );

    // Nullable de verdad: las filas existentes (creadas antes de 0089) leen NULL.
    let (qobuz_id,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .expect("qobuz seeded by migrations");
    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, email, is_active) VALUES (?, 'Legacy', 'legacy@test.example', 1) RETURNING id",
    )
    .bind(qobuz_id)
    .fetch_one(&pool)
    .await
    .expect("legacy insert");
    let external: Option<String> =
        sqlx::query_scalar("SELECT external_user_id FROM accounts WHERE id = ?")
            .bind(account_id)
            .fetch_one(&pool)
            .await
            .expect("row readable");
    assert!(
        external.is_none(),
        "las filas anteriores a 0089 quedan con external_user_id NULL (retrocompatible)"
    );
}

#[tokio::test]
async fn test_repair_leaves_exactly_one_active_per_service_keeping_max_id() {
    let pool = pool_pre_0089().await;

    let (qobuz,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .expect("service row");
    let (tidal,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(&pool)
        .await
        .expect("service row");

    // Estado que hoy es posible (toggle/add_account legacy): 3 activas de qobuz
    // y 2 de tidal.
    for email in ["a@x.com", "b@x.com", "c@x.com"] {
        sqlx::query("INSERT INTO accounts (service_id, email, is_active) VALUES (?, ?, 1)")
            .bind(qobuz)
            .bind(email)
            .execute(&pool)
            .await
            .expect("seed active");
    }
    for email in ["t1@x.com", "t2@x.com"] {
        sqlx::query("INSERT INTO accounts (service_id, email, is_active) VALUES (?, ?, 1)")
            .bind(tidal)
            .bind(email)
            .execute(&pool)
            .await
            .expect("seed active");
    }

    sqlx::raw_sql(M0089)
        .execute(&pool)
        .await
        .expect("0089 applies over the pre-0089 schema");

    let qobuz_actives: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM accounts WHERE service_id = ? AND is_active = 1 ORDER BY id",
    )
    .bind(qobuz)
    .fetch_all(&pool)
    .await
    .expect("actives readable");
    assert_eq!(
        qobuz_actives.len(),
        1,
        "el repair deja exactamente UNA activa por servicio"
    );

    let qobuz_max: (i64,) = sqlx::query_as("SELECT MAX(id) FROM accounts WHERE service_id = ?")
        .bind(qobuz)
        .fetch_one(&pool)
        .await
        .expect("max readable");
    assert_eq!(
        qobuz_actives[0], qobuz_max.0,
        "sobrevive la de MAX(id): la misma regla que `is_active = 1 ORDER BY id DESC LIMIT 1`"
    );

    let tidal_actives: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ? AND is_active = 1")
            .bind(tidal)
            .fetch_one(&pool)
            .await
            .expect("actives readable");
    assert_eq!(tidal_actives, 1);

    // Ninguna fila se borra: desactivar no es eliminar.
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .expect("count readable");
    assert_eq!(total, 5, "el repair no borra ninguna cuenta");

    // No-op con una sola cuenta por servicio.
    let (solo,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'deezer'")
        .fetch_one(&pool)
        .await
        .expect("service row");
    sqlx::query("INSERT INTO accounts (service_id, email, is_active) VALUES (?, 'solo@x.com', 1)")
        .bind(solo)
        .execute(&pool)
        .await
        .expect("seed single");
    let repair = repair_clause();
    sqlx::raw_sql(repair)
        .execute(&pool)
        .await
        .expect("repair statement re-runs without error");
    let solo_actives: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE service_id = ? AND is_active = 1")
            .bind(solo)
            .fetch_one(&pool)
            .await
            .expect("actives readable");
    assert_eq!(solo_actives, 1, "una cuenta única y activa no se toca");
}

#[tokio::test]
async fn test_repair_is_idempotent() {
    let pool = pool_pre_0089().await;

    let (qobuz,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .expect("service row");
    for email in ["a@x.com", "b@x.com", "c@x.com"] {
        sqlx::query("INSERT INTO accounts (service_id, email, is_active) VALUES (?, ?, 1)")
            .bind(qobuz)
            .bind(email)
            .execute(&pool)
            .await
            .expect("seed active");
    }

    sqlx::raw_sql(M0089)
        .execute(&pool)
        .await
        .expect("0089 applies");

    // Segunda pasada de la MISMA cláusula (la de esquema no es re-ejecutable).
    let second = sqlx::raw_sql(repair_clause())
        .execute(&pool)
        .await
        .expect("repair re-run");
    assert_eq!(
        second.rows_affected(),
        0,
        "el repair es idempotente: la segunda ejecución no cambia nada"
    );
}

#[tokio::test]
async fn test_partial_index_admits_nulls_and_rejects_duplicate_user_ids() {
    let pool = pool_pre_0089().await;
    let (qobuz,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .expect("service row");
    let (tidal,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(&pool)
        .await
        .expect("service row");

    sqlx::raw_sql(M0089)
        .execute(&pool)
        .await
        .expect("0089 applies");

    // Varias filas sin user_id (deezer/soundcloud, o lo que aún no ha vuelto a
    // logararse) conviven: el índice es PARCIAL y no las afecta.
    for email in ["n1@x.com", "n2@x.com", "n3@x.com"] {
        sqlx::query("INSERT INTO accounts (service_id, email) VALUES (?, ?)")
            .bind(qobuz)
            .bind(email)
            .execute(&pool)
            .await
            .expect("rows without external_user_id are allowed");
    }
    // Mismo user_id en servicios DISTINTOS: permitido (el índice es por servicio).
    sqlx::query("INSERT INTO accounts (service_id, email, external_user_id) VALUES (?, 'a@x.com', 'user-1')")
        .bind(qobuz)
        .execute(&pool)
        .await
        .expect("first user id insert");
    sqlx::query("INSERT INTO accounts (service_id, email, external_user_id) VALUES (?, 't@x.com', 'user-1')")
        .bind(tidal)
        .execute(&pool)
        .await
        .expect("same user id on another service is fine");

    // Mismo user_id dos veces en el MISMO servicio: rechazado.
    let duplicate = sqlx::query(
        "INSERT INTO accounts (service_id, email, external_user_id) VALUES (?, 'b@x.com', 'user-1')",
    )
    .bind(qobuz)
    .execute(&pool)
    .await;
    assert!(
        duplicate.is_err(),
        "dos filas del mismo servicio no pueden compartir external_user_id"
    );
}

#[tokio::test]
async fn test_email_normalization_is_symmetric_and_never_violates_unique() {
    let pool = pool_pre_0089().await;

    let (qobuz,): (i64,) = sqlx::query_as("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .expect("service row");

    // Residuos case-variantes: la UNIQUE(service_id, email) de 0002 es BINARY,
    // así que estas parejas PUEDEN convivir en una BD previa.
    sqlx::query("INSERT INTO accounts (service_id, email) VALUES (?, 'A@x.com')")
        .bind(qobuz)
        .execute(&pool)
        .await
        .expect("seed A");
    sqlx::query("INSERT INTO accounts (service_id, email) VALUES (?, 'a@x.com')")
        .bind(qobuz)
        .execute(&pool)
        .await
        .expect("seed lowercase twin");
    // Pareja que difiere además por ESPACIADO: solo la comparación simétrica
    // (TRIM también del lado `dup`) la detecta. Con la versión asimétrica, esta
    // fila se normalizaría a 'solo@x.com' y chocaría con su hermana.
    sqlx::query("INSERT INTO accounts (service_id, email) VALUES (?, ' Solo@X.COM ')")
        .bind(qobuz)
        .execute(&pool)
        .await
        .expect("seed padded");
    sqlx::query("INSERT INTO accounts (service_id, email) VALUES (?, 'SOLO@x.com')")
        .bind(qobuz)
        .execute(&pool)
        .await
        .expect("seed padded twin");
    // Una fila SIN colisión: sí debe normalizarse.
    sqlx::query("INSERT INTO accounts (service_id, email) VALUES (?, ' Unico@Example.COM ')")
        .bind(qobuz)
        .execute(&pool)
        .await
        .expect("seed unique");

    // Sin error ni violación de UNIQUE durante el UPDATE.
    sqlx::raw_sql(M0089)
        .execute(&pool)
        .await
        .expect("normalization never violates UNIQUE(service_id, email)");

    let emails: Vec<String> =
        sqlx::query_scalar("SELECT email FROM accounts WHERE service_id = ? ORDER BY id")
            .bind(qobuz)
            .fetch_all(&pool)
            .await
            .expect("emails readable");

    assert_eq!(
        emails.len(),
        5,
        "la normalización no borra ni fusiona filas"
    );
    assert_eq!(
        emails[0], "A@x.com",
        "la pareja case-variante queda como está: normalizar una crearía la UNIQUE"
    );
    assert_eq!(emails[1], "a@x.com");
    assert_eq!(
        emails[2], " Solo@X.COM ",
        "el espaciado detecta la colisión simétricamente (TRIM en ambos lados) y NO se normaliza"
    );
    assert_eq!(emails[3], "SOLO@x.com");
    assert_eq!(
        emails[4], "unico@example.com",
        "una fila sin colisión sí se normaliza a LOWER(TRIM(...))"
    );
}

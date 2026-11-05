//! D00 (#112): esquema base de jobs, evidencia y preferencias.
//!
//! Mina de AGENTS: una BD `:memory:` recién migrada no demuestra el upgrade,
//! así que el fixture de esquema anterior replica el estado previo a 0102/0103
//! a mano (convención de 0002 + 0046 + 0050 + 0052 + 0060 + 0062 para la cola,
//! 0085 para el ledger) y ejecuta los ficheros reales de migración encima.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Row, Sqlite};

const MIGRATION_0102: &str =
    include_str!("../migrations/0102_download_jobs_entitlements_preferred_accounts.sql");
const MIGRATION_0103: &str =
    include_str!("../migrations/0103_download_job_and_account_columns.sql");

/// Esquema legacy: padres reales + `download_queue` (0004 + ALTERs 0046/0050/0052/0060/0062)
/// + `downloads` (forma de 0085), todo sin las columnas de 0103.
const LEGACY_SCHEMA: &str = r#"
CREATE TABLE services (
    id INTEGER PRIMARY KEY,
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
    UNIQUE(service_id, email)
);

CREATE TABLE albums (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL
);

CREATE TABLE tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    album_id INTEGER REFERENCES albums(id) ON DELETE SET NULL,
    duration_ms INTEGER,
    track_number INTEGER,
    disc_number INTEGER DEFAULT 1,
    isrc TEXT UNIQUE,
    musicbrainz_id TEXT,
    explicit INTEGER DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE download_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    status TEXT DEFAULT 'queued' CHECK(status IN ('queued', 'downloading', 'complete', 'failed', 'cancelled')),
    priority INTEGER DEFAULT 50 CHECK(priority >= 0 AND priority <= 100),
    quality_preference TEXT CHECK(quality_preference IN ('hires', 'lossless', 'high', 'any') OR quality_preference IS NULL),
    progress_percent REAL DEFAULT 0.0 CHECK(progress_percent >= 0.0 AND progress_percent <= 100.0),
    bytes_downloaded INTEGER DEFAULT 0,
    total_bytes INTEGER,
    error_message TEXT,
    retry_count INTEGER DEFAULT 0 CHECK(retry_count >= 0),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    started_at TEXT,
    completed_at TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    staging_path TEXT,
    resumable INTEGER NOT NULL DEFAULT 1,
    last_error TEXT,
    service_id INTEGER REFERENCES services(id),
    service_name TEXT,
    service_track_id TEXT,
    service_album_id TEXT,
    target_title TEXT,
    target_artist TEXT,
    target_album TEXT,
    target_isrc TEXT,
    smart_studio_origin INTEGER NOT NULL DEFAULT 0,
    allow_fallback INTEGER NOT NULL DEFAULT 0,
    origin_service TEXT,
    origin_service_track_id TEXT,
    effective_service TEXT,
    effective_service_track_id TEXT,
    fallback_reason TEXT,
    match_method TEXT,
    match_confidence REAL,
    requested_quality TEXT,
    effective_quality TEXT,
    requested_format TEXT,
    effective_format TEXT,
    quality_decision TEXT,
    provider_fallback_used INTEGER NOT NULL DEFAULT 0,
    quality_fallback_used INTEGER NOT NULL DEFAULT 0,
    decision_reason TEXT,
    skip_reason TEXT
);

CREATE TABLE downloads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER UNIQUE REFERENCES tracks(id) ON DELETE SET NULL,
    source_service_id INTEGER REFERENCES services(id),
    file_path TEXT NOT NULL,
    file_format TEXT CHECK(file_format IN ('FLAC', 'ALAC', 'WAV', 'MP3', 'AAC', 'OGG', 'OPUS') OR file_format IS NULL),
    file_size_bytes INTEGER CHECK(file_size_bytes >= 0 OR file_size_bytes IS NULL),
    file_hash TEXT,
    bit_depth INTEGER CHECK(bit_depth IN (16, 24, 32) OR bit_depth IS NULL),
    sample_rate INTEGER CHECK(sample_rate > 0 OR sample_rate IS NULL),
    metadata_completeness INTEGER DEFAULT 0 CHECK(metadata_completeness >= 0 AND metadata_completeness <= 100),
    downloaded_at TEXT DEFAULT CURRENT_TIMESTAMP,
    musicbrainz_release_id TEXT,
    updated_at TEXT,
    origin_service TEXT,
    origin_service_track_id TEXT,
    effective_service TEXT,
    effective_service_track_id TEXT,
    fallback_reason TEXT,
    match_method TEXT,
    match_confidence REAL,
    file_disambiguator TEXT,
    requested_quality TEXT,
    effective_quality TEXT,
    requested_format TEXT,
    effective_format TEXT,
    quality_decision TEXT,
    provider_fallback_used INTEGER NOT NULL DEFAULT 0,
    quality_fallback_used INTEGER NOT NULL DEFAULT 0,
    decision_reason TEXT,
    skip_reason TEXT
);
"#;

const LEGACY_ROWS: &str = r#"
INSERT INTO services (id, name, supports_download) VALUES (1, 'qobuz', 1);
INSERT INTO accounts (id, service_id, display_name) VALUES (1, 1, 'Principal');
INSERT INTO tracks (id, title, isrc) VALUES (1, 'Canción uno', 'USXXX0000001');
INSERT INTO tracks (id, title, isrc) VALUES (2, 'Canción dos', 'USXXX0000002');
INSERT INTO download_queue (id, track_id, status, position, service_name) VALUES (10, 1, 'queued', 0, 'qobuz');
INSERT INTO download_queue (id, track_id, status, position, service_name) VALUES (11, 2, 'failed', 1, 'qobuz');
INSERT INTO downloads (id, track_id, file_path) VALUES (20, 1, '/tmp/fake/uno.flac');
"#;

async fn fresh_pool() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}

async fn column_names(pool: &Pool<Sqlite>, table: &str) -> Vec<String> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await
        .unwrap();
    rows.iter().map(|r| r.get::<String, _>("name")).collect()
}

#[tokio::test]
async fn fresh_db_creates_jobs_entitlements_and_preferred_accounts() {
    let pool = fresh_pool().await;

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table'
         AND name IN ('download_jobs', 'download_entitlements', 'service_preferred_accounts')
         ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        tables,
        vec![
            "download_entitlements",
            "download_jobs",
            "service_preferred_accounts"
        ]
    );

    let queue_columns = column_names(&pool, "download_queue").await;
    for column in [
        "job_id",
        "next_attempt_at",
        "failure_class",
        "resolved_account_id",
    ] {
        assert!(
            queue_columns.iter().any(|c| c == column),
            "falta {column} en download_queue"
        );
    }
    let downloads_columns = column_names(&pool, "downloads").await;
    assert!(
        downloads_columns.iter().any(|c| c == "account_id"),
        "falta account_id en downloads"
    );

    let indexes: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'index'
         AND name IN (
             'idx_download_entitlements_track_service',
             'idx_download_entitlements_account_expiry',
             'idx_download_queue_job',
             'idx_download_queue_next_attempt',
             'idx_downloads_account'
         )",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(indexes.len(), 5, "faltan índices de 0102/0103: {indexes:?}");

    // Padres para los inserts: 'qobuz' ya viene sembrado por las migraciones.
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'qobuz'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name) VALUES (?, 'Principal') RETURNING id",
    )
    .bind(service_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO tracks (id, title) VALUES (1, 'Canción')")
        .execute(&pool)
        .await
        .unwrap();

    // Un job válido se crea con defaults y contadores a cero.
    let job_id: i64 = sqlx::query_scalar(
        "INSERT INTO download_jobs (kind, label, requested_count, estimated_count)
         VALUES ('playlist', 'Mi playlist', 10, 10) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (status, completed, failed, skipped): (String, i64, i64, i64) = sqlx::query_as(
        "SELECT status, completed_count, failed_count, skipped_count FROM download_jobs WHERE id = ?",
    )
    .bind(job_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (status.as_str(), completed, failed, skipped),
        ("running", 0, 0, 0)
    );

    // CHECKs de job: kind y status cerrados.
    assert!(
        sqlx::query("INSERT INTO download_jobs (kind) VALUES ('nope')")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("INSERT INTO download_jobs (kind, status) VALUES ('single', 'pending')")
            .execute(&pool)
            .await
            .is_err()
    );

    // Entitlement: UNIQUE(track_id, service_name, account_id) y CHECKs de state/source.
    let entitlement = format!(
        "INSERT INTO download_entitlements (track_id, service_name, account_id, state, source, expires_at)
         VALUES (1, 'qobuz', {account_id}, 'granted', 'delivered', '2027-01-01T00:00:00Z')"
    );
    sqlx::query(&entitlement).execute(&pool).await.unwrap();
    assert!(
        sqlx::query(&entitlement).execute(&pool).await.is_err(),
        "UNIQUE no aplicado"
    );
    assert!(sqlx::query(
        "INSERT INTO download_entitlements (track_id, service_name, account_id, state)
         VALUES (1, 'qobuz', ?, 'maybe')"
    )
    .bind(account_id)
    .execute(&pool)
    .await
    .is_err());
    assert!(sqlx::query(
        "INSERT INTO download_entitlements (track_id, service_name, account_id, state, source)
         VALUES (1, 'tidal', ?, 'granted', 'delivered-ok')"
    )
    .bind(account_id)
    .execute(&pool)
    .await
    .is_err());

    // Cuenta preferida: service_name único y FK a cuentas.
    sqlx::query(
        "INSERT INTO service_preferred_accounts (service_name, account_id) VALUES ('qobuz', ?)",
    )
    .bind(account_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(sqlx::query(
        "INSERT INTO service_preferred_accounts (service_name, account_id) VALUES ('qobuz', ?)"
    )
    .bind(account_id)
    .execute(&pool)
    .await
    .is_err());
    assert!(
        sqlx::query("INSERT INTO service_preferred_accounts (service_name, account_id) VALUES ('tidal', 999)")
            .execute(&pool)
            .await
            .is_err(),
        "FK a accounts no aplicada"
    );

    // El job_id de la cola apunta de verdad al job.
    sqlx::query("INSERT INTO download_queue (track_id, status, job_id) VALUES (1, 'queued', ?)")
        .bind(job_id)
        .execute(&pool)
        .await
        .unwrap();
    let linked: Option<i64> =
        sqlx::query_scalar("SELECT job_id FROM download_queue WHERE job_id IS NOT NULL")
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert_eq!(linked, Some(job_id));
}

#[tokio::test]
async fn legacy_schema_migrates_without_losing_rows() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(LEGACY_SCHEMA).execute(&pool).await.unwrap();
    sqlx::raw_sql(LEGACY_ROWS).execute(&pool).await.unwrap();

    // Upgrade con los ficheros reales de migración, no con un esquema paralelo.
    sqlx::raw_sql(MIGRATION_0102).execute(&pool).await.unwrap();
    sqlx::raw_sql(MIGRATION_0103).execute(&pool).await.unwrap();

    let (queue_rows, legacy_without_job): (i64, i64) =
        sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(job_id IS NULL), 0) FROM download_queue")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (queue_rows, legacy_without_job),
        (2, 2),
        "filas legacy perdidas o con job_id"
    );

    let legacy_download_without_account: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE account_id IS NULL")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(legacy_download_without_account, 1);

    // El esquema nuevo funciona sobre datos legacy.
    let job_id: i64 = sqlx::query_scalar(
        "INSERT INTO download_jobs (kind, label) VALUES ('favorites', 'Favoritos') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE download_queue SET job_id = ?, resolved_account_id = 1 WHERE id = 10")
        .bind(job_id)
        .execute(&pool)
        .await
        .unwrap();
    let linked: (Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT job_id, resolved_account_id FROM download_queue WHERE id = 10")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(linked, (Some(job_id), Some(1)));
    let still_legacy: Option<i64> =
        sqlx::query_scalar("SELECT job_id FROM download_queue WHERE id = 11")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(still_legacy, None);

    sqlx::query(
        "INSERT INTO download_entitlements (track_id, service_name, account_id, state, source)
         VALUES (2, 'qobuz', 1, 'unknown', NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn download_queue_status_check_is_unchanged() {
    let pool = fresh_pool().await;

    sqlx::query("INSERT INTO tracks (id, title) VALUES (1, 'Canción')")
        .execute(&pool)
        .await
        .unwrap();

    // Los estados nuevos de job NO son estados de cola: el CHECK de 0004 sigue.
    assert!(
        sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (1, 'running')")
            .execute(&pool)
            .await
            .is_err(),
        "download_queue aceptó 'running': el CHECK cambió"
    );

    sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (1, 'complete')")
        .execute(&pool)
        .await
        .unwrap();
}

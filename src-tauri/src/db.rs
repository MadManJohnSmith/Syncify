//! Database module for SQLite connection and queries
// RECOMPILE_TIMESTAMP: 2026-04-26 15:10

use sqlx::migrate::{MigrateError, Migrator};
use sqlx::{sqlite::SqlitePoolOptions, Executor, Pool, Sqlite};
use std::path::PathBuf;
use tauri::Manager;

/// Database connection pool
pub type DbPool = Pool<Sqlite>;

/// Migraciones embebidas en el binario (`sqlx::migrate!`).
///
/// ═══════════════════════════════════════════════════════════════════════
/// POLÍTICA DE MIGRACIONES (BD-8, auditoría post-mortem 2026-10)
///
/// UNA MIGRACIÓN YA APLICADA ES INMUTABLE. sqlx identifica cada migración por
/// (versión, checksum SHA-384 del archivo) y valida ese checksum en CADA
/// arranque contra lo registrado en `_sqlx_migrations`
/// (sqlx-core migrate/migrator.rs: `migration.checksum != applied.checksum`
/// → `VersionMismatch`). Editar un archivo ya aplicado, borrarlo y reusar su
/// número, o reordenar versiones rompe TODAS las BDs que registraron la
/// variante previa ("migration was previously applied but has been modified")
/// y la app deja de arrancar.
///
/// Por tanto: cualquier cambio de esquema o de datos correctivo → UNA
/// MIGRACIÓN NUEVA con el siguiente número libre. Si una migración ya
/// aplicada quedó mal, NO se corrige in situ: se añade otra que corrija su
/// efecto.
///
/// Histórico conocido de violaciones (cuya reparación de arranque vive en
/// `repair_recycled_migrations` / `apply_recycled_migration_effects`):
///   - 0042: la original (0042_add_qobuz_id_to_albums.sql) fue borrada y su
///     número reusado por 0042_fix_tidal_album_artwork_urls.sql en 19fa05d.
///   - 0064: editada in situ en eb7c7dd (se añadió la deduplicación de ISRCs).
/// ═══════════════════════════════════════════════════════════════════════
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Versiones cuya migración fue reciclada/editada después de que existieran
/// BDs que registraron la variante previa. SOLO estas versiones son
/// susceptibles de la reparación automática de checksum del arranque; un
/// mismatch en cualquier otra versión falla de forma explícita (nunca se
/// acepta en silencio reescribiendo checksums de migraciones desconocidas).
const RECYCLED_MIGRATION_VERSIONS: &[i64] = &[42, 64];

/// Initialize the database connection pool
pub async fn init_db(app_handle: &tauri::AppHandle) -> Result<DbPool, sqlx::Error> {
    let db_path = get_db_path(app_handle).await;
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    tracing::info!("Connecting to database: {}", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(10) // Increase pool size for concurrent operations
        .acquire_timeout(std::time::Duration::from_secs(10)) // Timeout on pool acquire
        // Enable foreign key enforcement on EVERY connection
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                conn.execute("PRAGMA foreign_keys = ON;").await?;
                conn.execute("PRAGMA journal_mode = WAL;").await?; // Better concurrency
                conn.execute("PRAGMA busy_timeout = 30000;").await?; // 30 second timeout for parallel imports
                conn.execute("PRAGMA wal_autocheckpoint = 1000;").await?; // Checkpoint every 1000 pages
                tracing::debug!("SQLite pragmas enabled");
                Ok(())
            })
        })
        .connect(&db_url)
        .await?;

    // Run migrations if needed (con reparación de arranque BD-8; ver MIGRATOR)
    run_migrations(&pool).await?;

    tracing::info!("Database initialized successfully");
    Ok(pool)
}

/// Ejecuta las migraciones embebidas con reparación de arranque BD-8.
///
/// Flujo: `MIGRATOR.run` → si falla con `VersionMismatch`, repara los
/// checksums registrados de las versiones recicladas conocidas (42 y 64) y
/// re-registra 0063 si su efecto ya existía en la BD → reintenta `run` (que
/// valida de nuevo y aplica migraciones pendientes) → re-aplica de forma
/// idempotente los efectos de las versiones reparadas que una BD antigua aún
/// no tenga. Cualquier fallo posterior se propaga: el arranque falla de forma
/// explícita en lugar de arrancar con una BD en estado dudoso.
async fn run_migrations(pool: &DbPool) -> Result<(), sqlx::Error> {
    match MIGRATOR.run(pool).await {
        Ok(()) => Ok(()),
        Err(MigrateError::VersionMismatch(reported)) => {
            tracing::warn!(
                reported_version = reported,
                "BD-8: migración previamente aplicada tiene un checksum distinto al embebido — \
                 iniciando reparación de arranque por migración reciclada/editada"
            );
            let repaired = repair_recycled_migrations(pool).await?;
            if repaired.is_empty() {
                // Mismatch en una versión sin reparación conocida: por la
                // política documentada en `MIGRATOR` NO se acepta en silencio.
                return Err(MigrateError::VersionMismatch(reported).into());
            }
            // Segunda pasada: valida los checksums ya corregidos y aplica las
            // migraciones pendientes que antes no se podían alcanzar.
            MIGRATOR.run(pool).await?;
            // Efectos de las versiones reparadas que la BD antigua aún no
            // tiene (p. ej. la deduplicación de ISRCs añadida a 0064 en
            // eb7c7dd). Va DESPUÉS del re-run porque necesita el esquema
            // completo (albums.tidal_id de 0037, etc.).
            apply_recycled_migration_effects(pool, &repaired).await?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

/// Corrige el checksum registrado en `_sqlx_migrations` de las versiones
/// recicladas/editadas conocidas (BD-8) para que coincida con la migración
/// embebida actual. Devuelve la lista de versiones reparadas; vacío si el
/// mismatch pertenece a versiones desconocidas (el llamante fallará con el
/// `VersionMismatch` original). Cada corrección se loguea con ambos checksums.
async fn repair_recycled_migrations(pool: &DbPool) -> Result<Vec<i64>, sqlx::Error> {
    let applied: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await?;

    let mut repaired = Vec::new();
    for (version, recorded) in applied {
        let Some(embedded) = MIGRATOR.iter().find(|m| m.version == version) else {
            // Registrada pero ausente del árbol actual: el re-run de MIGRATOR
            // lo reportará como VersionMissing; no es reparable reescribiendo
            // checksums (implicaría aceptar el borrado de una migración).
            continue;
        };
        if embedded.checksum.as_ref() == recorded.as_slice() {
            continue;
        }
        if !RECYCLED_MIGRATION_VERSIONS.contains(&version) {
            tracing::error!(
                version,
                recorded_checksum = %hex_checksum(&recorded),
                "BD-8: versión de migración modificada SIN reparación conocida — no se \
                 reescribe su checksum (política: migración aplicada = inmutable; fallar \
                 explícito en vez de aceptación silenciosa)"
            );
            continue;
        }
        tracing::warn!(
            version,
            recorded_checksum = %hex_checksum(&recorded),
            embedded_checksum = %hex_checksum(embedded.checksum.as_ref()),
            "BD-8: corrigiendo checksum registrado de migración reciclada/editada"
        );
        sqlx::query("UPDATE _sqlx_migrations SET checksum = ?1 WHERE version = ?2")
            .bind(embedded.checksum.as_ref())
            .bind(version)
            .execute(pool)
            .await?;
        repaired.push(version);
    }

    if repaired.contains(&42) {
        ensure_qobuz_id_migration_recorded(pool).await?;
    }
    Ok(repaired)
}

/// Consecuencia directa de la 0042 reciclada: la variante original
/// (0042_add_qobuz_id_to_albums.sql) ya había creado `albums.qobuz_id`, pero
/// esas BDs nunca llegaron a registrar la 0063 actual (que añade la misma
/// columna). Sin este paso, el re-run de MIGRATOR fallaría en 0063 con
/// "duplicate column name: qobuz_id". Si el efecto real ya existe (la columna
/// está presente) y 0063 no está registrada: se crea su índice (IF NOT EXISTS,
/// mismo nombre y definición que declara 0063) y se registra 0063 como
/// aplicada, con el mismo INSERT que usa sqlx al aplicar una migración
/// (sqlx-sqlite 0.8.6 src/migrate.rs: INSERT INTO _sqlx_migrations(version,
/// description, success, checksum, execution_time) VALUES (..., TRUE, ..., -1)).
async fn ensure_qobuz_id_migration_recorded(pool: &DbPool) -> Result<(), sqlx::Error> {
    let column_exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('albums') WHERE name = 'qobuz_id'",
    )
    .fetch_one(pool)
    .await?;
    if column_exists == 0 {
        // BD normal: la 0063 se aplicará normalmente durante el re-run.
        return Ok(());
    }
    let recorded: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE version = 63")
            .fetch_one(pool)
            .await?;
    if recorded > 0 {
        return Ok(());
    }

    let embedded = MIGRATOR
        .iter()
        .find(|m| m.version == 63)
        .expect("la migración 0063 está embebida en el binario");
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_albums_qobuz_id \
         ON albums(qobuz_id) WHERE qobuz_id IS NOT NULL",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time) \
         VALUES (?1, ?2, TRUE, ?3, -1)",
    )
    .bind(63i64)
    .bind(embedded.description.as_ref())
    .bind(embedded.checksum.as_ref())
    .execute(pool)
    .await?;
    tracing::warn!(
        "BD-8: albums.qobuz_id ya existía (creado por la 0042 original reciclada) y 0063 no \
         estaba registrada — 0063 registrada como aplicada para evitar 'duplicate column'"
    );
    Ok(())
}

/// Re-aplica de forma idempotente los efectos de las versiones reparadas que
/// la BD aún no tenga. Solo se invoca tras un re-run exitoso de MIGRATOR, así
/// que el esquema completo ya existe.
async fn apply_recycled_migration_effects(
    pool: &DbPool,
    repaired: &[i64],
) -> Result<(), sqlx::Error> {
    if repaired.contains(&42) {
        apply_0042_effect(pool).await?;
    }
    if repaired.contains(&64) {
        apply_0064_effect(pool).await?;
    }
    Ok(())
}

/// Re-aplica de forma idempotente el efecto de la 0042 ACTUAL (normalización
/// de URLs de artwork de Tidal): la cláusula WHERE
/// (`cover_art_url NOT LIKE 'http%'`) la vuelve no-op sobre URLs ya
/// convertidas. Espeja el UPDATE de
/// migrations/0042_fix_tidal_album_artwork_urls.sql.
async fn apply_0042_effect(pool: &DbPool) -> Result<(), sqlx::Error> {
    let result = sqlx::query(
        "UPDATE albums \
         SET cover_art_url = \
           'https://resources.tidal.com/images/' || \
           REPLACE(cover_art_url, '-', '/') || \
           '/320x320.jpg' \
         WHERE tidal_id IS NOT NULL \
           AND cover_art_url IS NOT NULL \
           AND cover_art_url NOT LIKE 'http%'",
    )
    .execute(pool)
    .await?;
    tracing::info!(
        rows = result.rows_affected(),
        "BD-8: efecto de la 0042 re-aplicado idempotentemente (artwork Tidal normalizado)"
    );
    Ok(())
}

/// Efectos de la 0064 ACTUAL, verificados y re-aplicados de forma idempotente
/// para BDs que registraron la variante previa a eb7c7dd (que NO deduplicaba
/// ISRCs antes de crear el índice único NOCASE, y sí creó el resto: constraint
/// posicional de playlist_tracks, índice de track_sources y calidad de
/// SoundCloud). Todo en UNA transacción: si algo falla se revierte completo y
/// el arranque falla de forma explícita.
///
/// Las sentencias espejan 1:1
/// migrations/0064_pipeline_hardening_and_integrity.sql (secciones 1-4). La
/// deduplicación (sección 2) SOLO se ejecuta si existen colisiones reales
/// sobre el ISRC normalizado; sin colisiones sus DELETE masivos no tienen
/// nada que hacer y no se ejecutan en absoluto.
async fn apply_0064_effect(pool: &DbPool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    // ── Sección 1: UNIQUE(playlist_id, position) en playlist_tracks ──
    if !playlist_tracks_has_position_unique(&mut tx).await? {
        for stmt in PLAYLIST_TRACKS_REBUILD {
            sqlx::raw_sql(stmt).execute(&mut *tx).await?;
        }
        tracing::info!("BD-8: playlist_tracks reconstruida con UNIQUE(playlist_id, position)");
    }
    for stmt in PLAYLIST_TRACKS_INDEXES {
        sqlx::raw_sql(stmt).execute(&mut *tx).await?;
    }

    // ── Sección 2: deduplicación de ISRCs (solo si hay colisiones normalizadas) ──
    let collisions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ( \
           SELECT 1 FROM tracks WHERE isrc IS NOT NULL \
           GROUP BY UPPER(REPLACE(TRIM(isrc), '-', '')) HAVING COUNT(*) > 1 \
         )",
    )
    .fetch_one(&mut *tx)
    .await?;
    if collisions > 0 {
        tracing::warn!(
            collision_groups = collisions,
            "BD-8: colisiones de ISRC normalizado detectadas — aplicando la deduplicación de 0064"
        );
        for stmt in ISRC_DEDUP {
            sqlx::raw_sql(stmt).execute(&mut *tx).await?;
        }
    }
    for stmt in ISRC_NORMALIZE_AND_INDEX {
        sqlx::raw_sql(stmt).execute(&mut *tx).await?;
    }

    // ── Secciones 3 y 4: índice único de origen en track_sources y calidad SoundCloud ──
    for stmt in TRACK_SOURCES_UNIQUE_AND_SOUNDCLOUD {
        sqlx::raw_sql(stmt).execute(&mut *tx).await?;
    }

    tx.commit().await?;
    tracing::info!("BD-8: efectos de la 0064 verificados/re-aplicados idempotentemente");
    Ok(())
}

/// true si playlist_tracks ya tiene una restricción UNIQUE (origin 'u', no
/// parcial) sobre exactamente (playlist_id, position) — el efecto de la
/// sección 1 de 0064.
async fn playlist_tracks_has_position_unique(
    conn: &mut sqlx::SqliteConnection,
) -> Result<bool, sqlx::Error> {
    let indexes: Vec<(String, String, i64)> = sqlx::query_as(
        r#"SELECT name, origin, "partial" FROM pragma_index_list('playlist_tracks')"#,
    )
    .fetch_all(&mut *conn)
    .await?;
    for (name, origin, partial) in indexes {
        if origin != "u" || partial != 0 {
            continue;
        }
        let columns: Vec<String> =
            sqlx::query_as("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")
                .bind(&name)
                .fetch_all(&mut *conn)
                .await?
                .into_iter()
                .map(|(column,)| column)
                .collect();
        if columns == ["playlist_id", "position"] {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Sección 1 de 0064, verbatim: reconstrucción de playlist_tracks con
/// UNIQUE(playlist_id, position). Solo se ejecuta si la tabla aún no la tiene.
const PLAYLIST_TRACKS_REBUILD: &[&str] = &[
    "CREATE TABLE playlist_tracks_new (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        position INTEGER NOT NULL DEFAULT 0,
        added_at TEXT,
        UNIQUE(playlist_id, position)
    )",
    "INSERT OR IGNORE INTO playlist_tracks_new (id, playlist_id, track_id, position, added_at)
     SELECT id, playlist_id, track_id, position, added_at FROM playlist_tracks",
    "DROP TABLE playlist_tracks",
    "ALTER TABLE playlist_tracks_new RENAME TO playlist_tracks",
];

const PLAYLIST_TRACKS_INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist ON playlist_tracks(playlist_id)",
    "CREATE INDEX IF NOT EXISTS idx_playlist_tracks_track ON playlist_tracks(track_id)",
    "CREATE INDEX IF NOT EXISTS idx_playlist_tracks_pos ON playlist_tracks(playlist_id, position)",
];

/// Sección 2 de 0064 (2..2l), verbatim: deduplicación y fusión de tracks que
/// colisionan por ISRC normalizado. Ejecutarla de nuevo tras haber corrido es
/// no-op (el mapa saldría vacío), pero igualmente se guarda tras la compuerta
/// de colisiones de `apply_0064_effect`.
const ISRC_DEDUP: &[&str] = &[
    "DROP TABLE IF EXISTS _isrc_dedup_map",
    "CREATE TEMP TABLE _isrc_dedup_map AS
     WITH ranked AS (
       SELECT
         id,
         spotify_id,
         UPPER(REPLACE(TRIM(isrc), '-', '')) AS norm_isrc,
         ROW_NUMBER() OVER (
           PARTITION BY UPPER(REPLACE(TRIM(isrc), '-', ''))
           ORDER BY
             (SELECT count(*) FROM downloads d WHERE d.track_id = tracks.id) DESC,
             (isrc = UPPER(REPLACE(TRIM(isrc), '-', ''))) DESC,
             (SELECT count(*) FROM track_sources ts WHERE ts.track_id = tracks.id) DESC,
             id ASC
         ) AS rn
       FROM tracks
       WHERE isrc IS NOT NULL
     )
     SELECT
       loser.id AS loser_id,
       winner.id AS winner_id,
       loser.spotify_id AS loser_spotify_id
     FROM ranked loser
     JOIN ranked winner ON loser.norm_isrc = winner.norm_isrc AND winner.rn = 1
     WHERE loser.rn > 1",
    // 2a. fusionar metadata y favoritos en la track ganadora
    "UPDATE tracks
     SET
       is_favorite = MAX(tracks.is_favorite, (SELECT COALESCE(MAX(l.is_favorite), 0) FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id)),
       favorite_at = COALESCE(tracks.favorite_at, (SELECT l.favorite_at FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id AND l.favorite_at IS NOT NULL ORDER BY l.favorite_at DESC LIMIT 1)),
       album_id = COALESCE(tracks.album_id, (SELECT l.album_id FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id AND l.album_id IS NOT NULL LIMIT 1)),
       musicbrainz_id = COALESCE(tracks.musicbrainz_id, (SELECT l.musicbrainz_id FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id AND l.musicbrainz_id IS NOT NULL LIMIT 1)),
       qobuz_id = COALESCE(tracks.qobuz_id, (SELECT l.qobuz_id FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id AND l.qobuz_id IS NOT NULL LIMIT 1)),
       genre = COALESCE(tracks.genre, (SELECT l.genre FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id AND l.genre IS NOT NULL LIMIT 1)),
       release_year = COALESCE(tracks.release_year, (SELECT l.release_year FROM tracks l JOIN _isrc_dedup_map m ON l.id = m.loser_id WHERE m.winner_id = tracks.id AND l.release_year IS NOT NULL LIMIT 1))
     WHERE id IN (SELECT winner_id FROM _isrc_dedup_map)",
    // 2b. anular campos únicos de las perdedoras
    "UPDATE tracks SET spotify_id = NULL, isrc = NULL WHERE id IN (SELECT loser_id FROM _isrc_dedup_map)",
    "UPDATE tracks
     SET spotify_id = (
       SELECT m.loser_spotify_id
       FROM _isrc_dedup_map m
       WHERE m.winner_id = tracks.id AND m.loser_spotify_id IS NOT NULL
       LIMIT 1
     )
     WHERE id IN (SELECT winner_id FROM _isrc_dedup_map)
       AND tracks.spotify_id IS NULL",
    // 2c. track_sources
    "DELETE FROM track_sources
     WHERE id NOT IN (
       SELECT MIN(ts.id)
       FROM track_sources ts
       LEFT JOIN _isrc_dedup_map m ON ts.track_id = m.loser_id
       GROUP BY COALESCE(m.winner_id, ts.track_id), ts.service_id
     )",
    "UPDATE track_sources
     SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = track_sources.track_id)
     WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map)",
    // 2d. playlist_tracks
    "UPDATE playlist_tracks
     SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = playlist_tracks.track_id)
     WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map)",
    // 2e. library_entries
    "DELETE FROM library_entries
     WHERE id NOT IN (
       SELECT MIN(le.id)
       FROM library_entries le
       LEFT JOIN _isrc_dedup_map m ON le.track_id = m.loser_id
       GROUP BY le.account_id, COALESCE(m.winner_id, le.track_id)
     )",
    "UPDATE library_entries
     SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = library_entries.track_id)
     WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map)",
    // 2f. lyrics
    "DELETE FROM lyrics
     WHERE id NOT IN (
       SELECT MIN(l.id)
       FROM lyrics l
       LEFT JOIN _isrc_dedup_map m ON l.track_id = m.loser_id
       GROUP BY COALESCE(m.winner_id, l.track_id), l.format
     )",
    "UPDATE lyrics
     SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = lyrics.track_id)
     WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map)",
    // 2g. downloads
    "DELETE FROM downloads
     WHERE id NOT IN (
       SELECT MIN(d.id)
       FROM downloads d
       LEFT JOIN _isrc_dedup_map m ON d.track_id = m.loser_id
       GROUP BY COALESCE(m.winner_id, d.track_id)
     )",
    "UPDATE downloads
     SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = downloads.track_id)
     WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map)",
    // 2h. track_artists
    "DELETE FROM track_artists
     WHERE rowid NOT IN (
       SELECT MIN(ta.rowid)
       FROM track_artists ta
       LEFT JOIN _isrc_dedup_map m ON ta.track_id = m.loser_id
       GROUP BY COALESCE(m.winner_id, ta.track_id), ta.artist_id, COALESCE(ta.role, 'primary')
     )",
    "UPDATE track_artists
     SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = track_artists.track_id)
     WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map)",
    // 2i. track_credits
    "DELETE FROM track_credits
     WHERE rowid NOT IN (
       SELECT MIN(tc.rowid)
       FROM track_credits tc
       LEFT JOIN _isrc_dedup_map m ON tc.track_id = m.loser_id
       GROUP BY COALESCE(m.winner_id, tc.track_id), tc.artist_id, tc.role
     )",
    "UPDATE track_credits
     SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = track_credits.track_id)
     WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map)",
    // 2j. download_queue
    "UPDATE download_queue
     SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = download_queue.track_id)
     WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map)",
    // 2k. enrichment_progress
    "DELETE FROM enrichment_progress
     WHERE id NOT IN (
       SELECT MIN(ep.id)
       FROM enrichment_progress ep
       LEFT JOIN _isrc_dedup_map m ON ep.track_id = m.loser_id
       GROUP BY COALESCE(m.winner_id, ep.track_id), ep.service
     )",
    "UPDATE enrichment_progress
     SET track_id = (SELECT m.winner_id FROM _isrc_dedup_map m WHERE m.loser_id = enrichment_progress.track_id)
     WHERE track_id IN (SELECT loser_id FROM _isrc_dedup_map)",
    // 2l. eliminar tracks perdedoras y soltar el mapa temporal
    "DELETE FROM tracks WHERE id IN (SELECT loser_id FROM _isrc_dedup_map)",
    "DROP TABLE IF EXISTS _isrc_dedup_map",
];

/// Secciones 2m y 2n de 0064, verbatim: normalizar todos los ISRCs restantes
/// y garantizar el índice único case-insensitive. Seguro tras la compuerta de
/// colisiones: si no quedan colisiones normalizadas, normalizar no puede
/// violar el índice.
const ISRC_NORMALIZE_AND_INDEX: &[&str] = &[
    "UPDATE tracks SET isrc = UPPER(REPLACE(TRIM(isrc), '-', '')) WHERE isrc IS NOT NULL",
    "DROP INDEX IF EXISTS idx_tracks_isrc_unique",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_tracks_isrc_unique ON tracks(isrc COLLATE NOCASE) WHERE isrc IS NOT NULL",
];

/// Secciones 3 y 4 de 0064, verbatim: dedup + índice único de origen en
/// track_sources y valores por defecto de SoundCloud. Idempotentes tal cual.
const TRACK_SOURCES_UNIQUE_AND_SOUNDCLOUD: &[&str] = &[
    "DELETE FROM track_sources WHERE id NOT IN (SELECT MIN(id) FROM track_sources GROUP BY service_id, service_track_id)",
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_track_sources_service_track_unique ON track_sources(service_id, service_track_id)",
    "UPDATE services SET max_quality = 'lossy' WHERE name = 'soundcloud'",
    "UPDATE quality_preferences SET max_quality = 'lossy', preferred_format = 'mp3' WHERE service_name = 'soundcloud'",
];

fn hex_checksum(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Classifies SQLite "database is locked" failures (SQLITE_BUSY family, code 5) out of
/// stringified `sqlx::Error`s.
///
/// S195(c) — why this error class can STILL surface even though every pooled connection
/// runs `PRAGMA journal_mode = WAL` + `PRAGMA busy_timeout = 30000` (see `init_db`):
/// SQLite does NOT invoke the busy handler for `SQLITE_BUSY_SNAPSHOT`. That is exactly
/// what a DEFERRED transaction (`sqlx` `db.begin()`) gets when it reads first and writes
/// later while another writer commits in between — its read snapshot can no longer be
/// upgraded, so the statement fails immediately regardless of busy_timeout.
/// During a library import this races for real: the background `EnrichmentWorker`
/// (upserts into `enrichment_progress`, `UPDATE tracks SET enrichment_status = ...`)
/// and other pool writers interleave with import-time catalog upserts
/// (`enrich_and_persist_sync_track`: `BEGIN` → `SELECT artists` → `INSERT artists ...`).
/// A failed transaction rolls back completely, so retrying the WHOLE operation after a
/// short backoff is safe and removes the entire error class; see
/// `commands::service::enrich_persist_with_locked_retry`.
pub fn is_sqlite_locked_error(err: &str) -> bool {
    err.contains("database is locked")
        || err.contains("database table is locked")
        || err.contains("SQLITE_BUSY")
        || err.contains("(code: 5)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s195_classifies_sqlite_locked_errors() {
        // Exact production signature from the S195 owner report (1 failure / ~8,974):
        assert!(is_sqlite_locked_error(
            "error returned from database: (code: 5) database is locked"
        ));
        assert!(is_sqlite_locked_error("database table is locked"));
        assert!(is_sqlite_locked_error("SQLITE_BUSY: pool busy"));

        // Non-locked failures must NOT be retried by callers of this classifier.
        assert!(!is_sqlite_locked_error(
            "error returned from database: (code: 2067) UNIQUE constraint failed: artists.name"
        ));
        assert!(!is_sqlite_locked_error(
            "Failed to insert artist 'X': column null"
        ));
    }
}

/// Get the database file path, creating directories and migrating if necessary
pub async fn get_db_path(app_handle: &tauri::AppHandle) -> PathBuf {
    let db_dir = app_handle
        .path()
        .app_local_data_dir()
        .expect("No app local data dir available");

    tokio::fs::create_dir_all(&db_dir).await.ok();

    let new_db_path = db_dir.join("syncify.db");

    // Migration logic from legacy CWD/exe-based path to OS-native app data path
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));

    let old_db_path = exe_dir.as_ref().map(|d| d.join("data").join("syncify.db"));

    if let Some(old_path) = old_db_path {
        if old_path.exists() && !new_db_path.exists() {
            tracing::info!("Migrating pre-existing legacy database to OS app data directory...");
            if let Err(e) = tokio::fs::rename(&old_path, &new_db_path).await {
                tracing::error!(
                    "Failed to migrate database (old path: {}): {}",
                    old_path.display(),
                    e
                );
            } else {
                tracing::info!(
                    "Database successfully migrated to {}",
                    new_db_path.display()
                );

                // Attempt to move WAL and SHM files if they exist
                if let Some(parent) = exe_dir {
                    let old_wal = parent.join("data").join("syncify.db-wal");
                    let old_shm = parent.join("data").join("syncify.db-shm");
                    let new_wal = db_dir.join("syncify.db-wal");
                    let new_shm = db_dir.join("syncify.db-shm");
                    let _ = tokio::fs::rename(&old_wal, &new_wal).await;
                    let _ = tokio::fs::rename(&old_shm, &new_shm).await;
                }
            }
        }
    }

    new_db_path
}

/// S195-fix: lock de escritor POR SERVICIO+CUENTA basado en flock del SO.
///
/// Permite sincronizar servicios DISTINTOS en paralelo (qobuz + tidal + spotify
/// a la vez) pero impide dos syncs simultáneos del MISMO servicio+cuenta,
/// vengan de la misma ventana o de otra instancia de la app (el bug real de la
/// noche 2026-08-25: dos instancias vivas → tormenta SQLITE_BUSY que agotó los
/// reintentos y descartó pistas de playlists de Qobuz).
#[derive(Debug)]
pub struct SyncWriterLock {
    // El File se conserva vivo solo para que el flock persista hasta el Drop;
    // su valor nunca se lee directamente (de ahí el prefijo `_`).
    _file: std::fs::File,
}

impl SyncWriterLock {
    /// Intenta adquirir el lock `sync-<service>-<account>.writer.lock` junto a
    /// la base de datos. Reintenta ~4s (8 × 500ms) antes de rendirse; devuelve
    /// Err con mensaje accionable si otro sync idéntico sigue activo.
    /// `Ok(None)` si la BD es in-memory (sin contención multi-proceso posible).
    pub async fn acquire(
        db: &sqlx::SqlitePool,
        service_name: &str,
        account_id: i64,
    ) -> Result<Option<SyncWriterLock>, String> {
        let rows = sqlx::query("PRAGMA database_list")
            .fetch_all(db)
            .await
            .map_err(|e| format!("No pude resolver ruta de la BD para el lock: {}", e))?;
        let db_file: Option<String> = rows.iter().find_map(|row| {
            // PRAGMA database_list: (seq, name, file)
            let file: String = sqlx::Row::try_get(row, 2).ok()?;
            (!file.is_empty()).then_some(file)
        });
        let Some(db_file) = db_file else {
            tracing::debug!("[S195-fix] BD en memoria: lock de escritor omitido");
            return Ok(None);
        };
        let dir = std::path::Path::new(&db_file)
            .parent()
            .ok_or("La BD no tiene directorio padre")?
            .to_path_buf();
        let path = dir.join(format!("sync-{}-{}.writer.lock", service_name, account_id));

        for attempt in 0..8u32 {
            let f = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&path)
                .map_err(|e| format!("No pude abrir {}: {}", path.display(), e))?;
            match f.try_lock() {
                Ok(()) => return Ok(Some(SyncWriterLock { _file: f })),
                Err(_) => {
                    if attempt == 7 {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }
        Err(format!(
            "Ya existe una sincronización de {} (cuenta {}) en curso — probablemente en otra \
             ventana o instancia de Syncify. Espera a que termine o ciérrala antes de reintentar.",
            service_name, account_id
        ))
    }
}

#[cfg(test)]
mod migration_repair_tests {
    use super::*;
    use sha2::{Digest, Sha384};

    /// Checksum SHA-384 de la variante ORIGINAL de la 0042
    /// (0042_add_qobuz_id_to_albums.sql, borrada y reciclada en 19fa05d):
    /// `git show 19fa05d^:src-tauri/migrations/0042_add_qobuz_id_to_albums.sql | sha384sum`
    const OLD_0042_CHECKSUM_HEX: &str = "cca03671e4c9a2cbe41377b8414cf498ef8241d137efdef8bba200d76968330e1adb2082fefe636adc1614366adaee45";
    /// Checksum SHA-384 de la variante de la 0064 previa a su edición in situ
    /// en eb7c7dd:
    /// `git show eb7c7dd^:src-tauri/migrations/0064_pipeline_hardening_and_integrity.sql | sha384sum`
    const OLD_0064_CHECKSUM_HEX: &str = "ed7fdca922e4e6a66412f5b51be315b1e9c4785a5dbc1933a2cef32562f1a62e1c385a3d976f57d3c87f2a9439c301ad";

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex válido"))
            .collect()
    }

    /// BD en memoria con TODAS las migraciones actuales ya aplicadas (una sola
    /// conexión para que `sqlite::memory:` sea una única BD estable).
    async fn migrated_memory_pool() -> DbPool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool en memoria");
        MIGRATOR.run(&pool).await.expect("migraciones frescas");
        pool
    }

    async fn set_checksum(pool: &DbPool, version: i64, checksum: &[u8]) {
        sqlx::query("UPDATE _sqlx_migrations SET checksum = ?1 WHERE version = ?2")
            .bind(checksum)
            .bind(version)
            .execute(pool)
            .await
            .expect("corromper checksum registrado");
    }

    async fn stored_checksum(pool: &DbPool, version: i64) -> Vec<u8> {
        sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version = ?1")
            .bind(version)
            .fetch_one(pool)
            .await
            .expect("fila en _sqlx_migrations")
    }

    /// BD-8 (regresión): una BD que registró las variantes previas de 0042 y
    /// 0064 no debe quedarse sin arrancar por checksum mismatch. El arranque
    /// debe reescribir los checksums al embebido actual, re-registrar 0063 si
    /// su efecto ya existía y re-aplicar los efectos que la BD antigua no
    /// tenía, dejando la BD usable.
    #[tokio::test]
    async fn repairs_recycled_migrations_and_boots() {
        let pool = migrated_memory_pool().await;

        // Estado realista de una BD que registró las variantes previas:
        //  - checksums de la 0042 y 0064 ORIGINALES (hoy reciclada/editada);
        //  - albums.qobuz_id ya existe (lo creó la 0042 vieja) pero la 0063
        //    actual no llegó a registrarse;
        //  - una fila con URL de artwork relativa (la 0042 actual nunca corrió);
        //  - dos tracks que colisionan por ISRC normalizado (la deduplicación
        //    añadida a 0064 en eb7c7dd nunca corrió; el índice NOCASE de la
        //    variante vieja de 0064 las permite coexistir).
        set_checksum(&pool, 42, &unhex(OLD_0042_CHECKSUM_HEX)).await;
        set_checksum(&pool, 64, &unhex(OLD_0064_CHECKSUM_HEX)).await;
        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 63")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO albums (title, tidal_id, cover_art_url) VALUES ('T', 'tidal-1', 'abc-def-ghi')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO tracks (title, isrc) VALUES ('A', 'AB-12')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tracks (title, isrc) VALUES ('B', 'AB12')")
            .execute(&pool)
            .await
            .unwrap();

        // El arranque repara y funciona:
        run_migrations(&pool)
            .await
            .expect("el arranque debe reparar los checksums reciclados y funcionar");

        // Checksums reescritos al embebido actual (algoritmo sqlx: SHA-384 del
        // contenido del archivo — tripwire contra cambios silenciosos):
        assert_eq!(
            stored_checksum(&pool, 42).await,
            Sha384::digest(include_str!(
                "../migrations/0042_fix_tidal_album_artwork_urls.sql"
            ))
            .to_vec()
        );
        assert_eq!(
            stored_checksum(&pool, 64).await,
            Sha384::digest(include_str!(
                "../migrations/0064_pipeline_hardening_and_integrity.sql"
            ))
            .to_vec()
        );

        // 0063 re-registrada como aplicada (columna preexistente por la 0042 vieja):
        let (recorded_63,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM _sqlx_migrations WHERE version = 63 AND success = 1",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(recorded_63, 1, "0063 debe quedar registrada como aplicada");

        // Efecto 0042 re-aplicado sobre datos reales: la URL relativa se
        // convierte a absoluta y el trigger de recurrencia de 0073
        // (trg_upgrade_album_cover_art_url_update) la eleva a high-res.
        let cover: String =
            sqlx::query_scalar("SELECT cover_art_url FROM albums WHERE tidal_id = 'tidal-1'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            cover,
            "https://resources.tidal.com/images/abc/def/ghi/1280x1280.jpg"
        );

        // Efecto 0064: las colisiones de ISRC normalizado se deduplican — la
        // perdedora ('A', 'AB-12') se elimina y la ganadora conserva el ISRC
        // ya normalizado:
        let (loser_rows,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE title = 'A'")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(loser_rows, 0, "la track perdedora debe eliminarse");
        let winner_isrc: Option<String> =
            sqlx::query_scalar("SELECT isrc FROM tracks WHERE title = 'B'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(winner_isrc.as_deref(), Some("AB12"));

        // Resto de efectos 0064: constraint posicional presente e índices OK:
        let mut conn = pool.acquire().await.unwrap();
        assert!(playlist_tracks_has_position_unique(&mut conn)
            .await
            .unwrap());
        drop(conn);

        // Un segundo arranque es idempotente (ya no hay nada que reparar):
        run_migrations(&pool)
            .await
            .expect("segundo arranque tras la reparación");
    }

    /// Política BD-8: un checksum modificado en una versión SIN reparación
    /// conocida NO se acepta en silencio — el arranque falla con el error
    /// original y el checksum registrado queda intacto.
    #[tokio::test]
    async fn fails_loudly_on_unrecognized_checksum_modification() {
        let pool = migrated_memory_pool().await;
        set_checksum(&pool, 65, b"tampered-not-a-real-variant").await;

        let err = run_migrations(&pool)
            .await
            .expect_err("debe fallar de forma explícita");
        assert!(
            err.to_string()
                .contains("was previously applied but has been modified"),
            "error inesperado: {err}"
        );
        assert_eq!(
            stored_checksum(&pool, 65).await,
            b"tampered-not-a-real-variant".to_vec(),
            "no debe reescribirse el checksum de una versión desconocida"
        );
    }
}

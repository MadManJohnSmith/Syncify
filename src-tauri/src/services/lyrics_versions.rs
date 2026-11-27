//! Almacén de versiones de letras (LY08a, issue #404; ADR-0018 decisiones 1-3).
//!
//! Una **Versión de letra** vive en `lyrics_versions`. Su hueco es (pista,
//! Fuente de letras, idioma, formato) y dentro de él conviven varias versiones:
//! la huella SHA-256 del contenido las distingue. El formato describe el
//! contenido y forma parte del hueco; el orden de llegada no identifica nada.
//! La migración 0121 crea el almacén sin tocar `lyrics`; la lectura en unión,
//! la regla de Versión activa y el pin de lectura son #411.
//!
//! Este módulo es la única puerta de escritura del almacén nuevo: normaliza la
//! identidad (minúsculas, sin espacios sobrantes, `und` para idioma
//! desconocido), calcula la huella y hace idempotente la repetición de una
//! observación. No conoce IPC ni la UI; tampoco decide qué versión está activa.
//!
//! ```text
//! identidad de una versión:
//!   (track_id, provider_id, language, format) + content_hash
//!
//! active version (lo resuelve #411, no este módulo):
//!   sync_level → idioma preferido → prioridad de la Fuente
//! ```
//!
//! `provider_id` es el identificador canónico del catálogo de Fuentes
//! (`lyrics_provider_settings.provider_id`), no el nombre de presentación que
//! guarda `lyrics.source`. El pin (`pinned_at`) vive en la fila de la versión y
//! el índice parcial de la migración garantiza como mucho uno por pista.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};

/// Idioma de una versión cuando ninguna evidencia permite determinarlo.
pub const UNKNOWN_LANGUAGE: &str = "und";

/// Niveles de sincronía aceptados por el esquema, del más fino al más grueso.
pub const SYNC_LEVELS: [&str; 4] = ["syllable", "word", "line", "none"];

/// Una fila del almacén de versiones, tal como la leen los consumidores.
#[derive(Debug, Clone, PartialEq, Eq, FromRow, Serialize, Deserialize)]
pub struct LyricsVersion {
    pub id: i64,
    pub track_id: i64,
    pub provider_id: String,
    pub language: String,
    pub format: String,
    pub sync_level: String,
    pub content: String,
    pub content_hash: String,
    pub embedded_in_file: i64,
    pub obtained_at: String,
    /// `None` salvo que esta versión sea el Pin de la pista.
    pub pinned_at: Option<String>,
}

/// Datos de una observación nueva. La huella se calcula aquí a partir de
/// `content`; el llamador no puede inventarla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewLyricsVersion {
    pub track_id: i64,
    pub provider_id: String,
    /// Etiqueta BCP 47; `None` o vacío se guardan como [`UNKNOWN_LANGUAGE`].
    pub language: Option<String>,
    pub format: String,
    /// `None` o vacío se guarda como `none` (sin timings).
    pub sync_level: Option<String>,
    pub content: String,
    pub embedded_in_file: bool,
}

/// SHA-256 del contenido en hexadecimal minúscula: la misma forma que exige el
/// CHECK de la migración 0121.
pub fn content_hash(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

fn canonical_identity(value: &str, field: &str) -> Result<String, String> {
    let normalised = value.trim().to_ascii_lowercase();
    if normalised.is_empty() {
        return Err(format!("lyrics version {field} is empty"));
    }
    Ok(normalised)
}

fn canonical_language(language: Option<&str>) -> Result<String, String> {
    match language.map(str::trim).filter(|value| !value.is_empty()) {
        None => Ok(UNKNOWN_LANGUAGE.to_string()),
        Some(value) => canonical_identity(value, "language"),
    }
}

fn canonical_sync_level(sync_level: Option<&str>) -> Result<String, String> {
    let normalised = match sync_level.map(str::trim).filter(|value| !value.is_empty()) {
        None => "none".to_string(),
        Some(value) => value.to_ascii_lowercase(),
    };
    if SYNC_LEVELS.contains(&normalised.as_str()) {
        Ok(normalised)
    } else {
        Err(format!(
            "lyrics version sync_level '{normalised}' is not one of {SYNC_LEVELS:?}"
        ))
    }
}

/// Guarda una observación y devuelve la fila resultante.
///
/// Repetir la misma observación (misma identidad y mismo contenido) es
/// idempotente: conserva `id` y `obtained_at` y solo refresca el Nivel de
/// sincronía y la marca de embebido. Un contenido distinto en el mismo hueco
/// crea otra versión, que es el caso que el almacén antiguo no podía guardar.
pub async fn record_version(
    db: &SqlitePool,
    version: &NewLyricsVersion,
) -> Result<LyricsVersion, String> {
    let provider_id = canonical_identity(&version.provider_id, "provider_id")?;
    let language = canonical_language(version.language.as_deref())?;
    let format = canonical_identity(&version.format, "format")?;
    let sync_level = canonical_sync_level(version.sync_level.as_deref())?;
    let hash = content_hash(&version.content);

    sqlx::query_as::<_, LyricsVersion>(
        r#"
        INSERT INTO lyrics_versions
            (track_id, provider_id, language, format, sync_level, content,
             content_hash, embedded_in_file)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(track_id, provider_id, language, format, content_hash)
        DO UPDATE SET
            sync_level = excluded.sync_level,
            embedded_in_file = excluded.embedded_in_file
        RETURNING id, track_id, provider_id, language, format, sync_level, content,
                  content_hash, embedded_in_file, obtained_at, pinned_at
        "#,
    )
    .bind(version.track_id)
    .bind(&provider_id)
    .bind(&language)
    .bind(&format)
    .bind(&sync_level)
    .bind(&version.content)
    .bind(&hash)
    .bind(i64::from(version.embedded_in_file))
    .fetch_one(db)
    .await
    .map_err(|e| format!("lyrics version write failed: {e}"))
}

/// Historial de una pista, de la observación más reciente a la más antigua.
pub async fn list_versions(db: &SqlitePool, track_id: i64) -> Result<Vec<LyricsVersion>, String> {
    sqlx::query_as::<_, LyricsVersion>(
        r#"
        SELECT id, track_id, provider_id, language, format, sync_level, content,
               content_hash, embedded_in_file, obtained_at, pinned_at
        FROM lyrics_versions
        WHERE track_id = ?
        ORDER BY obtained_at DESC, id DESC
        "#,
    )
    .bind(track_id)
    .fetch_all(db)
    .await
    .map_err(|e| format!("lyrics version read failed: {e}"))
}

/// La versión fijada por el usuario, si hay Pin.
pub async fn pinned_version(
    db: &SqlitePool,
    track_id: i64,
) -> Result<Option<LyricsVersion>, String> {
    sqlx::query_as::<_, LyricsVersion>(
        r#"
        SELECT id, track_id, provider_id, language, format, sync_level, content,
               content_hash, embedded_in_file, obtained_at, pinned_at
        FROM lyrics_versions
        WHERE track_id = ? AND pinned_at IS NOT NULL
        "#,
    )
    .bind(track_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("lyrics version pin read failed: {e}"))
}

/// Fija una versión de la pista y retira el Pin anterior. La comprobación de
/// pertenencia y el cambio van en la misma transacción: una versión de otra
/// pista no deja la pista sin Pin.
pub async fn pin_version(
    db: &SqlitePool,
    track_id: i64,
    version_id: i64,
) -> Result<LyricsVersion, String> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| format!("lyrics version pin write failed: {e}"))?;

    let belongs: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM lyrics_versions WHERE id = ? AND track_id = ?")
            .bind(version_id)
            .bind(track_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("lyrics version pin read failed: {e}"))?;
    if belongs.is_none() {
        return Err(format!(
            "lyrics version {version_id} does not belong to track {track_id}"
        ));
    }

    sqlx::query(
        "UPDATE lyrics_versions SET pinned_at = NULL \
         WHERE track_id = ? AND pinned_at IS NOT NULL",
    )
    .bind(track_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("lyrics version pin write failed: {e}"))?;

    sqlx::query("UPDATE lyrics_versions SET pinned_at = datetime('now') WHERE id = ?")
        .bind(version_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("lyrics version pin write failed: {e}"))?;

    let pinned = sqlx::query_as::<_, LyricsVersion>(
        r#"
        SELECT id, track_id, provider_id, language, format, sync_level, content,
               content_hash, embedded_in_file, obtained_at, pinned_at
        FROM lyrics_versions
        WHERE id = ?
        "#,
    )
    .bind(version_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("lyrics version pin read failed: {e}"))?;

    tx.commit()
        .await
        .map_err(|e| format!("lyrics version pin write failed: {e}"))?;
    Ok(pinned)
}

/// Retira el Pin de la pista. Devuelve `true` si había uno.
pub async fn clear_pin(db: &SqlitePool, track_id: i64) -> Result<bool, String> {
    let result = sqlx::query(
        "UPDATE lyrics_versions SET pinned_at = NULL \
         WHERE track_id = ? AND pinned_at IS NOT NULL",
    )
    .bind(track_id)
    .execute(db)
    .await
    .map_err(|e| format!("lyrics version pin write failed: {e}"))?;
    Ok(result.rows_affected() > 0)
}

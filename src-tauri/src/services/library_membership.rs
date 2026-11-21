//! Pertenencia de biblioteca por cuenta, track y colección — L00 (issue #97).
//!
//! Única vía de escritura de `library_memberships` (migración 0112) y del
//! materializado `tracks.library_added_at`. El sync (motor de enriquecimiento
//! y brazos), el importador local y el importador de Apple Music llaman aquí;
//! ninguna de sus costuras construye el SQL a mano.
//!
//! Reglas del dominio (ADR-0008 decisión 1, issue #97):
//! - una pertenencia es (cuenta, Track canónico, colección de origen); la
//!   misma grabación produce UNA pertenencia por cuenta y colección, no una
//!   por release — el índice único de 0112 la deduplica;
//! - `added_at` se guarda CRUDO, tal como lo da el proveedor: puede ser NULL
//!   o «1970-01-01» y el materializado los descarta;
//! - `tracks.library_added_at` se recalcula tras cada escritura de
//!   pertenencia sobre TODAS las pertenencias vivas de la pista
//!   (library_entries + playlist_tracks + library_memberships), con
//!   `tracks.created_at` como respaldo — la misma expresión del backfill de
//!   0112, definida una sola vez aquí.

use sqlx::SqlitePool;

/// Colección de origen de una pertenencia. `favorite`/`purchase`/`library`
/// no tienen fila propia (`collection_id` NULL); `playlist`/`album`/`artist`
/// guardan el id local de la colección.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MembershipCollection {
    Favorite,
    Album,
    Playlist,
    Artist,
    Purchase,
    Library,
}

impl MembershipCollection {
    pub fn as_str(self) -> &'static str {
        match self {
            MembershipCollection::Favorite => "favorite",
            MembershipCollection::Album => "album",
            MembershipCollection::Playlist => "playlist",
            MembershipCollection::Artist => "artist",
            MembershipCollection::Purchase => "purchase",
            MembershipCollection::Library => "library",
        }
    }
}

/// Una pertenencia a escribir. `added_at` es el valor CRUDO del proveedor:
/// `None` guarda NULL y no se normaliza (el materializado descarta lo nulo
/// y lo «1970-01-01»).
#[derive(Debug, Clone)]
pub struct MembershipWrite {
    pub account_id: i64,
    pub track_id: i64,
    pub collection: MembershipCollection,
    pub collection_id: Option<i64>,
    pub provider_item: Option<String>,
    pub added_at: Option<String>,
}

/// Expresión de materialización de `tracks.library_added_at`: máximo
/// `added_at` no nulo y distinto de 1970 sobre las pertenencias vivas de la
/// pista, con `created_at` como respaldo. Correlacionada sobre `tracks.id`
/// para servir al refresco por pista y al masivo; definida una vez para que
/// el mantenimiento del sync/importador y el backfill de 0112 no diverjan.
pub const LIBRARY_ADDED_AT_SET_SQL: &str = "library_added_at = COALESCE(\
    (SELECT MAX(m.added_at) FROM (\
        SELECT le.added_at AS added_at FROM library_entries le WHERE le.track_id = tracks.id \
        UNION ALL \
        SELECT pt.added_at AS added_at FROM playlist_tracks pt WHERE pt.track_id = tracks.id \
        UNION ALL \
        SELECT lm.added_at AS added_at FROM library_memberships lm WHERE lm.track_id = tracks.id \
    ) m WHERE m.added_at IS NOT NULL AND m.added_at NOT LIKE '1970-01-01%'), \
    tracks.created_at)";

/// Recalcula `tracks.library_added_at` de una pista sobre sus pertenencias
/// vivas. Un solo statement: acepta un executor de transacción (el motor la
/// llama dentro de su `BEGIN IMMEDIATE`) o el pool.
pub async fn refresh_track_library_added_at<'e, E>(
    executor: E,
    track_id: i64,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(&format!(
        "UPDATE tracks SET {LIBRARY_ADDED_AT_SET_SQL} WHERE tracks.id = ?1"
    ))
    .bind(track_id)
    .execute(executor)
    .await
    .map(|_| ())
}

/// Inserta la pertenencia (idempotente: el índice único de 0112 la
/// deduplica) SIN recalcular el materializado. Para el motor, que escribe
/// dentro de su transacción y decide cuándo refrescar.
pub async fn insert_membership<'e, E>(
    executor: E,
    write: &MembershipWrite,
) -> Result<(), sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query(
        "INSERT OR IGNORE INTO library_memberships \
         (account_id, track_id, collection_type, collection_id, provider_item, added_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(write.account_id)
    .bind(write.track_id)
    .bind(write.collection.as_str())
    .bind(write.collection_id)
    .bind(&write.provider_item)
    .bind(&write.added_at)
    .execute(executor)
    .await
    .map(|_| ())
}

/// Escribe la pertenencia y refresca el materializado en una transacción
/// propia. Es la vía para los brazos del sync y los importadores, que
/// escriben fuera de la transacción del motor.
pub async fn record_membership(
    db: &SqlitePool,
    write: &MembershipWrite,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    insert_membership(&mut *tx, write).await?;
    refresh_track_library_added_at(&mut *tx, write.track_id).await?;
    tx.commit().await
}

/// Pertenencias de colección artista: TODAS las pistas ya conocidas del
/// artista (cualquier rol: «toda aparición es evidencia», ADR-0006 #26),
/// con el ítem del proveedor que declaró el favorito. Las pistas que el
/// proveedor no expuso y no están en el catálogo no se inventan.
pub async fn record_artist_memberships(
    db: &SqlitePool,
    account_id: i64,
    artist_id: i64,
    provider_artist_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT OR IGNORE INTO library_memberships \
         (account_id, track_id, collection_type, collection_id, provider_item, added_at) \
         SELECT ?, ta.track_id, ?, ?, ?, NULL \
         FROM track_artists ta WHERE ta.artist_id = ?",
    )
    .bind(account_id)
    .bind(MembershipCollection::Artist.as_str())
    .bind(artist_id)
    .bind(provider_artist_id)
    .bind(artist_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(&format!(
        "UPDATE tracks SET {LIBRARY_ADDED_AT_SET_SQL} \
         WHERE id IN (SELECT track_id FROM track_artists WHERE artist_id = ?)"
    ))
    .bind(artist_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

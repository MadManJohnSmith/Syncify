//! Pestaña de artistas — L08 (issue #105, spec #96).
//!
//! [`query_library_artists`] comparte con [`query_library`] el texto (FTS de
//! L02), los filtros de cuenta/servicio y la paginación: la traducción de
//! esos filtros es la MISMA ([`filter_where`] de `library_query.rs`), no una
//! copia. Encima de esa base agrega por artista:
//!
//! - una fila por artista con al menos una pista VIVA en el filtro (la
//!   pertenencia es la de L00/L01: favoritos, playlists y
//!   `library_memberships`); un artista sin pistas vivas no aparece;
//! - pistas distintas, álbumes distintos y colaboraciones (pistas vivas del
//!   artista en las que participa al menos otro artista), cuentas y
//!   servicios de esas pistas, agregados en UNA consulta para todos los
//!   artistas de la página (no una por artista: sin N+1);
//! - orden por nombre, pistas o fecha añadida, con nulos al final y
//!   desempates estables;
//! - `offset`/`limit` en servidor (100 por defecto, máximo 200) y total
//!   exacto del filtro.
//!
//! [`query_library_artist_tracks`] es la MISMA consulta de pistas que
//! alimenta `top_tracks` de `get_artist`: todas las pistas vivas del artista
//! —no solo cinco— paginadas en servidor, con sus colaboradores y sus
//! fuentes por servicio. `top_tracks` es la primera página de esta consulta
//! (límite 5), no una consulta distinta. Un artista solo existe aquí por su
//! relación `track_artists`; la cuenta sale SOLO de las pertenencias (L00) y
//! nunca se rellena con la cuenta activa: si no hay ninguna, el badge va
//! vacío (no se inventa «cuenta desconocida» con un id falso).

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

use crate::commands::library_query::{filter_where, LibraryQuery, LibrarySortKey, SortDirection};
use crate::AppState;

/// Página por defecto de la lista de artistas (misma que la de pistas).
const ARTIST_PAGE_SIZE_DEFAULT: i64 = 100;
/// Máximo de la página de artistas (misma que la de pistas).
const ARTIST_PAGE_SIZE_MAX: i64 = 200;
/// Página por defecto del detalle de pistas de un artista.
const ARTIST_TRACK_PAGE_SIZE_DEFAULT: i64 = 100;
/// Máximo de la página de pistas del detalle.
const ARTIST_TRACK_PAGE_SIZE_MAX: i64 = 200;
/// Tamaño de la primera página que viaja como `top_tracks` en `get_artist`.
pub const TOP_TRACKS_LIMIT: i64 = 5;

// ==========================================================================
// Contrato
// ==========================================================================

/// Orden de la lista de artistas. La dirección solo afecta a la clave
/// primaria; los desempates son siempre nombre `NOCASE` e id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LibraryArtistSortKey {
    /// Nombre del artista `NOCASE`.
    #[default]
    Name,
    /// Pistas distintas vivas del artista. La dirección la manda la
    /// consulta (la UI la envía explícita); el `Default` global es asc.
    Tracks,
    /// Máximo `library_added_at` vivo de las pistas del artista.
    DateAdded,
}

/// Consulta de la pestaña de artistas. Comparte con [`LibraryQuery`] el
/// texto y los filtros de cuenta/servicio (se traducen con `filter_where`),
/// más el orden y la paginación propios de la lista.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LibraryArtistQuery {
    /// Texto libre (título, artista, álbum, género, ISRC vía FTS de L02).
    pub text: Option<String>,
    /// Servicios por id (fuentes de las pistas del artista, `track_sources`).
    pub service_ids: Vec<i64>,
    /// Cuentas por id (pertenencia viva de las pistas del artista).
    pub account_ids: Vec<i64>,
    pub sort: LibraryArtistSortKey,
    pub direction: SortDirection,
    pub offset: i64,
    /// Por defecto 100, máximo 200.
    pub limit: Option<i64>,
}

impl Default for LibraryArtistQuery {
    fn default() -> Self {
        Self {
            text: None,
            service_ids: Vec::new(),
            account_ids: Vec::new(),
            sort: LibraryArtistSortKey::Name,
            // Una lista de artistas sin orden explícito se lee A→Z.
            direction: SortDirection::Asc,
            offset: 0,
            limit: None,
        }
    }
}

/// Cuenta que aporta al menos una pista viva del artista. La cuenta sale
/// SOLO de las pertenencias: si no hay ninguna, la lista va vacía.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistAccountBadge {
    pub account_id: i64,
    pub display_name: Option<String>,
    pub service_name: String,
}

/// Servicio donde existe al menos una pista viva del artista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistServiceBadge {
    pub service_id: i64,
    pub service_name: String,
}

/// Fila de la lista de artistas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryArtistRow {
    pub artist_id: i64,
    pub name: String,
    pub image_url: Option<String>,
    /// Pistas distintas con pertenencia viva en el filtro actual.
    pub track_count: i64,
    /// Álbumes distintos de esas pistas (sin contar álbumes nulos).
    pub album_count: i64,
    /// Pistas vivas del artista con al menos otro artista en `track_artists`.
    pub collaboration_count: i64,
    pub accounts: Vec<ArtistAccountBadge>,
    pub services: Vec<ArtistServiceBadge>,
    /// Máximo `library_added_at` vivo de las pistas del artista.
    pub date_added: Option<String>,
}

/// Página + total exacto + `has_more`. El total es el número de artistas
/// distintos con al menos una pista viva en el filtro (no de pistas).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryArtistsResult {
    pub artists: Vec<LibraryArtistRow>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
    pub has_more: bool,
}

/// Servicio con fuente de una pista del detalle de artista.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistTrackSource {
    pub service_id: i64,
    pub service_name: String,
}

/// Pista viva del detalle de artista. `collaborators` son los demás
/// artistas de la pista (cualquier rol de `track_artists`); vacío = pista
/// en solitario. `sources` son los servicios con `track_sources` de la
/// pista: así una colaboración cross-service se muestra con sus fuentes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistTrackRow {
    pub id: i64,
    pub title: String,
    pub album: Option<String>,
    pub album_id: Option<i64>,
    pub duration_ms: Option<i64>,
    pub collaborators: Vec<String>,
    pub sources: Vec<ArtistTrackSource>,
}

/// Consulta del detalle de pistas de un artista. `artist_id` es obligatorio;
/// el orden es título `NOCASE` con id como desempate estable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistTracksQuery {
    pub artist_id: i64,
    pub offset: i64,
    /// Por defecto 100, máximo 200.
    pub limit: Option<i64>,
}

impl ArtistTracksQuery {
    /// Primera página: `top_tracks` de `get_artist` ES esta consulta.
    pub fn first_page(artist_id: i64, limit: i64) -> Self {
        Self {
            artist_id,
            offset: 0,
            limit: Some(limit),
        }
    }
}

/// Página del detalle de pistas + total exacto + `has_more`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistTracksResult {
    pub tracks: Vec<ArtistTrackRow>,
    /// Pistas DISTINTAS vivas del artista (independiente de la página).
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
    pub has_more: bool,
}

// ==========================================================================
// SQL compartido con la consulta de pistas
// ==========================================================================

/// Traduce los filtros compartidos al contrato de pistas: así cuenta,
/// servicio, texto y pertenencia viva se evalúan EXACTAMENTE igual que en
/// `query_library`.
fn artist_track_filter(query: &LibraryArtistQuery) -> LibraryQuery {
    LibraryQuery {
        text: query.text.clone(),
        service_ids: query.service_ids.clone(),
        account_ids: query.account_ids.clone(),
        sort: LibrarySortKey::Title,
        direction: SortDirection::Asc,
        ..LibraryQuery::default()
    }
}

/// CTE que materializa las pistas vivas del filtro y agrega por artista.
fn artist_rows_cte(track_where: &str) -> String {
    format!(
        "WITH matched AS ( \
             SELECT t.id AS track_id, t.album_id AS album_id, \
                    NULLIF(t.library_added_at, '') AS library_added_at \
             FROM tracks t \
             WHERE ({track_where}) \
         ), artist_rows AS ( \
             SELECT ar.id AS artist_id, \
                 ar.name AS name, \
                 ar.image_url AS image_url, \
                 COUNT(DISTINCT m.track_id) AS track_count, \
                 COUNT(DISTINCT m.album_id) AS album_count, \
                 COUNT(DISTINCT CASE WHEN EXISTS ( \
                     SELECT 1 FROM track_artists ta_other \
                     WHERE ta_other.track_id = m.track_id \
                       AND ta_other.artist_id <> ar.id \
                 ) THEN m.track_id END) AS collaboration_count, \
                 MAX(m.library_added_at) AS date_added \
             FROM matched m \
             JOIN track_artists ta ON ta.track_id = m.track_id \
             JOIN artists ar ON ar.id = ta.artist_id \
             GROUP BY ar.id \
             HAVING COUNT(DISTINCT m.track_id) > 0 \
         )"
    )
}

/// `ORDER BY` de la lista. La dirección solo se aplica a la clave primaria;
/// nulos al final y desempates estables por nombre `NOCASE` e id.
fn order_clause(query: &LibraryArtistQuery) -> String {
    let dir = query.direction.sql();
    match query.sort {
        LibraryArtistSortKey::Name => format!("name COLLATE NOCASE {dir}, artist_id ASC"),
        LibraryArtistSortKey::Tracks => {
            format!("track_count {dir}, name COLLATE NOCASE ASC, artist_id ASC")
        }
        LibraryArtistSortKey::DateAdded => format!(
            "date_added IS NULL ASC, date_added {dir}, \
             name COLLATE NOCASE ASC, artist_id ASC"
        ),
    }
}

// ==========================================================================
// La consulta
// ==========================================================================

/// Fila base tal como la devuelve SQLite; los badges se completan con las
/// consultas agrupadas de la página.
#[derive(Debug, sqlx::FromRow)]
struct ArtistRowBase {
    artist_id: i64,
    name: String,
    image_url: Option<String>,
    track_count: i64,
    album_count: i64,
    collaboration_count: i64,
    date_added: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ArtistAccountBadgeRow {
    artist_id: i64,
    account_id: i64,
    display_name: Option<String>,
    service_name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ArtistServiceBadgeRow {
    artist_id: i64,
    service_id: i64,
    service_name: String,
}

/// Consulta de la pestaña de artistas. Recibe el pool — la costura de prueba
/// es esta función, no el comando de Tauri.
pub async fn query_library_artists_inner(
    db: &SqlitePool,
    query: LibraryArtistQuery,
) -> Result<LibraryArtistsResult, String> {
    let offset = query.offset.max(0);
    let limit = query
        .limit
        .unwrap_or(ARTIST_PAGE_SIZE_DEFAULT)
        .clamp(1, ARTIST_PAGE_SIZE_MAX);

    let track_query = artist_track_filter(&query);
    let (track_where, binds) = filter_where(&track_query)?;
    let cte = artist_rows_cte(&track_where);
    let order = order_clause(&query);

    // Total exacto del filtro: artistas DISTINTOS con al menos una pista viva.
    let total_sql = format!("{cte} SELECT COUNT(*) FROM artist_rows");
    let mut total_query = sqlx::query_as::<_, (i64,)>(&total_sql);
    for bind in &binds {
        total_query = total_query.bind(bind);
    }
    let (total,): (i64,) = total_query
        .fetch_one(db)
        .await
        .map_err(|e| format!("query_library_artists: error contando: {e}"))?;

    // Página agregada y ordenada en servidor.
    let page_sql = format!(
        "{cte} SELECT artist_id, name, image_url, track_count, album_count, \
         collaboration_count, date_added \
         FROM artist_rows ORDER BY {order} LIMIT {limit} OFFSET {offset}"
    );
    let mut page_query = sqlx::query_as::<_, ArtistRowBase>(&page_sql);
    for bind in &binds {
        page_query = page_query.bind(bind);
    }
    let rows: Vec<ArtistRowBase> = page_query
        .fetch_all(db)
        .await
        .map_err(|e| format!("query_library_artists: error paginando: {e}"))?;

    let mut artists: Vec<LibraryArtistRow> = rows
        .into_iter()
        .map(|row| LibraryArtistRow {
            artist_id: row.artist_id,
            name: row.name,
            image_url: row.image_url,
            track_count: row.track_count,
            album_count: row.album_count,
            collaboration_count: row.collaboration_count,
            accounts: Vec::new(),
            services: Vec::new(),
            date_added: row.date_added,
        })
        .collect();

    // Badges de la página: UNA consulta agrupada por faceta para todos los
    // artistas de la página, nunca una consulta por artista (sin N+1).
    if !artists.is_empty() {
        let page_ids = artists
            .iter()
            .map(|artist| artist.artist_id.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let service_sql = format!(
            "{cte} SELECT ta.artist_id AS artist_id, s.id AS service_id, \
                 s.name AS service_name \
             FROM matched m \
             JOIN track_artists ta ON ta.track_id = m.track_id \
             JOIN track_sources ts ON ts.track_id = m.track_id \
             JOIN services s ON s.id = ts.service_id \
             WHERE ta.artist_id IN ({page_ids}) \
             GROUP BY ta.artist_id, s.id, s.name \
             ORDER BY ta.artist_id, s.name COLLATE NOCASE"
        );
        let mut service_query = sqlx::query_as::<_, ArtistServiceBadgeRow>(&service_sql);
        for bind in &binds {
            service_query = service_query.bind(bind);
        }
        let service_rows = service_query
            .fetch_all(db)
            .await
            .map_err(|e| format!("query_library_artists: error leyendo servicios: {e}"))?;
        let mut services_by_artist: std::collections::HashMap<i64, Vec<ArtistServiceBadge>> =
            std::collections::HashMap::new();
        for row in service_rows {
            services_by_artist
                .entry(row.artist_id)
                .or_default()
                .push(ArtistServiceBadge {
                    service_id: row.service_id,
                    service_name: row.service_name,
                });
        }

        let account_sql = format!(
            "{cte} SELECT ta.artist_id AS artist_id, a.id AS account_id, \
                 a.display_name, sv.name AS service_name \
             FROM matched m \
             JOIN track_artists ta ON ta.track_id = m.track_id \
             JOIN ( \
                 SELECT le.account_id AS account_id, le.track_id AS track_id \
                 FROM library_entries le \
                 UNION \
                 SELECT pl.account_id, pt.track_id \
                 FROM playlist_tracks pt \
                 JOIN playlists pl ON pl.id = pt.playlist_id \
                 UNION \
                 SELECT lm.account_id, lm.track_id \
                 FROM library_memberships lm \
             ) membership ON membership.track_id = m.track_id \
             JOIN accounts a ON a.id = membership.account_id \
             JOIN services sv ON sv.id = a.service_id \
             WHERE ta.artist_id IN ({page_ids}) \
             GROUP BY ta.artist_id, a.id, a.display_name, sv.name \
             ORDER BY ta.artist_id, sv.name COLLATE NOCASE, a.id"
        );
        let mut account_query = sqlx::query_as::<_, ArtistAccountBadgeRow>(&account_sql);
        for bind in &binds {
            account_query = account_query.bind(bind);
        }
        let account_rows = account_query
            .fetch_all(db)
            .await
            .map_err(|e| format!("query_library_artists: error leyendo cuentas: {e}"))?;
        let mut accounts_by_artist: std::collections::HashMap<i64, Vec<ArtistAccountBadge>> =
            std::collections::HashMap::new();
        for row in account_rows {
            accounts_by_artist
                .entry(row.artist_id)
                .or_default()
                .push(ArtistAccountBadge {
                    account_id: row.account_id,
                    display_name: row.display_name,
                    service_name: row.service_name,
                });
        }

        for artist in &mut artists {
            artist.services = services_by_artist
                .remove(&artist.artist_id)
                .unwrap_or_default();
            artist.accounts = accounts_by_artist
                .remove(&artist.artist_id)
                .unwrap_or_default();
        }
    }

    let has_more = offset + (artists.len() as i64) < total;

    Ok(LibraryArtistsResult {
        artists,
        total,
        offset,
        limit,
        has_more,
    })
}

// ==========================================================================
// Detalle: todas las pistas vivas del artista, paginadas
// ==========================================================================

/// Fila base del detalle; fuentes y colaboradores se completan después.
#[derive(Debug, sqlx::FromRow)]
struct ArtistTrackBase {
    id: i64,
    title: String,
    album: Option<String>,
    album_id: Option<i64>,
    duration_ms: Option<i64>,
}

#[derive(Debug, sqlx::FromRow)]
struct ArtistTrackSourceRow {
    track_id: i64,
    service_id: i64,
    service_name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct ArtistCollaboratorRow {
    track_id: i64,
    name: String,
}

/// Pistas vivas del artista, paginadas en servidor. `top_tracks` de
/// `get_artist` es la primera página de ESTA consulta
/// ([`ArtistTracksQuery::first_page`]); no hay una segunda definición del
/// orden ni de la pertenencia.
pub async fn query_library_artist_tracks_inner(
    db: &SqlitePool,
    query: ArtistTracksQuery,
) -> Result<ArtistTracksResult, String> {
    let offset = query.offset.max(0);
    let limit = query
        .limit
        .unwrap_or(ARTIST_TRACK_PAGE_SIZE_DEFAULT)
        .clamp(1, ARTIST_TRACK_PAGE_SIZE_MAX);
    let artist_id = query.artist_id;

    // La pertenencia viva es la MISMA de la consulta de pistas: `filter_where`
    // con la consulta vacía produce solo el borde de visibilidad de L00/L01.
    let (live_where, live_binds) = filter_where(&LibraryQuery::default())?;
    let artist_track = format!(
        "EXISTS (SELECT 1 FROM track_artists ta \
         WHERE ta.track_id = t.id AND ta.artist_id = {artist_id})"
    );

    let total_sql = format!(
        "SELECT COUNT(DISTINCT t.id) FROM tracks t \
         WHERE ({live_where}) AND {artist_track}"
    );
    let mut total_query = sqlx::query_as::<_, (i64,)>(&total_sql);
    for bind in &live_binds {
        total_query = total_query.bind(bind);
    }
    let (total,): (i64,) = total_query
        .fetch_one(db)
        .await
        .map_err(|e| format!("query_library_artist_tracks: error contando: {e}"))?;

    let page_sql = format!(
        "SELECT t.id, t.title, al.title AS album, al.id AS album_id, t.duration_ms \
         FROM tracks t \
         LEFT JOIN albums al ON al.id = t.album_id \
         WHERE ({live_where}) AND {artist_track} \
         ORDER BY t.title COLLATE NOCASE ASC, t.id ASC \
         LIMIT {limit} OFFSET {offset}"
    );
    let mut page_query = sqlx::query_as::<_, ArtistTrackBase>(&page_sql);
    for bind in &live_binds {
        page_query = page_query.bind(bind);
    }
    let rows: Vec<ArtistTrackBase> = page_query
        .fetch_all(db)
        .await
        .map_err(|e| format!("query_library_artist_tracks: error paginando: {e}"))?;

    let mut tracks: Vec<ArtistTrackRow> = rows
        .into_iter()
        .map(|row| ArtistTrackRow {
            id: row.id,
            title: row.title,
            album: row.album,
            album_id: row.album_id,
            duration_ms: row.duration_ms,
            collaborators: Vec::new(),
            sources: Vec::new(),
        })
        .collect();

    if !tracks.is_empty() {
        let page_ids = tracks
            .iter()
            .map(|track| track.id.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        // Fuentes de la página en UNA consulta: cross-service visible.
        let source_sql = format!(
            "SELECT ts.track_id AS track_id, s.id AS service_id, s.name AS service_name \
             FROM track_sources ts \
             JOIN services s ON s.id = ts.service_id \
             WHERE ts.track_id IN ({page_ids}) \
             GROUP BY ts.track_id, s.id, s.name \
             ORDER BY ts.track_id, s.name COLLATE NOCASE"
        );
        let source_rows = sqlx::query_as::<_, ArtistTrackSourceRow>(&source_sql)
            .fetch_all(db)
            .await
            .map_err(|e| format!("query_library_artist_tracks: error leyendo fuentes: {e}"))?;
        let mut sources_by_track: std::collections::HashMap<i64, Vec<ArtistTrackSource>> =
            std::collections::HashMap::new();
        for row in source_rows {
            sources_by_track
                .entry(row.track_id)
                .or_default()
                .push(ArtistTrackSource {
                    service_id: row.service_id,
                    service_name: row.service_name,
                });
        }

        // Colaboradores de la página en UNA consulta: el resto de artistas
        // de `track_artists`, sin repetir al artista del detalle.
        let collaborator_sql = format!(
            "SELECT ta.track_id AS track_id, ar.name AS name \
             FROM track_artists ta \
             JOIN artists ar ON ar.id = ta.artist_id \
             WHERE ta.track_id IN ({page_ids}) AND ta.artist_id <> {artist_id} \
             GROUP BY ta.track_id, ar.id, ar.name \
             ORDER BY ta.track_id, ar.name COLLATE NOCASE"
        );
        let collaborator_rows = sqlx::query_as::<_, ArtistCollaboratorRow>(&collaborator_sql)
            .fetch_all(db)
            .await
            .map_err(|e| {
                format!("query_library_artist_tracks: error leyendo colaboradores: {e}")
            })?;
        let mut collaborators_by_track: std::collections::HashMap<i64, Vec<String>> =
            std::collections::HashMap::new();
        for row in collaborator_rows {
            collaborators_by_track
                .entry(row.track_id)
                .or_default()
                .push(row.name);
        }

        for track in &mut tracks {
            track.sources = sources_by_track.remove(&track.id).unwrap_or_default();
            track.collaborators = collaborators_by_track.remove(&track.id).unwrap_or_default();
        }
    }

    let has_more = offset + (tracks.len() as i64) < total;

    Ok(ArtistTracksResult {
        tracks,
        total,
        offset,
        limit,
        has_more,
    })
}

/// Plan de la consulta de artistas (EXPLAIN QUERY PLAN) para la verificación
/// del criterio de índices del issue #105. Entra por la pertenencia de
/// L00/L01 y por `idx_track_artists_artist_role` /
/// `idx_tracks_album` según el filtro.
#[allow(dead_code)] // Cubierta por `tests/library_artists_test.rs` (el IPC no la expone).
pub async fn artist_query_plan(
    db: &SqlitePool,
    query: &LibraryArtistQuery,
) -> Result<Vec<String>, String> {
    let track_query = artist_track_filter(query);
    let (track_where, binds) = filter_where(&track_query)?;
    let cte = artist_rows_cte(&track_where);
    let sql = format!("{cte} SELECT COUNT(*) FROM artist_rows");
    let plan_sql = format!("EXPLAIN QUERY PLAN {sql}");
    let mut q = sqlx::query_as::<_, (i64, i64, i64, String)>(&plan_sql);
    for bind in &binds {
        q = q.bind(bind);
    }
    let rows = q
        .fetch_all(db)
        .await
        .map_err(|e| format!("artist_query_plan: {e}"))?;
    Ok(rows.into_iter().map(|(_, _, _, detail)| detail).collect())
}

// ==========================================================================
// Comandos IPC
// ==========================================================================

/// Lista paginada de artistas de la biblioteca (L08, issue #105). Wrapper
/// fino: el cuerpo vive en este módulo.
#[tauri::command]
pub async fn query_library_artists(
    state: State<'_, AppState>,
    query: LibraryArtistQuery,
) -> Result<LibraryArtistsResult, String> {
    query_library_artists_inner(&state.db, query).await
}

/// Página del detalle de un artista (L08, issue #105): todas las pistas
/// vivas del artista, no solo las cinco de `top_tracks`.
#[tauri::command]
pub async fn query_library_artist_tracks(
    state: State<'_, AppState>,
    query: ArtistTracksQuery,
) -> Result<ArtistTracksResult, String> {
    query_library_artist_tracks_inner(&state.db, query).await
}

//! Pestaña de álbumes — L07 (issue #104, spec #96).
//!
//! [`query_library_albums`] comparte con [`query_library`] el texto (FTS de
//! L02), los filtros de cuenta/servicio y la paginación: la traducción de
//! esos filtros es la MISMA ([`filter_where`] de `library_query.rs`), no una
//! copia. Encima de esa base agrega por álbum:
//!
//! - una fila por álbum con al menos una pista VIVA en el filtro (la
//!   pertenencia es la de L00/L01: favoritos, playlists y
//!   `library_memberships`); un álbum sin pistas vivas no aparece;
//! - artista principal (álbum, con el artista primario más frecuente de sus
//!   pistas como respaldo), año, pistas distintas vivas, descargadas y
//!   cobertura;
//! - L14 (#109): los cinco estados de descarga de esas pistas, agregados con
//!   la MISMA expresión que la fila de pistas
//!   ([`crate::commands::library_query::DOWNLOAD_STATUS_SQL`]) — la regla no
//!   se duplica;
//! - cuentas y servicios de esas pistas, agregados en UNA consulta para
//!   todos los álbumes de la página (no una por álbum: sin N+1);
//! - filtro de cobertura (`complete`/`partial`/`none`) y orden por título,
//!   artista, año o fecha añadida, con nulos al final y desempates estables;
//! - `offset`/`limit` en servidor (100 por defecto, máximo 200) y total
//!   exacto del filtro.
//!
//! La cobertura es `descargadas / pistas distintas`: [`album_coverage`]
//! devuelve `None` cuando el total es cero — no se inventa un porcentaje.
//! «Descargada» usa la misma clasificación que la lista de pistas; una
//! descarga en cola, descargando o fallida no cuenta como cobertura, pero la
//! fila del álbum expone su contador para que la UI muestre el mismo estado
//! que el badge de la pista.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;

use crate::commands::library_query::{
    filter_where, LibraryQuery, LibrarySortKey, SortDirection, DOWNLOAD_STATUS_SQL,
};
use crate::AppState;

/// Página por defecto de la lista de álbumes (misma que la de pistas).
const ALBUM_PAGE_SIZE_DEFAULT: i64 = 100;
/// Máximo de la página de álbumes (misma que la de pistas).
const ALBUM_PAGE_SIZE_MAX: i64 = 200;

// ==========================================================================
// Contrato
// ==========================================================================

/// Orden de la lista de álbumes. La dirección solo afecta a la clave
/// primaria; los desempates son siempre título `NOCASE`, artista y álbum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LibraryAlbumSortKey {
    /// Máximo `library_added_at` vivo de las pistas del álbum.
    #[default]
    DateAdded,
    Title,
    Artist,
    Year,
}

/// Filtro de cobertura. Tres buckets que particionan la lista: todas las
/// pistas descargadas, algunas o ninguna. OR dentro de la lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlbumCoverageFilter {
    Complete,
    Partial,
    None,
}

impl AlbumCoverageFilter {
    fn sql(self) -> &'static str {
        match self {
            AlbumCoverageFilter::Complete => "downloaded_count = track_count",
            AlbumCoverageFilter::Partial => {
                "downloaded_count > 0 AND downloaded_count < track_count"
            }
            AlbumCoverageFilter::None => "downloaded_count = 0",
        }
    }
}

/// Consulta de la pestaña de álbumes. Comparte con [`LibraryQuery`] el texto
/// y los filtros de cuenta/servicio (se traducen con `filter_where`), más
/// el filtro de cobertura, el orden y la paginación propios de la lista.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LibraryAlbumQuery {
    /// Texto libre (título, artista, álbum, género, ISRC vía FTS de L02).
    pub text: Option<String>,
    /// Servicios por id (fuentes de las pistas del álbum, `track_sources`).
    pub service_ids: Vec<i64>,
    /// Cuentas por id (pertenencia viva de las pistas del álbum).
    pub account_ids: Vec<i64>,
    /// Cobertura de descarga (OR dentro de la lista).
    pub coverage: Vec<AlbumCoverageFilter>,
    pub sort: LibraryAlbumSortKey,
    pub direction: SortDirection,
    pub offset: i64,
    /// Por defecto 100, máximo 200.
    pub limit: Option<i64>,
}

/// Cuenta que aporta al menos una pista viva del álbum. La cuenta sale SOLO
/// de las pertenencias: si no hay ninguna, la lista va vacía (no se
/// rellena con la cuenta activa).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumAccountBadge {
    pub account_id: i64,
    pub display_name: Option<String>,
    pub service_name: String,
}

/// Servicio donde existe al menos una pista viva del álbum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumServiceBadge {
    pub service_id: i64,
    pub service_name: String,
}

/// Fila de la lista de álbumes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAlbumRow {
    pub album_id: i64,
    pub title: String,
    pub artist_name: Option<String>,
    pub release_year: Option<i64>,
    /// Pistas distintas con pertenencia viva en el filtro actual.
    pub track_count: i64,
    /// De esas pistas, las que tienen descarga con fichero.
    pub downloaded_count: i64,
    /// L14 (#109): pistas en cola (esperando al worker).
    pub queued_count: i64,
    /// L14 (#109): pistas descargándose ahora.
    pub downloading_count: i64,
    /// L14 (#109): pistas con la última descarga fallida.
    pub failed_count: i64,
    /// L14 (#109): pistas sin descarga ni cola.
    pub not_downloaded_count: i64,
    /// `downloaded_count / track_count`; `None` si el total es cero.
    pub coverage: Option<f64>,
    pub accounts: Vec<AlbumAccountBadge>,
    pub services: Vec<AlbumServiceBadge>,
    /// Máximo `library_added_at` vivo de las pistas del álbum.
    pub date_added: Option<String>,
    pub cover_art_url: Option<String>,
}

/// Página + total exacto + `has_more`. El total es el número de álbumes
/// distintos con al menos una pista viva en el filtro (no de pistas).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAlbumsResult {
    pub albums: Vec<LibraryAlbumRow>,
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
/// `query_library` (misma pertenencia para cuenta+servicio, mismo FTS).
fn album_track_filter(query: &LibraryAlbumQuery) -> LibraryQuery {
    LibraryQuery {
        text: query.text.clone(),
        service_ids: query.service_ids.clone(),
        account_ids: query.account_ids.clone(),
        sort: LibrarySortKey::Title,
        direction: SortDirection::Asc,
        ..LibraryQuery::default()
    }
}

/// CTE que materializa las pistas vivas del filtro y agrega por álbum.
fn album_rows_cte(track_where: &str) -> String {
    let download_status = DOWNLOAD_STATUS_SQL;
    format!(
        "WITH matched AS ( \
             SELECT t.id AS track_id, t.album_id AS album_id, \
                    NULLIF(t.library_added_at, '') AS library_added_at, \
                    NULLIF(t.release_year, 0) AS release_year, \
                    {download_status} AS download_status \
             FROM tracks t \
             WHERE ({track_where}) AND t.album_id IS NOT NULL \
         ), album_rows AS ( \
             SELECT al.id AS album_id, \
                 al.title AS title, \
                 COALESCE( \
                     (SELECT ar.name FROM album_artists aa \
                      JOIN artists ar ON ar.id = aa.artist_id \
                      WHERE aa.album_id = al.id \
                      ORDER BY aa.is_primary DESC, ar.name COLLATE NOCASE \
                      LIMIT 1), \
                     (SELECT ar2.name FROM matched m2 \
                      JOIN track_artists ta ON ta.track_id = m2.track_id AND ta.role = 'primary' \
                      JOIN artists ar2 ON ar2.id = ta.artist_id \
                      WHERE m2.album_id = al.id \
                      GROUP BY ar2.id \
                      ORDER BY COUNT(*) DESC, ar2.name COLLATE NOCASE \
                      LIMIT 1) \
                 ) AS artist_name, \
                 COALESCE( \
                     NULLIF(CAST(SUBSTR(al.release_date, 1, 4) AS INTEGER), 0), \
                     MAX(m.release_year) \
                 ) AS release_year, \
                 COUNT(DISTINCT m.track_id) AS track_count, \
                 COUNT(DISTINCT CASE WHEN m.download_status = 'downloaded' THEN m.track_id END) AS downloaded_count, \
                 COUNT(DISTINCT CASE WHEN m.download_status = 'queued' THEN m.track_id END) AS queued_count, \
                 COUNT(DISTINCT CASE WHEN m.download_status = 'downloading' THEN m.track_id END) AS downloading_count, \
                 COUNT(DISTINCT CASE WHEN m.download_status = 'failed' THEN m.track_id END) AS failed_count, \
                 COUNT(DISTINCT CASE WHEN m.download_status = 'not_downloaded' THEN m.track_id END) AS not_downloaded_count, \
                 MAX(m.library_added_at) AS date_added, \
                 al.cover_art_url AS cover_art_url \
             FROM matched m \
             JOIN albums al ON al.id = m.album_id \
             GROUP BY al.id \
             HAVING COUNT(DISTINCT m.track_id) > 0 \
         )"
    )
}

fn coverage_filter_sql(filters: &[AlbumCoverageFilter]) -> Option<String> {
    if filters.is_empty() {
        return None;
    }
    let parts: Vec<&str> = filters.iter().map(|f| f.sql()).collect();
    Some(format!("({})", parts.join(" OR ")))
}

fn where_coverage(filters: &[AlbumCoverageFilter]) -> String {
    coverage_filter_sql(filters)
        .map(|sql| format!(" WHERE {sql}"))
        .unwrap_or_default()
}

/// `ORDER BY` de la lista. La dirección solo se aplica a la clave primaria;
/// nulos al final y desempates estables por título `NOCASE`, artista y
/// `album_id` ascendentes.
fn order_clause(query: &LibraryAlbumQuery) -> String {
    let dir = query.direction.sql();
    match query.sort {
        LibraryAlbumSortKey::DateAdded => format!(
            "date_added IS NULL ASC, date_added {dir}, \
             title COLLATE NOCASE ASC, \
             artist_name IS NULL ASC, artist_name COLLATE NOCASE ASC, \
             album_id ASC"
        ),
        LibraryAlbumSortKey::Title => format!(
            "title IS NULL ASC, title COLLATE NOCASE {dir}, \
             artist_name IS NULL ASC, artist_name COLLATE NOCASE ASC, \
             album_id ASC"
        ),
        LibraryAlbumSortKey::Artist => format!(
            "artist_name IS NULL ASC, artist_name COLLATE NOCASE {dir}, \
             title COLLATE NOCASE ASC, album_id ASC"
        ),
        LibraryAlbumSortKey::Year => format!(
            "release_year IS NULL ASC, release_year {dir}, \
             title COLLATE NOCASE ASC, \
             artist_name IS NULL ASC, artist_name COLLATE NOCASE ASC, \
             album_id ASC"
        ),
    }
}

/// Cobertura como fracción. Un total cero NO produce porcentaje: `None`.
pub fn album_coverage(downloaded: i64, total: i64) -> Option<f64> {
    if total <= 0 {
        None
    } else {
        Some(downloaded as f64 / total as f64)
    }
}

// ==========================================================================
// La consulta
// ==========================================================================

/// Fila base tal como la devuelve SQLite; la cobertura y los badges se
/// completan en Rust / con las consultas agrupadas de la página.
#[derive(Debug, sqlx::FromRow)]
struct AlbumRowBase {
    album_id: i64,
    title: String,
    artist_name: Option<String>,
    release_year: Option<i64>,
    track_count: i64,
    downloaded_count: i64,
    queued_count: i64,
    downloading_count: i64,
    failed_count: i64,
    not_downloaded_count: i64,
    date_added: Option<String>,
    cover_art_url: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct AlbumAccountBadgeRow {
    album_id: i64,
    account_id: i64,
    display_name: Option<String>,
    service_name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct AlbumServiceBadgeRow {
    album_id: i64,
    service_id: i64,
    service_name: String,
}

/// Consulta de la pestaña de álbumes. Recibe el pool — la costura de prueba
/// es esta función, no el comando de Tauri.
pub async fn query_library_albums_inner(
    db: &SqlitePool,
    query: LibraryAlbumQuery,
) -> Result<LibraryAlbumsResult, String> {
    let offset = query.offset.max(0);
    let limit = query
        .limit
        .unwrap_or(ALBUM_PAGE_SIZE_DEFAULT)
        .clamp(1, ALBUM_PAGE_SIZE_MAX);

    let track_query = album_track_filter(&query);
    let (track_where, binds) = filter_where(&track_query)?;
    let cte = album_rows_cte(&track_where);
    let coverage_filter = where_coverage(&query.coverage);
    let order = order_clause(&query);

    // Total exacto del filtro: álbumes DISTINTOS con al menos una pista viva.
    let total_sql = format!("{cte} SELECT COUNT(*) FROM album_rows{coverage_filter}");
    let mut total_query = sqlx::query_as::<_, (i64,)>(&total_sql);
    for bind in &binds {
        total_query = total_query.bind(bind);
    }
    let (total,): (i64,) = total_query
        .fetch_one(db)
        .await
        .map_err(|e| format!("query_library_albums: error contando: {e}"))?;

    // Página agregada y ordenada en servidor.
    let page_sql = format!(
        "{cte} SELECT album_id, title, artist_name, release_year, track_count, \
         downloaded_count, queued_count, downloading_count, failed_count, \
         not_downloaded_count, date_added, cover_art_url \
         FROM album_rows{coverage_filter} ORDER BY {order} LIMIT {limit} OFFSET {offset}"
    );
    let mut page_query = sqlx::query_as::<_, AlbumRowBase>(&page_sql);
    for bind in &binds {
        page_query = page_query.bind(bind);
    }
    let rows: Vec<AlbumRowBase> = page_query
        .fetch_all(db)
        .await
        .map_err(|e| format!("query_library_albums: error paginando: {e}"))?;

    let mut albums: Vec<LibraryAlbumRow> = rows
        .into_iter()
        .map(|row| LibraryAlbumRow {
            coverage: album_coverage(row.downloaded_count, row.track_count),
            album_id: row.album_id,
            title: row.title,
            artist_name: row.artist_name,
            release_year: row.release_year,
            track_count: row.track_count,
            downloaded_count: row.downloaded_count,
            queued_count: row.queued_count,
            downloading_count: row.downloading_count,
            failed_count: row.failed_count,
            not_downloaded_count: row.not_downloaded_count,
            accounts: Vec::new(),
            services: Vec::new(),
            date_added: row.date_added,
            cover_art_url: row.cover_art_url,
        })
        .collect();

    // Badges de la página: UNA consulta agrupada por faceta para todos los
    // álbumes de la página, nunca una consulta por álbum (sin N+1).
    if !albums.is_empty() {
        let page_ids = albums
            .iter()
            .map(|album| album.album_id.to_string())
            .collect::<Vec<_>>()
            .join(", ");

        let service_sql = format!(
            "{cte} SELECT m.album_id, s.id AS service_id, s.name AS service_name \
             FROM matched m \
             JOIN track_sources ts ON ts.track_id = m.track_id \
             JOIN services s ON s.id = ts.service_id \
             WHERE m.album_id IN ({page_ids}) \
             GROUP BY m.album_id, s.id, s.name \
             ORDER BY m.album_id, s.name COLLATE NOCASE"
        );
        let mut service_query = sqlx::query_as::<_, AlbumServiceBadgeRow>(&service_sql);
        for bind in &binds {
            service_query = service_query.bind(bind);
        }
        let service_rows = service_query
            .fetch_all(db)
            .await
            .map_err(|e| format!("query_library_albums: error leyendo servicios: {e}"))?;
        let mut services_by_album: std::collections::HashMap<i64, Vec<AlbumServiceBadge>> =
            std::collections::HashMap::new();
        for row in service_rows {
            services_by_album
                .entry(row.album_id)
                .or_default()
                .push(AlbumServiceBadge {
                    service_id: row.service_id,
                    service_name: row.service_name,
                });
        }

        let account_sql = format!(
            "{cte} SELECT m.album_id, a.id AS account_id, a.display_name, \
                sv.name AS service_name \
             FROM matched m \
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
             WHERE m.album_id IN ({page_ids}) \
             GROUP BY m.album_id, a.id, a.display_name, sv.name \
             ORDER BY m.album_id, sv.name COLLATE NOCASE, a.id"
        );
        let mut account_query = sqlx::query_as::<_, AlbumAccountBadgeRow>(&account_sql);
        for bind in &binds {
            account_query = account_query.bind(bind);
        }
        let account_rows = account_query
            .fetch_all(db)
            .await
            .map_err(|e| format!("query_library_albums: error leyendo cuentas: {e}"))?;
        let mut accounts_by_album: std::collections::HashMap<i64, Vec<AlbumAccountBadge>> =
            std::collections::HashMap::new();
        for row in account_rows {
            accounts_by_album
                .entry(row.album_id)
                .or_default()
                .push(AlbumAccountBadge {
                    account_id: row.account_id,
                    display_name: row.display_name,
                    service_name: row.service_name,
                });
        }

        for album in &mut albums {
            album.services = services_by_album
                .remove(&album.album_id)
                .unwrap_or_default();
            album.accounts = accounts_by_album
                .remove(&album.album_id)
                .unwrap_or_default();
        }
    }

    let has_more = offset + (albums.len() as i64) < total;

    Ok(LibraryAlbumsResult {
        albums,
        total,
        offset,
        limit,
        has_more,
    })
}

/// Plan de la consulta de álbumes (EXPLAIN QUERY PLAN) para la verificación
/// del criterio «los planes usan índices» del issue #104. Entra por los
/// índices de pertenencia de L00/L01 y por `tracks.album_id`
/// (`idx_tracks_album` / `idx_tracks_search_filters`).
#[allow(dead_code)] // Cubierta por `tests/library_albums_test.rs` (el IPC no la expone).
pub async fn album_query_plan(
    db: &SqlitePool,
    query: &LibraryAlbumQuery,
) -> Result<Vec<String>, String> {
    let track_query = album_track_filter(query);
    let (track_where, binds) = filter_where(&track_query)?;
    let cte = album_rows_cte(&track_where);
    let coverage_filter = where_coverage(&query.coverage);
    let sql = format!("{cte} SELECT COUNT(*) FROM album_rows{coverage_filter}");
    let plan_sql = format!("EXPLAIN QUERY PLAN {sql}");
    let mut q = sqlx::query_as::<_, (i64, i64, i64, String)>(&plan_sql);
    for bind in &binds {
        q = q.bind(bind);
    }
    let rows = q
        .fetch_all(db)
        .await
        .map_err(|e| format!("album_query_plan: {e}"))?;
    Ok(rows.into_iter().map(|(_, _, _, detail)| detail).collect())
}

// ==========================================================================
// Comando IPC
// ==========================================================================

/// Lista paginada de álbumes de la biblioteca (L07, issue #104). Wrapper
/// fino: el cuerpo vive en este módulo, la serie L08/L12 comparte módulo.
#[tauri::command]
pub async fn query_library_albums(
    state: State<'_, AppState>,
    query: LibraryAlbumQuery,
) -> Result<LibraryAlbumsResult, String> {
    query_library_albums_inner(&state.db, query).await
}

//! Consulta de biblioteca v2 — L01 (issue #98, spec #96).
//!
//! Módulo propio extraído de `commands/library.rs` (8k líneas): es la pieza
//! que habilita el paralelismo de la serie (L02-L12 extienden ESTE módulo,
//! no `library.rs`). La función [`query_library`] recibe el pool de SQLite
//! y nada de Tauri; el comando `query_library` de `library.rs` es un
//! wrapper fino.
//!
//! Contrato (una sola llamada IPC):
//! - una fila por `tracks.id` — nunca una por release, entrada o item de
//!   proveedor; los favoritos no se expanden por release;
//! - una pista solo visible si tiene pertenencia viva (ver [`MEMBERSHIP_SQL`]);
//! - filtros con AND entre dimensiones y OR dentro de cada lista;
//! - cuenta y servicio evaluados sobre la MISMA pertenencia cuando ambos
//!   vienen (ver [`AccountsServicesCombined`]);
//! - total exacto del filtro (`COUNT(DISTINCT tracks.id)`) y contadores de
//!   cabecera en la primera página, sin consultas por fila ni por contador;
//! - orden en servidor con desempates estables (título `NOCASE`, artista,
//!   `tracks.id`) y nulos/centinelas al final; dirección solo en la clave
//!   primaria;
//! - paginación `offset`/`limit` con `limit` por defecto 100 y máximo 200.
//!
//! Piezas deshabilitadas con su motivo (no inventadas):
//! - colecciones `album`/`artist`: la pertenencia por álbum/artista no
//!   existe hasta L00 (#97);
//! - filtro `duplicates`: la identidad ISRC∪huella llega con L10 (#73);
//! - cuarentena `only`/`include`: no hay tabla de cuarentena hasta #72/L09;
//!   `exclude` (por defecto) es hoy un no-op honesto.
//!
//! Búsqueda de texto: LIKE sobre título/artista/álbum combinada con los
//! filtros (ya no sustituye la lista). La relevancia ponderada FTS es L02.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

// ==========================================================================
// Contrato
// ==========================================================================

/// Clave de orden. La dirección solo afecta a esta clave; los desempates son
/// siempre título `NOCASE`, artista y `tracks.id` ascendentes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LibrarySortKey {
    /// Fecha de añadido: `MAX(added_at)` sobre las pertenencias vivas.
    /// L00 materializará `tracks.library_added_at`; este es el otro punto
    /// de conexión (junto a [`MEMBERSHIP_SQL`]).
    #[default]
    DateAdded,
    Title,
    Artist,
    Album,
    Duration,
    Quality,
    MetadataScore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortDirection {
    Asc,
    #[default]
    Desc,
}

impl SortDirection {
    fn sql(self) -> &'static str {
        match self {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        }
    }
}

/// Modo de cuarentena del contrato. `exclude` (por defecto) es hoy un no-op:
/// no existe tabla de cuarentena hasta #72, así que no hay nada que excluir.
/// `only`/`include` quedan deshabilitados hasta L09 (#72) — ver [`LibraryQuery`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum QuarantineMode {
    #[default]
    Exclude,
    Only,
    Include,
}

/// Estado de descarga reconocido por el filtro. Misma clasificación que
/// `get_library` / `verify_page_files`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatusFilter {
    Downloaded,
    Queued,
    NotDownloaded,
}

impl DownloadStatusFilter {
    fn sql(self) -> &'static str {
        match self {
            DownloadStatusFilter::Downloaded => "downloaded",
            DownloadStatusFilter::Queued => "queued",
            DownloadStatusFilter::NotDownloaded => "not_downloaded",
        }
    }
}

/// Bucket de calidad reconocido por el filtro. Mismas etiquetas que
/// `get_audio_quality_distribution` (deep-link FE-10 del dashboard).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityBucketFilter {
    HiRes,
    CdQuality,
    Lossy,
}

impl QualityBucketFilter {
    fn label(self) -> &'static str {
        match self {
            QualityBucketFilter::HiRes => "Hi-Res (24-bit+)",
            QualityBucketFilter::CdQuality => "CD Quality",
            QualityBucketFilter::Lossy => "Lossy",
        }
    }
}

/// Colección del filtro. `favorite` y `playlist` existen hoy;
/// `album`/`artist` son del contrato y quedan deshabilitados hasta L00 (#97).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionFilter {
    Favorite,
    Playlist,
    Album,
    Artist,
}

/// Filtro de duplicados. Solo contrato: la identidad ISRC∪huella que lo hace
/// real llega con L10 (#73). Cualquier valor enviado hoy produce un error
/// explícito en vez de una semántica inventada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicatesFilter {
    Only,
    Exclude,
}

/// Consulta de biblioteca v2 (issue #98). AND entre dimensiones, OR dentro
/// de cada lista. Cuenta y servicio se evalúan sobre la misma pertenencia
/// cuando ambos vienen.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LibraryQuery {
    /// Texto libre (título, artista, álbum). L01: LIKE combinado con los
    /// filtros; la relevancia ponderada FTS es L02.
    pub text: Option<String>,
    /// Servicios por id. Solo: pertenencia por `track_sources` (badges).
    /// Con cuentas: pertenencia de esa cuenta en ese servicio.
    pub service_ids: Vec<i64>,
    /// Cuentas por id. La atribución de cuenta sale de las pertenencias
    /// (library_entries y playlists con dueño), no se inventa.
    pub account_ids: Vec<i64>,
    /// Colecciones; `album`/`artist` deshabilitados hasta L00 (#97).
    pub collections: Vec<CollectionFilter>,
    /// Solo favoritos (`tracks.is_favorite`).
    pub favorites_only: bool,
    /// Estado de descarga (OR dentro de la lista).
    pub download_statuses: Vec<DownloadStatusFilter>,
    /// Buckets de calidad (OR dentro de la lista).
    pub quality_buckets: Vec<QualityBucketFilter>,
    /// «En playlist»: `Some(true)` exige pertenencia, `Some(false)` la excluye.
    pub in_playlists: Option<bool>,
    /// Contrato únicamente; L10 (#73) la completa. Error si se envía.
    pub duplicates: Option<DuplicatesFilter>,
    /// Modo de cuarentena; por defecto `exclude` (no-op honesto hoy).
    pub quarantine: QuarantineMode,
    /// Géneros exactos (OR dentro de la lista).
    pub genre: Vec<String>,
    pub sort: LibrarySortKey,
    pub direction: SortDirection,
    pub offset: i64,
    /// Por defecto 100, máximo 200.
    pub limit: Option<i64>,
}

/// Contadores de cabecera (solo en la primera página: `offset == 0`).
/// Cada contador se calcula sobre el filtro actual SIN su propia dimensión
/// (story 11 del spec #96), para que el número del chip coincida con el
/// total que se ve al aplicarlo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryQueryCounts {
    /// Mismo valor que `total` de la página (redundante a propósito).
    pub total: i64,
    pub favorites: i64,
    pub downloaded: i64,
    pub in_playlists: i64,
    /// Pistas distintas con fuente en cada servicio (badges: track_sources).
    pub by_service: Vec<ServiceCount>,
    /// Pertenencias por cuenta (library_entries + playlists con dueño).
    /// PUEDE superar el total global: una pista en dos cuentas cuenta en las
    /// dos; la UI no debe presentar la suma como total.
    pub by_account: Vec<AccountCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ServiceCount {
    pub service_id: i64,
    pub service_name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AccountCount {
    pub account_id: i64,
    pub display_name: Option<String>,
    pub service_name: String,
    pub count: i64,
}

/// Resultado de una llamada: página + total exacto + `has_more` + contadores.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryQueryResult {
    pub tracks: Vec<crate::commands::types::LibraryTrack>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
    pub has_more: bool,
    /// `Some` solo en la primera página (`offset == 0`); las siguientes no
    /// recalculan contadores (los conserva la vista).
    pub counts: Option<LibraryQueryCounts>,
}

// ==========================================================================
// Expresiones SQL compartidas (una definición, varios usos: filtro y fila)
// ==========================================================================

/// Pertenencia viva (L01): favoritos por cuenta (`library_entries`) o pistas
/// de playlist (`playlist_tracks`). UNA pista sin ninguna pertenencia viva
/// no aparece. Ninguna consulta usa `is_active`.
///
/// BORDE ÚNICO DE L00 (#97): cuando aterrice la pertenencia completa
/// (álbumes, artistas), se enchufa AQUÍ — este es el único sitio donde la
/// pertenencia entra al filtro de la consulta.
const MEMBERSHIP_SQL: &str = "(EXISTS (SELECT 1 FROM library_entries le WHERE le.track_id = t.id) \
     OR EXISTS (SELECT 1 FROM playlist_tracks pt WHERE pt.track_id = t.id))";

/// Fecha de añadido: `MAX(added_at)` sobre las pertenencias vivas de la
/// pista. Segundo punto de conexión de L00: materializará
/// `tracks.library_added_at` y este cálculo pasará a leerla.
const ADDED_AT_SQL: &str = "(SELECT MAX(m.added_at) FROM (\
    SELECT le.added_at AS added_at FROM library_entries le WHERE le.track_id = t.id \
    UNION ALL \
    SELECT pt.added_at AS added_at FROM playlist_tracks pt WHERE pt.track_id = t.id) m)";

/// Nombre del artista primario (subconsulta, sin depender de joins externos).
const PRIMARY_ARTIST_NAME_SQL: &str = "(SELECT a2.name FROM track_artists ta2 \
    JOIN artists a2 ON a2.id = ta2.artist_id \
    WHERE ta2.track_id = t.id AND ta2.role = 'primary' LIMIT 1)";

/// Título del álbum canónico de la pista.
const ALBUM_TITLE_SQL: &str = "(SELECT al2.title FROM albums al2 WHERE al2.id = t.album_id)";

/// Estado de descarga como subescalar (mismas reglas que `get_library`):
/// descargado si hay `downloads.file_path`, en cola si hay cola activa.
const DOWNLOAD_STATUS_SQL: &str = "(CASE \
    WHEN EXISTS (SELECT 1 FROM downloads d WHERE d.track_id = t.id AND d.file_path IS NOT NULL) THEN 'downloaded' \
    WHEN EXISTS (SELECT 1 FROM download_queue dq WHERE dq.track_id = t.id AND dq.status IN ('queued', 'downloading')) THEN 'queued' \
    ELSE 'not_downloaded' END)";

/// Bucket de calidad (misma clasificación que `get_audio_quality_distribution`
/// y el deep-link FE-10; `downloads.track_id` es UNIQUE, así que la
/// subescalar es equivalente al join original).
const QUALITY_BUCKET_SQL: &str = "(CASE \
    WHEN NOT EXISTS (SELECT 1 FROM downloads d WHERE d.track_id = t.id) THEN NULL \
    WHEN EXISTS (SELECT 1 FROM downloads d WHERE d.track_id = t.id AND (COALESCE(d.bit_depth, 0) >= 24 OR COALESCE(d.sample_rate, 0) > 48000)) THEN 'Hi-Res (24-bit+)' \
    WHEN EXISTS (SELECT 1 FROM downloads d WHERE d.track_id = t.id AND d.file_format IN ('FLAC', 'ALAC', 'WAV')) THEN 'CD Quality' \
    ELSE 'Lossy' END)";

/// Calidad como texto: formato del download si existe, si no el de la mejor
/// fuente disponible (mismo orden que `idx_track_sources_best`).
const QUALITY_SQL: &str = "(COALESCE(\
    (SELECT d.file_format FROM downloads d WHERE d.track_id = t.id LIMIT 1), \
    (SELECT ts.format FROM track_sources ts WHERE ts.track_id = t.id AND ts.format IS NOT NULL \
     ORDER BY ts.available DESC, ts.quality_score DESC LIMIT 1)))";

/// Metadata score (0-100), idéntico al de `get_library` pero con subescalas
/// para no depender de joins externos.
const METADATA_SCORE_SQL: &str = "(
    CASE WHEN t.title IS NOT NULL AND t.title != '' THEN 10 ELSE 0 END +
    CASE WHEN EXISTS (SELECT 1 FROM track_artists WHERE track_id = t.id) THEN 10 ELSE 0 END +
    CASE WHEN (SELECT al2.title FROM albums al2 WHERE al2.id = t.album_id) IS NOT NULL AND (SELECT al2.title FROM albums al2 WHERE al2.id = t.album_id) != '' THEN 10 ELSE 0 END +
    CASE WHEN t.isrc IS NOT NULL AND t.isrc != '' THEN 20 ELSE 0 END +
    CASE WHEN t.musicbrainz_id IS NOT NULL AND t.musicbrainz_id NOT IN ('NOT_FOUND', 'MISMATCH') THEN 20 ELSE 0 END +
    CASE WHEN (SELECT al2.cover_art_url FROM albums al2 WHERE al2.id = t.album_id) IS NOT NULL AND (SELECT al2.cover_art_url FROM albums al2 WHERE al2.id = t.album_id) != '' THEN 10 ELSE 0 END +
    CASE WHEN t.release_year IS NOT NULL AND t.release_year > 0 THEN 10 ELSE 0 END +
    CASE WHEN t.genre IS NOT NULL AND t.genre != '' THEN 10 ELSE 0 END
)";

// ==========================================================================
// Construcción de WHERE (AND entre dimensiones, OR dentro de la lista)
// ==========================================================================

/// Dimensión que un contador excluye de sí mismo (story 11: cada chip se
/// calcula sin su propia condición).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dimension {
    All,
    Services,
    Accounts,
    Favorites,
    DownloadStatus,
    InPlaylists,
}

/// WHERE dinámico con binds de texto. Los ids (`i64` del contrato serde) se
/// interpolan: un `i64` no portador de SQL es seguro por construcción.
struct WhereBuilder {
    fragments: Vec<String>,
    binds: Vec<String>,
}

impl WhereBuilder {
    fn new() -> Self {
        // La pertenencia viva entra SIEMPRE: la lista refleja lo importado,
        // no un catálogo residual (story 3).
        WhereBuilder {
            fragments: vec![MEMBERSHIP_SQL.to_owned()],
            binds: Vec::new(),
        }
    }

    /// Un fragmento puede traer varios binds (p. ej. el LIKE repetido).
    fn push_binds(&mut self, sql: String, binds: Vec<String>) {
        self.binds.extend(binds);
        self.fragments.push(sql);
    }

    fn finish(self) -> (String, Vec<String>) {
        let WhereBuilder { fragments, binds } = self;
        let sql = fragments
            .iter()
            .map(|f| format!("({f})"))
            .collect::<Vec<_>>()
            .join(" AND ");
        (sql, binds)
    }
}

fn escape_like(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Construye el WHERE de la consulta (o de un contador) según la dimensión
/// que se ignora.
fn build_where(query: &LibraryQuery, skip: Dimension) -> Result<(String, Vec<String>), String> {
    let mut w = WhereBuilder::new();

    // Contrato deshabilitado, explícito con su motivo (no inventado).
    if query.duplicates.is_some() {
        return Err(
            "El filtro de duplicados es contrato pendiente: se completa en L10 (#73) \
             con la identidad ISRC/huella. No disponible todavía."
                .to_owned(),
        );
    }
    match query.quarantine {
        QuarantineMode::Exclude => {
            // No-op honesto: sin #72 no hay tabla de cuarentena y no hay nada
            // que excluir; cuando L09 aterrice este brazo filtrará de verdad.
        }
        QuarantineMode::Only | QuarantineMode::Include => {
            return Err(
                "La semántica de cuarentena es contrato pendiente: no hay tabla de \
                 cuarentena hasta #72 y la completa L09. Solo 'exclude' está activo."
                    .to_owned(),
            )
        }
    }

    // Texto: título, artista primario o álbum. Se COMBINA con los filtros
    // (no sustituye la lista, como hacía la vista).
    if let Some(text) = query
        .text
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let pattern = format!("%{}%", escape_like(text));
        let like = |col: &str| format!("{} LIKE ? ESCAPE '\\'", col);
        w.push_binds(
            format!(
                "({} OR {} OR {})",
                like("t.title"),
                like(PRIMARY_ARTIST_NAME_SQL),
                like(ALBUM_TITLE_SQL)
            ),
            vec![pattern.clone(), pattern.clone(), pattern],
        );
    }

    // Cuenta + servicio sobre la MISMA pertenencia (favorito o playlist con
    // dueño en ese servicio); servicio solo, sobre las fuentes (badges).
    let has_services = !query.service_ids.is_empty() && skip != Dimension::Services;
    let has_accounts = !query.account_ids.is_empty() && skip != Dimension::Accounts;
    if has_services && has_accounts {
        let accounts = inline_ids(&query.account_ids);
        let services = inline_ids(&query.service_ids);
        w.fragments.push(format!(
            "(EXISTS (SELECT 1 FROM library_entries le_c \
                JOIN accounts ac_c ON ac_c.id = le_c.account_id \
                JOIN services sv_c ON sv_c.id = ac_c.service_id \
                WHERE le_c.track_id = t.id AND le_c.account_id IN ({accounts}) AND sv_c.id IN ({services})) \
             OR EXISTS (SELECT 1 FROM playlist_tracks pt_c \
                JOIN playlists pl_c ON pl_c.id = pt_c.playlist_id \
                JOIN accounts ac_c ON ac_c.id = pl_c.account_id \
                JOIN services sv_c ON sv_c.id = ac_c.service_id \
                WHERE pt_c.track_id = t.id AND pl_c.account_id IN ({accounts}) AND sv_c.id IN ({services})))"
        ));
    } else {
        if has_services {
            let services = inline_ids(&query.service_ids);
            w.fragments.push(format!(
                "EXISTS (SELECT 1 FROM track_sources ts_f JOIN services s_f ON s_f.id = ts_f.service_id \
                 WHERE ts_f.track_id = t.id AND ts_f.service_id IN ({services}))"
            ));
        }
        if has_accounts {
            let accounts = inline_ids(&query.account_ids);
            w.fragments.push(format!(
                "(EXISTS (SELECT 1 FROM library_entries le_a \
                    WHERE le_a.track_id = t.id AND le_a.account_id IN ({accounts})) \
                 OR EXISTS (SELECT 1 FROM playlist_tracks pt_a \
                    JOIN playlists pl_a ON pl_a.id = pt_a.playlist_id \
                    WHERE pt_a.track_id = t.id AND pl_a.account_id IN ({accounts})))"
            ));
        }
    }

    // Colecciones: solo las que existen hoy. `album`/`artist` explícitos
    // con su motivo (pertenencia de L00, #97).
    if !query.collections.is_empty() {
        let mut kinds = Vec::new();
        for collection in &query.collections {
            match collection {
                CollectionFilter::Favorite => {
                    kinds.push(
                        "EXISTS (SELECT 1 FROM library_entries le_k WHERE le_k.track_id = t.id)"
                            .to_owned(),
                    );
                }
                CollectionFilter::Playlist => {
                    kinds.push(
                        "EXISTS (SELECT 1 FROM playlist_tracks pt_k WHERE pt_k.track_id = t.id)"
                            .to_owned(),
                    );
                }
                CollectionFilter::Album | CollectionFilter::Artist => {
                    return Err(format!(
                        "Colección {:?} es contrato pendiente: la pertenencia por \
                         álbum/artista aterriza con L00 (#97). No disponible todavía.",
                        match collection {
                            CollectionFilter::Album => "album",
                            CollectionFilter::Artist => "artist",
                            _ => unreachable!(),
                        }
                    ));
                }
            }
        }
        w.fragments.push(format!("({})", kinds.join(" OR ")));
    }

    // Solo favoritos. El contador propio (Dimension::Favorites) lo omite.
    if query.favorites_only && skip != Dimension::Favorites {
        w.fragments.push("t.is_favorite = 1".to_owned());
    }

    // Estado de descarga.
    if !query.download_statuses.is_empty() && skip != Dimension::DownloadStatus {
        let statuses = query
            .download_statuses
            .iter()
            .map(|s| format!("'{}'", s.sql()))
            .collect::<Vec<_>>()
            .join(", ");
        w.fragments
            .push(format!("{DOWNLOAD_STATUS_SQL} IN ({statuses})"));
    }

    // Buckets de calidad.
    if !query.quality_buckets.is_empty() {
        let buckets = query
            .quality_buckets
            .iter()
            .map(|b| format!("'{}'", b.label()))
            .collect::<Vec<_>>()
            .join(", ");
        w.fragments
            .push(format!("{QUALITY_BUCKET_SQL} IN ({buckets})"));
    }

    // «En playlist».
    if let Some(in_playlists) = query.in_playlists {
        if skip != Dimension::InPlaylists {
            match in_playlists {
                true => w.fragments.push(
                    "EXISTS (SELECT 1 FROM playlist_tracks pt_i WHERE pt_i.track_id = t.id)"
                        .to_owned(),
                ),
                false => w.fragments.push(
                    "NOT EXISTS (SELECT 1 FROM playlist_tracks pt_i WHERE pt_i.track_id = t.id)"
                        .to_owned(),
                ),
            }
        }
    }

    // Género exacto (OR dentro de la lista).
    if !query.genre.is_empty() {
        let placeholders = vec!["?"; query.genre.len()].join(", ");
        w.fragments.push(format!("t.genre IN ({placeholders})"));
        w.binds.extend(query.genre.iter().cloned());
    }

    Ok(w.finish())
}

fn inline_ids(ids: &[i64]) -> String {
    ids.iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Cláusula ORDER BY: nulos/centinelas al final, dirección SOLO en la clave
/// primaria, desempates siempre título `NOCASE`, artista e id ascendentes.
fn order_clause(query: &LibraryQuery) -> String {
    let dir = query.direction.sql();
    // DateAdded: centinela '' y NULL se tratan igual (COALESCE) para que el
    // desempate por título decida dentro del bloque de "sin fecha".
    let (primary, guard) = match query.sort {
        LibrarySortKey::DateAdded => (
            format!("COALESCE({ADDED_AT_SQL}, '')"),
            format!("({ADDED_AT_SQL} IS NULL OR {ADDED_AT_SQL} = '')"),
        ),
        LibrarySortKey::Title => (
            "t.title COLLATE NOCASE".to_owned(),
            "t.title IS NULL".to_owned(),
        ),
        LibrarySortKey::Artist => (
            PRIMARY_ARTIST_NAME_SQL.to_owned(),
            format!("{PRIMARY_ARTIST_NAME_SQL} IS NULL"),
        ),
        LibrarySortKey::Album => (
            ALBUM_TITLE_SQL.to_owned(),
            format!("{ALBUM_TITLE_SQL} IS NULL"),
        ),
        LibrarySortKey::Duration => (
            "t.duration_ms".to_owned(),
            "t.duration_ms IS NULL".to_owned(),
        ),
        LibrarySortKey::Quality => (QUALITY_SQL.to_owned(), format!("{QUALITY_SQL} IS NULL")),
        LibrarySortKey::MetadataScore => (
            METADATA_SCORE_SQL.to_owned(),
            format!("{METADATA_SCORE_SQL} IS NULL"),
        ),
    };
    format!(
        "{guard} ASC, {primary} {dir}, \
         t.title COLLATE NOCASE ASC, \
         {PRIMARY_ARTIST_NAME_SQL} IS NULL ASC, {PRIMARY_ARTIST_NAME_SQL} ASC, \
         t.id ASC"
    )
}

// ==========================================================================
// Proyección de fila (una fila por tracks.id, badges agregados en la misma
// consulta — sin N+1)
// ==========================================================================

/// Proyección de fila: idéntica semántica a la de `get_library` (la vista
/// mapea igual), pero con las expresiones compartidas de este módulo para
/// que el filtro y la fila mostrada nunca diverjan.
const ROW_COLUMNS_SQL: &str = "
        t.id,
        t.title,
        {PRIMARY_ARTIST} as artist_name,
        (SELECT a2.id FROM track_artists ta2
         JOIN artists a2 ON a2.id = ta2.artist_id
         WHERE ta2.track_id = t.id AND ta2.role = 'primary'
         LIMIT 1) as artist_id,
        al.title as album_name,
        al.id as album_id,
        t.duration_ms,
        t.isrc,
        GROUP_CONCAT(DISTINCT s.name) as services,
        (SELECT GROUP_CONCAT(DISTINCT s_imp.name)
         FROM library_entries le
         JOIN accounts acc ON acc.id = le.account_id
         JOIN services s_imp ON s_imp.id = acc.service_id
         WHERE le.track_id = t.id) as imported_from,
        COALESCE(d.effective_service, (SELECT s_dl.name FROM services s_dl WHERE s_dl.id = d.source_service_id)) as downloaded_from,
        (SELECT GROUP_CONCAT(DISTINCT s_avail.name)
         FROM track_sources ts_avail
         JOIN services s_avail ON s_avail.id = ts_avail.service_id
         WHERE ts_avail.track_id = t.id AND ts_avail.availability_status = 'available') as available_services,
        (SELECT GROUP_CONCAT(s_all.name || ':' || COALESCE(ts_all.availability_status, 'unknown_unchecked'), ', ')
         FROM track_sources ts_all
         JOIN services s_all ON s_all.id = ts_all.service_id
         WHERE ts_all.track_id = t.id) as availability_summary,
        {QUALITY} as quality,
        {QUALITY_BUCKET} as quality_bucket,
        {DOWNLOAD_STATUS} as download_status,
        {METADATA_SCORE} as metadata_score,
        CASE
            WHEN l.sync_level IN ('syllable', 'word') THEN 'synced'
            WHEN l.sync_level = 'line' THEN 'timed'
            WHEN l.content IS NOT NULL THEN 'plain'
            ELSE 'none'
        END as lyrics_type,
        al.cover_art_url as cover_art_url,
        ts_spot.service_track_id as spotify_track_id,
        t.track_number,
        t.disc_number,
        t.genre,
        t.bpm,
        t.musical_key,
        t.release_year,
        t.explicit,
        t.is_favorite,
        t.favorite_at,
        d.file_path,
        t.display_title,
        COALESCE(t.source_title, t.title) as source_title,
        COALESCE(t.file_disambiguator, d.file_disambiguator) as file_disambiguator";

fn page_rows_sql() -> String {
    ROW_COLUMNS_SQL
        .replace("{PRIMARY_ARTIST}", PRIMARY_ARTIST_NAME_SQL)
        .replace("{QUALITY}", QUALITY_SQL)
        .replace("{QUALITY_BUCKET}", QUALITY_BUCKET_SQL)
        .replace("{DOWNLOAD_STATUS}", DOWNLOAD_STATUS_SQL)
        .replace("{METADATA_SCORE}", METADATA_SCORE_SQL)
}

// ==========================================================================
// La consulta
// ==========================================================================

/// Consulta de biblioteca v2 (issue #98). Recibe el pool — la costura de
/// prueba es esta función, no el comando de Tauri.
pub async fn query_library(
    db: &SqlitePool,
    query: LibraryQuery,
) -> Result<LibraryQueryResult, String> {
    let offset = query.offset.max(0);
    let limit = query.limit.unwrap_or(100).clamp(1, 200);
    let order = order_clause(&query);

    // Total exacto del filtro (independiente de la página).
    let (where_all, binds_all) = build_where(&query, Dimension::All)?;
    let total_sql = format!("SELECT COUNT(DISTINCT t.id) FROM tracks t WHERE {where_all}");
    let mut total_query = sqlx::query_as::<_, (i64,)>(&total_sql);
    for bind in &binds_all {
        total_query = total_query.bind(bind);
    }
    let (total,): (i64,) = total_query
        .fetch_one(db)
        .await
        .map_err(|e| format!("query_library: error contando: {e}"))?;

    // Ids de la página, ordenados en servidor.
    let page_ids_sql = format!(
        "SELECT t.id FROM tracks t WHERE {where_all} ORDER BY {order} LIMIT {limit} OFFSET {offset}"
    );
    let mut page_ids_query = sqlx::query_as::<_, (i64,)>(&page_ids_sql);
    for bind in &binds_all {
        page_ids_query = page_ids_query.bind(bind);
    }
    let page_ids: Vec<(i64,)> = page_ids_query
        .fetch_all(db)
        .await
        .map_err(|e| format!("query_library: error paginando: {e}"))?;
    let page_ids = page_ids.into_iter().map(|(id,)| id).collect::<Vec<_>>();

    // Filas de la página: una consulta con los badges agregados (GROUP_CONCAT),
    // sin N+1. Sin ids no se consulta nada. El WHERE es solo la lista de ids
    // (ya pasaron el filtro): sin binds que encajar.
    let mut tracks = Vec::new();
    if !page_ids.is_empty() {
        let ids = inline_ids(&page_ids);
        let rows_sql = format!(
            "SELECT {} \
             FROM tracks t \
             LEFT JOIN albums al ON al.id = t.album_id \
             LEFT JOIN track_sources ts ON ts.track_id = t.id \
             LEFT JOIN services s ON s.id = ts.service_id \
             LEFT JOIN track_sources ts_spot ON ts_spot.track_id = t.id AND ts_spot.service_id = (SELECT id FROM services WHERE name = 'spotify') \
             LEFT JOIN downloads d ON d.track_id = t.id \
             LEFT JOIN lyrics l ON l.track_id = t.id \
             WHERE t.id IN ({ids}) \
             GROUP BY t.id \
             ORDER BY {order}",
            page_rows_sql()
        );
        tracks = sqlx::query_as::<_, crate::commands::types::LibraryTrack>(&rows_sql)
            .fetch_all(db)
            .await
            .map_err(|e| format!("query_library: error leyendo filas: {e}"))?;

        // Misma verificación de ficheros que get_library: un download cuyo
        // fichero desapareció baja a not_downloaded.
        crate::commands::library::verify_page_files(&mut tracks).await?;
    }

    let has_more = offset + (tracks.len() as i64) < total;

    // Contadores solo en la primera página; al paginar no se recalculan.
    let counts = if offset == 0 {
        Some(build_counts(db, &query, total).await?)
    } else {
        None
    };

    Ok(LibraryQueryResult {
        tracks,
        total,
        offset,
        limit,
        has_more,
        counts,
    })
}

/// Plan de la consulta de conteo (EXPLAIN QUERY PLAN) para la verificación
/// del issue #98. Hoy entra por los índices existentes; cuando L00 (#97)
/// aterrice con sus índices de pertenencia, esta función es el punto de
/// re-verificación del plan.
#[allow(dead_code)] // Cubierta por `tests/library_query_test.rs` (el IPC no la expone).
pub async fn query_plan(db: &SqlitePool, query: &LibraryQuery) -> Result<Vec<String>, String> {
    let (where_all, binds) = build_where(query, Dimension::All)?;
    let sql =
        format!("EXPLAIN QUERY PLAN SELECT COUNT(DISTINCT t.id) FROM tracks t WHERE {where_all}");
    let mut q = sqlx::query_as::<_, (i64, i64, i64, String)>(&sql);
    for bind in &binds {
        q = q.bind(bind);
    }
    let rows = q
        .fetch_all(db)
        .await
        .map_err(|e| format!("query_plan: {e}"))?;
    Ok(rows.into_iter().map(|(_, _, _, detail)| detail).collect())
}
async fn count_matching(
    db: &SqlitePool,
    query: &LibraryQuery,
    skip: Dimension,
    extra_condition: &str,
) -> Result<i64, String> {
    let (where_sql, binds) = build_where(query, skip)?;
    let sql = format!(
        "SELECT COUNT(DISTINCT t.id) FROM tracks t WHERE ({where_sql}) AND ({extra_condition})"
    );
    let mut q = sqlx::query_as::<_, (i64,)>(&sql);
    for bind in &binds {
        q = q.bind(bind);
    }
    let (count,): (i64,) = q
        .fetch_one(db)
        .await
        .map_err(|e| format!("query_library: error contando: {e}"))?;
    Ok(count)
}

/// Contadores de cabecera. Cada uno sobre el filtro SIN su propia dimensión.
async fn build_counts(
    db: &SqlitePool,
    query: &LibraryQuery,
    total: i64,
) -> Result<LibraryQueryCounts, String> {
    let favorites = count_matching(db, query, Dimension::Favorites, "t.is_favorite = 1").await?;
    let downloaded = count_matching(
        db,
        query,
        Dimension::DownloadStatus,
        &format!("{DOWNLOAD_STATUS_SQL} = 'downloaded'"),
    )
    .await?;
    let in_playlists = count_matching(
        db,
        query,
        Dimension::InPlaylists,
        "EXISTS (SELECT 1 FROM playlist_tracks pt_c2 WHERE pt_c2.track_id = t.id)",
    )
    .await?;

    // Por servicio (badges: track_sources), sin el propio filtro de servicios.
    let (where_svc, binds_svc) = build_where(query, Dimension::Services)?;
    let by_service_sql = format!(
        "SELECT s.id AS service_id, s.name AS service_name, COUNT(DISTINCT t.id) AS count \
         FROM tracks t \
         JOIN track_sources ts ON ts.track_id = t.id \
         JOIN services s ON s.id = ts.service_id \
         WHERE {where_svc} \
         GROUP BY s.id, s.name \
         ORDER BY s.name COLLATE NOCASE"
    );
    let mut by_service_query = sqlx::query_as::<_, ServiceCount>(&by_service_sql);
    for bind in &binds_svc {
        by_service_query = by_service_query.bind(bind);
    }
    let by_service = by_service_query
        .fetch_all(db)
        .await
        .map_err(|e| format!("query_library: error contando servicios: {e}"))?;

    // Por cuenta: pertenencias (favoritos por cuenta + playlists con dueño),
    // sin el propio filtro de cuentas. COUNT de pertenencias: puede superar
    // el total de pistas distintas. El conjunto filtrado va en un CTE para
    // no duplicar sus placeholders.
    let (where_acc, binds_acc) = build_where(query, Dimension::Accounts)?;
    let by_account_sql = format!(
        "WITH matched AS (SELECT t.id FROM tracks t WHERE {where_acc}) \
         SELECT a.id AS account_id, a.display_name, sv.name AS service_name, SUM(m.cnt) AS count \
         FROM ( \
           SELECT le.account_id AS account_id, le.track_id AS track_id, 1 AS cnt \
           FROM library_entries le \
           WHERE le.track_id IN (SELECT id FROM matched) \
           UNION ALL \
           SELECT pl.account_id AS account_id, pt.track_id AS track_id, 1 AS cnt \
           FROM playlist_tracks pt \
           JOIN playlists pl ON pl.id = pt.playlist_id \
           WHERE pt.track_id IN (SELECT id FROM matched) \
         ) m \
         JOIN accounts a ON a.id = m.account_id \
         JOIN services sv ON sv.id = a.service_id \
         GROUP BY a.id, a.display_name, sv.name \
         ORDER BY sv.name COLLATE NOCASE, a.id"
    );
    let mut by_account_query = sqlx::query_as::<_, AccountCount>(&by_account_sql);
    for bind in &binds_acc {
        by_account_query = by_account_query.bind(bind);
    }
    let by_account = by_account_query
        .fetch_all(db)
        .await
        .map_err(|e| format!("query_library: error contando cuentas: {e}"))?;

    Ok(LibraryQueryCounts {
        total,
        favorites,
        downloaded,
        in_playlists,
        by_service,
        by_account,
    })
}

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
//! Pertenencia (L00, issue #97): la biblioteca incluye TODO lo importado de
//! cualquier colección. El borde de visibilidad ([`MEMBERSHIP_SQL`]) es la
//! unión de las tres fuentes de pertenencia viva: favoritos por cuenta
//! (`library_entries`), pistas de playlist (`playlist_tracks`) y la
//! pertenencia completa por cuenta/track/colección (`library_memberships`,
//! migración 0112 — álbumes, artistas, compras e importación local). La
//! cuenta de una pista sale solo de esas pertenencias; el orden por fecha de
//! añadido lee el materializado `tracks.library_added_at` (índice
//! `idx_tracks_library_added_at` de 0112) y las colecciones `album`/`artist`
//! del filtro entran por `library_memberships`.
//!
//! Piezas deshabilitadas con su motivo (no inventadas):
//! - filtro `duplicates`: la identidad ISRC∪huella llega con L10 (#73);
//! - cuarentena `only`/`include`: no hay tabla de cuarentena hasta #72/L09;
//!   `exclude` (por defecto) es hoy un no-op honesto.
//!
//! Búsqueda de texto (L02, issue #99): FTS5 sobre `library_fts` (migración
//! 0107: título, artista, álbum, género e ISRC) combinada con los filtros
//! (no los sustituye). Relevancia `bm25` ponderada — título > artista >
//! álbum > género > ISRC —; si el texto tiene forma de ISRC (12 caracteres
//! normalizados sin guiones), las coincidencias exactas por ISRC van
//! DELANTE de las de relevancia y el ISRC no se busca solo por prefijo.
//! Los caracteres especiales de FTS se escapan y el último token va en modo
//! prefijo (escritura incremental). Con texto vacío se usa el orden elegido.
//!
//! Detalle de pista (L04, issue #101): [`get_track_detail`] devuelve en UNA
//! llamada IPC el Track canónico (misma proyección que la lista, más
//! MusicBrainz), sus TrackSource por servicio, sus pertenencias por cuenta y
//! colección (LibraryItem) y la descarga (estado, servicio efectivo,
//! calidad y ruta). Consultas por faceta —una para la fila, una para todas
//! las fuentes, una para todas las pertenencias—, nunca por fuente ni por
//! pertenencia (no N+1). Una pista inexistente es un error tipado
//! ([`TrackDetailError::TrackNotFound`]), no una fila vacía.
//!
//! Facetas deshabilitadas con su motivo (no inventadas), como en el resto
//! del contrato:
//! - `quarantine` es `None`: no hay tabla de cuarentena hasta #72; la
//!   sección la completa L09 (motivo y última fecha vista).
//! - `duplicate_group` es `None`: la identidad ISRC∪huella y el grupo de
//!   duplicados llegan con L10 (#73). No se simulan.

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
    /// Fecha de añadido: `tracks.library_added_at` materializado por L00
    /// (#97) — máximo `added_at` vivo no nulo/distinto de 1970, con
    /// `created_at` como respaldo. La consulta entra por
    /// `idx_tracks_library_added_at` (0112).
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

/// Colección del filtro (L00, #97): las cuatro colecciones del contrato.
/// `favorite` y `playlist` conservan sus bordes de L01; `album`/`artist`
/// entran por `library_memberships` (0112) con el id local de la colección.
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
    /// Texto libre (título, artista, álbum, género, ISRC). L02: FTS5 con
    /// relevancia bm25 ponderada, combinado con los filtros (no los
    /// sustituye). Con texto vacío manda el orden elegido.
    pub text: Option<String>,
    /// Servicios por id. Solo: pertenencia por `track_sources` (badges).
    /// Con cuentas: pertenencia de esa cuenta en ese servicio.
    pub service_ids: Vec<i64>,
    /// Cuentas por id. La atribución de cuenta sale de las pertenencias
    /// (library_entries, playlists con dueño y library_memberships de L00),
    /// no se inventa ni se rellena con la cuenta activa.
    pub account_ids: Vec<i64>,
    /// Colecciones (L00, #97): favorite, playlist, album y artist.
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
    /// Pistas distintas por cuenta (pertenencias: library_entries +
    /// playlists con dueño + library_memberships de L00). PUEDE superar el
    /// total global: una pista en dos cuentas cuenta en las dos; la UI no
    /// debe presentar la suma como total.
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

/// Pertenencia viva (L01 + L00): favoritos por cuenta (`library_entries`),
/// pistas de playlist (`playlist_tracks`) y la pertenencia completa por
/// cuenta y colección (`library_memberships`, migración 0112: álbumes,
/// artistas y demás colecciones del sync). UNA pista sin ninguna
/// pertenencia viva no aparece. Ninguna consulta usa `is_active`.
///
/// La pertenencia de L00 (#97) entra SOLO aquí: este es el único sitio donde
/// la pertenencia entra al filtro de la consulta.
const MEMBERSHIP_SQL: &str = "(EXISTS (SELECT 1 FROM library_entries le WHERE le.track_id = t.id) \
     OR EXISTS (SELECT 1 FROM playlist_tracks pt WHERE pt.track_id = t.id) \
     OR EXISTS (SELECT 1 FROM library_memberships lm WHERE lm.track_id = t.id))";

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
// Búsqueda FTS (L02, issue #99)
// ==========================================================================

/// Pesos bm25 en orden de columnas de `library_fts` (migración 0107:
/// título, artista, álbum, género, isrc). Criterio del issue #99:
/// título > artista > álbum > género > ISRC. Cuanto mayor el peso, más
/// manda la columna en la relevancia.
const FTS_BM25_WEIGHTS: &str = "10.0, 5.0, 3.0, 2.0, 1.0";

/// Normaliza un ISRC: sin guiones ni espacios, en mayúsculas (la columna
/// indexada aplica la misma normalización en los triggers de 0107).
fn normalize_isrc(raw: &str) -> String {
    raw.chars()
        .filter(|c| !matches!(c, '-' | ' ' | '\t'))
        .flat_map(char::to_uppercase)
        .collect()
}

/// ¿Tiene forma de ISRC tras normalizar? 12 caracteres: 2 letras, 5
/// alfanuméricos y 5 dígitos (CC-XXX-YY-NNNNN).
fn looks_like_isrc(normalized: &str) -> bool {
    let b = normalized.as_bytes();
    b.len() == 12
        && b[..2].iter().all(u8::is_ascii_alphabetic)
        && b[2..7].iter().all(u8::is_ascii_alphanumeric)
        && b[7..].iter().all(u8::is_ascii_digit)
}

/// Consulta MATCH de FTS5 desde el texto del usuario:
/// - cada token se envuelve entre comillas (los caracteres especiales de
///   FTS5 no se interpretan; `"` interna se duplica);
/// - el último token va en modo prefijo (`"tok"*`) para la escritura
///   incremental;
/// - si el texto completo tiene forma de ISRC, es un token EXACTO sin
///   prefijo (un ISRC no se busca solo por prefijo);
/// - los tokens sin ningún alfanumérico se descartan. `None` = texto sin
///   tokens de búsqueda: se comporta como texto vacío.
fn build_fts_match_query(raw: &str) -> Option<String> {
    let normalized = normalize_isrc(raw);
    if looks_like_isrc(&normalized) {
        return Some(format!("\"{normalized}\""));
    }
    let mut parts: Vec<String> = raw
        .split_whitespace()
        .filter(|t| t.chars().any(char::is_alphanumeric))
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect();
    if parts.is_empty() {
        return None;
    }
    if let Some(last) = parts.last_mut() {
        last.push('*');
    }
    Some(parts.join(" "))
}

/// Plan de búsqueda derivado del texto: la consulta MATCH ya construida y el
/// ISRC exacto si el texto tiene forma de ISRC. `None` = sin búsqueda activa
/// (texto vacío o sin tokens de búsqueda): manda el orden elegido.
struct SearchPlan {
    /// Consulta MATCH ya construida (escape + prefijo en el último token).
    fts_query: String,
    /// ISRC normalizado del texto, si tiene forma de ISRC. Con `Some`, los
    /// tracks con ese ISRC exacto van delante de las coincidencias por
    /// relevancia (criterio del issue #99).
    isrc_exact: Option<String>,
}

fn search_plan(query: &LibraryQuery) -> Option<SearchPlan> {
    let text = query
        .text
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())?;
    let normalized = normalize_isrc(text);
    let isrc_exact = looks_like_isrc(&normalized).then_some(normalized);
    let fts_query = build_fts_match_query(text)?;
    Some(SearchPlan {
        fts_query,
        isrc_exact,
    })
}

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

    // Texto: FTS5 sobre library_fts (título, artista, álbum, género, ISRC).
    // Se COMBINA con los filtros (no sustituye la lista). Los caracteres
    // especiales de FTS van escapados y el último token en prefijo; un texto
    // sin tokens de búsqueda se comporta como texto vacío.
    if let Some(plan) = search_plan(query) {
        w.push_binds(
            "t.id IN (SELECT rowid FROM library_fts WHERE library_fts MATCH ?)".to_owned(),
            vec![plan.fts_query],
        );
    }

    // Cuenta + servicio sobre la MISMA pertenencia (favorito, playlist con
    // dueño o pertenencia L00 en ese servicio); servicio solo, sobre las
    // fuentes (badges).
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
                WHERE pt_c.track_id = t.id AND pl_c.account_id IN ({accounts}) AND sv_c.id IN ({services})) \
             OR EXISTS (SELECT 1 FROM library_memberships lm_c \
                JOIN accounts ac_c ON ac_c.id = lm_c.account_id \
                JOIN services sv_c ON sv_c.id = ac_c.service_id \
                WHERE lm_c.track_id = t.id AND lm_c.account_id IN ({accounts}) AND sv_c.id IN ({services})))"
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
                    WHERE pt_a.track_id = t.id AND pl_a.account_id IN ({accounts})) \
                 OR EXISTS (SELECT 1 FROM library_memberships lm_a \
                    WHERE lm_a.track_id = t.id AND lm_a.account_id IN ({accounts})))"
            ));
        }
    }

    // Colecciones (L00, #97): las cuatro del contrato. `favorite` y
    // `playlist` ven sus DOS fuentes (los bordes de L01 y las pertenencias
    // de 0112); `album`/`artist` entran solo por la pertenencia completa
    // de `library_memberships` (0112).
    if !query.collections.is_empty() {
        let mut kinds = Vec::new();
        for collection in &query.collections {
            match collection {
                CollectionFilter::Favorite => {
                    kinds.push(
                        "(EXISTS (SELECT 1 FROM library_entries le_k WHERE le_k.track_id = t.id) \
                         OR EXISTS (SELECT 1 FROM library_memberships lm_k \
                         WHERE lm_k.track_id = t.id AND lm_k.collection_type = 'favorite'))"
                            .to_owned(),
                    );
                }
                CollectionFilter::Playlist => {
                    kinds.push(
                        "(EXISTS (SELECT 1 FROM playlist_tracks pt_k WHERE pt_k.track_id = t.id) \
                         OR EXISTS (SELECT 1 FROM library_memberships lm_k \
                         WHERE lm_k.track_id = t.id AND lm_k.collection_type = 'playlist'))"
                            .to_owned(),
                    );
                }
                CollectionFilter::Album => {
                    kinds.push(
                        "EXISTS (SELECT 1 FROM library_memberships lm_k \
                         WHERE lm_k.track_id = t.id AND lm_k.collection_type = 'album')"
                            .to_owned(),
                    );
                }
                CollectionFilter::Artist => {
                    kinds.push(
                        "EXISTS (SELECT 1 FROM library_memberships lm_k \
                         WHERE lm_k.track_id = t.id AND lm_k.collection_type = 'artist')"
                            .to_owned(),
                    );
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

/// WHERE del filtro de la consulta, sin paginación ni orden, para el runner
/// de acciones por lotes (L05, #102). Es la MISMA traducción que usa
/// [`query_library`]: el runner no reimplementa filtros ni pertenencia.
pub(crate) fn filter_where(query: &LibraryQuery) -> Result<(String, Vec<String>), String> {
    build_where(query, Dimension::All)
}

/// Cláusula ORDER BY. Con búsqueda activa: ISRC exacto delante (si el texto
/// tiene forma de ISRC), relevancia bm25 después (título > artista > álbum >
/// género > ISRC) y los desempates estables de siempre. Sin búsqueda: nulos/
/// centinelas al final, dirección SOLO en la clave primaria, desempates
/// siempre título `NOCASE`, artista e id ascendentes.
///
/// Devuelve la SQL y los binds de la cláusula (el ISRC exacto). La cláusula
/// exige `library_fts` en el FROM — solo la usa la consulta de ids de página;
/// las filas se devuelven siguiendo ese mismo orden de ids.
fn order_clause(query: &LibraryQuery) -> (String, Vec<String>) {
    let search = search_plan(query);
    let mut binds: Vec<String> = Vec::new();

    if let Some(search) = search {
        let mut clauses = Vec::new();
        if let Some(isrc) = &search.isrc_exact {
            clauses.push(
                "(CASE WHEN REPLACE(UPPER(t.isrc), '-', '') = ? THEN 0 ELSE 1 END) ASC".to_owned(),
            );
            binds.push(isrc.clone());
        }
        clauses.push(format!("bm25(library_fts, {FTS_BM25_WEIGHTS}) ASC"));
        clauses.push("t.title COLLATE NOCASE ASC".to_owned());
        clauses.push(format!(
            "{PRIMARY_ARTIST_NAME_SQL} IS NULL ASC, {PRIMARY_ARTIST_NAME_SQL} ASC"
        ));
        clauses.push("t.id ASC".to_owned());
        return (clauses.join(", "), binds);
    }

    let dir = query.direction.sql();
    // DateAdded (L00, #97): el materializado `tracks.library_added_at`
    // (máximo added_at vivo no nulo/distinto de 1970, con created_at como
    // respaldo). Centinela '' y NULL se tratan igual para que el desempate
    // por título decida dentro del bloque de "sin fecha".
    let (primary, guard) = match query.sort {
        LibrarySortKey::DateAdded => (
            "COALESCE(t.library_added_at, '')".to_owned(),
            "(t.library_added_at IS NULL OR t.library_added_at = '')".to_owned(),
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
    (
        format!(
            "{guard} ASC, {primary} {dir}, \
         t.title COLLATE NOCASE ASC, \
         {PRIMARY_ARTIST_NAME_SQL} IS NULL ASC, {PRIMARY_ARTIST_NAME_SQL} ASC, \
         t.id ASC"
        ),
        binds,
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
         FROM (SELECT le.account_id AS account_id FROM library_entries le WHERE le.track_id = t.id
               UNION
               SELECT lm_imp.account_id FROM library_memberships lm_imp WHERE lm_imp.track_id = t.id) m_imp
         JOIN accounts acc ON acc.id = m_imp.account_id
         JOIN services s_imp ON s_imp.id = acc.service_id) as imported_from,
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
        t.musicbrainz_id as musicbrainz_id,
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
    let plan = search_plan(&query);
    let searching = plan.is_some();
    let (order, order_binds) = order_clause(&query);
    // Con búsqueda activa la cláusula de relevancia (bm25) exige la tabla
    // FTS en el FROM con SU constraint MATCH (el de la subconsulta de filtro
    // no la cubre): la fila de FTS es 1:1 con el track (rebuild de 0107),
    // así que el join no descarta ni duplica filas.
    let fts_join = if searching {
        " JOIN library_fts ON library_fts.rowid = t.id"
    } else {
        ""
    };

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

    // Ids de la página, ordenados en servidor. Con búsqueda: ISRC exacto
    // delante y relevancia bm25 después (con el MATCH sobre la instancia
    // unida); sin ella, el orden elegido.
    let page_ids_sql = if searching {
        format!(
            "SELECT t.id FROM tracks t{fts_join} WHERE ({where_all}) AND library_fts MATCH ? \
             ORDER BY {order} LIMIT {limit} OFFSET {offset}"
        )
    } else {
        format!(
            "SELECT t.id FROM tracks t WHERE {where_all} ORDER BY {order} LIMIT {limit} OFFSET {offset}"
        )
    };
    let mut page_ids_query = sqlx::query_as::<_, (i64,)>(&page_ids_sql);
    for bind in &binds_all {
        page_ids_query = page_ids_query.bind(bind);
    }
    if let Some(plan) = &plan {
        page_ids_query = page_ids_query.bind(&plan.fts_query);
    }
    for bind in order_binds.iter() {
        page_ids_query = page_ids_query.bind(bind);
    }
    let page_ids: Vec<(i64,)> = page_ids_query
        .fetch_all(db)
        .await
        .map_err(|e| format!("query_library: error paginando: {e}"))?;
    let page_ids = page_ids.into_iter().map(|(id,)| id).collect::<Vec<_>>();

    // Filas de la página: una consulta con los badges agregados (GROUP_CONCAT),
    // sin N+1. Sin ids no se consulta nada. El WHERE es solo la lista de ids
    // (ya pasaron el filtro): sin binds que encajar. Sin ORDER BY en SQL: el
    // orden lo decidió la consulta de ids (con búsqueda, relevancia bm25 que
    // aquí no se puede reproducir sin contexto MATCH) y se restaura en Rust.
    let mut tracks: Vec<crate::commands::types::LibraryTrack> = Vec::new();
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
             GROUP BY t.id",
            page_rows_sql()
        );
        let mut rows = sqlx::query_as::<_, crate::commands::types::LibraryTrack>(&rows_sql)
            .fetch_all(db)
            .await
            .map_err(|e| format!("query_library: error leyendo filas: {e}"))?;

        // Misma verificación de ficheros que get_library: un download cuyo
        // fichero desapareció baja a not_downloaded.
        crate::commands::library::verify_page_files(&mut rows).await?;

        // Orden de la página = orden de los ids (relevancia o elegido).
        let mut by_id: std::collections::HashMap<i64, crate::commands::types::LibraryTrack> =
            rows.into_iter().map(|t| (t.id, t)).collect();
        for id in &page_ids {
            if let Some(track) = by_id.remove(id) {
                tracks.push(track);
            }
        }
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
/// de los issues #98 y #97. Entra por los índices de pertenencia: los de
/// L01 (`idx_library_track`, `idx_playlist_tracks_track`,
/// `idx_track_sources_*`) y los de L00 en `library_memberships`
/// (`idx_library_memberships_track_added`, `idx_library_memberships_account_track`,
/// 0112). Esta función es el punto de re-verificación del plan.
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

    // Por cuenta (L00, #97): pertenencias vivas de las TRES fuentes
    // (favoritos por cuenta, playlists con dueño y pertenencias completas
    // de library_memberships: álbumes, artistas, compras e importación
    // local), sin el propio filtro de cuentas. El UNION deduplica los pares
    // (cuenta, pista) — las pertenencias espejo de L00 no duplican — y el
    // COUNT es de PISTAS DISTINTAS por cuenta: puede superar el total global
    // (una pista en dos cuentas cuenta en las dos). El conjunto filtrado va
    // en un CTE para no duplicar sus placeholders.
    let (where_acc, binds_acc) = build_where(query, Dimension::Accounts)?;
    let by_account_sql = format!(
        "WITH matched AS (SELECT t.id FROM tracks t WHERE {where_acc}) \
         SELECT a.id AS account_id, a.display_name, sv.name AS service_name, COUNT(DISTINCT m.track_id) AS count \
         FROM ( \
           SELECT le.account_id AS account_id, le.track_id AS track_id \
           FROM library_entries le \
           WHERE le.track_id IN (SELECT id FROM matched) \
           UNION \
           SELECT pl.account_id AS account_id, pt.track_id AS track_id \
           FROM playlist_tracks pt \
           JOIN playlists pl ON pl.id = pt.playlist_id \
           WHERE pt.track_id IN (SELECT id FROM matched) \
           UNION \
           SELECT lm.account_id AS account_id, lm.track_id AS track_id \
           FROM library_memberships lm \
           WHERE lm.track_id IN (SELECT id FROM matched) \
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

// ==========================================================================
// Detalle de pista (L04, issue #101)
// ==========================================================================

/// Error tipado del panel de detalle. Una pista inexistente es
/// [`TrackDetailError::TrackNotFound`], no una fila vacía ni un error de
/// texto suelto: la UI puede discriminarse por `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TrackDetailError {
    #[serde(rename_all = "camelCase")]
    TrackNotFound { track_id: i64 },
    #[serde(rename_all = "camelCase")]
    Database { message: String },
}

impl From<String> for TrackDetailError {
    fn from(message: String) -> Self {
        TrackDetailError::Database { message }
    }
}

/// TrackSource de la pista, por servicio (story 17 del spec #96): formato,
/// profundidad de bits, frecuencia, bitrate y disponibilidad.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TrackDetailSource {
    pub service_id: i64,
    pub service_name: String,
    pub service_track_id: Option<String>,
    pub format: Option<String>,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<i32>,
    pub bitrate: Option<i32>,
    pub quality_score: Option<i32>,
    pub available: bool,
    pub availability_status: String,
    pub availability_reason: Option<String>,
    pub last_checked: Option<String>,
}

/// LibraryItem de la pista: pertenencia por cuenta y colección (story 18).
/// `favorite` es library_entries; `playlist` es playlist_tracks con el dueño
/// de la playlist atribuyendo la cuenta (la misma regla de [`MEMBERSHIP_SQL`]
/// y los contadores de cuenta de L01); `album`/`artist`/`purchase`/`library`
/// son `library_memberships` (0112, L00). La cuenta sale solo de las
/// pertenencias; no se infiere ni se rellena con la cuenta activa.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TrackDetailMembership {
    pub account_id: i64,
    pub display_name: Option<String>,
    pub service_name: String,
    /// "favorite" | "playlist" | "album" | "artist" | "purchase" | "library".
    pub collection: String,
    pub collection_id: Option<i64>,
    pub collection_name: Option<String>,
    pub added_at: Option<String>,
}

/// Estado de descarga, servicio efectivo, calidad y ruta (story 19). La
/// faceta existe siempre; sin fila `downloads` (o con el fichero perdido,
/// que `verify_page_files` baja) los campos de fichero son `None`. El
/// `status` usa la MISMA clasificación que la lista para que el panel y la
/// fila nunca diverjan.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDetailDownload {
    pub status: String,
    pub effective_service: Option<String>,
    pub file_path: Option<String>,
    pub file_format: Option<String>,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<i32>,
    pub downloaded_at: Option<String>,
}

/// Contrato del panel para la cuarentena por pertenencia (motivo y última
/// fecha vista). Sin tabla de cuarentena hasta #72, `quarantine` es `None`:
/// la UI muestra la sección deshabilitada con su motivo. L09 la completa.
#[allow(dead_code)] // Contrato IPC del panel; L09 (#72) es quien lo construye.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDetailQuarantine {
    /// Pertenencia en cuarentena: cuenta o colección que ya no la devuelve.
    pub collection: String,
    pub reason: String,
    pub last_seen_at: Option<String>,
}

/// Contrato del panel para el grupo de duplicados. La identidad ISRC∪huella
/// llega con L10 (#73): `duplicate_group` es `None` hoy y la UI muestra la
/// sección deshabilitada con su motivo. No se agrupa por título/artista.
#[allow(dead_code)] // Contrato IPC del panel; L10 (#73) es quien lo construye.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDetailDuplicateGroup {
    /// "isrc" | "fingerprint": la clave que formó el clúster.
    pub matched_on: String,
    pub member_track_ids: Vec<i64>,
}

/// Respuesta de UNA llamada (`get_library_track_detail`): el panel abre con
/// todo, sin consulta por campo, fuente o pertenencia.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDetail {
    /// Track canónico (título, artista, álbum, duración, ISRC, género, año,
    /// MusicBrainz): la MISMA proyección que la fila de la lista, más
    /// `musicbrainz_id`.
    pub track: crate::commands::types::LibraryTrack,
    pub sources: Vec<TrackDetailSource>,
    pub memberships: Vec<TrackDetailMembership>,
    pub download: TrackDetailDownload,
    /// `None` hasta #72/L09 (ver cabecera del módulo).
    pub quarantine: Option<TrackDetailQuarantine>,
    /// `None` hasta #73/L10 (ver cabecera del módulo).
    pub duplicate_group: Option<TrackDetailDuplicateGroup>,
}

/// Detalle de pista para el panel lateral (L04, issue #101). Recibe el pool
/// — la costura de prueba es esta función, no el comando de Tauri.
pub async fn get_track_detail(
    db: &SqlitePool,
    track_id: i64,
) -> Result<TrackDetail, TrackDetailError> {
    // Fila canónica: la MISMA proyección de la lista (los badges agregados en
    // la misma consulta) filtrada a un id. Una pista inexistente es error
    // tipado, no una fila vacía.
    let rows_sql = format!(
        "SELECT {} \
         FROM tracks t \
         LEFT JOIN albums al ON al.id = t.album_id \
         LEFT JOIN track_sources ts ON ts.track_id = t.id \
         LEFT JOIN services s ON s.id = ts.service_id \
         LEFT JOIN track_sources ts_spot ON ts_spot.track_id = t.id AND ts_spot.service_id = (SELECT id FROM services WHERE name = 'spotify') \
         LEFT JOIN downloads d ON d.track_id = t.id \
         LEFT JOIN lyrics l ON l.track_id = t.id \
         WHERE t.id = ? \
         GROUP BY t.id",
        page_rows_sql()
    );
    let mut rows = sqlx::query_as::<_, crate::commands::types::LibraryTrack>(&rows_sql)
        .bind(track_id)
        .fetch_all(db)
        .await
        .map_err(|e| TrackDetailError::Database {
            message: format!("get_track_detail: error leyendo la pista: {e}"),
        })?;
    if rows.len() != 1 {
        // UNIQUE de downloads y FTS garantizan que nunca sea >1; un resultado
        // vacío es pista inexistente.
        return Err(TrackDetailError::TrackNotFound { track_id });
    }
    let mut row = rows.remove(0);

    // Misma verificación de ficheros que la lista: un download cuyo fichero
    // desapareció baja a not_downloaded AQUÍ TAMBIÉN.
    crate::commands::library::verify_page_files(std::slice::from_mut(&mut row)).await?;

    // Todas las fuentes en UNA consulta (no una por servicio).
    let sources_sql = "SELECT ts.service_id, s.name AS service_name, ts.service_track_id, \
         ts.format, ts.bit_depth, ts.sample_rate, ts.bitrate, ts.quality_score, \
         COALESCE(ts.available, 0) != 0 AS available, ts.availability_status, ts.availability_reason, ts.last_checked \
         FROM track_sources ts \
         JOIN services s ON s.id = ts.service_id \
         WHERE ts.track_id = ? \
         ORDER BY ts.quality_score DESC, s.name COLLATE NOCASE";
    let sources = sqlx::query_as::<_, TrackDetailSource>(sources_sql)
        .bind(track_id)
        .fetch_all(db)
        .await
        .map_err(|e| TrackDetailError::Database {
            message: format!("get_track_detail: error leyendo fuentes: {e}"),
        })?;

    // Todas las pertenencias en UNA consulta (no una por cuenta ni por
    // colección): favoritos por cuenta, playlists con dueño y la
    // pertenencia completa de library_memberships (álbumes, artistas,
    // compras e importación local — L00).
    let memberships_sql = "SELECT le.account_id, a.display_name, sv.name AS service_name, \
         'favorite' AS collection, NULL AS collection_id, NULL AS collection_name, le.added_at \
         FROM library_entries le \
         JOIN accounts a ON a.id = le.account_id \
         JOIN services sv ON sv.id = a.service_id \
         WHERE le.track_id = ?1 \
         UNION ALL \
         SELECT pl.account_id, a2.display_name, sv2.name, \
         'playlist', pl.id, pl.name, pt.added_at \
         FROM playlist_tracks pt \
         JOIN playlists pl ON pl.id = pt.playlist_id \
         JOIN accounts a2 ON a2.id = pl.account_id \
         JOIN services sv2 ON sv2.id = a2.service_id \
         WHERE pt.track_id = ?1 \
         UNION ALL \
         SELECT lm.account_id, a3.display_name, sv3.name, \
         lm.collection_type, lm.collection_id, \
         CASE lm.collection_type \
             WHEN 'album' THEN (SELECT al2.title FROM albums al2 WHERE al2.id = lm.collection_id) \
             WHEN 'playlist' THEN (SELECT pl2.name FROM playlists pl2 WHERE pl2.id = lm.collection_id) \
             WHEN 'artist' THEN (SELECT ar2.name FROM artists ar2 WHERE ar2.id = lm.collection_id) \
             ELSE NULL \
         END, \
         lm.added_at \
         FROM library_memberships lm \
         JOIN accounts a3 ON a3.id = lm.account_id \
         JOIN services sv3 ON sv3.id = a3.service_id \
         WHERE lm.track_id = ?1 \
         ORDER BY service_name COLLATE NOCASE, collection, collection_name, account_id";
    let memberships = sqlx::query_as::<_, TrackDetailMembership>(memberships_sql)
        .bind(track_id)
        .fetch_all(db)
        .await
        .map_err(|e| TrackDetailError::Database {
            message: format!("get_track_detail: error leyendo pertenencias: {e}"),
        })?;

    // Descarga: servicio efectivo, calidad y ruta de la fila `downloads`.
    // `status` viene de la fila canónica ya verificada contra el disco.
    let download_sql = "SELECT \
         COALESCE(d.effective_service, (SELECT s2.name FROM services s2 WHERE s2.id = d.source_service_id)) AS effective_service, \
         d.file_path, d.file_format, d.bit_depth, d.sample_rate, d.downloaded_at \
         FROM downloads d WHERE d.track_id = ? LIMIT 1";
    let download_row = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i32>,
            Option<i32>,
            Option<String>,
        ),
    >(download_sql)
    .bind(track_id)
    .fetch_optional(db)
    .await
    .map_err(|e| TrackDetailError::Database {
        message: format!("get_track_detail: error leyendo descarga: {e}"),
    })?;
    // La faceta existe SIEMPRE: el estado viene de la fila canónica (en cola
    // sin fila `downloads` incluido); los datos del fichero solo cuando hay
    // fila. Si verify_page_files bajó el estado, el panel lo refleja aunque
    // la fila `downloads` siga existiendo.
    let download = TrackDetailDownload {
        status: row
            .download_status
            .clone()
            .unwrap_or_else(|| "not_downloaded".to_owned()),
        effective_service: download_row.as_ref().and_then(|d| d.0.clone()),
        file_path: download_row.as_ref().and_then(|d| d.1.clone()),
        file_format: download_row.as_ref().and_then(|d| d.2.clone()),
        bit_depth: download_row.as_ref().and_then(|d| d.3),
        sample_rate: download_row.as_ref().and_then(|d| d.4),
        downloaded_at: download_row.as_ref().and_then(|d| d.5.clone()),
    };

    Ok(TrackDetail {
        track: row,
        sources,
        memberships,
        download,
        // Sin #72 no hay tabla de cuarentena; sin #73 no hay identidad
        // ISRC∪huella. `None` con motivo documentado: no se simulan.
        quarantine: None,
        duplicate_group: None,
    })
}

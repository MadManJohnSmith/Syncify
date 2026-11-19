//! Acciones por lotes de la biblioteca — L05 (issue #102, spec #96).
//!
//! La selección tiene DOS modos explícitos y no se confunden entre sí:
//!
//! - [`LibrarySelection::Ids`]: los ids que el usuario marcó, tal cual;
//! - [`LibrarySelection::Filter`]: «todo el filtro» + exclusiones. El servidor
//!   resuelve el filtro completo (no la página cargada) y descarta las
//!   exclusiones antes de contar.
//!
//! [`preview_library_action_inner`] devuelve el recuento elegible y los motivos
//! de omisión agregados por acción. [`run_library_action_inner`] vuelve a
//! resolver la selección y vuelve a evaluar la elegibilidad: si el sync cambió
//! la biblioteca entre la vista previa y la ejecución, manda el estado nuevo y
//! `recalculated` lo declara. Nunca se actúa sobre los números que trae la UI.
//!
//! La ejecución va en bloques de [`LIBRARY_ACTION_BLOCK_SIZE`] con UNA
//! transacción por bloque. Un bloque fallido se revierte entero y no impide los
//! siguientes; su recuento va a `failed` y el resumen conserva el error del
//! bloque. Ni la clasificación ni la escritura son una consulta por pista:
//! cada bloque resuelve sus ids con SQL agregado y una sola escritura por
//! acción (la cola de descargas, el flag de favorito y la playlist son las
//! superficies existentes; no hay una segunda cola).
//!
//! Acciones destructivas (borrar audio, restaurar, tocar cuarentena) quedan
//! representadas solo para rechazarlas con un error explícito: son por pista y
//! con confirmación, nunca por lote ni en «seleccionar todo».
//!
//! Los eventos [`LIBRARY_ACTION_PROGRESS_EVENT`] y
//! [`LIBRARY_ACTION_FINISHED_EVENT`] llevan el agregado por bloque / final, no
//! un evento por pista. El emisor es un trait para que las pruebas de
//! integración recojan los eventos sin Tauri.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sqlx::{SqliteConnection, SqlitePool};
use tauri::{Emitter, Runtime, State};

use crate::commands::library_query::{filter_where, LibraryQuery};
use crate::commands::queue::normalize_quality_preference;
use crate::services::library_membership::LIBRARY_ADDED_AT_SET_SQL;
use crate::AppState;

/// Tamaño de bloque del runner (criterio del issue #102). Publicado para que
/// las pruebas de bloque/eventos no copien el número.
pub const LIBRARY_ACTION_BLOCK_SIZE: usize = 500;

/// Evento agregado de avance (uno por bloque, no uno por pista).
pub const LIBRARY_ACTION_PROGRESS_EVENT: &str = "library-action-progress";
/// Evento final con el resumen agregado.
pub const LIBRARY_ACTION_FINISHED_EVENT: &str = "library-action-finished";

// ==========================================================================
// Contrato
// ==========================================================================

/// Selección por ids explícitos o por «todo el filtro» con exclusiones.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "mode",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum LibrarySelection {
    /// Ids explícitos: la selección es exactamente esta lista (deduplicada).
    Ids { ids: Vec<i64> },
    /// Todo el filtro del servidor, menos las exclusiones. `offset`/`limit`
    /// del filtro se ignoran: la selección no es la página cargada.
    Filter {
        query: LibraryQuery,
        #[serde(default)]
        excluded_ids: Vec<i64>,
    },
}

impl LibrarySelection {
    fn mode(&self) -> &'static str {
        match self {
            LibrarySelection::Ids { .. } => "ids",
            LibrarySelection::Filter { .. } => "filter",
        }
    }

    fn excluded_ids(&self) -> &[i64] {
        match self {
            LibrarySelection::Ids { .. } => &[],
            LibrarySelection::Filter { excluded_ids, .. } => excluded_ids,
        }
    }
}

/// Acción por lotes. Las destructivas existen en el contrato solo para
/// devolver un error claro: son operaciones por pista con confirmación.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum LibraryBatchAction {
    /// Encola en la cola de descargas existente. Idempotente: no duplica una
    /// entrada ya encolada/descargándose ni una pista ya descargada.
    QueueDownload {
        #[serde(default)]
        priority: Option<i64>,
        #[serde(default)]
        quality_preference: Option<String>,
        #[serde(default)]
        allow_fallback: Option<bool>,
    },
    /// Marca o desmarca favorito. Con `account_id`, escribe además la
    /// pertenencia por cuenta de forma idempotente (índices únicos de
    /// `library_entries`/`library_memberships`).
    SetFavorite {
        favorite: bool,
        #[serde(default)]
        account_id: Option<i64>,
    },
    /// Añade a una playlist local/sincronizada. Ignora lo que ya está en la
    /// playlist y no duplica pertenencias de playlists con dueño.
    AddToPlaylist { playlist_id: i64 },
    /// Solo por pista (`delete_track_audio`), nunca por lote.
    DeleteAudio,
    /// Solo por pista (restauración de cuarentena, #72/L09).
    Restore,
    /// Solo por pista (tocar cuarentena, #72/L09).
    QuarantineTouch,
}

impl LibraryBatchAction {
    fn id(&self) -> &'static str {
        match self {
            LibraryBatchAction::QueueDownload { .. } => "queue_download",
            LibraryBatchAction::SetFavorite { .. } => "set_favorite",
            LibraryBatchAction::AddToPlaylist { .. } => "add_to_playlist",
            LibraryBatchAction::DeleteAudio => "delete_audio",
            LibraryBatchAction::Restore => "restore",
            LibraryBatchAction::QuarantineTouch => "quarantine_touch",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            LibraryBatchAction::QueueDownload { .. } => "Add to download queue",
            LibraryBatchAction::SetFavorite { favorite: true, .. } => "Mark as favorite",
            LibraryBatchAction::SetFavorite {
                favorite: false, ..
            } => "Remove from favorites",
            LibraryBatchAction::AddToPlaylist { .. } => "Add to playlist",
            LibraryBatchAction::DeleteAudio => "Delete audio",
            LibraryBatchAction::Restore => "Restore",
            LibraryBatchAction::QuarantineTouch => "Touch quarantine",
        }
    }

    fn per_track_only(&self) -> bool {
        matches!(
            self,
            LibraryBatchAction::DeleteAudio
                | LibraryBatchAction::Restore
                | LibraryBatchAction::QuarantineTouch
        )
    }

    fn playlist_id(&self) -> Option<i64> {
        match self {
            LibraryBatchAction::AddToPlaylist { playlist_id } => Some(*playlist_id),
            _ => None,
        }
    }
}

/// Motivo de omisión agregado (recuento, no una fila por pista).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryActionOmission {
    /// Clave estable para la UI y las pruebas.
    pub reason: String,
    /// Texto humano del motivo.
    pub message: String,
    pub count: i64,
}

/// Vista previa de una acción: recuento elegible y omisiones agregadas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryActionPreview {
    pub action: String,
    pub label: String,
    pub selection_mode: String,
    /// Tracks de la selección tras aplicar exclusiones.
    pub total: i64,
    pub eligible: i64,
    pub omitted: i64,
    pub omissions: Vec<LibraryActionOmission>,
}

/// Error de un bloque con su recuento de pistas afectadas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryActionBlockError {
    pub block: i64,
    pub message: String,
    pub tracks: i64,
}

/// Resumen agregado de una ejecución.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryActionSummary {
    pub action: String,
    pub label: String,
    pub selection_mode: String,
    /// Tracks resueltos por el servidor (tras exclusiones) al ejecutar.
    pub total: i64,
    /// Tracks evaluados. Siempre `succeeded + failed + skipped`.
    pub processed: i64,
    /// Tracks que eran elegibles al ejecutar (recalculado, no el de la UI).
    pub eligible: i64,
    pub succeeded: i64,
    pub failed: i64,
    pub skipped: i64,
    pub omissions: Vec<LibraryActionOmission>,
    pub blocks: i64,
    pub blocks_failed: i64,
    pub errors: Vec<LibraryActionBlockError>,
    /// `true` si los totales esperados por la UI no coinciden con los
    /// resueltos al ejecutar (el sync cambió la biblioteca).
    pub recalculated: bool,
}

/// Avance agregado por bloque.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryActionProgress {
    pub action: String,
    pub total: i64,
    pub processed: i64,
    pub succeeded: i64,
    pub failed: i64,
    pub skipped: i64,
    pub blocks_done: i64,
    pub blocks_total: i64,
}

/// Emisor de eventos del runner (AppHandle en producción, callback en tests).
pub trait LibraryActionEmitter: Send + Sync {
    fn emit_progress(&self, event: &LibraryActionProgress);
    fn emit_finished(&self, event: &LibraryActionSummary);
}

impl<R: Runtime> LibraryActionEmitter for tauri::AppHandle<R> {
    fn emit_progress(&self, event: &LibraryActionProgress) {
        if let Err(error) = self.emit(LIBRARY_ACTION_PROGRESS_EVENT, event) {
            tracing::warn!("library-action-progress emit failed: {error}");
        }
    }

    fn emit_finished(&self, event: &LibraryActionSummary) {
        if let Err(error) = self.emit(LIBRARY_ACTION_FINISHED_EVENT, event) {
            tracing::warn!("library-action-finished emit failed: {error}");
        }
    }
}

/// Emisor de pruebas: dos closures, sin runtime de Tauri.
#[allow(dead_code)] // Cubierta por `tests/library_actions_test.rs` (el binario no la construye).
pub struct LibraryActionCallback<F, G>(pub F, pub G);

impl<F, G> LibraryActionEmitter for LibraryActionCallback<F, G>
where
    F: Fn(&LibraryActionProgress) + Send + Sync,
    G: Fn(&LibraryActionSummary) + Send + Sync,
{
    fn emit_progress(&self, event: &LibraryActionProgress) {
        (self.0)(event);
    }

    fn emit_finished(&self, event: &LibraryActionSummary) {
        (self.1)(event);
    }
}

// ==========================================================================
// Comandos IPC
// ==========================================================================

/// Vista previa de una acción por lotes (recuento elegible + omisiones).
#[tauri::command]
pub async fn preview_library_action(
    state: State<'_, AppState>,
    selection: LibrarySelection,
    action: LibraryBatchAction,
) -> Result<LibraryActionPreview, String> {
    preview_library_action_inner(&state.db, &selection, &action).await
}

/// Ejecuta una acción por lotes. `expected_total`/`expected_eligible` son los
/// números de la vista previa; solo se usan para declarar `recalculated`.
#[tauri::command]
pub async fn run_library_action(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    selection: LibrarySelection,
    action: LibraryBatchAction,
    expected_total: Option<i64>,
    expected_eligible: Option<i64>,
) -> Result<LibraryActionSummary, String> {
    run_library_action_inner(
        &state.db,
        selection,
        action,
        expected_total,
        expected_eligible,
        &app,
    )
    .await
}

// ==========================================================================
// Vista previa
// ==========================================================================

/// Núcleo testeable de la vista previa (pool, no Tauri).
pub async fn preview_library_action_inner(
    db: &SqlitePool,
    selection: &LibrarySelection,
    action: &LibraryBatchAction,
) -> Result<LibraryActionPreview, String> {
    validate_action(action)?;
    if let Some(playlist_id) = action.playlist_id() {
        ensure_playlist(db, playlist_id).await?;
    }

    let mut tally = PreviewTally::default();
    let mut conn = db
        .acquire()
        .await
        .map_err(|e| format!("preview_library_action: no se pudo abrir conexión: {e}"))?;

    match selection {
        LibrarySelection::Ids { ids } => {
            let ids = normalize_ids(ids, selection.excluded_ids());
            tally.total = ids.len() as i64;
            for chunk in ids.chunks(LIBRARY_ACTION_BLOCK_SIZE) {
                let block = classify_block(&mut conn, action, chunk).await?;
                tally.absorb(block);
            }
        }
        LibrarySelection::Filter {
            query,
            excluded_ids,
        } => {
            let (where_sql, binds) = filter_where(query)?;
            tally.total = count_matching(&mut conn, &where_sql, &binds, excluded_ids).await?;
            let mut after_id = 0i64;
            loop {
                let page = filter_ids_page(
                    &mut conn,
                    &where_sql,
                    &binds,
                    excluded_ids,
                    after_id,
                    i64::MAX,
                    LIBRARY_ACTION_BLOCK_SIZE,
                )
                .await?;
                if page.is_empty() {
                    break;
                }
                after_id = *page.last().expect("page no vacía");
                let block = classify_block(&mut conn, action, &page).await?;
                tally.absorb(block);
            }
        }
    }

    Ok(LibraryActionPreview {
        action: action.id().to_string(),
        label: action.label().to_string(),
        selection_mode: selection.mode().to_string(),
        total: tally.total,
        eligible: tally.eligible,
        omitted: tally.omissions.values().sum(),
        omissions: omissions_to_vec(&tally.omissions),
    })
}

#[derive(Default)]
struct PreviewTally {
    total: i64,
    eligible: i64,
    omissions: OmissionCounts,
}

impl PreviewTally {
    fn absorb(&mut self, block: BlockClassification) {
        self.eligible += block.eligible.len() as i64;
        merge_omissions(&mut self.omissions, block.omissions);
    }
}

// ==========================================================================
// Ejecución
// ==========================================================================

/// Núcleo testeable de la ejecución. El emisor permite recoger los eventos
/// agregados en pruebas de integración sin levantar Tauri.
pub async fn run_library_action_inner<E: LibraryActionEmitter + ?Sized>(
    db: &SqlitePool,
    selection: LibrarySelection,
    action: LibraryBatchAction,
    expected_total: Option<i64>,
    expected_eligible: Option<i64>,
    emitter: &E,
) -> Result<LibraryActionSummary, String> {
    validate_action(&action)?;
    if let Some(playlist_id) = action.playlist_id() {
        ensure_playlist(db, playlist_id).await?;
    }

    let action_id = action.id().to_string();
    let action_label = action.label().to_string();
    let selection_mode = selection.mode().to_string();
    let excluded = selection.excluded_ids().to_vec();

    // Materializar la selección en bloques. Para el filtro se fija el máximo
    // id al arrancar: el sync puede añadir pistas con ids mayores a mitad de
    // la ejecución y el resumen no debe crecer bajo los pies del usuario.
    let explicit_ids = match &selection {
        LibrarySelection::Ids { ids } => Some(normalize_ids(ids, &excluded)),
        LibrarySelection::Filter { .. } => None,
    };

    let (total, max_id, where_sql, binds) = match &selection {
        LibrarySelection::Ids { .. } => (
            explicit_ids.as_ref().map_or(0, |ids| ids.len()) as i64,
            i64::MAX,
            String::new(),
            Vec::new(),
        ),
        LibrarySelection::Filter { query, .. } => {
            let (where_sql, binds) = filter_where(query)?;
            let mut conn = db
                .acquire()
                .await
                .map_err(|e| format!("run_library_action: no se pudo abrir conexión: {e}"))?;
            let max_id = max_matching(&mut conn, &where_sql, &binds, &excluded).await?;
            let total = if max_id == 0 {
                0
            } else {
                count_matching_upto(&mut conn, &where_sql, &binds, &excluded, max_id).await?
            };
            (total, max_id, where_sql, binds)
        }
    };

    let blocks_total = blocks_for(total);
    let mut summary = RunTally {
        total,
        ..RunTally::default()
    };
    let mut after_id = 0i64;
    let mut block_index = 0i64;

    loop {
        let ids: Vec<i64> = if let Some(ids) = &explicit_ids {
            let start = block_index as usize * LIBRARY_ACTION_BLOCK_SIZE;
            if start >= ids.len() {
                break;
            }
            ids[start..(start + LIBRARY_ACTION_BLOCK_SIZE).min(ids.len())].to_vec()
        } else {
            let mut conn = db
                .acquire()
                .await
                .map_err(|e| format!("run_library_action: no se pudo abrir conexión: {e}"))?;
            filter_ids_page(
                &mut conn,
                &where_sql,
                &binds,
                &excluded,
                after_id,
                max_id,
                LIBRARY_ACTION_BLOCK_SIZE,
            )
            .await?
        };

        if ids.is_empty() {
            break;
        }
        if explicit_ids.is_none() {
            after_id = *ids.last().expect("bloque no vacío");
        }

        let outcome = execute_block(db, &action, &ids, block_index).await;
        summary.absorb(&outcome);
        block_index += 1;

        let progress = LibraryActionProgress {
            action: action_id.clone(),
            total: summary.total,
            processed: summary.processed(),
            succeeded: summary.succeeded,
            failed: summary.failed,
            skipped: summary.skipped,
            blocks_done: block_index,
            blocks_total,
        };
        emitter.emit_progress(&progress);
    }

    // Una pista que desapareció entre la resolución y su bloque (solo puede
    // pasar en modo filtro con escrituras concurrentes) se contabiliza como
    // omitida, nunca se pierde del resumen.
    if summary.processed() < summary.total {
        let missing = summary.total - summary.processed();
        summary.skipped += missing;
        *summary
            .omissions
            .entry("no_longer_in_library".to_string())
            .or_insert(0) += missing;
    }

    let eligible = summary.eligible;
    let recalculated = expected_total.is_some_and(|expected| expected != summary.total)
        || expected_eligible.is_some_and(|expected| expected != eligible);

    let finished = LibraryActionSummary {
        action: action_id,
        label: action_label,
        selection_mode,
        total: summary.total,
        processed: summary.processed(),
        eligible,
        succeeded: summary.succeeded,
        failed: summary.failed,
        skipped: summary.skipped,
        omissions: omissions_to_vec(&summary.omissions),
        blocks: block_index,
        blocks_failed: summary.blocks_failed,
        errors: summary.errors,
        recalculated,
    };
    emitter.emit_finished(&finished);
    Ok(finished)
}

#[derive(Default)]
struct RunTally {
    total: i64,
    eligible: i64,
    succeeded: i64,
    failed: i64,
    skipped: i64,
    blocks_failed: i64,
    omissions: OmissionCounts,
    errors: Vec<LibraryActionBlockError>,
}

impl RunTally {
    fn absorb(&mut self, outcome: &BlockOutcome) {
        self.eligible += outcome.eligible;
        self.succeeded += outcome.succeeded;
        self.failed += outcome.failed;
        self.skipped += outcome.skipped;
        merge_omissions(&mut self.omissions, outcome.omissions.clone());
        if let Some(error) = &outcome.error {
            self.blocks_failed += 1;
            self.errors.push(error.clone());
        }
    }

    fn processed(&self) -> i64 {
        self.succeeded + self.failed + self.skipped
    }
}

struct BlockOutcome {
    eligible: i64,
    succeeded: i64,
    failed: i64,
    skipped: i64,
    omissions: OmissionCounts,
    error: Option<LibraryActionBlockError>,
}

/// Un bloque = una transacción. Un fallo dentro del bloque se revierte entero
/// y se reporta; los bloques siguientes se ejecutan igual.
async fn execute_block(
    db: &SqlitePool,
    action: &LibraryBatchAction,
    ids: &[i64],
    block_index: i64,
) -> BlockOutcome {
    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(error) => {
            return BlockOutcome {
                eligible: 0,
                succeeded: 0,
                failed: ids.len() as i64,
                skipped: 0,
                omissions: OmissionCounts::new(),
                error: Some(LibraryActionBlockError {
                    block: block_index,
                    message: format!("no se pudo abrir la transacción del bloque: {error}"),
                    tracks: ids.len() as i64,
                }),
            }
        }
    };

    let classification = match classify_block(&mut tx, action, ids).await {
        Ok(classification) => classification,
        Err(error) => {
            rollback_or_log(tx, "clasificación").await;
            return BlockOutcome {
                eligible: 0,
                succeeded: 0,
                failed: ids.len() as i64,
                skipped: 0,
                omissions: OmissionCounts::new(),
                error: Some(LibraryActionBlockError {
                    block: block_index,
                    message: error,
                    tracks: ids.len() as i64,
                }),
            };
        }
    };

    let eligible_ids = classification.eligible.clone();
    let eligible = eligible_ids.len() as i64;
    let skipped = classification.omissions.values().sum();

    let applied = match apply_action(&mut tx, action, &eligible_ids).await {
        Ok(applied) => applied,
        Err(error) => {
            rollback_or_log(tx, "acción").await;
            return BlockOutcome {
                eligible,
                succeeded: 0,
                failed: eligible,
                skipped,
                omissions: classification.omissions,
                error: Some(LibraryActionBlockError {
                    block: block_index,
                    message: error,
                    tracks: eligible,
                }),
            };
        }
    };

    if let Err(error) = tx.commit().await {
        return BlockOutcome {
            eligible,
            succeeded: 0,
            failed: eligible,
            skipped,
            omissions: classification.omissions,
            error: Some(LibraryActionBlockError {
                block: block_index,
                message: format!("no se pudo confirmar la transacción del bloque: {error}"),
                tracks: eligible,
            }),
        };
    }

    let mut omissions = classification.omissions;
    let mut applied = applied;
    if applied > eligible {
        applied = eligible;
    }
    if applied < eligible {
        let changed = eligible - applied;
        *omissions.entry("state_changed".to_string()).or_insert(0) += changed;
        return BlockOutcome {
            eligible,
            succeeded: applied,
            failed: 0,
            skipped: skipped + changed,
            omissions,
            error: None,
        };
    }

    BlockOutcome {
        eligible,
        succeeded: applied,
        failed: 0,
        skipped,
        omissions,
        error: None,
    }
}

async fn rollback_or_log(tx: sqlx::Transaction<'_, sqlx::Sqlite>, context: &str) {
    if let Err(error) = tx.rollback().await {
        tracing::warn!("run_library_action: rollback del bloque ({context}) falló: {error}");
    }
}

// ==========================================================================
// Clasificación por bloque (una consulta agregada por bloque y acción)
// ==========================================================================

struct BlockClassification {
    eligible: Vec<i64>,
    omissions: OmissionCounts,
}

async fn classify_block(
    conn: &mut SqliteConnection,
    action: &LibraryBatchAction,
    ids: &[i64],
) -> Result<BlockClassification, String> {
    if ids.is_empty() {
        return Ok(BlockClassification {
            eligible: Vec::new(),
            omissions: OmissionCounts::new(),
        });
    }

    let sql = classification_sql(action, ids);
    let mut query = sqlx::query_as::<_, (i64, Option<String>)>(&sql);
    if let Some(playlist_id) = action.playlist_id() {
        query = query.bind(playlist_id);
    }
    let rows = query
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| format!("clasificación del bloque: {e}"))?;

    let mut eligible = Vec::with_capacity(rows.len());
    let mut omissions = OmissionCounts::new();
    for (track_id, omission) in rows {
        match omission {
            Some(reason) => {
                *omissions.entry(reason).or_insert(0) += 1;
            }
            None => eligible.push(track_id),
        }
    }
    let missing = ids.len() as i64 - eligible.len() as i64 - omissions.values().sum::<i64>();
    if missing > 0 {
        *omissions.entry("not_in_library".to_string()).or_insert(0) += missing;
    }

    Ok(BlockClassification {
        eligible,
        omissions,
    })
}

/// SQL de clasificación. Las listas de ids son `i64` interpolados (no
/// portadores de SQL); la playlist, cuando aplica, va como bind.
fn classification_sql(action: &LibraryBatchAction, ids: &[i64]) -> String {
    let ids = inline_ids(ids);
    match action {
        LibraryBatchAction::QueueDownload { .. } => format!(
            "SELECT t.id, CASE \
               WHEN EXISTS (SELECT 1 FROM downloads d WHERE d.track_id = t.id AND d.file_path IS NOT NULL) THEN 'already_downloaded' \
               WHEN EXISTS (SELECT 1 FROM download_queue dq WHERE dq.track_id = t.id AND dq.status IN ('queued', 'downloading')) THEN 'already_queued' \
               WHEN NOT EXISTS ( \
                   SELECT 1 FROM track_sources ts JOIN services s ON s.id = ts.service_id \
                   WHERE ts.track_id = t.id \
                     AND COALESCE(s.supports_download, 0) = 1 \
                     AND COALESCE(ts.available, 1) = 1 \
                     AND ts.service_track_id IS NOT NULL AND TRIM(ts.service_track_id) != '' \
                     AND EXISTS (SELECT 1 FROM accounts acc \
                         WHERE acc.service_id = s.id AND acc.is_active = 1)) \
                 THEN 'no_downloadable_source' \
               ELSE NULL END \
             FROM tracks t WHERE t.id IN ({ids})"
        ),
        LibraryBatchAction::SetFavorite { favorite: true, .. } => format!(
            "SELECT t.id, CASE WHEN COALESCE(t.is_favorite, 0) = 1 THEN 'already_favorite' ELSE NULL END \
             FROM tracks t WHERE t.id IN ({ids})"
        ),
        LibraryBatchAction::SetFavorite { favorite: false, .. } => format!(
            "SELECT t.id, CASE WHEN COALESCE(t.is_favorite, 0) = 0 THEN 'not_favorite' ELSE NULL END \
             FROM tracks t WHERE t.id IN ({ids})"
        ),
        LibraryBatchAction::AddToPlaylist { .. } => format!(
            "SELECT t.id, CASE WHEN EXISTS ( \
                 SELECT 1 FROM playlist_tracks pt WHERE pt.playlist_id = ? AND pt.track_id = t.id \
             ) THEN 'already_in_playlist' ELSE NULL END \
             FROM tracks t WHERE t.id IN ({ids})"
        ),
        // `validate_action` los rechaza antes de llegar aquí.
        LibraryBatchAction::DeleteAudio
        | LibraryBatchAction::Restore
        | LibraryBatchAction::QuarantineTouch => String::new(),
    }
}

// ==========================================================================
// Escritura por bloque (una sola sentencia por acción)
// ==========================================================================

async fn apply_action(
    conn: &mut SqliteConnection,
    action: &LibraryBatchAction,
    ids: &[i64],
) -> Result<i64, String> {
    if ids.is_empty() {
        return Ok(0);
    }
    match action {
        LibraryBatchAction::QueueDownload {
            priority,
            quality_preference,
            allow_fallback,
        } => {
            apply_queue_download(
                conn,
                ids,
                *priority,
                quality_preference.as_deref(),
                *allow_fallback,
            )
            .await
        }
        LibraryBatchAction::SetFavorite {
            favorite,
            account_id,
        } => apply_favorite(conn, ids, *favorite, *account_id).await,
        LibraryBatchAction::AddToPlaylist { playlist_id } => {
            apply_add_to_playlist(conn, ids, *playlist_id).await
        }
        LibraryBatchAction::DeleteAudio
        | LibraryBatchAction::Restore
        | LibraryBatchAction::QuarantineTouch => {
            Err("acción destructiva: solo por pista y con confirmación".to_string())
        }
    }
}

async fn apply_queue_download(
    conn: &mut SqliteConnection,
    ids: &[i64],
    priority: Option<i64>,
    quality_preference: Option<&str>,
    allow_fallback: Option<bool>,
) -> Result<i64, String> {
    let priority = priority.unwrap_or(50).clamp(0, 100);
    let quality = normalize_quality_preference(quality_preference);
    let fallback = i64::from(allow_fallback.unwrap_or(true));
    let base: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), 0) FROM download_queue WHERE status = 'queued'",
    )
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| format!("posición de la cola: {e}"))?;

    // Una sola sentencia: la pista, la mejor fuente descargable y la posición
    // se resuelven en SQL, no con una consulta por pista. Las entradas de la
    // cola pasan por la misma deduplicación de siempre: la clasificación ya
    // descartó descargadas y activas, y esta transacción no las reintroduce.
    let values = value_pairs(ids.len());
    let sql = format!(
        "WITH candidates(track_id, ord) AS (VALUES {values}) \
         INSERT INTO download_queue ( \
             track_id, priority, quality_preference, status, progress_percent, retry_count, \
             position, resumable, service_id, service_name, service_track_id, \
             target_title, target_artist, target_album, target_isrc, allow_fallback, created_at \
         ) \
         SELECT t.id, ?, ?, 'queued', 0.0, 0, ? + c.ord, 1, \
                ts.service_id, src.name, ts.service_track_id, \
                t.title, \
                (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta \
                 JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id), \
                (SELECT al.title FROM albums al WHERE al.id = t.album_id), \
                t.isrc, ?, CURRENT_TIMESTAMP \
         FROM candidates c \
         JOIN tracks t ON t.id = c.track_id \
         LEFT JOIN track_sources ts ON ts.id = ( \
             SELECT ts2.id FROM track_sources ts2 JOIN services s2 ON s2.id = ts2.service_id \
             WHERE ts2.track_id = t.id \
               AND COALESCE(s2.supports_download, 0) = 1 \
               AND COALESCE(ts2.available, 1) = 1 \
               AND ts2.service_track_id IS NOT NULL AND TRIM(ts2.service_track_id) != '' \
               AND EXISTS (SELECT 1 FROM accounts acc \
                   WHERE acc.service_id = s2.id AND acc.is_active = 1) \
             ORDER BY ts2.available DESC, ts2.quality_score DESC, ts2.id ASC \
             LIMIT 1) \
         LEFT JOIN services src ON src.id = ts.service_id"
    );
    let mut query = sqlx::query(&sql);
    for (index, track_id) in ids.iter().enumerate() {
        query = query.bind(track_id).bind(index as i64 + 1);
    }
    query
        .bind(priority)
        .bind(quality)
        .bind(base)
        .bind(fallback)
        .execute(&mut *conn)
        .await
        .map(|result| result.rows_affected() as i64)
        .map_err(|e| format!("encolando descargas: {e}"))
}

async fn apply_favorite(
    conn: &mut SqliteConnection,
    ids: &[i64],
    favorite: bool,
    account_id: Option<i64>,
) -> Result<i64, String> {
    let ids_sql = inline_ids(ids);
    let value = i64::from(favorite);
    let favorite_at = if favorite {
        "COALESCE(favorite_at, datetime('now'))"
    } else {
        "NULL"
    };
    let updated = sqlx::query(&format!(
        "UPDATE tracks SET is_favorite = {value}, favorite_at = {favorite_at} \
         WHERE id IN ({ids_sql})"
    ))
    .execute(&mut *conn)
    .await
    .map_err(|e| format!("actualizando favoritos: {e}"))?
    .rows_affected() as i64;

    if let Some(account_id) = account_id {
        if favorite {
            sqlx::query(&format!(
                "INSERT INTO library_entries (account_id, track_id, added_at, is_liked) \
                 SELECT ?, t.id, CURRENT_TIMESTAMP, 1 FROM tracks t WHERE t.id IN ({ids_sql}) \
                 ON CONFLICT(account_id, track_id) DO UPDATE SET is_liked = 1"
            ))
            .bind(account_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("pertenencia de favorito: {e}"))?;

            // Índice único de 0112: repetir el favorito no duplica la fila.
            sqlx::query(&format!(
                "INSERT OR IGNORE INTO library_memberships \
                 (account_id, track_id, collection_type, collection_id, provider_item, added_at) \
                 SELECT ?, t.id, 'favorite', NULL, NULL, CURRENT_TIMESTAMP \
                 FROM tracks t WHERE t.id IN ({ids_sql})"
            ))
            .bind(account_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("pertenencia de favorito (memberships): {e}"))?;
        } else {
            sqlx::query(&format!(
                "UPDATE library_entries SET is_liked = 0 \
                 WHERE account_id = ? AND track_id IN ({ids_sql})"
            ))
            .bind(account_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("retirando favorito: {e}"))?;

            sqlx::query(&format!(
                "DELETE FROM library_memberships \
                 WHERE account_id = ? AND collection_type = 'favorite' \
                   AND track_id IN ({ids_sql})"
            ))
            .bind(account_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("retirando pertenencia de favorito: {e}"))?;
        }

        sqlx::query(&format!(
            "UPDATE tracks SET {LIBRARY_ADDED_AT_SET_SQL} WHERE id IN ({ids_sql})"
        ))
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("recalculando library_added_at: {e}"))?;
    }

    Ok(updated)
}

async fn apply_add_to_playlist(
    conn: &mut SqliteConnection,
    ids: &[i64],
    playlist_id: i64,
) -> Result<i64, String> {
    let base: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), 0) FROM playlist_tracks WHERE playlist_id = ?",
    )
    .bind(playlist_id)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| format!("posición de la playlist: {e}"))?;

    // `NOT EXISTS` es la idempotencia de pertenencia: la misma pista no entra
    // dos veces aunque el usuario repita la acción. La posición usa
    // ROW_NUMBER para no dejar huecos si una pista ya estaba.
    let values = value_pairs(ids.len());
    let sql = format!(
        "WITH candidates(track_id, ord) AS (VALUES {values}) \
         INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at) \
         SELECT ?, c.track_id, ? + ROW_NUMBER() OVER (ORDER BY c.ord), CURRENT_TIMESTAMP \
         FROM candidates c \
         WHERE NOT EXISTS ( \
             SELECT 1 FROM playlist_tracks pt \
             WHERE pt.playlist_id = ? AND pt.track_id = c.track_id)"
    );
    let mut query = sqlx::query(&sql);
    for (index, track_id) in ids.iter().enumerate() {
        query = query.bind(track_id).bind(index as i64 + 1);
    }
    let inserted = query
        .bind(playlist_id)
        .bind(base)
        .bind(playlist_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| format!("añadiendo a la playlist: {e}"))?
        .rows_affected() as i64;

    sqlx::query(
        "UPDATE playlists SET track_count = \
         (SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id = ?) WHERE id = ?",
    )
    .bind(playlist_id)
    .bind(playlist_id)
    .execute(&mut *conn)
    .await
    .map_err(|e| format!("actualizando track_count: {e}"))?;

    // Pertenencia de playlist con dueño (L00): la playlist local no tiene
    // cuenta y su pertenencia viva es playlist_tracks. `INSERT OR IGNORE`
    // mantiene la idempotencia del índice único de 0112.
    let ids_sql = inline_ids(ids);
    sqlx::query(&format!(
        "INSERT OR IGNORE INTO library_memberships \
         (account_id, track_id, collection_type, collection_id, provider_item, added_at) \
         SELECT p.account_id, pt.track_id, 'playlist', p.id, NULL, pt.added_at \
         FROM playlists p JOIN playlist_tracks pt ON pt.playlist_id = p.id \
         WHERE p.id = ? AND p.account_id IS NOT NULL AND pt.track_id IN ({ids_sql})"
    ))
    .bind(playlist_id)
    .execute(&mut *conn)
    .await
    .map_err(|e| format!("pertenencia de playlist: {e}"))?;

    sqlx::query(&format!(
        "UPDATE tracks SET {LIBRARY_ADDED_AT_SET_SQL} WHERE id IN ({ids_sql})"
    ))
    .execute(&mut *conn)
    .await
    .map_err(|e| format!("recalculando library_added_at: {e}"))?;

    Ok(inserted)
}

// ==========================================================================
// Helpers de selección / agregación
// ==========================================================================

fn validate_action(action: &LibraryBatchAction) -> Result<(), String> {
    if action.per_track_only() {
        return Err(format!(
            "'{}' es una acción por pista con confirmación: no se ofrece en \
             «seleccionar todo» ni se ejecuta por lotes.",
            action.id()
        ));
    }
    Ok(())
}

async fn ensure_playlist(db: &SqlitePool, playlist_id: i64) -> Result<(), String> {
    let deleted_at: Option<Option<String>> =
        sqlx::query_scalar("SELECT deleted_at FROM playlists WHERE id = ?")
            .bind(playlist_id)
            .fetch_optional(db)
            .await
            .map_err(|e| format!("comprobando la playlist: {e}"))?;
    match deleted_at {
        None => Err(format!("La playlist {playlist_id} no existe")),
        Some(Some(_)) => Err(format!("La playlist {playlist_id} está borrada")),
        Some(None) => Ok(()),
    }
}

fn normalize_ids(ids: &[i64], excluded: &[i64]) -> Vec<i64> {
    let excluded: std::collections::HashSet<i64> = excluded.iter().copied().collect();
    let mut ids: Vec<i64> = ids
        .iter()
        .copied()
        .filter(|id| !excluded.contains(id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

async fn count_matching(
    conn: &mut SqliteConnection,
    where_sql: &str,
    binds: &[String],
    excluded: &[i64],
) -> Result<i64, String> {
    let sql = format!(
        "SELECT COUNT(*) FROM tracks t WHERE ({where_sql}){}",
        exclusion_sql(excluded)
    );
    run_count(conn, &sql, binds).await
}

async fn count_matching_upto(
    conn: &mut SqliteConnection,
    where_sql: &str,
    binds: &[String],
    excluded: &[i64],
    max_id: i64,
) -> Result<i64, String> {
    let sql = format!(
        "SELECT COUNT(*) FROM tracks t WHERE ({where_sql}) AND t.id <= ?{}",
        exclusion_sql(excluded)
    );
    let mut query = sqlx::query_scalar::<_, i64>(&sql);
    for bind in binds {
        query = query.bind(bind);
    }
    query
        .bind(max_id)
        .fetch_one(conn)
        .await
        .map_err(|e| format!("contando la selección: {e}"))
}

async fn max_matching(
    conn: &mut SqliteConnection,
    where_sql: &str,
    binds: &[String],
    excluded: &[i64],
) -> Result<i64, String> {
    let sql = format!(
        "SELECT COALESCE(MAX(t.id), 0) FROM tracks t WHERE ({where_sql}){}",
        exclusion_sql(excluded)
    );
    run_count(conn, &sql, binds).await
}

async fn run_count(
    conn: &mut SqliteConnection,
    sql: &str,
    binds: &[String],
) -> Result<i64, String> {
    let mut query = sqlx::query_scalar::<_, i64>(sql);
    for bind in binds {
        query = query.bind(bind);
    }
    query
        .fetch_one(conn)
        .await
        .map_err(|e| format!("resolviendo la selección: {e}"))
}

async fn filter_ids_page(
    conn: &mut SqliteConnection,
    where_sql: &str,
    binds: &[String],
    excluded: &[i64],
    after_id: i64,
    max_id: i64,
    limit: usize,
) -> Result<Vec<i64>, String> {
    let sql = format!(
        "SELECT t.id FROM tracks t \
         WHERE ({where_sql}) AND t.id > ? AND t.id <= ?{} \
         ORDER BY t.id ASC LIMIT ?",
        exclusion_sql(excluded)
    );
    let mut query = sqlx::query_scalar::<_, i64>(&sql);
    for bind in binds {
        query = query.bind(bind);
    }
    query
        .bind(after_id)
        .bind(max_id)
        .bind(limit as i64)
        .fetch_all(conn)
        .await
        .map_err(|e| format!("resolviendo ids del filtro: {e}"))
}

fn exclusion_sql(excluded: &[i64]) -> String {
    if excluded.is_empty() {
        String::new()
    } else {
        format!(" AND t.id NOT IN ({})", inline_ids(excluded))
    }
}

fn inline_ids(ids: &[i64]) -> String {
    ids.iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn value_pairs(count: usize) -> String {
    vec!["(?, ?)"; count].join(", ")
}

fn blocks_for(total: i64) -> i64 {
    if total <= 0 {
        0
    } else {
        (total + LIBRARY_ACTION_BLOCK_SIZE as i64 - 1) / LIBRARY_ACTION_BLOCK_SIZE as i64
    }
}

type OmissionCounts = BTreeMap<String, i64>;

fn merge_omissions(target: &mut OmissionCounts, source: OmissionCounts) {
    for (reason, count) in source {
        *target.entry(reason).or_insert(0) += count;
    }
}

fn omissions_to_vec(counts: &OmissionCounts) -> Vec<LibraryActionOmission> {
    counts
        .iter()
        .map(|(reason, count)| LibraryActionOmission {
            reason: reason.clone(),
            message: omission_message(reason).to_string(),
            count: *count,
        })
        .collect()
}

fn omission_message(reason: &str) -> &'static str {
    match reason {
        "already_downloaded" => "Already downloaded",
        "already_queued" => "Already in the download queue",
        "no_downloadable_source" => "No downloadable source",
        "already_favorite" => "Already a favorite",
        "not_favorite" => "Not a favorite",
        "already_in_playlist" => "Already in the playlist",
        "not_in_library" => "No longer in the library",
        "no_longer_in_library" => "Left the library while the action was running",
        "state_changed" => "Changed while the action was running",
        _ => "Not eligible",
    }
}

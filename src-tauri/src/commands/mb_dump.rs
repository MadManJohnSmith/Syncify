// IPC Commands - submodule of crate::commands
//
// Superficie IPC del catálogo local MusicBrainz (`services::mb_dump_import`,
// tablas `mb_dump_*` de la migration 0096): importación en streaming del
// mbdump y lectura del último estado. Sin red: el dump es local y el progreso
// viaja por el evento `mb-dump-progress`.

use super::*;
use crate::services::mb_dump_import::{
    import_state, start_import, ImportCounts, ImportProgress, ImportState,
};

/// Importa el mbdump legacy de MusicBrainz al catálogo local de la app.
/// Emite `mb-dump-progress` con los chunk counts comprometidos mientras corre;
/// el comando resuelve con los recuentos finales o con el error de la etapa.
#[tauri::command]
pub async fn import_mb_dump(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    dump_path: String,
) -> Result<ImportCounts, String> {
    let dump_path = dump_path.trim();
    if dump_path.is_empty() {
        return Err("dump path is empty".into());
    }
    let db = state.db.clone();
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<ImportProgress>();
    let emitter = app.clone();
    let forwarder = tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let _ = emitter.emit("mb-dump-progress", &progress);
        }
    });
    let result = start_import(dump_path, &db, progress_tx).await;
    let _ = forwarder.await;
    result
}

/// Último intento de importación del catálogo (`mb_dump_state`); `None` si
/// este catálogo nunca se importó.
#[tauri::command]
pub async fn get_mb_dump_state(state: State<'_, AppState>) -> Result<Option<ImportState>, String> {
    import_state(&state.db).await
}

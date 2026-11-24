//! IPC de la cola de revisión de identidad (#383).
//!
//! Los comandos son envoltorios finos sobre `services::track_identity_review`:
//! la decisión (leer solo `pending`, cerrar en una transacción, plegar sin
//! perder filas) vive en el servicio y se prueba allí contra una BD `:memory:`
//! migrada de verdad.

use tauri::State;

use crate::services::track_identity_review::{
    self, IdentityReviewResolution, IdentityReviewResolutionResult, TrackIdentityReview,
};

/// Revisiones `pending`, con la evidencia congelada de ambos lados.
#[tauri::command]
pub async fn list_track_identity_reviews(
    state: State<'_, crate::AppState>,
) -> Result<Vec<TrackIdentityReview>, String> {
    track_identity_review::list_pending(&state.db).await
}

/// Cierra una revisión como `merged` (exige el Track canónico elegido),
/// `separate` o `dismissed`. Una revisión ya cerrada no se reabre.
#[tauri::command]
pub async fn resolve_track_identity_review(
    state: State<'_, crate::AppState>,
    review_id: i64,
    resolution: String,
    track_id: Option<i64>,
) -> Result<IdentityReviewResolutionResult, String> {
    let resolution = IdentityReviewResolution::parse(&resolution)?;
    track_identity_review::resolve(&state.db, review_id, resolution, track_id).await
}

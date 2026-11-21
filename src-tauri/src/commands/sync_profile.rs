//! Comandos IPC del perfil de sincronización (issue #69, ADR-0006 decisiones
//! 3-5, 7 y 30): `get_sync_profile` devuelve el perfil ya resuelto —default del
//! servicio más override de la cuenta— y `update_sync_profile` persiste el
//! override. La UI nunca mezcla las dos fuentes por su cuenta.

use crate::commands::AppState;
use crate::sync::profile::{self as sync_profile, SyncProfile, SyncProfileUpdate};
use tauri::State;

/// Perfil efectivo de una cuenta: modo, intervalo, colecciones y participación.
#[tauri::command]
pub async fn get_sync_profile(
    state: State<'_, AppState>,
    account_id: i64,
) -> Result<SyncProfile, String> {
    sync_profile::load_sync_profile(&state.db, account_id).await
}

/// Guarda el override de la cuenta y devuelve el perfil resuelto resultante.
///
/// Un campo `null` hereda del servicio; el bidireccional se rechaza con su
/// motivo (`write_back_not_available`) porque no es seleccionable antes de beta.
#[tauri::command]
pub async fn update_sync_profile(
    state: State<'_, AppState>,
    account_id: i64,
    profile: SyncProfileUpdate,
) -> Result<SyncProfile, String> {
    sync_profile::save_account_profile(&state.db, account_id, &profile).await
}

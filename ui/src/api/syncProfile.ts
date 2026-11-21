/**
 * Perfil de sincronización (issue #69, ADR-0006 decisiones 3-5, 7 y 30).
 *
 * Única puerta IPC del perfil: `get_sync_profile` devuelve el perfil YA
 * RESUELTO (default del servicio + override de la cuenta) y
 * `update_sync_profile` persiste el override. Ninguna vista mezcla fuentes.
 */

import { invokeCommand } from './tauri';

/** Colecciones del contrato que el perfil puede activar. */
export interface SyncProfileCollections {
    favorites: boolean;
    playlists: boolean;
    albums: boolean;
    artists: boolean;
}

/** Origen del valor resuelto: el default del servicio o el override de la cuenta. */
export type SyncProfileSource = 'service' | 'account';

/** Una opción de modo con su disponibilidad y motivo. */
export interface SyncModeOption {
    /** import_only | mirror_quarantine | strict_mirror | bidirectional */
    value: string;
    enabled: boolean;
    /** `write_back_not_available` cuando el modo no es seleccionable. */
    reason: string | null;
}

/** Perfil efectivo de una cuenta. */
export interface SyncProfile {
    account_id: number;
    service_name: string;
    enabled: boolean;
    mode: string;
    mode_source: SyncProfileSource;
    interval_hours: number;
    interval_source: SyncProfileSource;
    collections: SyncProfileCollections;
    collections_source: SyncProfileSource;
    modes: SyncModeOption[];
}

/**
 * Payload de guardado. `null` en `mode`, `interval_hours` o `collections`
 * significa «hereda del servicio»; `enabled` es siempre explícito.
 */
export interface SyncProfileUpdate {
    enabled: boolean;
    mode: string | null;
    interval_hours: number | null;
    collections: SyncProfileCollections | null;
}

/** Perfil resuelto de la cuenta. */
export async function getSyncProfile(accountId: number): Promise<SyncProfile> {
    return invokeCommand<SyncProfile>('get_sync_profile', { accountId });
}

/** Guarda el override y devuelve el perfil resuelto resultante. */
export async function updateSyncProfile(
    accountId: number,
    profile: SyncProfileUpdate
): Promise<SyncProfile> {
    return invokeCommand<SyncProfile>('update_sync_profile', { accountId, profile });
}

export const syncProfileApi = {
    getSyncProfile,
    updateSyncProfile,
};

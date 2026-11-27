// Barrel split from api/types.ts; tipos espejo de los comandos Rust.
export interface Playlist {
    id: number;
    name: string;
    description: string | null;
    track_count: number;
    owner_name?: string | null;
    image_url?: string | null;
    service_name?: string | null;
    source_service?: string | null;
    source_id?: string | null;
    account_id?: number | null;
    service_playlist_id?: string | null;
    is_public?: boolean;
    last_synced?: string | null;
    created_at?: string | null;
    updated_at?: string | null;
    tracks?: PlaylistTrack[];
    is_smart?: boolean;
    rules_json?: string | null;
}

// ==============================================
// PLAYLIST TYPES
// ==============================================

export interface PlaylistTrack {
    id: number;
    track_id: number;
    position: number;
    title: string;
    artist_name: string;
    album_name: string | null;
}

// ==============================================
// PLAYLIST CONFLICT MODES / REVIEWS (#410)
// ==============================================

/** Modo persistido por playlist (espejo de `PlaylistConflictMode` en Rust). */
export type PlaylistConflictMode = 'editable' | 'gana_proveedor' | 'gana_local';

/** Tipo de evidencia de una revisión de conflicto. */
export type PlaylistConflictKind = 'name' | 'membership';

/** Estados de la cola de revisión de conflictos. */
export type PlaylistConflictReviewStatus =
    | 'needs_review'
    | 'resolved_local'
    | 'resolved_provider'
    | 'dismissed';

/**
 * Una revisión de conflicto con la evidencia de ambos lados. Los valores
 * `*_track_ids` solo llegan en conflictos de pertenencia.
 */
export interface PlaylistConflictReview {
    id: number;
    playlist_id: number;
    playlist_name: string;
    service_name: string | null;
    conflict_kind: PlaylistConflictKind;
    local_value: string | null;
    provider_value: string | null;
    status: PlaylistConflictReviewStatus;
    created_at: string;
    updated_at: string;
    resolved_at: string | null;
    local_track_ids: number[] | null;
    provider_track_ids: number[] | null;
}

/**
 * Ficha de conflicto de una playlist. `requires_mode_choice` es verdadero
 * mientras la playlist remota siga en el modo de fábrica «gana el proveedor»:
 * la primera edición local debe preguntar antes de arriesgar el cambio.
 */
export interface PlaylistConflictState {
    playlist_id: number;
    conflict_mode: PlaylistConflictMode;
    service_name: string | null;
    is_remote: boolean;
    requires_mode_choice: boolean;
    pending_reviews: PlaylistConflictReview[];
}

/** Acción explícita al resolver una revisión. Nunca se resuelve sola. */
export type PlaylistConflictResolution = 'local' | 'provider' | 'dismiss';

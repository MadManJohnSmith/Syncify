// Tipos espejo del agregado de cola de reproducción (Rust: commands/playback_queue.rs).

export interface PlaybackQueueEntryDto {
    entry_id: string;
    /** Track de biblioteca; `0` identifica una entrada que sigue efímera. */
    track_id: number;
    /** Identidad del proveedor mientras la entrada sigue efímera. */
    service_id: number | null;
    service_track_id: string | null;
    /** `pending` | `history` | `current`. */
    kind: string;
    position: number;
    /** Motivo persistido cuando la promoción aún no pudo resolver identidad. */
    promotion_error: string | null;
}

export interface PlaybackQueueSnapshotDto {
    pending: PlaybackQueueEntryDto[];
    /** Orden de reproducción (el más reciente al final). */
    history: PlaybackQueueEntryDto[];
    current: PlaybackQueueEntryDto | null;
    position_sec: number;
    shuffle: boolean;
    /** `off` | `all` | `one`. */
    repeat: string;
    volume: number;
    muted: boolean;
}

/** Payload de `player:advanced`: entrada que el backend acaba de activar. */
export interface PlayerAdvanced {
    seq: number;
    /** `0` mientras la entrada actual sigue siendo efímera. */
    trackId: number;
    serviceId?: number | null;
    serviceTrackId?: string | null;
    entryId: string;
    previousEntryId?: string;
}

/**
 * Cola de reproducción persistente en SQLite (#131, P00 — parte de #130).
 *
 * Única puerta IPC del agregado de cola: cola, historial, pista actual,
 * posición, modos y volumen viven en el backend (migración 0109) y la UI es
 * una vista sobre estos comandos. El transporte del motor sigue en
 * `api/player.ts` y la cola de DESCARGAS está en `api/queue.ts` — dominios
 * distintos.
 *
 * No confundir: `api/queue.ts` es la cola de descargas.
 *
 * P03 (#134): una entrada puede referenciar un Track o una identidad de
 * proveedor. `track_id = 0` identifica estas últimas hasta su promoción.
 */

import { invokeCommand } from './tauri';
import type { PlaybackQueueEntryDto, PlaybackQueueSnapshotDto } from './types';

export type { PlaybackQueueEntryDto, PlaybackQueueSnapshotDto } from './types';

/** Lee la instantánea completa: cola, historial, current, posición y modos. */
export function getPlaybackQueueSnapshot(): Promise<PlaybackQueueSnapshotDto> {
    return invokeCommand<PlaybackQueueSnapshotDto>('playback_queue_get');
}

/**
 * Encola tracks de biblioteca y devuelve sus `entry_id` en el mismo orden.
 * `atFront` inserta delante de los pendientes (regla play-next); si no, al
 * final.
 */
export function playbackQueueEnqueue(trackIds: number[], atFront: boolean): Promise<string[]> {
    return invokeCommand<string[]>('playback_queue_enqueue', { trackIds, atFront });
}

/**
 * Encola un ítem del proveedor sin crear Track ni TrackSource. La promoción
 * ocurre sólo después de 30 s reales de escucha.
 */
export function playbackQueueEnqueueProviderItem(
    serviceId: number,
    serviceTrackId: string,
    atFront: boolean,
): Promise<string> {
    return invokeCommand<string>('playback_queue_enqueue_provider_item', {
        serviceId,
        serviceTrackId,
        atFront,
    });
}

/** Quita una entrada pendiente de la cola. */
export function playbackQueueRemove(entryId: string): Promise<void> {
    return invokeCommand<void>('playback_queue_remove', { entryId });
}

/** Fija el orden exacto de la cola pendiente (reordenar, shuffle on/off). */
export function playbackQueueReorder(entryIds: string[]): Promise<void> {
    return invokeCommand<void>('playback_queue_reorder', { entryIds });
}

/**
 * Mueve una entrada a «pista actual» (la anterior pasa al historial en orden
 * de reproducción) y resetea la posición a 0.
 */
export function playbackQueueSetCurrent(entryId: string, trackId: number): Promise<void> {
    return invokeCommand<void>('playback_queue_set_current', { entryId, trackId });
}

/** Activa una entrada persistida por su identidad, sin exigir un Track. */
export function playbackQueueSetCurrentEntry(entryId: string): Promise<PlaybackQueueEntryDto> {
    return invokeCommand<PlaybackQueueEntryDto>('playback_queue_set_current_entry', { entryId });
}

/** Vacía la cola pendiente (la pista actual y el historial se conservan). */
export function playbackQueueClear(): Promise<void> {
    return invokeCommand<void>('playback_queue_clear');
}

/** Vacía el historial de reproducción. */
export function playbackHistoryClear(): Promise<void> {
    return invokeCommand<void>('playback_history_clear');
}

/** Guarda la posición de reproducción (pausa, seek, ocultar ventana). */
export function playbackStateSetPosition(positionSec: number): Promise<void> {
    return invokeCommand<void>('playback_state_set_position', { positionSec });
}

/** Guarda shuffle y repeat. */
export function playbackStateSetModes(shuffle: boolean, repeat: string): Promise<void> {
    return invokeCommand<void>('playback_state_set_modes', { shuffle, repeat });
}

/** Guarda volumen y silencio. */
export function playbackStateSetVolume(volume: number, muted: boolean): Promise<void> {
    return invokeCommand<void>('playback_state_set_volume', { volume, muted });
}

/** Vuelca todo el agregado de cola (cola, historial, current, posición). */
export function playbackStateReset(): Promise<void> {
    return invokeCommand<void>('playback_state_reset');
}

/**
 * Fase 1 del player: envoltorio tipado del motor nativo (`syncify-player` en
 * Rust, rodio + symphonia) — comandos de transporte y eventos.
 *
 * La cola, el historial, los modos y el crédito de escucha viven en
 * `usePlayer.ts`; aquí solo está el transporte de UNA pista (cargar, pausar,
 * reanudar, parar, buscar, volumen, ritmo) y la suscripción a sus eventos.
 *
 * El backend mantiene un `seq` por carga: cada `player_load_track` lo
 * incrementa y los eventos lo llevan. El frontend descarta todo evento cuyo
 * `seq` no sea el de la pista que cree cargada.
 */
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invokeCommand, TauriEvents } from './tauri'
import type { PlaybackQueueEntryDto, PlayerAdvanced } from './types'

export type { PlayerAdvanced } from './types'

export type PlayerEngineState = 'stopped' | 'playing' | 'paused'

/** Estado + posición del motor (payload de `player:tick`). */
export interface PlayerTick {
    seq: number;
    state: PlayerEngineState;
    positionSec: number;
    durationSec: number | null;
}

/** Generación y duración que devuelve cada carga del motor. */
export interface PlayerLoadResult {
    seq: number;
    durationSec: number | null;
}

/**
 * Calidad realmente obtenida al resolver una fuente de streaming.
 * `qualityLabel` es la etiqueta honesta que debe mostrar la UI: refleja lo
 * obtenido (p. ej. `FLAC 16 bits / 44.1 kHz`), no lo pedido.
 */
export interface PlayerStreamInfo {
    serviceName: string;
    serviceTrackId: string;
    format: string;
    codec: string | null;
    container: string | null;
    extension: string;
    bitDepth: number | null;
    sampleRate: number | null;
    qualityLabel: string;
    qualityTier: string;
    sourceName: string;
    /** `direct` (la pista ya tenía esa fuente) | `isrc` | `metadata`. */
    matchMethod: string;
}

/** Subconjunto de la calidad para badges/indicadores de la UI. */
export interface PlayerStreamQuality {
    serviceName: string;
    qualityLabel: string;
    qualityTier: string;
    /** Cómo se identificó la grabación (ver `PlayerStreamInfo.matchMethod`). */
    matchMethod: string;
}

/** Resultado de una carga de biblioteca o proveedor (local o streaming). */
export interface PlayerTrackLoadResult extends PlayerLoadResult {
    /** `null` cuando la pista se reprodujo desde un archivo local. */
    stream: PlayerStreamInfo | null;
}

/** Payload de `player:stream-state` (backend camelCase). */
export interface PlayerStreamState extends PlayerStreamQuality {
    seq: number;
    /** `0` para una entrada efímera de proveedor. */
    trackId: number;
    entryId?: string;
    buffering: boolean;
    downloadedBytes: number;
    totalBytes: number | null;
    complete: boolean;
    pendingSeekSec: number | null;
    error: string | null;
}

interface RawTick {
    seq: number;
    state: PlayerEngineState;
    position_sec: number;
    duration_sec: number | null;
}

interface RawLoadResult {
    seq: number;
    duration_sec: number | null;
    stream?: PlayerStreamInfo | null;
}

/**
 * Carga una pista de la biblioteca: usa el archivo descargado si existe y, si
 * no, resuelve streaming (Qobuz/Tidal) en el backend. La URL y las
 * credenciales nunca viajan al webview.
 *
 * Puede tardar mientras se llena el buffer inicial (`isLoadingSource`), pero
 * el resultado ya trae la calidad obtenida para la etiqueta honesta.
 */
export function playerLoadTrack(
    trackId: number,
    autoplay: boolean,
    volume: number,
): Promise<PlayerTrackLoadResult> {
    return invokeCommand<RawLoadResult>('player_load_track', { trackId, autoplay, volume })
        .then(r => ({
            seq: r.seq,
            durationSec: r.duration_sec,
            stream: r.stream ?? null,
        }));
}

/** Carga una entrada de cola de biblioteca o proveedor por su entry_id. */
export function playerLoadQueueEntry(
    entryId: string,
    autoplay: boolean,
    volume: number,
): Promise<PlayerTrackLoadResult> {
    return invokeCommand<RawLoadResult>('player_load_queue_entry', {
        entryId,
        autoplay,
        volume,
    }).then(r => ({
        seq: r.seq,
        durationSec: r.duration_sec,
        stream: r.stream ?? null,
    }));
}

export function playerPause(): Promise<void> {
    return invokeCommand<void>('player_pause');
}

export function playerResume(): Promise<void> {
    return invokeCommand<void>('player_resume');
}

export function playerStop(): Promise<void> {
    return invokeCommand<void>('player_stop');
}

export function playerSeek(sec: number): Promise<void> {
    return invokeCommand<void>('player_seek', { sec });
}

export function playerSetVolume(volume: number): Promise<void> {
    return invokeCommand<void>('player_set_volume', { volume });
}

export function playerSetRate(rate: number): Promise<void> {
    return invokeCommand<void>('player_set_rate', { rate });
}

/** Suscripción a `player:tick` (250 ms mientras suena; 1 por cambio de transporte). */
export function onPlayerTick(cb: (tick: PlayerTick) => void): Promise<UnlistenFn> {
    return listen<RawTick>(TauriEvents.PLAYER_TICK, (event) => {
        const { seq, state, position_sec, duration_sec } = event.payload;
        cb({ seq, state, positionSec: position_sec, durationSec: duration_sec });
    });
}

/** Suscripción a `player:ended` (final natural de la pista; nunca con `player_stop`). */
export function onPlayerEnded(cb: (seq: number) => void): Promise<UnlistenFn> {
    return listen<{ seq: number }>(TauriEvents.PLAYER_ENDED, (event) => {
        cb(event.payload.seq);
    });
}

/** Suscripción a `player:advanced`: el backend avanzó de pista por su cuenta. */
export function onPlayerAdvanced(cb: (advanced: PlayerAdvanced) => void): Promise<UnlistenFn> {
    return listen<{
        seq: number;
        track_id: number;
        service_id: number | null;
        service_track_id: string | null;
        entry_id: string;
        previous_entry_id?: string;
    }>(TauriEvents.PLAYER_ADVANCED, (event) => {
        cb({
            seq: event.payload.seq,
            trackId: event.payload.track_id,
            serviceId: event.payload.service_id,
            serviceTrackId: event.payload.service_track_id,
            entryId: event.payload.entry_id,
            previousEntryId: event.payload.previous_entry_id,
        });
    });
}

/** Delta de una entrada tras promoción o cambio de su motivo de promoción. */
export function onPlayerQueueEntryChanged(
    cb: (entry: PlaybackQueueEntryDto) => void,
): Promise<UnlistenFn> {
    return listen<{ entry: PlaybackQueueEntryDto }>(
        TauriEvents.PLAYER_QUEUE_ENTRY_CHANGED,
        event => cb(event.payload.entry),
    )
}

/** Modos persistidos por el backend (MPRIS reordena la cola pendiente). */
export interface PlayerModesChanged {
    shuffle: boolean;
    repeat: 'off' | 'all' | 'one';
    /** Sólo cuando el shuffle cambió el orden de los pendientes. */
    pendingEntryIds?: string[];
}

/** Suscripción a `player:modes-changed` (P02, #133). */
export function onPlayerModesChanged(cb: (modes: PlayerModesChanged) => void): Promise<UnlistenFn> {
    return listen<PlayerModesChanged>(TauriEvents.PLAYER_MODES_CHANGED, (event) => {
        cb(event.payload);
    });
}

/**
 * Suscripción a `player:stream-state`: progreso de la caché progresiva,
 * pausas por buffer y errores de descarga. `seq` identifica la carga, igual
 * que en el resto de eventos del motor.
 */
export function onPlayerStreamState(cb: (state: PlayerStreamState) => void): Promise<UnlistenFn> {
    return listen<PlayerStreamState>(TauriEvents.PLAYER_STREAM_STATE, (event) => {
        cb(event.payload);
    });
}


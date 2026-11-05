/**
 * Fase 1 del player: envoltorio tipado del motor nativo (`syncify-player` en
 * Rust, rodio + symphonia) — comandos de transporte y eventos.
 *
 * La cola, el historial, los modos y el crédito de escucha viven en
 * `usePlayer.ts`; aquí solo está el transporte de UNA pista (cargar, pausar,
 * reanudar, parar, buscar, volumen, ritmo) y la suscripción a sus eventos.
 *
 * El backend mantiene un `seq` por carga: cada `player_load` lo incrementa y
 * los eventos lo llevan. El frontend descarta todo evento cuyo `seq` no sea el
 * de la pista que cree cargada.
 */
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { invokeCommand, TauriEvents } from './tauri'

export type PlayerEngineState = 'stopped' | 'playing' | 'paused'

/** Estado + posición del motor (payload de `player:tick`). */
export interface PlayerTick {
    seq: number;
    state: PlayerEngineState;
    positionSec: number;
    durationSec: number | null;
}

/** Resultado de `player_load`. */
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

/** Resultado de `player_load_track` (local o streaming). */
export interface PlayerTrackLoadResult extends PlayerLoadResult {
    /** `null` cuando la pista se reprodujo desde un archivo local. */
    stream: PlayerStreamInfo | null;
}

/** Payload de `player:stream-state` (backend camelCase). */
export interface PlayerStreamState extends PlayerStreamQuality {
    seq: number;
    trackId: number;
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

export function playerLoad(path: string, autoplay: boolean, volume: number): Promise<PlayerLoadResult> {
    return invokeCommand<RawLoadResult>('player_load', { path, autoplay, volume })
        .then(r => ({ seq: r.seq, durationSec: r.duration_sec }));
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

export type MprisLoopStatus = 'None' | 'Track' | 'Playlist'

/** Estado que la UI publica para el panel MPRIS del escritorio (Linux). */
export interface MprisStateUpdate {
    trackId: number | null;
    title: string | null;
    artist: string | null;
    album: string | null;
    durationSec: number | null;
    artUrl: string | null;
    shuffle: boolean;
    loopStatus: MprisLoopStatus;
}

export type MprisAction =
    | 'play' | 'pause' | 'playPause' | 'next' | 'previous' | 'stop'
    | 'seek' | 'setPosition'

/** Acción MPRIS reenviada por el backend (payload de `player:mpris-action`). */
export interface MprisActionPayload {
    action: MprisAction;
    /** `seek`: offset relativo en microsegundos (puede ser negativo). */
    offsetUs?: number;
    /** `setPosition`: posición absoluta en microsegundos. */
    positionUs?: number;
}

/** Publica metadatos y modos para MPRIS (no-op en plataformas sin panel MPRIS). */
export function playerMprisUpdate(state: MprisStateUpdate): Promise<void> {
    return invokeCommand<void>('player_mpris_update', { metadata: state });
}

/** Suscripción a `player:mpris-action` (media keys / panel del escritorio). */
export function onMprisAction(cb: (action: MprisActionPayload) => void): Promise<UnlistenFn> {
    return listen<MprisActionPayload>(TauriEvents.PLAYER_MPRIS_ACTION, (event) => {
        cb(event.payload);
    });
}

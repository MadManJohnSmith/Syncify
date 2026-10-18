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

interface RawTick {
    seq: number;
    state: PlayerEngineState;
    position_sec: number;
    duration_sec: number | null;
}

interface RawLoadResult {
    seq: number;
    duration_sec: number | null;
}

export function playerLoad(path: string, autoplay: boolean, volume: number): Promise<PlayerLoadResult> {
    return invokeCommand<RawLoadResult>('player_load', { path, autoplay, volume })
        .then(r => ({ seq: r.seq, durationSec: r.duration_sec }));
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

/**
 * Local player — Phase 1 (queue with identity, persistent modes, listen
 * crediting, restore after restart, MediaSession).
 *
 * Singleton driven by the native engine (`syncify-player` in Rust,
 * rodio + symphonia) through the typed transport in `api/player.ts`: the
 * backend command `player_load_track` picks the downloaded file when it
 * exists and otherwise resolves a progressive Qobuz/Tidal stream (Fase 2;
 * the URL never leaves the backend). The legacy HTML5 Audio +
 * `syncify-media://` path was removed after Fase 0 proved the WebKitGTK
 * media engine cannot consume custom-scheme audio (see
 * `docs/auditoria/playback-fase0-evidencia-2026-10-17.md`).
 *
 * Since #131 (P00) the queue, history, current track, position, modes and
 * volume live in SQLite on the backend (`api/playbackQueue.ts`); this
 * composable is a view over those commands. The old localStorage key
 * `syncify.playerState.v1` is discarded without migration.
 *
 * Since #132 (P01) the backend also decides the next track and credits the
 * listen: on the natural end of a track it advances the queue itself
 * (`repeat one/all` and the shuffle order are resolved in Rust, over the
 * persisted aggregate) and notifies through `player:advanced`; it accumulates
 * the 30 real seconds and calls its own crediting once per session. This
 * composable no longer times playback and no longer advances on `ended` —
 * it syncs its projection from the backend event.
 *
 * Phase rules:
 * - Restoring after a restart ALWAYS resolves again: library entries use
 *   `playerLoadTrack` and provider entries use `playerLoadQueueEntry`. The
 *   persisted state never stores `file_path`, the full track DTO, or engine
 *   state. The engine loads paused at the saved position.
 * - Importing this module fires no playback: restoration
 *   is lazy (`restoreFromPersistence`) and the status bar player invokes it on mount.
 * - Engine events carry the load `seq`: everything from an already replaced
 *   track is discarded.
 */
import { computed, ref } from 'vue'
import { getTrackMetadata, type TrackMetadataWithSources } from '../api/library'
import {
    onPlayerAdvanced,
    onPlayerEnded,
    onPlayerModesChanged,
    onPlayerQueueEntryChanged,
    onPlayerStreamState,
    onPlayerTick,
    playerLoadQueueEntry,
    playerLoadTrack,
    playerPause,
    playerResume,
    playerSeek,
    playerSetRate,
    playerSetVolume,
    playerStop,
    type PlayerAdvanced,
    type PlayerEngineState,
    type PlayerStreamQuality,
} from '../api/player'
import {
    getPlaybackQueueSnapshot,
    playbackHistoryClear,
    playbackQueueClear,
    playbackQueueEnqueue,
    playbackQueueRemove,
    playbackQueueReorder,
    playbackQueueSetCurrent,
    playbackQueueSetCurrentEntry,
    playbackStateReset,
    playbackStateSetModes,
    playbackStateSetPosition,
    playbackStateSetVolume,
    type PlaybackQueueEntryDto,
} from '../api/playbackQueue'

/**
 * Outcome of queueing a track: 'started' means playback began right away
 * (nothing was loaded), 'queued' means it waits in the up-next queue.
 */
export type QueueOutcome = 'queued' | 'started'

export interface PlayerTrack {
    id: number;
    title: string;
    artist: string;
    album?: string | null;
    coverUrl?: string | null;
}

export type RepeatMode = 'off' | 'all' | 'one'

/**
 * Queue entry with its own identity: allows duplicates and stable
 * reordering. `track` is the safe projection (without file_path) when known;
 * restored entries get it just before playing.
 */
export interface PlaybackQueueEntry {
    entryId: string;
    /** `0` mientras la entrada sigue siendo un ítem de proveedor. */
    trackId: number;
    serviceId?: number | null;
    serviceTrackId?: string | null;
    promotionError?: string | null;
    track?: PlayerTrack;
}

// ---------------------------------------------------------------------------
// Constants and utilities
// ---------------------------------------------------------------------------

const HISTORY_LIMIT = 100;
/**
 * Esquema antiguo de localStorage de reproducción (#131): se descarta sin
 * migración en cada restauración; la UI ya no escribe nunca esta clave.
 */
const LEGACY_STORAGE_KEY = 'syncify.playerState.v1';

function clampVolume(v: number): number {
    if (!Number.isFinite(v)) return 1;
    return Math.min(1, Math.max(0, v));
}

/** Canonical UUID v4 (lowercase with hyphens); manual fallback for environments without randomUUID. */
function uuidV4(): string {
    const c = globalThis.crypto;
    if (c && typeof c.randomUUID === 'function') return c.randomUUID();
    const bytes = new Uint8Array(16);
    if (c && typeof c.getRandomValues === 'function') {
        c.getRandomValues(bytes);
    } else {
        for (let i = 0; i < 16; i++) bytes[i] = Math.floor(Math.random() * 256);
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const hex = Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function messageOf(err: unknown): string {
    return err instanceof Error ? err.message : String(err);
}

/**
 * Classifier for definitive library errors (deleted track, invalid UUID,
 * broken FK): they are discarded without retrying. Everything else (busy DB,
 * IPC down) is treated as transient. Messages come from the backend as
 * text; the classifier tolerates wording variations.
 */
export function isDefinitiveLibraryError(message: string): boolean {
    const m = message.toLowerCase();
    return m.includes('no existe')
        || m.includes('inexistente')
        || m.includes('no such')
        || m.includes('not found')
        || m.includes('foreign key')
        || m.includes('inválido')
        || m.includes('invalid');
}

/** Strict projection of the DTO to PlayerTrack: never drags file_path/URLs along. */
export function projectMetadata(meta: TrackMetadataWithSources): PlayerTrack {
    return {
        id: meta.track_id,
        title: meta.title,
        artist: meta.artist_name ?? 'Unknown Artist',
        album: meta.album_name ?? null,
        coverUrl: meta.cover_art_url ?? null,
    };
}

function isProviderEntry(entry: PlaybackQueueEntry): boolean {
    return entry.trackId === 0;
}

/** Proyección honesta: una efímera no tiene todavía metadatos de biblioteca. */
function providerProjection(entry: PlaybackQueueEntry): PlayerTrack {
    return entry.track ?? {
        id: 0,
        title: entry.serviceTrackId ? `Provider item ${entry.serviceTrackId}` : 'Provider item',
        artist: '',
        album: null,
        coverUrl: null,
    };
}

function mapQueueEntry(dto: PlaybackQueueEntryDto): PlaybackQueueEntry {
    const entry: PlaybackQueueEntry = {
        entryId: dto.entry_id,
        trackId: dto.track_id,
        serviceId: dto.service_id ?? null,
        serviceTrackId: dto.service_track_id ?? null,
        promotionError: dto.promotion_error ?? null,
    };
    return isProviderEntry(entry) ? { ...entry, track: providerProjection(entry) } : entry;
}

function shuffled<T>(arr: T[]): T[] {
    const a = [...arr];
    for (let i = a.length - 1; i > 0; i--) {
        const j = Math.floor(Math.random() * (i + 1));
        [a[i], a[j]] = [a[j], a[i]];
    }
    return a;
}

// ---------------------------------------------------------------------------
// Singleton state
// ---------------------------------------------------------------------------

/**
 * Generation of the engine load owned by this composable (0 = none). Engine
 * events carry it; anything else belongs to a replaced track and is ignored.
 */
let engineSeq = 0;
/** Path loaded in the engine; `null` plays the role `audio.src` had (nothing loaded). */
let loadedPath: string | null = null;
/** Last engine state (tick-confirmed or optimistic); mirrors `audio.paused`. */
let engineState: PlayerEngineState = 'stopped';

const currentEntry = ref<PlaybackQueueEntry | null>(null);
const current = computed<PlayerTrack | null>({
    get: () => currentEntry.value?.track ?? null,
    // Direct write supported (state injection; layout specs use it
    // to simulate "playing"): wraps the track in an entry with its own
    // identity without touching queue or history.
    set: (track) => {
        currentEntry.value = track ? { entryId: uuidV4(), trackId: track.id, track } : null;
    },
});
const isPlaying = ref(false);
const positionSec = ref(0);
const durationSec = ref(0);
const playbackRate = ref(1);
const error = ref<string | null>(null);
const isLoadingSource = ref(false);
/** Streaming: pausa por buffer (el motor espera a que la descarga avance). */
const isBuffering = ref(false);
/** Calidad realmente en reproducción (etiqueta honesta), si es streaming. */
const streamQuality = ref<PlayerStreamQuality | null>(null);
/** Error de descarga del stream (se limpia en la siguiente carga). */
const streamError = ref<string | null>(null);

// PL-09 / PL-10: Fullscreen / expanded player state and actions
const isFullscreenOpen = ref(false);

function openFullscreen(): void {
    isFullscreenOpen.value = true;
}

function closeFullscreen(): void {
    isFullscreenOpen.value = false;
}

function toggleFullscreen(): void {
    isFullscreenOpen.value = !isFullscreenOpen.value;
}

// PL-10: Listen for the event fired from the sidebar menu (App.vue:483)
if (typeof window !== 'undefined') {
    window.addEventListener('syncify:open-fullscreen-player', () => {
        openFullscreen();
    });
}

// Queue (extended FE-9): pending entries with their own identity; `upNext` is kept
// as a compatible projection for previous consumers.
const queue = ref<PlaybackQueueEntry[]>([]);
const history = ref<PlaybackQueueEntry[]>([]);
const shuffle = ref(false);
const repeat = ref<RepeatMode>('off');
const volume = ref(1);
const muted = ref(false);

/** Pre-shuffle base order of the pending entries (entryIds). Not reactive: the mutations keep it in sync. */
let baseOrder: string[] = [];

/**
 * Transición de reproducción en curso: cada acción que cambia de pista la
 * invalida (y `stop` aborta la activa). NO es una generación: la generación es
 * la `seq` que publica el backend — viaja en el resultado de la carga
 * (`engineSeq`) y en los eventos, que se descartan por ella; este token sólo
 * cancela trabajo asíncrono cuya respuesta ya no interesa.
 */
interface Transition {
    aborted: boolean;
}

let activeTransition: Transition | null = null;

/** Abre una transición y deja sin efecto la anterior (si la hay). */
function beginTransition(): Transition {
    const transition: Transition = { aborted: false };
    if (activeTransition) activeTransition.aborted = true;
    activeTransition = transition;
    return transition;
}

/** Effective engine volume: mute is sent as 0, like `audio.muted` did. */
function effectiveEngineVolume(): number {
    return muted.value ? 0 : clampVolume(volume.value);
}

/**
 * Engine failures are shown with the backend's sanitized message (the engine
 * never puts paths in its error text).
 */
function reportEngineFailure(err: unknown): void {
    error.value = messageOf(err);
    isPlaying.value = false;
    setMediaSessionPlaybackState('paused');
}

const upNext = computed<PlayerTrack[]>(() =>
    queue.value.map(entry => isProviderEntry(entry)
        ? providerProjection(entry)
        : entry.track ?? {
            id: entry.trackId,
            title: `Track #${entry.trackId}`,
            artist: '',
            album: null,
            coverUrl: null,
        }),
);

// ---------------------------------------------------------------------------
// Persistence (backend, SQLite — api/playbackQueue.ts; #131)
// ---------------------------------------------------------------------------

function isValidRepeat(v: string): v is RepeatMode {
    return v === 'off' || v === 'all' || v === 'one';
}

/**
 * Cada mutación del agregado se refleja en el backend con SU comando (la
 * cola, el historial, la pista actual, la posición, los modos y el volumen
 * viven en SQLite desde #131). Un fallo de persistencia jamás corta la
 * reproducción: se registra y el estado en memoria sigue mandando hasta la
 * siguiente mutación que lo reconcilie.
 */
function persistQueueWrite(what: string, op: () => Promise<unknown>): void {
    try {
        // Promise.resolve: nunca debe romper la reproducción, ni siquiera si
        // la capa IPC devolviera algo que no es promesa.
        Promise.resolve(op()).catch((err: unknown) => {
            console.warn(`[player] persisting queue failed (${what}): ${messageOf(err)}`);
        });
    } catch (err) {
        console.warn(`[player] persisting queue failed (${what}): ${messageOf(err)}`);
    }
}

// ---------------------------------------------------------------------------
// Listen crediting: owned by the backend since P01 (#132). Rust accumulates
// the 30 real seconds in its ticker and credits once per session; the UI
// never times playback.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// MediaSession (feature detection by symbol, clean degradation)
// ---------------------------------------------------------------------------

function mediaSessionAvailable(): boolean {
    return typeof navigator !== 'undefined' && 'mediaSession' in navigator;
}

function setMediaSessionPlaybackState(state: MediaSessionPlaybackState): void {
    if (!mediaSessionAvailable()) return;
    // `playbackState` is a property of the standard, not a method.
    try {
        navigator.mediaSession.playbackState = state;
    } catch {
        // Clean degradation.
    }
}

function updateMediaSessionMetadata(track: PlayerTrack): void {
    if (!mediaSessionAvailable()) return;
    if (typeof MediaMetadata !== 'function') return;
    try {
        navigator.mediaSession.metadata = new MediaMetadata({
            title: track.title,
            artist: track.artist,
            ...(track.album ? { album: track.album } : {}),
            ...(track.coverUrl ? { artwork: [{ src: track.coverUrl }] } : {}),
        });
    } catch {
        // Clean degradation.
    }
}

/**
 * Publica metadatos y modos en el panel MPRIS del escritorio (Linux). El
 * transporte (estado + posición) lo publica el ticker del backend; aquí va
 * sólo lo que únicamente sabe la UI. En el resto de plataformas es un no-op.
 *
 * La pista llega por parámetro: al empezar a sonar la carga se confirma antes
 * de que `currentEntry` apunte a la pista nueva.
 */
function updateMediaSessionPosition(): void {
    if (!mediaSessionAvailable()) return;
    const ms = navigator.mediaSession;
    if (typeof ms?.setPositionState !== 'function') return;
    const duration = durationSec.value;
    const position = positionSec.value;
    const rate = playbackRate.value;
    if (!Number.isFinite(duration) || duration <= 0) return;
    if (!Number.isFinite(position) || position < 0) return;
    try {
        ms.setPositionState({
            duration,
            position: Math.min(position, duration),
            playbackRate: Number.isFinite(rate) && rate > 0 ? rate : 1,
        });
    } catch {
        // Some backends reject out-of-range combinations.
    }
}

/**
 * True after clearMediaSession: the action handlers were unregistered and
 * the next playback start must re-register them. Without this, stop()
 * killed the media keys/MPRIS until an app restart: bindEngineEvents
 * (which registers the handlers) only runs ONCE per module lifetime.
 */
let mediaSessionHandlersCleared = false;

function clearMediaSession(): void {
    if (!mediaSessionAvailable()) return;
    const ms = navigator.mediaSession;
    try {
        ms.metadata = null;
    } catch {
        // Clean degradation.
    }
    if (typeof ms?.setActionHandler !== 'function') return;
    const unset = (action: MediaSessionAction) => {
        try {
            ms.setActionHandler(action, null);
        } catch {
            // Action not supported by the backend.
        }
    };
    unset('play');
    unset('pause');
    unset('previoustrack');
    unset('nexttrack');
    unset('seekto');
    mediaSessionHandlersCleared = true;
    setMediaSessionPlaybackState('none');
}

function bindMediaSessionHandlers(): void {
    if (!mediaSessionAvailable()) return;
    const ms = navigator.mediaSession;
    if (typeof ms?.setActionHandler !== 'function') return;
    const set = (action: MediaSessionAction, handler: MediaSessionActionHandler) => {
        try {
            ms.setActionHandler(action, handler);
        } catch {
            // Action not supported by the backend.
        }
    };
    // The handlers delegate to the same player actions (same next/previous
    // rules as the UI).
    set('play', () => { void playIdle(); });
    set('pause', () => { pausePlayback(); });
    set('previoustrack', () => { void previous(); });
    set('nexttrack', () => { next(); });
    set('seekto', details => {
        if (details && typeof details.seekTime === 'number' && Number.isFinite(details.seekTime)) {
            seek(details.seekTime);
        }
    });
    mediaSessionHandlersCleared = false;
}

// ---------------------------------------------------------------------------
// Engine events (native player)
// ---------------------------------------------------------------------------

let bound = false;

function bindEngineEvents(): void {
    if (bound) return;
    bound = true;
    void onPlayerTick(tick => {
        // Stale events from an already replaced track are discarded.
        if (tick.seq !== engineSeq) return;
        const wasPlaying = isPlaying.value;
        engineState = tick.state;
        // Pausa por buffer: el motor está pausado pero la intención del usuario
        // sigue siendo reproducir. Un pause manual (isPlaying ya `false`) no se
        // revierte con ticks posteriores.
        isPlaying.value = tick.state === 'playing'
            || (tick.state === 'paused' && isBuffering.value && wasPlaying);
        positionSec.value = tick.positionSec;
        if (tick.durationSec !== null && tick.durationSec > 0) {
            durationSec.value = tick.durationSec;
        }
        updateMediaSessionPosition();
    }).catch((err: unknown) => {
        console.warn(`[player] could not subscribe to player:tick: ${messageOf(err)}`);
    });
    void onPlayerStreamState(state => {
        if (state.seq !== engineSeq) return;
        isBuffering.value = state.buffering;
        streamError.value = state.error ?? null;
        streamQuality.value = {
            serviceName: state.serviceName,
            qualityLabel: state.qualityLabel,
            qualityTier: state.qualityTier,
            matchMethod: state.matchMethod,
        };
    }).catch((err: unknown) => {
        console.warn(`[player] could not subscribe to player:stream-state: ${messageOf(err)}`);
    });
    void onPlayerEnded(seq => {
        if (seq !== engineSeq) return;
        engineState = 'stopped';
        isPlaying.value = false;
        // On natural end the standard expects 'paused'. The BACKEND decides
        // the next track now (#132): if it advances, `player:advanced`
        // reinstates the playing state; if it does not (repeat off, empty
        // queue), the track stays stopped here — the UI never advances.
        setMediaSessionPlaybackState('paused');
    }).catch((err: unknown) => {
        console.warn(`[player] could not subscribe to player:ended: ${messageOf(err)}`);
    });
    void onPlayerAdvanced(advanced => {
        syncFromBackendAdvance(advanced);
    }).catch((err: unknown) => {
        console.warn(`[player] could not subscribe to player:advanced: ${messageOf(err)}`);
    });
    void onPlayerQueueEntryChanged(syncQueueEntryChanged).catch((err: unknown) => {
        console.warn(`[player] could not subscribe to player:queue-entry-changed: ${messageOf(err)}`);
    });
    void onPlayerModesChanged(modes => {
        // P02 (#133): las acciones de MPRIS las ejecuta el backend. La UI sólo
        // proyecta el modo persistido; si shuffle reordenó los pendientes, el
        // orden nuevo viaja en el evento para no divergir del agregado.
        shuffle.value = !!modes.shuffle;
        if (isValidRepeat(modes.repeat)) repeat.value = modes.repeat;
        if (Array.isArray(modes.pendingEntryIds)) {
            const byId = new Map(queue.value.map(entry => [entry.entryId, entry]));
            const reordered = modes.pendingEntryIds
                .map(entryId => byId.get(entryId))
                .filter((entry): entry is PlaybackQueueEntry => Boolean(entry));
            if (reordered.length === queue.value.length) {
                queue.value = reordered;
                if (!modes.shuffle) baseOrder = reordered.map(entry => entry.entryId);
            }
        }
    }).catch((err: unknown) => {
        console.warn(`[player] could not subscribe to player:modes-changed: ${messageOf(err)}`);
    });
    bindMediaSessionHandlers();
}

// ---------------------------------------------------------------------------
// Playback core
// ---------------------------------------------------------------------------

function pushHistory(entry: PlaybackQueueEntry): void {
    history.value = [...history.value, { ...entry }].slice(-HISTORY_LIMIT);
}

function removePending(entryId: string): void {
    queue.value = queue.value.filter(e => e.entryId !== entryId);
    baseOrder = baseOrder.filter(id => id !== entryId);
}

/**
 * Sincroniza la proyección de la UI con un avance decidido por el backend
 * (#132): el motor ya carga la pista nueva (generación mayor), la cabeza de
 * la cola quedó consumida y la pista anterior archivada en el historial —
 * ambos los refleja aquí el agregado local. Un evento con `seq <= engineSeq`
 * es el eco de una generación ya sustituida (la UI cargó otra pista después)
 * y se descarta. Los metadatos se proyectan perezosos: la reproducción no
 * espera a la IPC de biblioteca.
 *
 * P02 (#133): en un `previous` manual la entrada avanzada sale del historial
 * y la pista que sonaba vuelve al FRENTE de los pendientes (`previousEntryId`),
 * igual que dejó el agregado el backend.
 */
async function syncFromBackendAdvance(advanced: PlayerAdvanced): Promise<void> {
    if (advanced.seq <= engineSeq) return;
    const previous = currentEntry.value;
    engineSeq = advanced.seq;
    loadedPath = advanced.trackId > 0
        ? `backend:${advanced.trackId}`
        : `backend-entry:${advanced.entryId}`;
    engineState = 'playing';
    isPlaying.value = true;
    setMediaSessionPlaybackState('playing');
    if (mediaSessionHandlersCleared) bindMediaSessionHandlers();
    isLoadingSource.value = false;
    isBuffering.value = false;
    streamQuality.value = null;
    streamError.value = null;
    error.value = null;
    positionSec.value = 0;
    durationSec.value = 0;
    if (advanced.previousEntryId) {
        history.value = history.value.filter(entry => entry.entryId !== advanced.entryId);
        if (previous && previous.entryId !== advanced.entryId) {
            queue.value = [previous, ...queue.value.filter(e => e.entryId !== previous.entryId)];
            baseOrder = [previous.entryId, ...baseOrder.filter(id => id !== previous.entryId)];
        }
    } else if (previous && previous.entryId !== advanced.entryId) {
        pushHistory(previous);
        removePending(advanced.entryId);
    }
    const nextEntry: PlaybackQueueEntry = {
        entryId: advanced.entryId,
        trackId: advanced.trackId,
        serviceId: advanced.serviceId ?? null,
        serviceTrackId: advanced.serviceTrackId ?? null,
    };
    if (isProviderEntry(nextEntry)) {
        const track = providerProjection(nextEntry);
        currentEntry.value = { ...nextEntry, track };
        updateMediaSessionMetadata(track);
        return;
    }
    currentEntry.value = nextEntry;
    try {
        const meta = await getTrackMetadata(advanced.trackId);
        if (advanced.seq !== engineSeq) return;
        const track = projectMetadata(meta);
        if (currentEntry.value?.entryId === advanced.entryId) {
            currentEntry.value = { ...currentEntry.value, track };
            updateMediaSessionMetadata(track);
        }
    } catch (err) {
        console.warn(`[player] metadata of advanced track ${advanced.trackId}: ${messageOf(err)}`);
    }
}

function preserveProjection(
    previous: PlaybackQueueEntry | undefined,
    updated: PlaybackQueueEntry,
): PlaybackQueueEntry {
    return previous?.trackId === updated.trackId && previous.track
        ? { ...updated, track: previous.track }
        : updated;
}

/** Reconcilia el delta de promoción sin reordenar ni recargar el motor. */
function syncQueueEntryChanged(dto: PlaybackQueueEntryDto): void {
    const updated = mapQueueEntry(dto);
    queue.value = queue.value.map(entry => entry.entryId === updated.entryId
        ? preserveProjection(entry, updated)
        : entry);
    history.value = history.value.map(entry => entry.entryId === updated.entryId
        ? preserveProjection(entry, updated)
        : entry);
    const previous = currentEntry.value;
    if (!previous || previous.entryId !== updated.entryId) return;
    const promoted = previous.trackId === 0 && updated.trackId > 0;
    currentEntry.value = promoted
        ? {
            ...updated,
            track: previous.track && previous.track.id > 0 ? previous.track : {
                id: updated.trackId,
                title: `Track #${updated.trackId}`,
                artist: '',
                album: null,
                coverUrl: null,
            },
        }
        : preserveProjection(previous, updated);
    if (!promoted) return;
    void getTrackMetadata(updated.trackId).then(meta => {
        const currentEntryNow = currentEntry.value;
        if (currentEntryNow?.entryId !== updated.entryId || currentEntryNow.trackId !== updated.trackId) {
            return;
        }
        const track = projectMetadata(meta);
        currentEntry.value = { ...currentEntryNow, track };
        updateMediaSessionMetadata(track);
    }).catch((err: unknown) => {
        console.warn(`[player] metadata of promoted track ${updated.trackId}: ${messageOf(err)}`);
    });
}

/**
 * Result of `ensureProjection` for a pending entry without a projection:
 * the validated track, 'dead' (ID gone → discard on the fly) or 'error'
 * (unclassifiable failure → keep the head and stop advancing).
 */
type ProjectionResult = PlayerTrack | 'dead' | 'error';

async function ensureProjection(entry: PlaybackQueueEntry): Promise<ProjectionResult> {
    if (entry.track) return entry.track;
    if (isProviderEntry(entry)) {
        entry.track = providerProjection(entry);
        return entry.track;
    }
    try {
        const meta = await getTrackMetadata(entry.trackId);
        entry.track = projectMetadata(meta);
        return entry.track;
    } catch (err) {
        return isDefinitiveLibraryError(messageOf(err)) ? 'dead' : 'error';
    }
}

/**
 * Resolves the source (always fresh: grants and URLs are never reused), loads
 * it into the native engine and starts playback. A same-file reload is safe:
 * `player_load_track` always restarts the engine from zero (never inherits an
 * element's latent 'ended' loop). Returns:
 * - 'played'  → the engine is playing; the caller commits the state.
 * - 'failed'  → resolve/load failure; `error` carries the message, state untouched.
 * - 'aborted' → the continuation became stale (stop()/new transition); it
 *               touches no state, reports no error.
 */
async function resolveAndPlay(entry: PlaybackQueueEntry, transition: Transition): Promise<'played' | 'failed' | 'aborted'> {
    const provider = isProviderEntry(entry);
    const track = provider ? providerProjection(entry) : entry.track;
    if (!track) {
        reportEngineFailure(new Error('La entrada no tiene metadatos para reproducir'));
        return 'failed';
    }
    isLoadingSource.value = true;
    streamQuality.value = null;
    streamError.value = null;
    isBuffering.value = false;
    try {
        const result = provider
            ? await playerLoadQueueEntry(entry.entryId, true, effectiveEngineVolume())
            : await playerLoadTrack(entry.trackId, true, effectiveEngineVolume());
        if (transition.aborted) return 'aborted';
        engineSeq = result.seq;
        loadedPath = provider
            ? (result.stream ? `provider-stream:${entry.entryId}` : `provider:${entry.entryId}`)
            : (result.stream ? `stream:${entry.trackId}` : `track:${entry.trackId}`);
        streamQuality.value = result.stream
            ? {
                serviceName: result.stream.serviceName,
                qualityLabel: result.stream.qualityLabel,
                qualityTier: result.stream.qualityTier,
                matchMethod: result.stream.matchMethod,
            }
            : null;
        engineState = 'playing';
        isPlaying.value = true;
        setMediaSessionPlaybackState('playing');
        positionSec.value = 0;
        durationSec.value = result.durationSec ?? 0;
        // Metadata once playback start is confirmed. If stop()
        // unregistered the MediaSession handlers, re-register them here: the
        // initial bind from bindEngineEvents does not repeat (module guard) and
        // without this re-bind PlayPause/Next/Prev via media key or D-Bus/MPRIS
        // did nothing after the first stop().
        if (mediaSessionHandlersCleared) bindMediaSessionHandlers();
        updateMediaSessionMetadata(track);
        return 'played';
    } catch (err) {
        if (transition.aborted) return 'aborted';
        reportEngineFailure(err);
        return 'failed';
    } finally {
        if (!transition.aborted) isLoadingSource.value = false;
    }
}

type AdvanceResult = 'played' | 'stopped' | 'empty';

/**
 * Advances by consuming the head of the queue. The head is NOT removed until
 * resolution and startup succeed; IDs that disappeared are discarded on the
 * fly and an unclassifiable failure keeps the head and stops advancing.
 */
async function advanceFromHead(transition: Transition): Promise<AdvanceResult> {
    while (queue.value.length > 0) {
        const entry = queue.value[0];
        const proj = await ensureProjection(entry);
        if (transition.aborted) return 'stopped';
        if (proj === 'dead') {
            removePending(entry.entryId);
            continue;
        }
        if (proj === 'error') {
            error.value = `Could not validate track ${entry.trackId} in the queue`;
            return 'stopped';
        }
        error.value = null;
        const outcome = await resolveAndPlay(entry, transition);
        if (transition.aborted) return 'stopped';
        if (outcome !== 'played') return 'stopped';
        removePending(entry.entryId);
        if (currentEntry.value) pushHistory(currentEntry.value);
        currentEntry.value = { ...entry, track: proj };
        // El agregado del backend hace la transición completa de una vez:
        // cabeza consumida, current anterior → historial y posición a 0.
        persistQueueWrite('set-current', () => playbackQueueSetCurrentEntry(entry.entryId));
        return 'played';
    }
    return 'empty';
}

/**
 * What happens when a MANUAL advance (next with an empty queue) runs out of
 * pending entries: only `repeat all` rebuilds the cycle there. The natural
 * end of a track — with every repeat mode and the shuffle order — is resolved
 * by the backend since #132; the UI never advances on `ended`.
 */
async function handleExhausted(transition: Transition): Promise<void> {
    if (repeat.value === 'all') {
        // Rebuilds the cycle from the live entries (history in playback
        // order + current). Walking a full cycle plays every entry
        // once, so keeping the LAST occurrence per entryId
        // returns the cycle order and avoids duplicates accumulated across
        // rounds. With shuffle off that order matches the base order; with
        // shuffle on the pending entries are re-shuffled.
        const cycleRaw: PlaybackQueueEntry[] = [...history.value];
        if (currentEntry.value) cycleRaw.push({ ...currentEntry.value });
        if (cycleRaw.length === 0) return;
        const lastIndexOf = new Map<string, number>();
        cycleRaw.forEach((e, i) => lastIndexOf.set(e.entryId, i));
        let pend = cycleRaw.filter((e, i) => lastIndexOf.get(e.entryId) === i);
        if (shuffle.value) pend = shuffled(pend);
        queue.value = pend.map(e => ({ ...e }));
        baseOrder = pend.map(e => e.entryId);
        history.value = [];
        await advanceFromHead(transition);
    }
    // repeat off (or manual next() with no pending entries): no-op.
}

async function advanceTrack(): Promise<void> {
    const transition = beginTransition();
    bindEngineEvents();
    const result = await advanceFromHead(transition);
    if (transition.aborted) return;
    if (result !== 'empty') return;
    await handleExhausted(transition);
}

/** Immediate playback of a specific track (the previous one moves to the history). */
async function playTrack(track: PlayerTrack): Promise<void> {
    const transition = beginTransition();
    bindEngineEvents();
    error.value = null;
    const entry: PlaybackQueueEntry = { entryId: uuidV4(), trackId: track.id, track };
    const outcome = await resolveAndPlay(entry, transition);
    if (transition.aborted) return;
    if (outcome === 'failed') {
        throw new Error(error.value ?? 'Could not play the track');
    }
    if (outcome === 'played') {
        if (currentEntry.value) pushHistory(currentEntry.value);
        currentEntry.value = entry;
        persistQueueWrite('set-current', () => playbackQueueSetCurrent(entry.entryId, track.id));
    }
}

/**
 * Resumes engine playback (shared by playIdle/toggle). The listen session is
 * owned by the backend (#132): the restored position never credits previous
 * listening time because the backend accumulator starts at 0 per load.
 */
async function resumeEngine(): Promise<void> {
    await playerResume();
    engineState = 'playing';
    isPlaying.value = true;
    setMediaSessionPlaybackState('playing');
}

/**
 * `play()` with no arguments: resume, restart the track that already ended,
 * or start the first pending entry of a restored queue.
 */
async function playIdle(): Promise<void> {
    if (loadedPath !== null) {
        if (engineState === 'playing') return;
        if (engineState === 'paused') {
            try {
                await resumeEngine();
            } catch (err) {
                reportEngineFailure(err);
            }
            return;
        }
        // Stopped with a loaded track: it reached the end; play restarts it
        // (the HTML5 element did the same: play() after 'ended' seeks to 0).
        if (currentEntry.value) {
            const transition = beginTransition();
            await resolveAndPlay(currentEntry.value, transition);
        }
        return;
    }
    if (queue.value.length > 0) {
        await advanceTrack();
    }
}

/**
 * `play(track)` plays that track; `play()` resumes or starts the first
 * pending entry (plan rule: play/MediaSession-play with current == null and a
 * restored queue starts the first pending entry; `pause` without src is a no-op and
 * `toggle` remains a no-op without src).
 */
async function play(track?: PlayerTrack): Promise<void> {
    if (track) return playTrack(track);
    return playIdle();
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Encola un track y devuelve la identidad de la entrada: el backend la
 * genera al persistir (#131); si la persistencia falla, la entrada vive solo
 * en memoria con identidad local (degradación, nunca bloquea la cola).
 */
async function enqueueEntry(track: PlayerTrack, atFront: boolean): Promise<PlaybackQueueEntry> {
    let entryId = uuidV4();
    try {
        const ids = await playbackQueueEnqueue([track.id], atFront);
        if (ids.length > 0 && ids[0]) entryId = ids[0];
    } catch (err) {
        console.warn(`[player] enqueue not persisted: ${messageOf(err)}`);
    }
    return { entryId, trackId: track.id, track };
}

async function playNext(track: PlayerTrack): Promise<QueueOutcome> {
    if (loadedPath === null) {
        await playTrack(track);
        return 'started';
    }
    const entry = await enqueueEntry(track, true);
    queue.value = [entry, ...queue.value];
    baseOrder = [entry.entryId, ...baseOrder];
    return 'queued';
}

async function enqueue(track: PlayerTrack): Promise<QueueOutcome> {
    if (loadedPath === null) {
        await playTrack(track);
        return 'started';
    }
    const entry = await enqueueEntry(track, false);
    queue.value = [...queue.value, entry];
    baseOrder = [...baseOrder, entry.entryId];
    return 'queued';
}

/**
 * Plays the first track of the list and queues the rest **in their order**
 * (fixes the inversion that chaining playNext front-inserts caused).
 * Opens a new cycle: history and base order start clean. If the first
 * track fails to resolve, the previous state stays intact and `error` carries
 * the message.
 */
async function replaceQueueAndPlay(tracks: PlayerTrack[]): Promise<void> {
    if (!Array.isArray(tracks) || tracks.length === 0) return;
    const transition = beginTransition();
    bindEngineEvents();
    error.value = null;
    const firstEntry: PlaybackQueueEntry = {
        entryId: uuidV4(),
        trackId: tracks[0].id,
        track: tracks[0],
    };
    const outcome = await resolveAndPlay(firstEntry, transition);
    if (transition.aborted) return;
    if (outcome === 'failed') {
        throw new Error(error.value ?? 'Could not play the track');
    }
    if (outcome !== 'played') return;
    // Ciclo nuevo: se piden las identidades de las pendientes AL BACKEND para
    // que las entradas locales y las persistidas compartan entry_id. El
    // orden en el backend importa: primero el current nuevo (archiva el
    // current anterior en el historial), después se vacían pendientes e
    // historial y se encola el resto. Un fallo de persistencia degrada a
    // identidades locales (la cola suena igual; solo se pierde la copia en
    // SQLite de esta reconstrucción).
    const currentEntryId = firstEntry.entryId;
    let ids: string[] | null = null;
    try {
        await playbackQueueSetCurrent(currentEntryId, tracks[0].id);
        await playbackQueueClear();
        await playbackHistoryClear();
        ids = await playbackQueueEnqueue(
            tracks.slice(1).map(t => t.id),
            false,
        );
    } catch (err) {
        console.warn(`[player] queue replacement not persisted: ${messageOf(err)}`);
    }
    const rest: PlaybackQueueEntry[] = tracks.slice(1).map((t, i) => ({
        entryId: ids && ids[i] ? ids[i] : uuidV4(),
        trackId: t.id,
        track: t,
    }));
    const ordered = shuffle.value ? shuffled(rest) : rest;
    history.value = [];
    queue.value = ordered;
    // El orden base es el PRE-permutación (apagar shuffle lo restaura); la
    // permutación vigente es la que se persiste en el backend.
    baseOrder = rest.map(e => e.entryId);
    currentEntry.value = { entryId: currentEntryId, trackId: tracks[0].id, track: tracks[0] };
    if (ids !== null && shuffle.value) {
        persistQueueWrite('reorder-shuffle', () => playbackQueueReorder(ordered.map(e => e.entryId)));
    }
}

function clearQueue(): void {
    // Pending entries only: the current track is never touched.
    queue.value = [];
    baseOrder = [];
    persistQueueWrite('clear', () => playbackQueueClear());
}

function removeEntry(entryId: string): void {
    removePending(entryId);
    persistQueueWrite('remove', () => playbackQueueRemove(entryId));
}

function moveEntry(entryId: string, toIndex: number): void {
    const from = queue.value.findIndex(e => e.entryId === entryId);
    if (from === -1) return;
    const clamped = Math.max(0, Math.min(Math.trunc(toIndex), queue.value.length - 1));
    const next = [...queue.value];
    const [moved] = next.splice(from, 1);
    next.splice(clamped, 0, moved);
    queue.value = next;
    // Manual reordering sets the new base (turning shuffle off respects it).
    baseOrder = next.map(e => e.entryId);
    persistQueueWrite('reorder', () => playbackQueueReorder(next.map(e => e.entryId)));
}

function next(): void {
    // next() ignores repeat-one: it jumps to the pending entries; with the queue exhausted,
    // repeat 'all' rebuilds and plays, and 'off'/'one' are no-ops.
    void advanceTrack();
}

async function previous(): Promise<void> {
    bindEngineEvents();
    if (currentEntry.value && positionSec.value > 3) {
        seek(0);
        return;
    }
    const transition = beginTransition();
    while (history.value.length > 0) {
        const entry = history.value[history.value.length - 1];
        history.value = history.value.slice(0, -1);
        const proj = await ensureProjection(entry);
        if (transition.aborted) return;
        if (proj === 'dead') continue;
        if (proj === 'error') {
            error.value = `Could not validate track ${entry.trackId} in the history`;
            return;
        }
        error.value = null;
        const outcome = await resolveAndPlay(entry, transition);
        if (transition.aborted) return;
        if (outcome !== 'played') return;
        // The track left behind returns to the front of the pending entries.
        if (currentEntry.value) {
            const back = currentEntry.value;
            queue.value = [back, ...queue.value];
            baseOrder = [back.entryId, ...baseOrder.filter(id => id !== back.entryId)];
        }
        currentEntry.value = { ...entry, track: proj };
        persistQueueWrite('set-current', () => playbackQueueSetCurrentEntry(entry.entryId));
        return;
    }
    // No history: seek(0).
    seek(0);
}

function toggle(): void {
    if (loadedPath === null) return;
    if (engineState === 'playing') {
        pausePlayback();
        return;
    }
    // Paused: resume. Stopped (ended): restart. playIdle handles both.
    void playIdle();
}

function pausePlayback(): void {
    // Only when actually playing: after 'ended' this was a no-op too.
    if (engineState !== 'playing') return;
    engineState = 'paused';
    isPlaying.value = false;
    setMediaSessionPlaybackState('paused');
    void playerPause()
        .then(() => {
            persistQueueWrite('position', () => playbackStateSetPosition(positionSec.value));
        })
        .catch((err: unknown) => {
            // A failed pause is corrected by the next tick (ticks self-heal state).
            reportEngineFailure(err);
        });
}

function stop(): void {
    // Aborta la transición en curso (las cargas que lleguen tarde no tocan el
    // estado); la siguiente acción abre una transición nueva.
    if (activeTransition) activeTransition.aborted = true;
    activeTransition = null;
    void playerStop().catch(() => {
        // Without a loaded track it is a no-op; IPC failure is unrecoverable anyway.
    });
    loadedPath = null;
    engineSeq = 0;
    engineState = 'stopped';
    isPlaying.value = false;
    currentEntry.value = null;
    positionSec.value = 0;
    durationSec.value = 0;
    error.value = null;
    isBuffering.value = false;
    streamQuality.value = null;
    streamError.value = null;
    queue.value = [];
    baseOrder = [];
    history.value = [];
    clearMediaSession();
    isLoadingSource.value = false;
    persistQueueWrite('reset', () => playbackStateReset());
}

function seek(sec: number): void {
    if (!Number.isFinite(sec)) return;
    const max = durationSec.value > 0 ? durationSec.value : Number.MAX_SAFE_INTEGER;
    const clamped = Math.min(Math.max(0, sec), max);
    positionSec.value = clamped;
    updateMediaSessionPosition();
    // Seek persistence is explicit here: the engine emits no 'seeked' event.
    void playerSeek(clamped)
        .then(() => {
            persistQueueWrite('position', () => playbackStateSetPosition(clamped));
        })
        .catch((err: unknown) => {
            reportEngineFailure(err);
        });
}

function setRate(rate: number): void {
    if (!Number.isFinite(rate) || rate <= 0) return;
    playbackRate.value = rate;
    void playerSetRate(rate).catch((err: unknown) => {
        reportEngineFailure(err);
    });
}

function setVolume(v: number): void {
    volume.value = clampVolume(v);
    void playerSetVolume(effectiveEngineVolume()).catch(() => {
        // Before the first load there is no engine yet; the load applies the volume.
    });
    persistQueueWrite('volume', () => playbackStateSetVolume(volume.value, muted.value));
}

function setMuted(m: boolean): void {
    muted.value = !!m;
    void playerSetVolume(effectiveEngineVolume()).catch(() => {
        // Before the first load there is no engine yet; the load applies the volume.
    });
    persistQueueWrite('volume', () => playbackStateSetVolume(volume.value, muted.value));
}

function toggleShuffle(): void {
    if (shuffle.value) {
        // Turning shuffle off restores the base order of the pending entries: the
        // shuffle is not kept.
        shuffle.value = false;
        const byId = new Map(queue.value.map(e => [e.entryId, e]));
        const restored: PlaybackQueueEntry[] = [];
        for (const id of baseOrder) {
            const e = byId.get(id);
            if (e) restored.push(e);
        }
        queue.value = restored;
        persistQueueWrite('reorder', () => playbackQueueReorder(restored.map(e => e.entryId)));
    } else {
        // Turning shuffle on permutes the pending entries; the pre-shuffle base order
        // is kept so it can be restored. The permuted order is what persists: the
        // backend stores the queue as it plays now.
        shuffle.value = true;
        queue.value = shuffled(queue.value);
        persistQueueWrite('reorder', () => playbackQueueReorder(queue.value.map(e => e.entryId)));
    }
    persistQueueWrite('modes', () => playbackStateSetModes(shuffle.value, repeat.value));
}

function cycleRepeat(): void {
    repeat.value = repeat.value === 'off' ? 'all' : repeat.value === 'all' ? 'one' : 'off';
    persistQueueWrite('modes', () => playbackStateSetModes(shuffle.value, repeat.value));
}

// ---------------------------------------------------------------------------
// Lazy restore after a restart
// ---------------------------------------------------------------------------

let restoreStarted = false;

/**
 * Restores the persisted state from the BACKEND (SQLite, #131). It is only
 * invoked explicitly (the status bar player on mount); importing the module
 * fires no invokes and no playback. As part of the migration the old
 * localStorage key `syncify.playerState.v1` is discarded without migration.
 * Resolves the source again by entry identity (never reuses paths or stream
 * URLs) and loads it into the engine PAUSED at the saved position:
 * reading the saved state plays nothing and the restored position does not
 * credit previous listening time.
 */
async function restoreFromPersistence(): Promise<void> {
    if (restoreStarted) return;
    restoreStarted = true;
    bindEngineEvents();
    // Descarte del esquema antiguo de reproducción en localStorage: no se
    // migra nada, la fuente de verdad pasa a ser la BD del backend.
    try {
        localStorage.removeItem(LEGACY_STORAGE_KEY);
    } catch {
        // Storage unavailable: nothing to clean.
    }
    const snap = await Promise.resolve(getPlaybackQueueSnapshot()).catch((err: unknown) => {
        console.warn(`[player] queue restore failed: ${messageOf(err)}`);
        return null;
    });
    if (!snap || typeof snap !== 'object') return;

    // Una instantánea malformada (lista ausente) no revienta el arranque: se
    // reponen las listas vacías y el resto de campos con defaults seguros.
    const pending = Array.isArray(snap.pending) ? snap.pending : [];
    const historyRows = Array.isArray(snap.history) ? snap.history : [];
    shuffle.value = !!snap.shuffle;
    repeat.value = isValidRepeat(snap.repeat) ? snap.repeat : 'off';
    volume.value = clampVolume(snap.volume);
    muted.value = !!snap.muted;
    queue.value = pending.map(mapQueueEntry);
    baseOrder = pending.map(e => e.entry_id);
    history.value = historyRows.map(mapQueueEntry);

    if (!snap.current) return;

    // Lazy N×1 validation: only a library current track is resolved on restore;
    // pending entries are validated just before playing.
    const transition = beginTransition();
    isLoadingSource.value = true;
    try {
        const restoredEntry = mapQueueEntry(snap.current);
        const provider = isProviderEntry(restoredEntry);
        if (!provider) {
            const meta = await getTrackMetadata(restoredEntry.trackId);
            if (transition.aborted) return;
            restoredEntry.track = projectMetadata(meta);
        }
        const result = provider
            ? await playerLoadQueueEntry(restoredEntry.entryId, false, effectiveEngineVolume())
            : await playerLoadTrack(restoredEntry.trackId, false, effectiveEngineVolume());
        if (transition.aborted) return;
        engineSeq = result.seq;
        loadedPath = provider
            ? (result.stream ? `provider-stream:${restoredEntry.entryId}` : `provider:${restoredEntry.entryId}`)
            : (result.stream ? `stream:${restoredEntry.trackId}` : `track:${restoredEntry.trackId}`);
        streamQuality.value = result.stream
            ? {
                serviceName: result.stream.serviceName,
                qualityLabel: result.stream.qualityLabel,
                qualityTier: result.stream.qualityTier,
                matchMethod: result.stream.matchMethod,
            }
            : null;
        streamError.value = null;
        isBuffering.value = false;
        engineState = 'paused';
        const savedPos = Number.isFinite(snap.position_sec) && snap.position_sec >= 0 ? snap.position_sec : 0;
        const maxPos = result.durationSec !== null && result.durationSec > 0 ? result.durationSec : savedPos;
        const pos = Math.min(savedPos, maxPos);
        if (pos > 0) {
            await playerSeek(pos);
            if (transition.aborted) return;
        }
        positionSec.value = pos;
        durationSec.value = result.durationSec ?? 0;
        // Se conserva la identidad persistida de la pista actual.
        currentEntry.value = restoredEntry;
    } catch {
        if (transition.aborted) return;
        // Track gone or without a local file: start with no current; the
        // restored queue is kept for the first play().
        currentEntry.value = null;
    } finally {
        if (!transition.aborted) isLoadingSource.value = false;
    }
}

// Position persistence when the window is hidden (not on every timeupdate).
if (typeof document !== 'undefined') {
    document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'hidden') {
            persistQueueWrite('position', () => playbackStateSetPosition(positionSec.value));
        }
    });
}

export function usePlayer() {
    return {
        // Previous state (surface unchanged)
        current, isPlaying, positionSec, durationSec, playbackRate, error, isLoadingSource,
        isBuffering, streamQuality, streamError,
        upNext,
        // New state (phase 1)
        queue, history, shuffle, repeat, volume, muted,
        // Previous actions
        play, playNext, toggle, stop, seek, setRate,
        // New actions (phase 1)
        enqueue, replaceQueueAndPlay, clearQueue, removeEntry, moveEntry,
        next, previous, setVolume, setMuted, toggleShuffle, cycleRepeat,
        restoreFromPersistence,
        // PL-09 / PL-10: Fullscreen / expanded player
        isFullscreenOpen, openFullscreen, closeFullscreen, toggleFullscreen,
    };
}

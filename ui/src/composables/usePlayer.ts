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
 * Phase rules:
 * - Restoring after a restart ALWAYS resolves again with
 *   `player_load_track`: localStorage never stores `file_path`, the full
 *   track DTO, or engine state. The engine loads the track paused at the
 *   saved position.
 * - Listen crediting uses 30 s of real playback accumulated per
 *   tick; never `position >= 30`. The restored position does not credit
 *   previous time.
 * - Importing this module fires no invokes and no playback: restoration
 *   is lazy (`restoreFromPersistence`) and the status bar player invokes it on mount.
 * - Engine events carry the load `seq`: everything from an already replaced
 *   track is discarded.
 */
import { computed, ref } from 'vue'
import { getTrackMetadata, recordLocalPlay, type TrackMetadataWithSources } from '../api/library'
import {
    onMprisAction,
    onPlayerEnded,
    onPlayerStreamState,
    onPlayerTick,
    playerLoadTrack,
    playerMprisUpdate,
    playerPause,
    playerResume,
    playerSeek,
    playerSetRate,
    playerSetVolume,
    playerStop,
    type PlayerEngineState,
    type PlayerStreamQuality,
} from '../api/player'

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
    trackId: number;
    track?: PlayerTrack;
}

/** Listen pending confirmation from the backend. */
export interface PendingListen {
    trackId: number;
    listenSessionId: string;
    attempts: number;
}

// ---------------------------------------------------------------------------
// Constants and utilities
// ---------------------------------------------------------------------------

export const LISTEN_THRESHOLD_SEC = 30;
const LISTEN_MAX_ATTEMPTS = 5;
const LISTEN_BACKOFF_BASE_MS = 1000;
const LISTEN_BACKOFF_CAP_MS = 16000;
const HISTORY_LIMIT = 100;
const STORAGE_KEY = 'syncify.playerState.v1';
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

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

function isCanonicalUuid(v: unknown): v is string {
    return typeof v === 'string' && UUID_RE.test(v);
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
const pendingListens = ref<PendingListen[]>([]);

/** Pre-shuffle base order of the pending entries (entryIds). Not reactive: the mutations keep it in sync. */
let baseOrder: string[] = [];
/** Transition generation: every async continuation checks that its gen is still alive. */
let transitionGen = 0;

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
    queue.value.map(e => e.track ?? {
        id: e.trackId,
        title: `Track #${e.trackId}`,
        artist: '',
        album: null,
        coverUrl: null,
    }),
);

// ---------------------------------------------------------------------------
// Persistence (localStorage, single versioned key)
// ---------------------------------------------------------------------------

interface PersistedQueueEntry { entryId: string; trackId: number }

interface PersistedPlayerState {
    version: 1;
    queue: PersistedQueueEntry[];
    baseOrder: string[];
    history: PersistedQueueEntry[];
    currentTrackId: number | null;
    positionSec: number;
    shuffle: boolean;
    repeat: RepeatMode;
    volume: number;
    muted: boolean;
    pendingListens: PendingListen[];
}

function persistState(): void {
    try {
        const state: PersistedPlayerState = {
            version: 1,
            queue: queue.value.map(({ entryId, trackId }) => ({ entryId, trackId })),
            baseOrder: [...baseOrder],
            history: history.value.map(({ entryId, trackId }) => ({ entryId, trackId })),
            currentTrackId: current.value?.id ?? null,
            positionSec: Number.isFinite(positionSec.value) && positionSec.value >= 0 ? positionSec.value : 0,
            shuffle: shuffle.value,
            repeat: repeat.value,
            volume: clampVolume(volume.value),
            muted: muted.value,
            pendingListens: pendingListens.value.map(p => ({ ...p })),
        };
        localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch {
        // Quota/storage unavailable: playback continues without persistence.
    }
}

function validatePersisted(v: unknown): PersistedPlayerState | null {
    if (typeof v !== 'object' || v === null) return null;
    const rec = v as Record<string, unknown>;
    // Unknown version or invalid structure → discard everything, start clean.
    if (rec.version !== 1) return null;

    const parseEntry = (e: unknown): PersistedQueueEntry | null => {
        if (typeof e !== 'object' || e === null) return null;
        const r = e as Record<string, unknown>;
        if (typeof r.entryId !== 'string' || r.entryId.length === 0) return null;
        if (typeof r.trackId !== 'number' || !Number.isInteger(r.trackId) || r.trackId <= 0) return null;
        return { entryId: r.entryId, trackId: r.trackId };
    };
    const parseEntries = (x: unknown): PersistedQueueEntry[] | null => {
        if (!Array.isArray(x)) return null;
        const out: PersistedQueueEntry[] = [];
        for (const e of x) {
            const q = parseEntry(e);
            if (!q) return null;
            out.push(q);
        }
        return out;
    };

    const persistedQueue = parseEntries(rec.queue);
    if (!persistedQueue) return null;
    const persistedHistory = parseEntries(rec.history);
    if (!persistedHistory) return null;
    if (!Array.isArray(rec.baseOrder) || !rec.baseOrder.every(id => typeof id === 'string' && id.length > 0)) return null;

    let currentTrackId: number | null = null;
    if (rec.currentTrackId !== null && rec.currentTrackId !== undefined) {
        if (typeof rec.currentTrackId !== 'number' || !Number.isInteger(rec.currentTrackId) || rec.currentTrackId <= 0) return null;
        currentTrackId = rec.currentTrackId;
    }
    if (typeof rec.positionSec !== 'number' || !Number.isFinite(rec.positionSec) || rec.positionSec < 0) return null;
    if (typeof rec.shuffle !== 'boolean') return null;
    if (rec.repeat !== 'off' && rec.repeat !== 'all' && rec.repeat !== 'one') return null;
    if (typeof rec.volume !== 'number' || !Number.isFinite(rec.volume) || rec.volume < 0 || rec.volume > 1) return null;
    if (typeof rec.muted !== 'boolean') return null;
    if (!Array.isArray(rec.pendingListens)) return null;

    // Pending listens: each corrupt row (non-UUID / invalid trackId) is
    // discarded individually; the rest of the state is kept.
    const listens: PendingListen[] = [];
    for (const item of rec.pendingListens) {
        if (typeof item !== 'object' || item === null) continue;
        const r = item as Record<string, unknown>;
        const sessionId = typeof r.listenSessionId === 'string' ? r.listenSessionId.toLowerCase() : null;
        if (!isCanonicalUuid(sessionId)) continue;
        if (typeof r.trackId !== 'number' || !Number.isInteger(r.trackId) || r.trackId <= 0) continue;
        const attempts = typeof r.attempts === 'number' && Number.isInteger(r.attempts) && r.attempts >= 0 ? r.attempts : 0;
        listens.push({ trackId: r.trackId, listenSessionId: sessionId, attempts });
    }

    return {
        version: 1,
        queue: persistedQueue,
        baseOrder: rec.baseOrder as string[],
        history: persistedHistory,
        currentTrackId,
        positionSec: rec.positionSec,
        shuffle: rec.shuffle,
        repeat: rec.repeat,
        volume: rec.volume,
        muted: rec.muted,
        pendingListens: listens,
    };
}

function loadPersistedState(): PersistedPlayerState | null {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        if (!raw) return null;
        return validatePersisted(JSON.parse(raw));
    } catch {
        return null;
    }
}

function stripEntry(e: PlaybackQueueEntry): PersistedQueueEntry {
    return { entryId: e.entryId, trackId: e.trackId };
}

// ---------------------------------------------------------------------------
// Listen crediting (30 real seconds per tick)
// ---------------------------------------------------------------------------

let listenSessionId: string | null = null;
let listenTrackId: number | null = null;
let listenAccumSec = 0;
/** One pending entry per listen session: it is created only once when the threshold is crossed. */
let listenScheduled = false;
let tickWallMs: number | null = null;
let tickMediaSec: number | null = null;

function beginListenSession(trackId: number): void {
    listenSessionId = uuidV4();
    listenTrackId = trackId;
    listenAccumSec = 0;
    listenScheduled = false;
    tickWallMs = null;
    tickMediaSec = null;
}

function resetListenSession(): void {
    listenSessionId = null;
    listenTrackId = null;
    listenAccumSec = 0;
    listenScheduled = false;
    tickWallMs = null;
    tickMediaSec = null;
}

function creditListenTick(): void {
    if (!listenSessionId || listenTrackId === null || current.value === null) return;
    const eligible = engineState === 'playing';
    if (!eligible) {
        // Pause/buffering: the tick does not credit; the accumulated time is kept
        // and the baseline re-anchors on the next eligible tick.
        tickWallMs = null;
        tickMediaSec = null;
        return;
    }
    const now = Date.now();
    if (tickWallMs !== null && tickMediaSec !== null) {
        const dtWall = (now - tickWallMs) / 1000;
        const dtMedia = positionSec.value - tickMediaSec;
        const rate = Number.isFinite(playbackRate.value) && playbackRate.value > 0 ? playbackRate.value : 1;
        if (Number.isFinite(dtWall) && Number.isFinite(dtMedia) && dtWall >= 0 && dtMedia >= 0) {
            listenAccumSec += Math.min(dtWall, dtMedia / rate);
        }
        // Invalid delta (seek, currentTime jump): the tick does not credit but
        // does not wipe the accumulated time either; the re-anchor below keeps it from dragging on.
    }
    tickWallMs = now;
    tickMediaSec = positionSec.value;
    if (!listenScheduled && listenAccumSec >= LISTEN_THRESHOLD_SEC) {
        listenScheduled = true;
        scheduleListen(listenTrackId, listenSessionId);
    }
}

// ---------------------------------------------------------------------------
// Pending listens: single send per session, capped retries
// ---------------------------------------------------------------------------

const listenInFlight = new Set<string>();
let listenRetryTimer: ReturnType<typeof setTimeout> | null = null;

function discardListen(p: PendingListen): void {
    pendingListens.value = pendingListens.value.filter(x => x.listenSessionId !== p.listenSessionId);
    persistState();
}

function scheduleRetry(): void {
    if (listenRetryTimer !== null) return;
    const retryable = pendingListens.value.filter(p => p.attempts < LISTEN_MAX_ATTEMPTS);
    if (retryable.length === 0) return;
    const minAttempts = Math.min(...retryable.map(p => p.attempts));
    const delayMs = Math.min(LISTEN_BACKOFF_CAP_MS, LISTEN_BACKOFF_BASE_MS * 2 ** Math.max(0, minAttempts - 1));
    listenRetryTimer = setTimeout(() => {
        listenRetryTimer = null;
        void flushPendingListens();
    }, delayMs);
}

function scheduleListen(trackId: number, sessionId: string): void {
    pendingListens.value = [...pendingListens.value, { trackId, listenSessionId: sessionId, attempts: 0 }];
    persistState();
    void flushPendingListens();
}

async function flushPendingListens(): Promise<void> {
    for (const p of [...pendingListens.value]) {
        if (listenInFlight.has(p.listenSessionId)) continue;
        if (p.attempts >= LISTEN_MAX_ATTEMPTS) {
            // Cap of 5 attempts reached: it is discarded and logged.
            console.warn(`[player] discarding listen for track ${p.trackId} after ${p.attempts} attempts`);
            discardListen(p);
            continue;
        }
        listenInFlight.add(p.listenSessionId);
        p.attempts += 1;
        persistState();
        try {
            await recordLocalPlay(p.trackId, p.listenSessionId);
            pendingListens.value = pendingListens.value.filter(x => x.listenSessionId !== p.listenSessionId);
            persistState();
        } catch (err) {
            const msg = messageOf(err);
            console.warn(`[player] record_local_play failed (attempt ${p.attempts}) for track ${p.trackId}: ${msg}`);
            // Definitive (deleted track/invalid UUID) or cap of 5 attempts: discard.
            if (isDefinitiveLibraryError(msg) || p.attempts >= LISTEN_MAX_ATTEMPTS) discardListen(p);
            // Transient below the cap: keep the pending entry; scheduleRetry programs the backoff.
        } finally {
            listenInFlight.delete(p.listenSessionId);
        }
    }
    scheduleRetry();
}

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
function syncMprisState(track: PlayerTrack | null = current.value): void {
    void playerMprisUpdate({
        trackId: track?.id ?? null,
        title: track?.title ?? null,
        artist: track?.artist ?? null,
        album: track?.album ?? null,
        durationSec: durationSec.value > 0 ? durationSec.value : null,
        artUrl: track?.coverUrl ?? null,
        shuffle: shuffle.value,
        loopStatus: repeat.value === 'one' ? 'Track' : repeat.value === 'all' ? 'Playlist' : 'None',
    }).catch(() => {
        // MPRIS es opcional (y sólo Linux): jamás escala a la UI.
    });
}

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
        creditListenTick();
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
        };
    }).catch((err: unknown) => {
        console.warn(`[player] could not subscribe to player:stream-state: ${messageOf(err)}`);
    });
    void onPlayerEnded(seq => {
        if (seq !== engineSeq) return;
        engineState = 'stopped';
        isPlaying.value = false;
        // On natural end the standard expects 'paused'; if playback advances,
        // the next 'playing' tick returns it to 'playing'.
        setMediaSessionPlaybackState('paused');
        void advanceTrack(true);
    }).catch((err: unknown) => {
        console.warn(`[player] could not subscribe to player:ended: ${messageOf(err)}`);
    });
    void onMprisAction(payload => {
        // Misma delegación que los handlers de MediaSession: la UI es la dueña
        // de la cola, los modos y el crédito de escucha.
        switch (payload.action) {
            case 'play':
                void playIdle();
                break;
            case 'pause':
                pausePlayback();
                break;
            case 'playPause':
                toggle();
                break;
            case 'next':
                next();
                break;
            case 'previous':
                void previous();
                break;
            case 'stop':
                stop();
                break;
            case 'seek':
                if (typeof payload.offsetUs === 'number' && Number.isFinite(payload.offsetUs)) {
                    seek(positionSec.value + payload.offsetUs / 1_000_000);
                }
                break;
            case 'setPosition':
                if (typeof payload.positionUs === 'number' && Number.isFinite(payload.positionUs)) {
                    seek(payload.positionUs / 1_000_000);
                }
                break;
        }
    }).catch((err: unknown) => {
        console.warn(`[player] could not subscribe to player:mpris-action: ${messageOf(err)}`);
    });
    bindMediaSessionHandlers();
}

// ---------------------------------------------------------------------------
// Playback core
// ---------------------------------------------------------------------------

function pushHistory(entry: PlaybackQueueEntry): void {
    history.value = [...history.value, stripEntry(entry)].slice(-HISTORY_LIMIT);
}

function removePending(entryId: string): void {
    queue.value = queue.value.filter(e => e.entryId !== entryId);
    baseOrder = baseOrder.filter(id => id !== entryId);
}

/**
 * Result of `ensureProjection` for a pending entry without a projection:
 * the validated track, 'dead' (ID gone → discard on the fly) or 'error'
 * (unclassifiable failure → keep the head and stop advancing).
 */
type ProjectionResult = PlayerTrack | 'dead' | 'error';

async function ensureProjection(entry: PlaybackQueueEntry): Promise<ProjectionResult> {
    if (entry.track) return entry.track;
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
 * `player_load` always restarts the engine from zero (never inherits an
 * element's latent 'ended' loop). Returns:
 * - 'played'  → the engine is playing; the caller commits the state.
 * - 'failed'  → resolve/load failure; `error` carries the message, state untouched.
 * - 'aborted' → the continuation became stale (stop()/new transition); it
 *               touches no state, reports no error.
 */
async function resolveAndPlay(track: PlayerTrack, gen: number): Promise<'played' | 'failed' | 'aborted'> {
    isLoadingSource.value = true;
    streamQuality.value = null;
    streamError.value = null;
    isBuffering.value = false;
    try {
        const result = await playerLoadTrack(track.id, true, effectiveEngineVolume());
        if (gen !== transitionGen) return 'aborted';
        engineSeq = result.seq;
        loadedPath = result.stream ? `stream:${track.id}` : `track:${track.id}`;
        streamQuality.value = result.stream
            ? {
                serviceName: result.stream.serviceName,
                qualityLabel: result.stream.qualityLabel,
                qualityTier: result.stream.qualityTier,
            }
            : null;
        engineState = 'playing';
        isPlaying.value = true;
        setMediaSessionPlaybackState('playing');
        positionSec.value = 0;
        durationSec.value = result.durationSec ?? 0;
        beginListenSession(track.id);
        // Metadata once playback start is confirmed. If stop()
        // unregistered the MediaSession handlers, re-register them here: the
        // initial bind from bindEngineEvents does not repeat (module guard) and
        // without this re-bind PlayPause/Next/Prev via media key or D-Bus/MPRIS
        // did nothing after the first stop().
        if (mediaSessionHandlersCleared) bindMediaSessionHandlers();
        updateMediaSessionMetadata(track);
        syncMprisState(track);
        return 'played';
    } catch (err) {
        if (gen !== transitionGen) return 'aborted';
        reportEngineFailure(err);
        return 'failed';
    } finally {
        if (gen === transitionGen) isLoadingSource.value = false;
    }
}

type AdvanceResult = 'played' | 'stopped' | 'empty';

/**
 * Advances by consuming the head of the queue. The head is NOT removed until
 * resolution and startup succeed; IDs that disappeared are discarded on the
 * fly and an unclassifiable failure keeps the head and stops advancing.
 */
async function advanceFromHead(gen: number): Promise<AdvanceResult> {
    while (queue.value.length > 0) {
        const entry = queue.value[0];
        const proj = await ensureProjection(entry);
        if (gen !== transitionGen) return 'stopped';
        if (proj === 'dead') {
            removePending(entry.entryId);
            continue;
        }
        if (proj === 'error') {
            error.value = `Could not validate track ${entry.trackId} in the queue`;
            return 'stopped';
        }
        error.value = null;
        const outcome = await resolveAndPlay(proj, gen);
        if (gen !== transitionGen) return 'stopped';
        if (outcome !== 'played') return 'stopped';
        removePending(entry.entryId);
        if (currentEntry.value) pushHistory(currentEntry.value);
        currentEntry.value = { ...entry, track: proj };
        persistState();
        return 'played';
    }
    return 'empty';
}

/**
 * What happens when the queue runs out (the only role of `repeat`): the queue always
 * advances on `ended` while there are pending entries, in any mode.
 */
async function handleExhausted(auto: boolean, gen: number): Promise<void> {
    if (auto && repeat.value === 'one' && currentEntry.value && current.value) {
        // Repeat-one loop: new listen token and currentTime = 0; it does not
        // duplicate the history (it is the same track continuing).
        await resolveAndPlay(current.value, gen);
        return;
    }
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
        await advanceFromHead(gen);
    }
    // repeat off (or manual next() with no pending entries): no-op.
}

async function advanceTrack(auto: boolean): Promise<void> {
    const gen = ++transitionGen;
    bindEngineEvents();
    const result = await advanceFromHead(gen);
    if (gen !== transitionGen) return;
    if (result !== 'empty') return;
    await handleExhausted(auto, gen);
}

/** Immediate playback of a specific track (the previous one moves to the history). */
async function playTrack(track: PlayerTrack): Promise<void> {
    const gen = ++transitionGen;
    bindEngineEvents();
    error.value = null;
    const outcome = await resolveAndPlay(track, gen);
    if (gen !== transitionGen) return;
    if (outcome === 'failed') {
        throw new Error(error.value ?? 'Could not play the track');
    }
    if (outcome === 'played') {
        if (currentEntry.value) pushHistory(currentEntry.value);
        currentEntry.value = { entryId: uuidV4(), trackId: track.id, track };
        persistState();
    }
}

/**
 * Resumes engine playback (shared by playIdle/toggle). Resuming after a
 * restore starts the listen session here: the restored position does not
 * credit previous listening time.
 */
async function resumeEngine(): Promise<void> {
    await playerResume();
    engineState = 'playing';
    isPlaying.value = true;
    setMediaSessionPlaybackState('playing');
    if (currentEntry.value && !listenSessionId) {
        beginListenSession(currentEntry.value.trackId);
    }
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
        if (current.value) {
            const gen = ++transitionGen;
            await resolveAndPlay(current.value, gen);
        }
        return;
    }
    if (queue.value.length > 0) {
        await advanceTrack(false);
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

async function playNext(track: PlayerTrack): Promise<QueueOutcome> {
    if (loadedPath === null) {
        await playTrack(track);
        return 'started';
    }
    const entry: PlaybackQueueEntry = { entryId: uuidV4(), trackId: track.id, track };
    queue.value = [entry, ...queue.value];
    baseOrder = [entry.entryId, ...baseOrder];
    persistState();
    return 'queued';
}

async function enqueue(track: PlayerTrack): Promise<QueueOutcome> {
    if (loadedPath === null) {
        await playTrack(track);
        return 'started';
    }
    const entry: PlaybackQueueEntry = { entryId: uuidV4(), trackId: track.id, track };
    queue.value = [...queue.value, entry];
    baseOrder = [...baseOrder, entry.entryId];
    persistState();
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
    const gen = ++transitionGen;
    bindEngineEvents();
    error.value = null;
    const outcome = await resolveAndPlay(tracks[0], gen);
    if (gen !== transitionGen) return;
    if (outcome === 'failed') {
        throw new Error(error.value ?? 'Could not play the track');
    }
    if (outcome !== 'played') return;
    const entries: PlaybackQueueEntry[] = tracks.map(t => ({ entryId: uuidV4(), trackId: t.id, track: t }));
    const rest = entries.slice(1);
    history.value = [];
    queue.value = shuffle.value ? shuffled(rest) : rest;
    baseOrder = rest.map(e => e.entryId);
    currentEntry.value = entries[0];
    persistState();
}

function clearQueue(): void {
    // Pending entries only: the current track is never touched.
    queue.value = [];
    baseOrder = [];
    persistState();
}

function removeEntry(entryId: string): void {
    removePending(entryId);
    persistState();
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
    persistState();
}

function next(): void {
    // next() ignores repeat-one: it jumps to the pending entries; with the queue exhausted,
    // repeat 'all' rebuilds and plays, and 'off'/'one' are no-ops.
    void advanceTrack(false);
}

async function previous(): Promise<void> {
    bindEngineEvents();
    if (current.value && positionSec.value > 3) {
        seek(0);
        return;
    }
    const gen = ++transitionGen;
    while (history.value.length > 0) {
        const entry = history.value[history.value.length - 1];
        history.value = history.value.slice(0, -1);
        const proj = await ensureProjection(entry);
        if (gen !== transitionGen) return;
        if (proj === 'dead') continue;
        if (proj === 'error') {
            error.value = `Could not validate track ${entry.trackId} in the history`;
            return;
        }
        error.value = null;
        const outcome = await resolveAndPlay(proj, gen);
        if (gen !== transitionGen) return;
        if (outcome !== 'played') return;
        // The track left behind returns to the front of the pending entries.
        if (currentEntry.value) {
            const back = currentEntry.value;
            queue.value = [back, ...queue.value];
            baseOrder = [back.entryId, ...baseOrder.filter(id => id !== back.entryId)];
        }
        currentEntry.value = { ...entry, track: proj };
        persistState();
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
            persistState();
        })
        .catch((err: unknown) => {
            // A failed pause is corrected by the next tick (ticks self-heal state).
            reportEngineFailure(err);
        });
}

function stop(): void {
    transitionGen++;
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
    resetListenSession();
    clearMediaSession();
    syncMprisState();
    isLoadingSource.value = false;
    persistState();
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
            persistState();
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
    persistState();
}

function setMuted(m: boolean): void {
    muted.value = !!m;
    void playerSetVolume(effectiveEngineVolume()).catch(() => {
        // Before the first load there is no engine yet; the load applies the volume.
    });
    persistState();
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
    } else {
        // Turning shuffle on permutes the pending entries; the pre-shuffle base order
        // is kept so it can be restored and persisted alongside the permutation.
        shuffle.value = true;
        queue.value = shuffled(queue.value);
    }
    persistState();
    syncMprisState();
}

function cycleRepeat(): void {
    repeat.value = repeat.value === 'off' ? 'all' : repeat.value === 'all' ? 'one' : 'off';
    persistState();
    syncMprisState();
}

// ---------------------------------------------------------------------------
// Lazy restore after a restart
// ---------------------------------------------------------------------------

let restoreStarted = false;

/**
 * Restores the persisted state. It is only invoked explicitly (the status bar player
 * on mount); importing the module touches no localStorage and fires no invokes.
 * Resolves the source again with `player_load_track` (never reuses paths or
 * stream URLs) and loads it into the engine PAUSED at the saved position:
 * reading the saved state plays nothing and the restored position does not
 * credit previous listening time.
 */
async function restoreFromPersistence(): Promise<void> {
    if (restoreStarted) return;
    restoreStarted = true;
    bindEngineEvents();
    const saved = loadPersistedState();
    if (!saved) return;

    shuffle.value = saved.shuffle;
    repeat.value = saved.repeat;
    volume.value = clampVolume(saved.volume);
    muted.value = saved.muted;
    pendingListens.value = saved.pendingListens;
    queue.value = saved.queue.map(e => ({ ...e }));
    baseOrder = [...saved.baseOrder];
    history.value = saved.history.map(e => ({ ...e }));
    if (pendingListens.value.length > 0) scheduleRetry();

    if (saved.currentTrackId === null) {
        persistState();
        return;
    }

    // Lazy N×1 validation: only the current track is resolved on restore;
    // pending entries are validated just before playing.
    const gen = ++transitionGen;
    isLoadingSource.value = true;
    try {
        const meta = await getTrackMetadata(saved.currentTrackId);
        if (gen !== transitionGen) return;
        const track = projectMetadata(meta);
        const result = await playerLoadTrack(saved.currentTrackId, false, effectiveEngineVolume());
        if (gen !== transitionGen) return;
        engineSeq = result.seq;
        loadedPath = result.stream ? `stream:${track.id}` : `track:${track.id}`;
        streamQuality.value = result.stream
            ? {
                serviceName: result.stream.serviceName,
                qualityLabel: result.stream.qualityLabel,
                qualityTier: result.stream.qualityTier,
            }
            : null;
        streamError.value = null;
        isBuffering.value = false;
        engineState = 'paused';
        const maxPos = result.durationSec !== null && result.durationSec > 0 ? result.durationSec : saved.positionSec;
        const pos = Math.min(saved.positionSec, maxPos);
        if (pos > 0) {
            await playerSeek(pos);
            if (gen !== transitionGen) return;
        }
        positionSec.value = pos;
        durationSec.value = result.durationSec ?? 0;
        currentEntry.value = { entryId: uuidV4(), trackId: track.id, track };
    } catch {
        if (gen !== transitionGen) return;
        // Track gone or without a local file: start with no current; the
        // restored queue is kept for the first play().
        currentEntry.value = null;
    } finally {
        if (gen === transitionGen) isLoadingSource.value = false;
        persistState();
        syncMprisState();
    }
}

// Persistence when the window is hidden (not on every timeupdate).
if (typeof document !== 'undefined') {
    document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'hidden') persistState();
    });
}

export function usePlayer() {
    return {
        // Previous state (surface unchanged)
        current, isPlaying, positionSec, durationSec, playbackRate, error, isLoadingSource,
        isBuffering, streamQuality, streamError,
        upNext,
        // New state (phase 1)
        queue, history, shuffle, repeat, volume, muted, pendingListens,
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

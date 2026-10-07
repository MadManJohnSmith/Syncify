/**
 * Player local — Fase 1 (cola con identidad, modos persistentes, acreditación
 * de escucha, restauración tras reinicio, MediaSession).
 *
 * Singleton HTML5 Audio driven through the `syncify-media://` protocol:
 * the backend command `resolve_playback_source` verifies the track has a
 * downloaded file, grants exactly that file (in-memory allowlist), and the
 * frontend converts the path with convertFileSrc. Provider streaming is out
 * of scope (S194 verdict).
 *
 * Regulares de la fase:
 * - La restauración tras reinicio SIEMPRE vuelve a resolver con
 *   `resolve_playback_source`: en localStorage jamás se guardan `file_path`,
 *   `audio.src`, URLs convertidas ni el DTO completo de la pista.
 * - La acreditación de escucha usa 30 s de reproducción real acumulados por
 *   tick; nunca `currentTime >= 30`. La posición restaurada no acredita tiempo
 *   previo.
 * - Importar este módulo no dispara invokes ni reproducción: la restauración
 *   es perezosa (`restoreFromPersistence`) y la invoca NowPlayingBar al montar.
 */
import { computed, ref } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import { invokeCommand } from '../api/tauri'
import { getTrackMetadata, recordLocalPlay, type TrackMetadataWithSources } from '../api/library'

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
 * Entrada de cola con identidad propia: permite duplicados y reordenación
 * estable. `track` es la proyección segura (sin file_path) cuando se conoce;
 * las entradas restauradas la obtienen justo antes de sonar.
 */
export interface PlaybackQueueEntry {
    entryId: string;
    trackId: number;
    track?: PlayerTrack;
}

/** Pendiente de escucha esperando confirmación del backend. */
export interface PendingListen {
    trackId: number;
    listenSessionId: string;
    attempts: number;
}

interface PlaybackSource {
    track_id: number;
    file_path: string;
    format?: string | null;
}

// ---------------------------------------------------------------------------
// Constantes y utilidades
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

/** UUID v4 canónico (minúsculas con guiones); fallback manual para entornos sin randomUUID. */
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

function isAbortError(err: unknown): boolean {
    return typeof err === 'object' && err !== null
        && 'name' in err && (err as { name?: unknown }).name === 'AbortError';
}

/**
 * Clasificador de errores definitivos de biblioteca (pista borrada, UUID
 * inválido, FK rota): descartan sin reintentar. Todo lo demás (DB ocupada,
 * IPC caído) se trata como transitorio. Los mensajes vienen del backend como
 * texto; el clasificador es tolerante a variaciones de redacción.
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

/** Proyección estricta del DTO a PlayerTrack: jamás arrastra file_path/URLs. */
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
// Estado del singleton
// ---------------------------------------------------------------------------

const audio = new Audio();

const currentEntry = ref<PlaybackQueueEntry | null>(null);
const current = computed<PlayerTrack | null>({
    get: () => currentEntry.value?.track ?? null,
    // Escritura directa soportada (inyección de estado; la usan specs de layout
    // para simular «reproduciendo»): envuelve la pista en una entrada con
    // identidad propia sin tocar cola ni historial.
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

// Cola (FE-9 extendido): pendientes con identidad propia; `upNext` se mantiene
// como proyección compatible para consumidores previos.
const queue = ref<PlaybackQueueEntry[]>([]);
const history = ref<PlaybackQueueEntry[]>([]);
const shuffle = ref(false);
const repeat = ref<RepeatMode>('off');
const volume = ref(1);
const muted = ref(false);
const pendingListens = ref<PendingListen[]>([]);

/** Orden base pre-barajado de los pendientes (entryIds). No reactivo: lo sincronizan las mutaciones. */
let baseOrder: string[] = [];
/** Última ruta resuelta con éxito; comparación EXACTA (un substring daría falsos positivos con rutas sufijo). */
let lastResolvedPath: string | null = null;
/** Generación de transición: toda continuación asíncrona comprueba que su gen sigue vivo. */
let transitionGen = 0;
/** Suprime el informe del listener `error` del elemento cuando el flujo ya gestionó el fallo. */
let suppressElementError = false;

const upNext = computed<PlayerTrack[]>(() =>
    queue.value.map(e => e.track ?? {
        id: e.trackId,
        title: `Pista #${e.trackId}`,
        artist: '',
        album: null,
        coverUrl: null,
    }),
);

// ---------------------------------------------------------------------------
// Persistencia (localStorage, clave versionada única)
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
        // Cuota/almacenamiento no disponible: la reproducción sigue sin persistencia.
    }
}

function validatePersisted(v: unknown): PersistedPlayerState | null {
    if (typeof v !== 'object' || v === null) return null;
    const rec = v as Record<string, unknown>;
    // Versión desconocida o estructura inválida → descartar completo, arrancar limpio.
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

    // Pendientes de escucha: cada fila corrupta (no-UUID / trackId inválido) se
    // descarta individualmente; el resto del estado se conserva.
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
// Acreditación de escucha (30 s reales por tick)
// ---------------------------------------------------------------------------

let listenSessionId: string | null = null;
let listenTrackId: number | null = null;
let listenAccumSec = 0;
/** Un pendiente por sesión de escucha: se crea una sola vez al cruzar el umbral. */
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
    const eligible = !audio.paused && audio.readyState >= 3;
    if (!eligible) {
        // Pausa/buffering: el tick no acredita; el acumulado se conserva y la
        // línea base se re-ancla en el próximo tick elegible.
        tickWallMs = null;
        tickMediaSec = null;
        return;
    }
    const now = Date.now();
    if (tickWallMs !== null && tickMediaSec !== null) {
        const dtWall = (now - tickWallMs) / 1000;
        const dtMedia = audio.currentTime - tickMediaSec;
        const rate = Number.isFinite(audio.playbackRate) && audio.playbackRate > 0 ? audio.playbackRate : 1;
        if (Number.isFinite(dtWall) && Number.isFinite(dtMedia) && dtWall >= 0 && dtMedia >= 0) {
            listenAccumSec += Math.min(dtWall, dtMedia / rate);
        }
        // Delta inválido (seek, salto de currentTime): el tick no acredita pero
        // tampoco borra el acumulado; la re-anclaje de abajo evita arrastrarlo.
    }
    tickWallMs = now;
    tickMediaSec = audio.currentTime;
    if (!listenScheduled && listenAccumSec >= LISTEN_THRESHOLD_SEC) {
        listenScheduled = true;
        scheduleListen(listenTrackId, listenSessionId);
    }
}

// ---------------------------------------------------------------------------
// Pendientes de escucha: envío único por sesión, reintentos con tope
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
            // Tope de 5 intentos alcanzado: se descarta y se registra.
            console.warn(`[player] descartando escucha de track ${p.trackId} tras ${p.attempts} intentos`);
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
            console.warn(`[player] record_local_play falló (intento ${p.attempts}) para track ${p.trackId}: ${msg}`);
            // Definitivo (pista borrada/UUID inválido) o tope de 5 intentos: descarta.
            if (isDefinitiveLibraryError(msg) || p.attempts >= LISTEN_MAX_ATTEMPTS) discardListen(p);
            // Transitorio por debajo del tope: conserva el pendiente; scheduleRetry programa el backoff.
        } finally {
            listenInFlight.delete(p.listenSessionId);
        }
    }
    scheduleRetry();
}

// ---------------------------------------------------------------------------
// MediaSession (feature detection por símbolo, degradación limpia)
// ---------------------------------------------------------------------------

function mediaSessionAvailable(): boolean {
    return typeof navigator !== 'undefined' && 'mediaSession' in navigator;
}

function setMediaSessionPlaybackState(state: MediaSessionPlaybackState): void {
    if (!mediaSessionAvailable()) return;
    // `playbackState` es una propiedad del estándar, no un método.
    try {
        navigator.mediaSession.playbackState = state;
    } catch {
        // Degradación limpia.
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
        // Degradación limpia.
    }
}

function updateMediaSessionPosition(): void {
    if (!mediaSessionAvailable()) return;
    const ms = navigator.mediaSession;
    if (typeof ms?.setPositionState !== 'function') return;
    const duration = audio.duration;
    const position = audio.currentTime;
    const rate = audio.playbackRate;
    if (!Number.isFinite(duration) || duration <= 0) return;
    if (!Number.isFinite(position) || position < 0) return;
    try {
        ms.setPositionState({
            duration,
            position: Math.min(position, duration),
            playbackRate: Number.isFinite(rate) && rate > 0 ? rate : 1,
        });
    } catch {
        // Algunos backends rechazan combinaciones fuera de rango.
    }
}

/**
 * True tras clearMediaSession: los action handlers quedaron desregistrados y
 * el próximo inicio de reproducción debe re-registrarlos. Sin esto, stop()
 * mataba las teclas multimedia/MPRIS hasta reiniciar la app: bindAudioEvents
 * (que registra los handlers) solo se ejecuta UNA vez por vida del módulo.
 */
let mediaSessionHandlersCleared = false;

function clearMediaSession(): void {
    if (!mediaSessionAvailable()) return;
    const ms = navigator.mediaSession;
    try {
        ms.metadata = null;
    } catch {
        // Degradación limpia.
    }
    if (typeof ms?.setActionHandler !== 'function') return;
    const unset = (action: MediaSessionAction) => {
        try {
            ms.setActionHandler(action, null);
        } catch {
            // Acción no soportada por el backend.
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
            // Acción no soportada por el backend.
        }
    };
    // Los handlers delegan en las mismas acciones del player (mismas reglas de
    // next/previous que la UI).
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
// Eventos del elemento de audio
// ---------------------------------------------------------------------------

let bound = false;

function bindAudioEvents(): void {
    if (bound) return;
    bound = true;
    audio.addEventListener('timeupdate', () => {
        positionSec.value = audio.currentTime;
        creditListenTick();
        updateMediaSessionPosition();
    });
    audio.addEventListener('durationchange', () => {
        durationSec.value = Number.isFinite(audio.duration) ? audio.duration : 0;
    });
    audio.addEventListener('play', () => {
        isPlaying.value = true;
        suppressElementError = false;
        if (currentEntry.value && !listenSessionId) {
            // Reproducción reanudada tras restaurar: la sesión de escucha nace
            // aquí y el acumulado está a 0 (la posición restaurada no acredita
            // tiempo previo).
            beginListenSession(currentEntry.value.trackId);
        }
        setMediaSessionPlaybackState('playing');
    });
    audio.addEventListener('pause', () => {
        isPlaying.value = false;
        setMediaSessionPlaybackState('paused');
        persistState();
    });
    audio.addEventListener('seeked', () => {
        persistState();
    });
    audio.addEventListener('canplay', () => {
        suppressElementError = false;
    });
    audio.addEventListener('ended', () => {
        isPlaying.value = false;
        // Al terminar naturalmente el estándar espera 'paused' (WebKitGTK no
        // siempre dispara 'pause'); si hay avance, 'play' lo vuelve a 'playing'.
        setMediaSessionPlaybackState('paused');
        void advanceTrack(true);
    });
    audio.addEventListener('error', () => {
        if (!audio.src) return; // ruido de teardown
        if (suppressElementError) {
            suppressElementError = false;
            return; // el flujo ya informó por `error`
        }
        error.value = 'No se pudo reproducir el archivo de audio';
        isPlaying.value = false;
        setMediaSessionPlaybackState('paused');
    });
    bindMediaSessionHandlers();
}

// ---------------------------------------------------------------------------
// Núcleo de reproducción
// ---------------------------------------------------------------------------

function pushHistory(entry: PlaybackQueueEntry): void {
    history.value = [...history.value, stripEntry(entry)].slice(-HISTORY_LIMIT);
}

function removePending(entryId: string): void {
    queue.value = queue.value.filter(e => e.entryId !== entryId);
    baseOrder = baseOrder.filter(id => id !== entryId);
}

/**
 * Resultado de `ensureProjection` para una entrada pendiente sin proyección:
 * la pista validada, 'dead' (ID desaparecido → descartar al vuelo) o 'error'
 * (fallo no clasificable → conservar la cabeza y detener el avance).
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
 * Resuelve la fuente (siempre de nuevo: nunca se reusan concesiones ni URLs),
 * asigna el src y arranca la reproducción. Devuelve:
 * - 'played'  → el audio está sonando; el llamador hace el commit del estado.
 * - 'failed'  → fallo de resolve/play; `error` lleva el mensaje, estado intacto.
 * - 'aborted' → la continuación quedó obsoleta (stop()/nueva transición); no
 *               toca estado, no informa error.
 */
async function resolveAndPlay(track: PlayerTrack, gen: number): Promise<'played' | 'failed' | 'aborted'> {
    isLoadingSource.value = true;
    try {
        const src = await invokeCommand<PlaybackSource>('resolve_playback_source', { trackId: track.id });
        if (gen !== transitionGen) return 'aborted';
        if (lastResolvedPath !== null && src.file_path === lastResolvedPath) {
            // Mismo archivo ya cargado (repeat-one, duplicado adyacente): reinicia
            // el medio sin reasignar src, lo que evita heredar el bucle de 'ended'
            // latente del elemento.
            audio.currentTime = 0;
        } else {
            suppressElementError = true;
            audio.src = convertFileSrc(src.file_path, 'syncify-media');
        }
        lastResolvedPath = src.file_path;
        positionSec.value = 0;
        durationSec.value = 0;
        beginListenSession(track.id);
        await audio.play();
        if (gen !== transitionGen) return 'aborted';
        // Metadatos al confirmar el inicio de la reproducción. Si stop()
        // desregistró los handlers de MediaSession, re-registrarlos aquí: el
        // bind inicial de bindAudioEvents no se repite (guard de módulo) y sin
        // este re-bind PlayPause/Next/Prev por tecla multimedia o D-Bus/MPRIS
        // no hacían nada tras el primer stop().
        if (mediaSessionHandlersCleared) bindMediaSessionHandlers();
        updateMediaSessionMetadata(track);
        return 'played';
    } catch (err) {
        // Un play() interrumpido por stop()/load() se traga: sin error, sin current.
        if (isAbortError(err)) return 'aborted';
        if (gen !== transitionGen) return 'aborted';
        error.value = messageOf(err);
        return 'failed';
    } finally {
        if (gen === transitionGen) isLoadingSource.value = false;
    }
}

type AdvanceResult = 'played' | 'stopped' | 'empty';

/**
 * Avanza consumiendo la cabeza de la cola. La cabeza NO se retira hasta que la
 * resolución y el arranque tienen éxito; los IDs desaparecidos se descartan al
 * vuelo y un fallo no clasificable conserva la cabeza y detiene el avance.
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
            error.value = `No se pudo validar la pista ${entry.trackId} de la cola`;
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
 * Qué pasa cuando la cola se agota (el único papel de `repeat`): la cola avanza
 * siempre al `ended` mientras haya pendientes, en cualquier modo.
 */
async function handleExhausted(auto: boolean, gen: number): Promise<void> {
    if (auto && repeat.value === 'one' && currentEntry.value && current.value) {
        // Vuelta de repeat-one: token de escucha nuevo y currentTime = 0; no
        // duplica el historial (es la misma pista continuando).
        await resolveAndPlay(current.value, gen);
        return;
    }
    if (repeat.value === 'all') {
        // Reconstruye el ciclo con las entradas vivas (historial en orden de
        // reproducción + actual). Al recorrer un ciclo completo cada entrada se
        // reproduce una vez, así que conservar la ÚLTIMA aparición por entryId
        // devuelve el orden del ciclo y evita duplicados acumulados entre
        // vueltas. Con shuffle desactivado ese orden coincide con el base; con
        // shuffle activo los pendientes se re-barajan.
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
    // repeat off (o next() manual sin pendientes): no-op.
}

async function advanceTrack(auto: boolean): Promise<void> {
    const gen = ++transitionGen;
    bindAudioEvents();
    const result = await advanceFromHead(gen);
    if (gen !== transitionGen) return;
    if (result !== 'empty') return;
    await handleExhausted(auto, gen);
}

/** Reproducción inmediata de una pista concreta (la anterior pasa al historial). */
async function playTrack(track: PlayerTrack): Promise<void> {
    const gen = ++transitionGen;
    bindAudioEvents();
    error.value = null;
    const outcome = await resolveAndPlay(track, gen);
    if (gen !== transitionGen) return;
    if (outcome === 'failed') {
        throw new Error(error.value ?? 'No se pudo reproducir la pista');
    }
    if (outcome === 'played') {
        if (currentEntry.value) pushHistory(currentEntry.value);
        currentEntry.value = { entryId: uuidV4(), trackId: track.id, track };
        persistState();
    }
}

/** `play()` sin argumentos: reanuda, o arranca la primera pendiente de una cola restaurada. */
async function playIdle(): Promise<void> {
    if (audio.src) {
        if (audio.paused) {
            void audio.play().catch(() => { isPlaying.value = false; });
        }
        return;
    }
    if (queue.value.length > 0) {
        await advanceTrack(false);
    }
}

/**
 * `play(track)` reproduce esa pista; `play()` reanuda o arranca la primera
 * pendiente (regla del plan: play/MediaSession-play con current == null y cola
 * restaurada arranca la primera pendiente; `pause` sin src es no-op y
 * `toggle` sigue siendo no-op sin src).
 */
async function play(track?: PlayerTrack): Promise<void> {
    if (track) return playTrack(track);
    return playIdle();
}

// ---------------------------------------------------------------------------
// API pública
// ---------------------------------------------------------------------------

async function playNext(track: PlayerTrack): Promise<QueueOutcome> {
    if (!audio.src) {
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
    if (!audio.src) {
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
 * Reproduce la primera pista del listado y encola el resto **en su orden**
 * (corrige la inversión que producía el encadenado de playNext front-inserts).
 * Abre un ciclo nuevo: historial y orden base empiezan limpios. Si la primera
 * pista falla al resolver, el estado previo queda intacto y `error` lleva el
 * mensaje.
 */
async function replaceQueueAndPlay(tracks: PlayerTrack[]): Promise<void> {
    if (!Array.isArray(tracks) || tracks.length === 0) return;
    const gen = ++transitionGen;
    bindAudioEvents();
    error.value = null;
    const outcome = await resolveAndPlay(tracks[0], gen);
    if (gen !== transitionGen) return;
    if (outcome === 'failed') {
        throw new Error(error.value ?? 'No se pudo reproducir la pista');
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
    // Solo pendientes: la pista en curso nunca se toca.
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
    // El reordenado manual define la nueva base (desactivar shuffle lo respeta).
    baseOrder = next.map(e => e.entryId);
    persistState();
}

function next(): void {
    // next() ignora repeat-one: salta a los pendientes; con la cola agotada,
    // repeat 'all' reconstruye y reproduce, y 'off'/'one' son no-op.
    void advanceTrack(false);
}

async function previous(): Promise<void> {
    bindAudioEvents();
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
            error.value = `No se pudo validar la pista ${entry.trackId} del historial`;
            return;
        }
        error.value = null;
        const outcome = await resolveAndPlay(proj, gen);
        if (gen !== transitionGen) return;
        if (outcome !== 'played') return;
        // La pista dejada vuelve al frente de los pendientes.
        if (currentEntry.value) {
            const back = currentEntry.value;
            queue.value = [back, ...queue.value];
            baseOrder = [back.entryId, ...baseOrder.filter(id => id !== back.entryId)];
        }
        currentEntry.value = { ...entry, track: proj };
        persistState();
        return;
    }
    // Sin historial: seek(0).
    seek(0);
}

function toggle(): void {
    if (!audio.src) return;
    if (audio.paused) {
        // Reanudar conserva la sesión de escucha (la pausa no la rompe).
        void audio.play().catch(() => { isPlaying.value = false; });
    } else {
        audio.pause();
    }
}

function pausePlayback(): void {
    // Sin src es un no-op efectivo.
    audio.pause();
}

function stop(): void {
    transitionGen++;
    audio.pause();
    audio.removeAttribute('src');
    audio.load();
    lastResolvedPath = null;
    currentEntry.value = null;
    positionSec.value = 0;
    durationSec.value = 0;
    error.value = null;
    queue.value = [];
    baseOrder = [];
    history.value = [];
    resetListenSession();
    clearMediaSession();
    suppressElementError = false;
    isLoadingSource.value = false;
    persistState();
}

function seek(sec: number): void {
    if (!Number.isFinite(sec)) return;
    const max = Number.isFinite(audio.duration) && audio.duration > 0 ? audio.duration : Number.MAX_SAFE_INTEGER;
    audio.currentTime = Math.min(Math.max(0, sec), max);
    positionSec.value = audio.currentTime;
    // La persistencia del seek la hace el evento `seeked` (cubre también las
    // líneas de LyricsView).
}

function setRate(rate: number): void {
    if (Number.isFinite(rate) && rate > 0) {
        audio.playbackRate = rate;
        playbackRate.value = rate;
    }
}

function setVolume(v: number): void {
    volume.value = clampVolume(v);
    audio.volume = volume.value;
    persistState();
}

function setMuted(m: boolean): void {
    muted.value = !!m;
    audio.muted = muted.value;
    persistState();
}

function toggleShuffle(): void {
    if (shuffle.value) {
        // Desactivar shuffle restaura el orden base de los pendientes: no se
        // conserva el barajado.
        shuffle.value = false;
        const byId = new Map(queue.value.map(e => [e.entryId, e]));
        const restored: PlaybackQueueEntry[] = [];
        for (const id of baseOrder) {
            const e = byId.get(id);
            if (e) restored.push(e);
        }
        queue.value = restored;
    } else {
        // Activar shuffle permuta los pendientes; el orden base pre-barajado se
        // conserva para poder restaurarlo y persistirse junto a la permutación.
        shuffle.value = true;
        queue.value = shuffled(queue.value);
    }
    persistState();
}

function cycleRepeat(): void {
    repeat.value = repeat.value === 'off' ? 'all' : repeat.value === 'all' ? 'one' : 'off';
    persistState();
}

// ---------------------------------------------------------------------------
// Restauración perezosa tras reinicio
// ---------------------------------------------------------------------------

let restoreStarted = false;

function waitForLoadedmetadata(el: HTMLMediaElement): Promise<void> {
    if (el.readyState >= 1) return Promise.resolve();
    return new Promise((resolve, reject) => {
        const ok = () => { cleanup(); resolve(); };
        const fail = () => { cleanup(); reject(new Error('No se pudo cargar la duración del archivo')); };
        const cleanup = () => {
            el.removeEventListener('loadedmetadata', ok);
            el.removeEventListener('error', fail);
        };
        el.addEventListener('loadedmetadata', ok, { once: true });
        el.addEventListener('error', fail, { once: true });
    });
}

/**
 * Restaura el estado persistido. Solo se invoca explícitamente (NowPlayingBar
 * al montar); importar el módulo no toca localStorage ni dispara invokes.
 * Resuelve `resolve_playback_source` de nuevo (jamás reusa concesiones ni
 * URLs), coloca la posición tras `loadedmetadata` y queda PAUSADO: leer el
 * estado guardado no reproduce nada y la posición restaurada no acredita
 * tiempo de escucha previo.
 */
async function restoreFromPersistence(): Promise<void> {
    if (restoreStarted) return;
    restoreStarted = true;
    bindAudioEvents();
    const saved = loadPersistedState();
    if (!saved) return;

    shuffle.value = saved.shuffle;
    repeat.value = saved.repeat;
    volume.value = clampVolume(saved.volume);
    muted.value = saved.muted;
    audio.volume = volume.value;
    audio.muted = muted.value;
    pendingListens.value = saved.pendingListens;
    queue.value = saved.queue.map(e => ({ ...e }));
    baseOrder = [...saved.baseOrder];
    history.value = saved.history.map(e => ({ ...e }));
    if (pendingListens.value.length > 0) scheduleRetry();

    if (saved.currentTrackId === null) {
        persistState();
        return;
    }

    // Validación perezosa N×1: solo la pista actual se resuelve al restaurar;
    // los pendientes se validan justo antes de sonar.
    const gen = ++transitionGen;
    isLoadingSource.value = true;
    try {
        const meta = await getTrackMetadata(saved.currentTrackId);
        if (gen !== transitionGen) return;
        const track = projectMetadata(meta);
        const src = await invokeCommand<PlaybackSource>('resolve_playback_source', { trackId: saved.currentTrackId });
        if (gen !== transitionGen) return;
        suppressElementError = true;
        audio.src = convertFileSrc(src.file_path, 'syncify-media');
        lastResolvedPath = src.file_path;
        await waitForLoadedmetadata(audio);
        if (gen !== transitionGen) return;
        const maxPos = Number.isFinite(audio.duration) && audio.duration > 0 ? audio.duration : saved.positionSec;
        const pos = Math.min(saved.positionSec, maxPos);
        audio.currentTime = pos;
        positionSec.value = pos;
        currentEntry.value = { entryId: uuidV4(), trackId: track.id, track };
    } catch {
        if (gen !== transitionGen) return;
        // Pista desaparecida o sin archivo local: arranca sin current; la cola
        // restaurada se conserva para el primer play().
        currentEntry.value = null;
    } finally {
        if (gen === transitionGen) isLoadingSource.value = false;
        persistState();
    }
}

// Persistencia en ocultado de la ventana (no en cada timeupdate).
if (typeof document !== 'undefined') {
    document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'hidden') persistState();
    });
}

export function usePlayer() {
    return {
        // Estado (superficie previa intacta)
        current, isPlaying, positionSec, durationSec, playbackRate, error, isLoadingSource,
        upNext,
        // Estado nuevo (fase 1)
        queue, history, shuffle, repeat, volume, muted, pendingListens,
        // Acciones previas
        play, playNext, toggle, stop, seek, setRate,
        // Acciones nuevas (fase 1)
        enqueue, replaceQueueAndPlay, clearQueue, removeEntry, moveEntry,
        next, previous, setVolume, setMuted, toggleShuffle, cycleRepeat,
        restoreFromPersistence,
    };
}

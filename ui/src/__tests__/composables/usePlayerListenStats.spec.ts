/**
 * Fase 1 — acreditación de escucha (30 s de reproducción real) y política de
 * reintentos de `record_local_play`.
 *
 * jsdom no reproduce audio: `play`/`pause`/`load` están stubbed, `paused` y
 * `readyState` (get-only en jsdom) se sombrean con defineProperty, y los ticks
 * de `timeupdate` se simulan controlando reloj (fake timers) y currentTime.
 */
import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest'
import { flushPromises } from '@vue/test-utils'

const playStub = vi.fn(() => Promise.resolve())
const pauseStub = vi.fn()
const loadStub = vi.fn()

// El elemento de audio creado por el módulo bajo test.
let audioInstance: HTMLMediaElement | null = null

vi.mock('@tauri-apps/api/core', () => ({
    convertFileSrc: (path: string, protocol?: string) =>
        `${protocol ?? 'asset'}://localhost/${encodeURIComponent(path)}`,
}));

vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}));

import { invokeCommand } from '../../api/tauri'

const mockInvoke = vi.mocked(invokeCommand);

let usePlayer: typeof import('../../composables/usePlayer').usePlayer;
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

type MutableMedia = HTMLMediaElement & {
    currentTime: number;
    playbackRate: number;
};

function media(): MutableMedia {
    if (!audioInstance) throw new Error('audio singleton no capturado');
    return audioInstance as unknown as MutableMedia;
}

/** paused/readyState son get-only en jsdom: se sombrean como propiedades propias. */
function setPaused(m: MutableMedia, paused: boolean): void {
    Object.defineProperty(m, 'paused', { configurable: true, value: paused });
}
function setReadyState(m: MutableMedia, readyState: number): void {
    Object.defineProperty(m, 'readyState', { configurable: true, value: readyState });
}

/** Un tick de timeupdate: avanza el reloj (fake timers) y el medio, y despacha el evento. */
function tick(m: MutableMedia, mediaSec: number, wallMs = 250): void {
    vi.advanceTimersByTime(wallMs);
    m.currentTime = mediaSec;
    m.dispatchEvent(new Event('timeupdate'));
}

function recordCalls(): Array<{ args?: Record<string, unknown> }> {
    return mockInvoke.mock.calls
        .filter(c => c[0] === 'record_local_play')
        .map(c => ({ args: c[1] as Record<string, unknown> | undefined }));
}

function mockHappyPath(): void {
    mockInvoke.mockImplementation((cmd: string, args?: Record<string, unknown>) => {
        if (cmd === 'resolve_playback_source') {
            return Promise.resolve({ track_id: args?.trackId, file_path: `/lib/${args?.trackId}.flac`, format: 'FLAC' });
        }
        if (cmd === 'record_local_play') {
            return Promise.resolve({ track_id: args?.trackId, play_count: 1, last_played: '2026-10-06T00:00:00Z', counted: true });
        }
        return Promise.resolve(null);
    });
}

beforeAll(async () => {
    Object.defineProperty(window.HTMLMediaElement.prototype, 'play', { configurable: true, value: playStub });
    Object.defineProperty(window.HTMLMediaElement.prototype, 'pause', { configurable: true, value: pauseStub });
    Object.defineProperty(window.HTMLMediaElement.prototype, 'load', { configurable: true, value: loadStub });
    class CapturingAudio extends window.Audio {
        constructor() {
            super();
            audioInstance = this;
        }
    }
    (window as unknown as { Audio: typeof Audio }).Audio = CapturingAudio;
    // Import dinámico: el patch de CapturingAudio debe estar en sitio ANTES de
    // que el módulo bajo test cree su Audio a nivel de módulo.
    ({ usePlayer } = await import('../../composables/usePlayer'));
});

describe('usePlayer — acreditación de escucha (30 s reales)', () => {
    beforeEach(() => {
        vi.useFakeTimers();
        mockInvoke.mockReset();
        localStorage.clear();
        mockHappyPath();
        usePlayer().stop();
        playStub.mockClear();
        pauseStub.mockClear();
        loadStub.mockClear();
    });
    afterEach(() => {
        vi.useRealTimers();
    });

    function startListening(): { player: ReturnType<typeof usePlayer>; m: MutableMedia } {
        const player = usePlayer();
        const m = media();
        setPaused(m, false);
        setReadyState(m, 3);
        m.playbackRate = 1;
        return { player, m };
    }

    it('29,75 s acreditados no cuentan; 30,0 s sí (acumulación por tick, no currentTime)', async () => {
        const { player, m } = startListening();
        await player.play({ id: 7, title: 'Song', artist: 'Artist' });
        mockInvoke.mockClear();

        tick(m, 0); // primer tick elegible: solo re-ancla, acredita 0
        let t = 0;
        for (let i = 0; i < 119; i++) { t += 0.25; tick(m, t); }
        await flushPromises();
        expect(recordCalls()).toHaveLength(0); // 29,75 s acreditados

        tick(m, 30); // 30,0 s acreditados
        await flushPromises();
        const calls = recordCalls();
        expect(calls).toHaveLength(1);
        expect(calls[0].args?.trackId).toBe(7);
        expect(calls[0].args?.listenSessionId).toMatch(UUID_RE);
        // El mock confirma: el pendiente se retira.
        expect(player.pendingListens.value).toHaveLength(0);
    });

    it('las pausas suspenden la acreditación sin borrar el acumulado', async () => {
        const { m } = startListening();
        await player0(m);
        mockInvoke.mockClear();

        tick(m, 0);
        let t = 0;
        for (let i = 0; i < 40; i++) { t += 0.25; tick(m, t); }      // 10 s
        setPaused(m, true);
        for (let i = 0; i < 8; i++) { t += 0.25; tick(m, t); }       // 2 s en pausa: nada
        setPaused(m, false);
        for (let i = 0; i < 79; i++) { t += 0.25; tick(m, t); }      // 19,5 s (el primero tras reanudar solo re-ancla)
        await flushPromises();
        expect(recordCalls()).toHaveLength(0); // 29,5 acreditados (la pausa no acreditó ni borró)
        tick(m, 32);      // +0,25 → 29,75
        tick(m, 32.25);   // +0,25 → 30,0
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);
    });

    it('waiting/stalled (readyState < 3) suspenden sin borrar', async () => {
        const { m } = startListening();
        await player0(m);
        mockInvoke.mockClear();

        tick(m, 0);
        let t = 0;
        for (let i = 0; i < 40; i++) { t += 0.25; tick(m, t); }      // 10 s
        setReadyState(m, 2); // buffering
        for (let i = 0; i < 12; i++) { t += 0.25; tick(m, t); }      // 3 s con buffering: nada
        setReadyState(m, 3);
        for (let i = 0; i < 79; i++) { t += 0.25; tick(m, t); }      // 19,5 s (el primero tras el stall solo re-ancla)
        await flushPromises();
        expect(recordCalls()).toHaveLength(0); // 29,5 acreditados
        tick(m, 35);      // el medio quedó en 34,75: +0,25 → 29,75
        tick(m, 35.25);   // +0,25 → 30,0
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);
    });

    it('playbackRate 2× acredita la mitad del progreso de medio por segundo de reloj', async () => {
        const { m } = startListening();
        await player0(m);
        mockInvoke.mockClear();
        m.playbackRate = 2;

        tick(m, 0);
        let t = 0;
        // 0,5 s de reloj por tick; el medio avanza 1 s por tick → acredita 0,5.
        for (let i = 0; i < 59; i++) { t += 1; tick(m, t, 500); }
        await flushPromises();
        expect(recordCalls()).toHaveLength(0); // 29,5 s acreditados con el medio en 59 s
        tick(m, 60, 500);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1); // 30 s de reloj → 60 s de medio
    });

    it('los seeks repetidos de Lyrics no fabrican ni pierden escucha', async () => {
        const { m } = startListening();
        await player0(m);
        mockInvoke.mockClear();

        tick(m, 0);
        let t = 0;
        for (let i = 0; i < 40; i++) { t += 0.25; tick(m, t); } // 10 s
        // Seek atrás (línea anterior): Δmedia negativo → el tick no acredita.
        m.currentTime = 5;
        tick(m, 5);
        await flushPromises();
        expect(recordCalls()).toHaveLength(0);
        // Desde 5 s: 19,75 s de reloj más → 29,75 acreditados en total.
        for (let i = 0; i < 79; i++) { t = m.currentTime + 0.25; tick(m, t); }
        await flushPromises();
        expect(recordCalls()).toHaveLength(0);
        tick(m, m.currentTime + 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);
    });

    it('nunca usa currentTime >= 30 como criterio: un seek a 100 s no acredita', async () => {
        const { m } = startListening();
        await player0(m);
        mockInvoke.mockClear();

        tick(m, 0);
        for (let i = 0; i < 8; i++) tick(m, (i + 1) * 0.25); // 2 s
        m.currentTime = 100;
        tick(m, 100);
        await flushPromises();
        expect(recordCalls()).toHaveLength(0); // ~2,25 s acreditados pese a currentTime 100
    });

    it('pausa, seek y rate conservan la sesión: una sola acreditación por sesión', async () => {
        const { m } = startListening();
        await player0(m);
        mockInvoke.mockClear();

        tick(m, 0);
        for (let i = 0; i < 120; i++) tick(m, (i + 1) * 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);

        // Pausa, reanudación, seek atrás y cambio de velocidad: la sesión sigue.
        setPaused(m, true);
        for (let i = 120; i < 128; i++) tick(m, i * 0.25);
        setPaused(m, false);
        m.currentTime = 10;
        tick(m, 10);
        m.playbackRate = 1.5;
        for (let i = 0; i < 121; i++) tick(m, 10 + (i + 1) * 0.375, 250);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1); // otra sesión habría acreditado de nuevo
    });

    it('cambiar de pista crea un token nuevo; repeat-one también', async () => {
        const { player, m } = startListening();
        await player.play({ id: 7, title: 'Song', artist: 'Artist' });
        mockInvoke.mockClear();

        tick(m, 0);
        for (let i = 0; i < 120; i++) tick(m, (i + 1) * 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);
        const sid1 = recordCalls()[0].args?.listenSessionId;

        await player.play({ id: 8, title: 'Other', artist: 'Artist' });
        tick(m, 0);
        for (let i = 0; i < 120; i++) tick(m, (i + 1) * 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(2);
        const sid2 = recordCalls()[1].args?.listenSessionId;
        expect(sid2).not.toBe(sid1);

        // repeat-one: ended sin pendientes reinicia la pista con token nuevo.
        player.cycleRepeat(); // off → all
        player.cycleRepeat(); // all → one
        m.dispatchEvent(new Event('ended'));
        await flushPromises();
        tick(m, 0);
        for (let i = 0; i < 120; i++) tick(m, (i + 1) * 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(3);
        const sid3 = recordCalls()[2].args?.listenSessionId;
        expect(sid3).not.toBe(sid2);
    });

    it('error transitorio: reintenta el mismo token con backoff hasta el tope de 5 y descarta', async () => {
        const { m } = startListening();
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'resolve_playback_source') return Promise.resolve({ track_id: 7, file_path: '/lib/7.flac' });
            if (cmd === 'record_local_play') return Promise.reject(new Error('database is locked'));
            return Promise.resolve(null);
        });
        await player0(m);
        mockInvoke.mockClear();

        tick(m, 0);
        for (let i = 0; i < 120; i++) tick(m, (i + 1) * 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);
        const player = usePlayer();
        expect(player.pendingListens.value).toHaveLength(1);
        expect(player.pendingListens.value[0]?.attempts).toBe(1);

        await vi.advanceTimersByTimeAsync(1000);  // intento 2
        await vi.advanceTimersByTimeAsync(2000);  // intento 3
        await vi.advanceTimersByTimeAsync(4000);  // intento 4
        await vi.advanceTimersByTimeAsync(8000);  // intento 5 → tope → descarta
        expect(recordCalls()).toHaveLength(5);
        expect(player.pendingListens.value).toHaveLength(0);
        await vi.advanceTimersByTimeAsync(60000); // nada más programado
        expect(recordCalls()).toHaveLength(5);
    });

    it('error transitorio seguido de éxito: el pendiente se confirma y se elimina', async () => {
        const { m } = startListening();
        let recordAttempts = 0;
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'resolve_playback_source') return Promise.resolve({ track_id: 7, file_path: '/lib/7.flac' });
            if (cmd === 'record_local_play') {
                recordAttempts++;
                return recordAttempts === 1
                    ? Promise.reject(new Error('database is locked'))
                    : Promise.resolve({ track_id: 7, play_count: 1, last_played: null, counted: true });
            }
            return Promise.resolve(null);
        });
        await player0(m);
        mockInvoke.mockClear();

        tick(m, 0);
        for (let i = 0; i < 120; i++) tick(m, (i + 1) * 0.25);
        await flushPromises();
        const player = usePlayer();
        expect(player.pendingListens.value).toHaveLength(1);

        await vi.advanceTimersByTimeAsync(1000);
        expect(recordCalls()).toHaveLength(2);
        expect(player.pendingListens.value).toHaveLength(0);
    });

    it('error definitivo (pista borrada): descarta sin reintentar', async () => {
        const { m } = startListening();
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'resolve_playback_source') return Promise.resolve({ track_id: 7, file_path: '/lib/7.flac' });
            if (cmd === 'record_local_play') return Promise.reject(new Error('El track 7 no existe en la biblioteca'));
            return Promise.resolve(null);
        });
        await player0(m);
        mockInvoke.mockClear();

        tick(m, 0);
        for (let i = 0; i < 120; i++) tick(m, (i + 1) * 0.25);
        await flushPromises();
        expect(recordCalls()).toHaveLength(1);
        const player = usePlayer();
        expect(player.pendingListens.value).toHaveLength(0);
        await vi.advanceTimersByTimeAsync(60000);
        expect(recordCalls()).toHaveLength(1); // sin reintentos
    });

    /** Reproduce la pista base con el estado del elemento ya preparado. */
    async function player0(m: MutableMedia): Promise<void> {
        const player = usePlayer();
        await player.play({ id: 7, title: 'Song', artist: 'Artist' });
        void m;
    }
});

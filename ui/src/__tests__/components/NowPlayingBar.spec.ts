/**
 * Fase 1 — controles globales de NowPlayingBar: transporte (next/previous),
 * modos persistentes (shuffle/repeat), volumen/mute y panel de cola con
 * reordenación accesible (botones subir/bajar/quitar + vaciar).
 *
 * usePlayer se mockea con una forma fija; los valores crudos (no refs) son
 * suficientes para el template y los labels usan toValue().
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount } from '@vue/test-utils';
import NowPlayingBar from '@/components/NowPlayingBar.vue';

const { state, fns } = vi.hoisted(() => ({
    state: {
        current: null as null | { id: number; title: string; artist: string; album?: string | null; coverUrl?: string | null },
        isPlaying: false,
        positionSec: 0,
        durationSec: 0,
        error: null as string | null,
        queue: [] as Array<{ entryId: string; trackId: number; track?: { id: number; title: string; artist: string; album?: string | null; coverUrl?: string | null } }>,
        shuffle: false,
        repeat: 'off' as 'off' | 'all' | 'one',
        volume: 1,
        muted: false,
    },
    fns: {
        toggle: vi.fn(),
        stop: vi.fn(),
        seek: vi.fn(),
        next: vi.fn(),
        previous: vi.fn(),
        toggleShuffle: vi.fn(),
        cycleRepeat: vi.fn(),
        setVolume: vi.fn(),
        setMuted: vi.fn(),
        clearQueue: vi.fn(),
        removeEntry: vi.fn(),
        moveEntry: vi.fn(),
        restoreFromPersistence: vi.fn(),
    },
}));

vi.mock('@/composables/usePlayer', () => ({
    usePlayer: () => ({ ...state, ...fns }),
}));

function resetState(): void {
    state.current = null;
    state.isPlaying = false;
    state.positionSec = 0;
    state.durationSec = 0;
    state.error = null;
    state.queue = [];
    state.shuffle = false;
    state.repeat = 'off';
    state.volume = 1;
    state.muted = false;
}

function trackBar() {
    state.current = { id: 7, title: 'Come Together', artist: 'The Beatles', album: 'Abbey Road', coverUrl: null };
    return mount(NowPlayingBar);
}

describe('NowPlayingBar — controles de fase 1', () => {
    beforeEach(() => {
        resetState();
        vi.clearAllMocks();
    });

    it('invoca restoreFromPersistence al montar (restauración perezosa) y no renderiza sin pista', () => {
        const wrapper = mount(NowPlayingBar);
        expect(fns.restoreFromPersistence).toHaveBeenCalledTimes(1);
        expect(wrapper.find('[data-testid="player-queue-toggle"]').exists()).toBe(false);
    });

    it('muestra transporte, modos, volumen y cola con pista en curso', () => {
        const wrapper = trackBar();
        for (const id of ['player-previous', 'player-next', 'player-shuffle', 'player-repeat', 'player-mute', 'player-volume', 'player-queue-toggle']) {
            expect(wrapper.find(`[data-testid="${id}"]`).exists()).toBe(true);
        }
    });

    it('next/previous delegan en el player', async () => {
        const wrapper = trackBar();
        await wrapper.find('[data-testid="player-next"]').trigger('click');
        await wrapper.find('[data-testid="player-previous"]').trigger('click');
        expect(fns.next).toHaveBeenCalledTimes(1);
        expect(fns.previous).toHaveBeenCalledTimes(1);
    });

    it('shuffle y repeat reflejan estado (aria-pressed/icono) y delegan', async () => {
        const wrapper = trackBar();
        expect(wrapper.find('[data-testid="player-shuffle"]').attributes('aria-pressed')).toBe('false');
        expect(wrapper.find('[data-testid="player-repeat"]').attributes('aria-pressed')).toBe('false');
        await wrapper.find('[data-testid="player-shuffle"]').trigger('click');
        await wrapper.find('[data-testid="player-repeat"]').trigger('click');
        expect(fns.toggleShuffle).toHaveBeenCalledTimes(1);
        expect(fns.cycleRepeat).toHaveBeenCalledTimes(1);

        state.shuffle = true;
        state.repeat = 'one';
        const wrapper2 = mount(NowPlayingBar);
        expect(wrapper2.find('[data-testid="player-shuffle"]').attributes('aria-pressed')).toBe('true');
        expect(wrapper2.find('[data-testid="player-repeat"]').attributes('aria-pressed')).toBe('true');
        expect(wrapper2.find('[data-testid="player-repeat"]').text()).toContain('repeat_one');
    });

    it('volumen y mute delegan con los valores del control', async () => {
        const wrapper = trackBar();
        await wrapper.find('[data-testid="player-volume"]').setValue('0.3');
        expect(fns.setVolume).toHaveBeenCalledWith(0.3);
        await wrapper.find('[data-testid="player-mute"]').trigger('click');
        expect(fns.setMuted).toHaveBeenCalledWith(true);
    });

    it('panel de cola: abre, lista entradas con placeholder, reordena por teclado, elimina y vacía', async () => {
        state.queue = [
            { entryId: 'e1', trackId: 11, track: { id: 11, title: 'Alpha', artist: 'A', album: null, coverUrl: null } },
            { entryId: 'e2', trackId: 99 },
        ];
        const wrapper = trackBar();
        expect(wrapper.find('[data-testid="player-queue-panel"]').exists()).toBe(false);

        await wrapper.find('[data-testid="player-queue-toggle"]').trigger('click');
        const panel = wrapper.find('[data-testid="player-queue-panel"]');
        expect(panel.exists()).toBe(true);
        const items = wrapper.findAll('[data-testid="player-queue-item"]');
        expect(items).toHaveLength(2);
        expect(items[0].text()).toContain('Alpha');
        // Entrada restaurada sin proyección: placeholder honesto con su ID.
        expect(items[1].text()).toContain('Pista #99');
        expect(wrapper.find('[data-testid="player-queue-count"]').text()).toBe('2');

        // Reordenación accesible: subir el último, bajar el primero.
        await wrapper.find('[data-testid="queue-up-1"]').trigger('click');
        expect(fns.moveEntry).toHaveBeenLastCalledWith('e2', 0);
        await wrapper.find('[data-testid="queue-down-0"]').trigger('click');
        expect(fns.moveEntry).toHaveBeenLastCalledWith('e1', 1);
        // El primero no puede subir más: deshabilitado.
        expect(wrapper.find('[data-testid="queue-up-0"]').attributes('disabled')).toBeDefined();

        // Eliminar una entrada.
        await wrapper.find('[data-testid="queue-remove-1"]').trigger('click');
        expect(fns.removeEntry).toHaveBeenCalledWith('e2');

        // Vaciar la cola (solo pendientes).
        await wrapper.find('[data-testid="player-queue-clear"]').trigger('click');
        expect(fns.clearQueue).toHaveBeenCalledTimes(1);
    });

    it('panel de cola con cola vacía muestra estado vacío y deshabilita Vaciar', async () => {
        const wrapper = trackBar();
        await wrapper.find('[data-testid="player-queue-toggle"]').trigger('click');
        expect(wrapper.find('[data-testid="player-queue-empty"]').exists()).toBe(true);
        expect(wrapper.find('[data-testid="player-queue-clear"]').attributes('disabled')).toBeDefined();
    });

    it('renderiza el error del player como role=alert cuando existe', () => {
        // Regresión: player.error se escribía sin ningún consumidor — los
        // fallos de avance de cola eran 100% silenciosos (sin DOM, sin
        // aria-live). role=alert implica aria-live=assertive.
        state.error = 'El track 7 no tiene archivo local descargado; descárgalo primero';
        const wrapper = trackBar();
        const alert = wrapper.find('[data-testid="player-error"]');
        expect(alert.exists()).toBe(true);
        expect(alert.attributes('role')).toBe('alert');
        expect(alert.text()).toContain('no tiene archivo local');
    });

    it('no renderiza la región de error cuando error es null', () => {
        const wrapper = trackBar();
        expect(wrapper.find('[data-testid="player-error"]').exists()).toBe(false);
    });

    it('el slider de seek tiene nombre accesible y valuetext temporal', () => {
        // Regresión: el input de transporte era el único control de formulario
        // sin aria-label (el de volumen ya lo tenía).
        state.positionSec = 83;
        state.durationSec = 200;
        const wrapper = trackBar();
        const seek = wrapper.find('[data-testid="player-seek"]');
        expect(seek.attributes('aria-label')).toBe('Posición de la pista');
        expect(seek.attributes('aria-valuetext')).toBe('1:23 de 3:20');
    });
});

/**
 * NowPlayingBarCover.spec.ts — regresión de R14 en «Ahora suena».
 *
 * La barra antes pintaba un `music_note` sobre un cuadro gris cuando la pista
 * en curso no traía carátula; ahora usa el mismo componente que el resto de la
 * aplicación, con su gradiente estable.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';

const state = {
    current: null as null | { id: number; title: string; artist: string; album?: string | null; coverUrl?: string | null },
    isPlaying: false,
};

vi.mock('@/composables/usePlayer', () => ({
    usePlayer: () => ({
        current: state.current,
        isPlaying: state.isPlaying,
        positionSec: { value: 0 },
        durationSec: { value: 0 },
        toggle: vi.fn(),
        stop: vi.fn(),
        seek: vi.fn(),
    }),
}));

import NowPlayingBar from '@/components/NowPlayingBar.vue';

function bar() {
    const wrapper = mount(NowPlayingBar);
    return wrapper;
}

describe('NowPlayingBar — portada de la pista en curso', () => {
    beforeEach(() => {
        state.current = null;
        state.isPlaying = false;
    });

    it('no se pinta nada sin una pista en curso', () => {
        const wrapper = bar();
        expect(wrapper.find('[data-testid="track-cover"]').exists()).toBe(false);
    });

    it('muestra la carátula cuando la pista la trae', async () => {
        state.current = { id: 7, title: 'Come Together', artist: 'The Beatles', album: 'Abbey Road', coverUrl: 'https://cdn.test/a.jpg' };
        const wrapper = bar();
        await flushPromises();

        const img = wrapper.find('[data-testid="track-cover-img"]');
        expect(img.exists()).toBe(true);
        expect(img.attributes('src')).toBe('https://cdn.test/a.jpg');
    });

    it('cae al gradiente estable cuando la pista no trae carátula', async () => {
        state.current = { id: 7, title: 'Come Together', artist: 'The Beatles', coverUrl: null };
        const wrapper = bar();
        await flushPromises();

        expect(wrapper.find('[data-testid="track-cover-img"]').exists()).toBe(false);
        const classes = wrapper.find('[data-testid="track-cover"]').classes().join(' ');
        expect(classes).toContain('bg-gradient-to-br');
    });
});
import { describe, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import PlaylistCover from '@/components/PlaylistCover.vue';

const tiles = [
    { src: 'https://cdn.test/a.jpg', itemId: 1 },
    { src: 'https://cdn.test/b.jpg', itemId: 2 },
    { src: 'https://cdn.test/c.jpg', itemId: 3 },
    { src: 'https://cdn.test/d.jpg', itemId: 4 },
];

describe('PlaylistCover', () => {
    it('da prioridad a la imagen del proveedor', () => {
        const wrapper = mount(PlaylistCover, {
            props: {
                src: 'https://cdn.test/provider.jpg',
                alt: 'Playlist remota',
                itemId: 10,
                tiles,
            },
        });

        const image = wrapper.get('[data-testid="playlist-cover-provider-img"]');
        expect(image.attributes('src')).toBe('https://cdn.test/provider.jpg');
        expect(image.attributes('alt')).toBe('Playlist remota');
        expect(wrapper.find('[data-testid="playlist-cover-mosaic"]').exists()).toBe(false);
    });

    it('pinta un mosaico estable de hasta cuatro portadas cuando no hay imagen', () => {
        const wrapper = mount(PlaylistCover, { props: { itemId: 10, tiles } });

        const mosaic = wrapper.get('[data-testid="playlist-cover-mosaic"]');
        expect(mosaic.findAll('[data-testid="track-cover-img"]').map(image => image.attributes('src'))).toEqual([
            'https://cdn.test/a.jpg',
            'https://cdn.test/b.jpg',
            'https://cdn.test/c.jpg',
            'https://cdn.test/d.jpg',
        ]);
    });

    it('cae al mosaico si la imagen del proveedor no carga', async () => {
        const wrapper = mount(PlaylistCover, {
            props: { src: 'https://cdn.test/missing.jpg', itemId: 10, tiles },
        });

        await wrapper.get('[data-testid="playlist-cover-provider-img"]').trigger('error');

        expect(wrapper.find('[data-testid="playlist-cover-provider-img"]').exists()).toBe(false);
        expect(wrapper.findAll('[data-testid="playlist-cover-mosaic"] [data-testid="track-cover-img"]')).toHaveLength(4);
    });

    it('reintenta una imagen nueva después de un fallo', async () => {
        const wrapper = mount(PlaylistCover, {
            props: { src: 'https://cdn.test/missing.jpg', itemId: 10, tiles },
        });
        await wrapper.get('[data-testid="playlist-cover-provider-img"]').trigger('error');

        await wrapper.setProps({ src: 'https://cdn.test/fixed.jpg' });

        expect(wrapper.get('[data-testid="playlist-cover-provider-img"]').attributes('src')).toBe('https://cdn.test/fixed.jpg');
    });

    it('usa un gradiente determinista si tampoco hay portadas de pistas', () => {
        const wrapper = mount(PlaylistCover, { props: { itemId: 10, tiles: [] } });

        expect(wrapper.find('[data-testid="playlist-cover-mosaic"]').exists()).toBe(false);
        expect(wrapper.get('[data-testid="playlist-cover"]').classes()).toContain('from-green-500');
        expect(wrapper.get('[data-testid="playlist-cover"]').classes()).toContain('to-emerald-500');
    });
});

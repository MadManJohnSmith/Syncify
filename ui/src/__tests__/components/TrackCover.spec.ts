/**
 * TrackCover.spec.ts — regresión de R14 (portadas en todas las pantallas).
 *
 * La portada es el componente que decide cómo se ve una carátula en Library,
 * Playlists, Descargas, Búsqueda, las fichas de álbum/artista y Ahora suena.
 * PlaylistCover añade la precedencia de imagen del proveedor y el mosaico.
 */
import { describe, it, expect } from 'vitest';
import { mount } from '@vue/test-utils';
import TrackCover from '@/components/TrackCover.vue';
import PlaylistCover from '@/components/PlaylistCover.vue';

function coverClasses(wrapper: ReturnType<typeof mount>) {
    return wrapper.find('[data-testid="track-cover"]').classes().join(' ');
}

describe('TrackCover', () => {
    it('pinta la imagen cuando hay URL', () => {
        const wrapper = mount(TrackCover, {
            props: { src: 'https://cdn.test/cover.jpg', alt: 'Abbey Road', itemId: 7 },
        });

        const img = wrapper.find('[data-testid="track-cover-img"]');
        expect(img.exists()).toBe(true);
        expect(img.attributes('src')).toBe('https://cdn.test/cover.jpg');
        expect(img.attributes('alt')).toBe('Abbey Road');
    });

    it('cae al gradiente determinista cuando no hay URL', () => {
        const wrapper = mount(TrackCover, { props: { src: null, itemId: 3 } });

        expect(wrapper.find('[data-testid="track-cover-img"]').exists()).toBe(false);
        expect(coverClasses(wrapper)).toContain('bg-gradient-to-br');
    });

    it('el mismo id produce el mismo gradiente entre instancias', () => {
        const a = mount(TrackCover, { props: { src: null, itemId: 42 } });
        const b = mount(TrackCover, { props: { src: null, itemId: 42 } });
        expect(coverClasses(a)).toBe(coverClasses(b));
    });

    it('cae al gradiente si la URL existe pero el fichero ya no se puede cargar', async () => {
        const wrapper = mount(TrackCover, { props: { src: 'https://cdn.test/missing.jpg', itemId: 1 } });
        expect(wrapper.find('[data-testid="track-cover-img"]').exists()).toBe(true);

        await wrapper.find('[data-testid="track-cover-img"]').trigger('error');

        expect(wrapper.find('[data-testid="track-cover-img"]').exists()).toBe(false);
        expect(coverClasses(wrapper)).toContain('bg-gradient-to-br');
    });

    it('reintenta cuando llega una URL nueva tras un fallo', async () => {
        const wrapper = mount(TrackCover, { props: { src: 'https://cdn.test/missing.jpg', itemId: 1 } });
        await wrapper.find('[data-testid="track-cover-img"]').trigger('error');
        expect(wrapper.find('[data-testid="track-cover-img"]').exists()).toBe(false);

        await wrapper.setProps({ src: 'https://cdn.test/fixed.jpg' });

        const img = wrapper.find('[data-testid="track-cover-img"]');
        expect(img.exists()).toBe(true);
        expect(img.attributes('src')).toBe('https://cdn.test/fixed.jpg');
    });

    it('el ancho y alto por defecto salen de `size` cuando no hay sizeClass', () => {
        const wrapper = mount(TrackCover, { props: { src: 'https://cdn.test/a.jpg', size: 56 } });
        const style = wrapper.find('[data-testid="track-cover"]').attributes('style') ?? '';
        expect(style).toContain('width: 56px');
        expect(style).toContain('height: 56px');
    });

    it('sizeClass manda sobre `size`', () => {
        const wrapper = mount(TrackCover, {
            props: { src: 'https://cdn.test/a.jpg', size: 56, sizeClass: 'w-10 h-10' },
        });
        const style = wrapper.find('[data-testid="track-cover"]').attributes('style') ?? '';
        expect(style).not.toContain('56px');
        expect(wrapper.find('[data-testid="track-cover"]').classes()).toContain('w-10');
    });
});

const playlistCoverTiles = [
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
                tiles: playlistCoverTiles,
            },
        });

        const image = wrapper.get('[data-testid="playlist-cover-provider-img"]');
        expect(image.attributes('src')).toBe('https://cdn.test/provider.jpg');
        expect(image.attributes('alt')).toBe('Playlist remota');
        expect(wrapper.find('[data-testid="playlist-cover-mosaic"]').exists()).toBe(false);
    });

    it('pinta un mosaico estable de hasta cuatro portadas cuando no hay imagen', () => {
        const wrapper = mount(PlaylistCover, { props: { itemId: 10, tiles: playlistCoverTiles } });

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
            props: { src: 'https://cdn.test/missing.jpg', itemId: 10, tiles: playlistCoverTiles },
        });

        await wrapper.get('[data-testid="playlist-cover-provider-img"]').trigger('error');

        expect(wrapper.find('[data-testid="playlist-cover-provider-img"]').exists()).toBe(false);
        expect(wrapper.findAll('[data-testid="playlist-cover-mosaic"] [data-testid="track-cover-img"]')).toHaveLength(4);
    });

    it('reintenta una imagen nueva después de un fallo', async () => {
        const wrapper = mount(PlaylistCover, {
            props: { src: 'https://cdn.test/missing.jpg', itemId: 10, tiles: playlistCoverTiles },
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
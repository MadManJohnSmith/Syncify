import { describe, expect, it } from 'vitest';
import { buildPlaylistMosaic, reconcilePlaylistMosaic, aggregatePlaylists, type PlaylistMosaicCache } from '@/utils/playlistCatalog';
import type { Playlist } from '@/api/types';

describe('buildPlaylistMosaic', () => {
    it('toma hasta cuatro portadas distintas en orden y omite huecos y duplicados', () => {
        const tracks = [
            { id: 1, position: 1, cover_art_url: null },
            { id: 2, position: 2, cover_art_url: 'https://cdn.test/a.jpg' },
            { id: 3, position: 3, cover_art_url: 'https://cdn.test/a.jpg' },
            { id: 4, position: 4, cover_art_url: 'https://cdn.test/b.jpg' },
            { id: 5, position: 5, cover_art_url: '' },
            { id: 6, position: 6, cover_art_url: 'https://cdn.test/c.jpg' },
            { id: 7, position: 7, cover_art_url: 'https://cdn.test/d.jpg' },
            { id: 8, position: 8, cover_art_url: 'https://cdn.test/e.jpg' },
        ];

        expect(buildPlaylistMosaic(tracks)).toEqual([
            { src: 'https://cdn.test/a.jpg', itemId: 2 },
            { src: 'https://cdn.test/b.jpg', itemId: 4 },
            { src: 'https://cdn.test/c.jpg', itemId: 6 },
            { src: 'https://cdn.test/d.jpg', itemId: 7 },
        ]);
    });

    it('no inventa portadas cuando ninguna pista las tiene', () => {
        expect(buildPlaylistMosaic([
            { id: 1, position: 1, cover_art_url: null },
            { id: 2, position: 2, cover_art_url: '' },
        ])).toEqual([]);
    });
});

describe('reconcilePlaylistMosaic', () => {
    it('reutiliza el mosaico mientras la pertenencia no cambia', () => {
        const cache: PlaylistMosaicCache = new Map();
        const tracks = [{ id: 1, position: 1, cover_art_url: 'https://cdn.test/a.jpg' }];

        const first = reconcilePlaylistMosaic(cache, 10, tracks);

        expect(reconcilePlaylistMosaic(cache, 10, tracks)).toBe(first);
    });

    it('invalida el mosaico al añadir o quitar una ocurrencia', () => {
        const cache: PlaylistMosaicCache = new Map();
        const original = [{ id: 1, position: 1, cover_art_url: 'https://cdn.test/a.jpg' }];
        const first = reconcilePlaylistMosaic(cache, 10, original);

        const added = reconcilePlaylistMosaic(cache, 10, [
            ...original,
            { id: 2, position: 2, cover_art_url: 'https://cdn.test/b.jpg' },
        ]);
        expect(added).toEqual([
            { src: 'https://cdn.test/a.jpg', itemId: 1 },
            { src: 'https://cdn.test/b.jpg', itemId: 2 },
        ]);
        expect(added).not.toBe(first);

        const removed = reconcilePlaylistMosaic(cache, 10, [original[0]]);
        expect(removed).toEqual([{ src: 'https://cdn.test/a.jpg', itemId: 1 }]);
        expect(removed).not.toBe(added);
    });

    it('invalida el mosaico al reordenar las ocurrencias', () => {
        const cache: PlaylistMosaicCache = new Map();
        const a = { id: 1, position: 1, cover_art_url: 'https://cdn.test/a.jpg' };
        const b = { id: 2, position: 2, cover_art_url: 'https://cdn.test/b.jpg' };
        reconcilePlaylistMosaic(cache, 10, [a, b]);

        expect(reconcilePlaylistMosaic(cache, 10, [
            { ...b, position: 1 },
            { ...a, position: 2 },
        ])).toEqual([
            { src: 'https://cdn.test/b.jpg', itemId: 2 },
            { src: 'https://cdn.test/a.jpg', itemId: 1 },
        ]);
    });
});

describe('aggregatePlaylists', () => {
    it('agrupa locales, servicio y smart y aplica la portada correcta a cada una', () => {
        const playlists: Playlist[] = [
            { id: 1, name: 'Local', description: null, track_count: 2, image_url: 'https://cdn.test/fallback.jpg' },
            {
                id: 2,
                name: 'Remota',
                description: null,
                track_count: 3,
                image_url: 'https://cdn.test/fallback.jpg',
                service_name: 'Spotify',
            },
            {
                id: 3,
                name: 'Smart',
                description: null,
                track_count: 4,
                image_url: 'https://cdn.test/ignored.jpg',
                service_name: 'Qobuz',
                is_smart: true,
            },
        ];
        const localTiles = [{ src: 'https://cdn.test/local.jpg', itemId: 10 }];
        const smartTiles = [{ src: 'https://cdn.test/smart.jpg', itemId: 30 }];

        const catalog = aggregatePlaylists(playlists, {
            1: { tiles: localTiles },
            2: { providerImageUrl: 'https://cdn.test/provider.jpg', tiles: [] },
            3: { tiles: smartTiles },
        });

        expect(catalog.local).toHaveLength(1);
        expect(catalog.local[0]).toMatchObject({
            id: 1,
            category: 'local',
            coverImageUrl: null,
            mosaicTiles: localTiles,
        });
        expect(catalog.services).toHaveLength(1);
        expect(catalog.services[0]).toMatchObject({ id: 'spotify', name: 'Spotify' });
        expect(catalog.services[0].playlists[0]).toMatchObject({
            id: 2,
            category: 'service',
            serviceKey: 'spotify',
            coverImageUrl: 'https://cdn.test/provider.jpg',
        });
        expect(catalog.smart[0]).toMatchObject({
            id: 3,
            category: 'smart',
            serviceKey: null,
            coverImageUrl: null,
            mosaicTiles: smartTiles,
        });
    });

    it('usa source_service, reconoce servicios nuevos y no confunde local con un proveedor', () => {
        const playlists: Playlist[] = [
            { id: 1, name: 'Local', description: null, track_count: 0, service_name: 'local' },
            {
                id: 2,
                name: 'Nuevo',
                description: null,
                track_count: 0,
                service_name: 'spotify',
                source_service: 'bandcamp',
            },
        ];

        const catalog = aggregatePlaylists(playlists, {});

        expect(catalog.local.map(item => item.id)).toEqual([1]);
        expect(catalog.services.map(group => group.id)).toEqual(['bandcamp']);
        expect(catalog.services[0].name).toBe('Bandcamp');
    });
});

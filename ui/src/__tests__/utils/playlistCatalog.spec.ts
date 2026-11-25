import { describe, expect, it } from 'vitest';
import { buildPlaylistMosaic, reconcilePlaylistMosaic, type PlaylistMosaicCache } from '@/utils/playlistCatalog';

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

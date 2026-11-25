import { describe, expect, it } from 'vitest';
import { buildPlaylistMosaic } from '@/utils/playlistCatalog';

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

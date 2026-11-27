/**
 * catalogSearch.spec.ts — P10b (#409): envoltorios IPC del catálogo.
 *
 * Contrato: los nombres de comando son literales (la puerta
 * dead_commands_hygiene_test los busca en el código de producción), el
 * `service_id` viaja en el resultado de búsqueda y las fichas piden al backend
 * solo la identidad que la vista ya está mirando.
 */
import { beforeEach, describe, expect, it } from 'vitest';
import {
    CATALOG_PAGE_SIZE,
    catalogAlbumTracks,
    catalogArtistAlbums,
    catalogSearchProviders,
} from '@/api/catalogSearch';
import { mockInvoke, resetMocks } from '../setup';

describe('catalogSearch API', () => {
    beforeEach(() => {
        resetMocks();
    });

    it('busca en proveedores con la página por defecto del gateway', async () => {
        mockInvoke((command, args) => {
            expect(command).toBe('catalog_search');
            expect(args).toEqual({ query: 'daft punk', offset: 0, limit: CATALOG_PAGE_SIZE });
            return { query: 'daft punk', page: { offset: 0, limit: 20 }, providers: [] };
        });

        await expect(catalogSearchProviders('daft punk')).resolves.toEqual({
            query: 'daft punk',
            page: { offset: 0, limit: 20 },
            providers: [],
        });
    });

    it('reenvía offset y limit explícitos sin inventar totales', async () => {
        mockInvoke((command, args) => {
            expect(command).toBe('catalog_search');
            expect(args).toEqual({ query: 'discovery', offset: 40, limit: 10 });
            return {
                query: 'discovery',
                page: { offset: 40, limit: 10 },
                providers: [{
                    provider: 'qobuz',
                    service_id: 2,
                    account_id: 7,
                    tracks: { page: { items: [], total: null, offset: 40, limit: 10 } },
                    albums: { page: { items: [], total: null, offset: 40, limit: 10 } },
                    artists: { page: { items: [], total: null, offset: 40, limit: 10 } },
                }],
            };
        });

        const response = await catalogSearchProviders('discovery', 40, 10);
        expect(response.providers[0].service_id).toBe(2);
        expect(response.providers[0].tracks.page?.total).toBeNull();
    });

    it('pide las pistas del álbum con título y artista', async () => {
        mockInvoke((command, args) => {
            expect(command).toBe('catalog_album_tracks');
            expect(args).toEqual({
                provider: 'qobuz',
                title: 'Discovery',
                artist: 'Daft Punk',
            });
            return { page: { items: [], total: null, offset: 0, limit: 100 } };
        });

        await catalogAlbumTracks('qobuz', 'Discovery', 'Daft Punk');
    });

    it('omite el artista cuando el álbum no lo declara', async () => {
        mockInvoke((command, args) => {
            expect(command).toBe('catalog_album_tracks');
            expect(args).toEqual({
                provider: 'qobuz',
                title: 'Discovery',
                artist: undefined,
            });
            return { page: { items: [], total: null, offset: 0, limit: 100 } };
        });

        await catalogAlbumTracks('qobuz', 'Discovery', null);
    });

    it('pide los álbumes del artista por nombre', async () => {
        mockInvoke((command, args) => {
            expect(command).toBe('catalog_artist_albums');
            expect(args).toEqual({ provider: 'qobuz', name: 'Daft Punk' });
            return { page: { items: [], total: null, offset: 0, limit: 100 } };
        });

        await catalogArtistAlbums('qobuz', 'Daft Punk');
    });
});

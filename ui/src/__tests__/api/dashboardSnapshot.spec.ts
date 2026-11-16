/**
 * dashboardSnapshot.spec.ts
 * FE-PERF-04 (#51): contrato de la lectura unificada del Dashboard.
 *
 * El payload de `get_dashboard_snapshot` queda fijado por test: las mismas
 * cifras que el Dashboard mostraba leyendo por separado
 * (`get_library_stats` + `create_library_snapshot` + `get_library_snapshots`
 * + `get_metadata_stats` + `get_lyrics_stats`) llegan ahora en un único
 * resultado. Criterio de aceptación: "hay test de contrato del snapshot".
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { getDashboardSnapshot } from '@/api/dashboard';
import { mockInvoke, resetMocks } from '../setup';

describe('dashboard_snapshot_unified_contract_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('mapea campo a campo el payload unificado completo', async () => {
        mockInvoke((cmd) => (cmd === 'get_dashboard_snapshot'
            ? {
                stats: {
                    total_tracks: 120, total_artists: 45, total_albums: 60,
                    total_downloads: 80, queued_downloads: 3, active_downloads: 2,
                    library_entries: 150, playlists: 7, services_with_data: 2,
                },
                snapshot: {
                    id: 9, snapshot_date: '2026-11-16', total_tracks: 120,
                    total_albums: 60, total_artists: 45, total_size_bytes: 48000,
                    tracks_with_lyrics: 40, tracks_lossless: 55, tracks_hires: 12,
                    downloaded_tracks: 80,
                },
                snapshots: [
                    {
                        id: 8, snapshot_date: '2026-11-15', total_tracks: 118,
                        total_albums: 59, total_artists: 44, total_size_bytes: 47000,
                        tracks_with_lyrics: 39, tracks_lossless: 54, tracks_hires: 11,
                        downloaded_tracks: 79,
                    },
                ],
                metadata: {
                    total_tracks: 120, with_isrc: 100, with_musicbrainz_id: 95,
                    with_album: 118, with_year: 90, with_genre: 88, with_art: 70,
                    average_completeness: 82.5,
                },
                lyrics: {
                    total_tracks: 120, with_lyrics: 40, synced_lyrics: 30,
                    embedded_lyrics: 5,
                },
            }
            : null));

        const payload = await getDashboardSnapshot(30);

        // Vista library_stats — idéntica a la que devolvía get_library_stats.
        expect(payload.stats).toEqual({
            total_tracks: 120, total_artists: 45, total_albums: 60,
            total_downloads: 80, queued_downloads: 3, active_downloads: 2,
            library_entries: 150, playlists: 7, services_with_data: 2,
        });
        // Snapshot del día — idéntico al de create_library_snapshot.
        expect(payload.snapshot).toEqual({
            id: 9, snapshot_date: '2026-11-16', total_tracks: 120,
            total_albums: 60, total_artists: 45, total_size_bytes: 48000,
            tracks_with_lyrics: 40, tracks_lossless: 55, tracks_hires: 12,
            downloaded_tracks: 80,
        });
        // Historial — idéntico al de get_library_snapshots.
        expect(payload.snapshots).toHaveLength(1);
        expect(payload.snapshots[0]).toEqual({
            id: 8, snapshot_date: '2026-11-15', total_tracks: 118,
            total_albums: 59, total_artists: 44, total_size_bytes: 47000,
            tracks_with_lyrics: 39, tracks_lossless: 54, tracks_hires: 11,
            downloaded_tracks: 79,
        });
        // Metadatos — idénticos a los de get_metadata_stats.
        expect(payload.metadata).toEqual({
            total_tracks: 120, with_isrc: 100, with_musicbrainz_id: 95,
            with_album: 118, with_year: 90, with_genre: 88, with_art: 70,
            average_completeness: 82.5,
        });
        // Letras — idénticas a las de get_lyrics_stats.
        expect(payload.lyrics).toEqual({
            total_tracks: 120, with_lyrics: 40, synced_lyrics: 30, embedded_lyrics: 5,
        });
    });

    it('acepta el número de días como argumento de la lectura', async () => {
        const seen: Record<string, unknown> = {};
        mockInvoke((cmd, args) => {
            seen[cmd] = args;
            return null;
        });
        await getDashboardSnapshot(7);
        expect(seen['get_dashboard_snapshot']).toMatchObject({ days: 7 });
    });

    it('normaliza a cero todo campo ausente sin lanzar', async () => {
        mockInvoke((cmd) => (cmd === 'get_dashboard_snapshot' ? {
            stats: { total_tracks: 3 },
            snapshot: { id: 1 },
            snapshots: 'not-an-array',
            metadata: {},
            lyrics: null,
        } : null));

        const payload = await getDashboardSnapshot();

        expect(payload.stats.total_tracks).toBe(3);
        expect(payload.stats.total_albums).toBe(0);
        expect(payload.stats.services_with_data).toBe(0);
        expect(payload.snapshot.id).toBe(1);
        expect(payload.snapshot.snapshot_date).toBe('');
        expect(payload.snapshot.downloaded_tracks).toBe(0);
        expect(payload.snapshots).toEqual([]);
        expect(payload.metadata.total_tracks).toBe(0);
        expect(payload.metadata.average_completeness).toBe(0);
        expect(payload.lyrics.with_lyrics).toBe(0);
    });

    it('normaliza a un payload vacío una respuesta nula', async () => {
        mockInvoke(() => null);

        const payload = await getDashboardSnapshot();

        expect(payload.stats).toEqual({
            total_tracks: 0, total_artists: 0, total_albums: 0, total_downloads: 0,
            queued_downloads: 0, active_downloads: 0, library_entries: 0,
            playlists: 0, services_with_data: 0,
        });
        expect(payload.snapshot).toEqual({
            id: 0, snapshot_date: '', total_tracks: 0, total_albums: 0,
            total_artists: 0, total_size_bytes: 0, tracks_with_lyrics: 0,
            tracks_lossless: 0, tracks_hires: 0, downloaded_tracks: 0,
        });
        expect(payload.snapshots).toEqual([]);
        expect(payload.metadata.total_tracks).toBe(0);
        expect(payload.lyrics.total_tracks).toBe(0);
    });
});

/**
 * dashboard.spec.ts
 * Regression tests: missing fields in dashboard command responses must default safely.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    getDashboardStats,
    getHealthChecks,
    getServiceHealth,
    getLibrarySnapshots,
    getDuplicateStats,
    autoResolveDuplicates,
} from '@/api/dashboard';
import { mockInvoke, resetMocks } from '../setup';

describe('dashboard_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('defaults every rendered counter and list of get_dashboard_stats', async () => {
        mockInvoke((cmd) => (cmd === 'get_dashboard_stats' ? { total_tracks: 12 } : null));

        const stats = await getDashboardStats();

        expect(stats.total_tracks).toBe(12);
        expect(stats.total_albums).toBe(0);
        expect(stats.total_artists).toBe(0);
        expect(stats.total_playlists).toBe(0);
        expect(stats.total_downloads).toBe(0);
        expect(stats.total_favorites).toBe(0);
        expect(stats.lyrics_coverage_percentage).toBe(0);
        expect(stats.enriched_metadata_percentage).toBe(0);
        expect(stats.services).toEqual([]);
        expect(stats.quality_distribution).toEqual([]);
    });

    it('resolves to a fully zeroed DashboardStats on null payload', async () => {
        mockInvoke(() => null);

        const stats = await getDashboardStats();

        expect(stats.total_tracks).toBe(0);
        expect(stats.services).toEqual([]);
        expect(stats.quality_distribution).toEqual([]);
    });

    it('normalizes get_health_checks with boolean coercion and service defaults', async () => {
        mockInvoke((cmd) => (cmd === 'get_health_checks'
            ? { database_ok: true, ffmpeg_ok: 'yes', services: [{ service: 'tidal', is_connected: true }] }
            : null));

        const health = await getHealthChecks();

        expect(health.database_ok).toBe(true);
        expect(health.ffmpeg_ok).toBe(false); // non-boolean → false
        expect(health.background_worker_active).toBe(false);
        expect(health.services).toHaveLength(1);
        expect(health.services[0].service).toBe('tidal');
        expect(health.services[0].is_connected).toBe(true);
        expect(health.services[0].token_status).toBe('');
    });

    it('coerces non-array list responses to empty arrays', async () => {
        mockInvoke(() => null);

        expect(await getServiceHealth()).toEqual([]);
        expect(await getLibrarySnapshots(7)).toEqual([]);
        expect(await getDuplicateStats()).toBe(0);
    });

    it('normalizes auto_resolve_duplicates counters', async () => {
        mockInvoke((cmd) => (cmd === 'auto_resolve_duplicates' ? { groups_resolved: 3, tracksRemoved: 5 } : null));
        const res = await autoResolveDuplicates();
        expect(res.groups_resolved).toBe(3);
        expect(res.tracks_removed).toBe(5);

        mockInvoke(() => null);
        const empty = await autoResolveDuplicates();
        expect(empty.groups_resolved).toBe(0);
        expect(empty.tracks_removed).toBe(0);
    });
});

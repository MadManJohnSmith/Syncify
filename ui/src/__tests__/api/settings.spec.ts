/**
 * settings.spec.ts
 * Regression tests: get_download_settings tolerates every legacy payload shape.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { getDownloadSettings, deriveStagingRoot, determinePathStatus, getServicePreferredAccounts, setServicePreferredAccount } from '@/api/settings';
import { invoke } from '@tauri-apps/api/core';
import { mockInvoke, resetMocks } from '../setup';

describe('settings_handles_missing_fields_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('maps the canonical snake_case DownloadSettingsDto fields', async () => {
        mockInvoke((cmd) => (cmd === 'get_download_settings'
            ? {
                library_root: 'D:\\Music',
                staging_root: 'D:\\Music\\.staging',
                max_concurrent_downloads: 4,
                fallback_action: 'keep_both',
            }
            : null));

        const settings = await getDownloadSettings();

        expect(settings.download_path).toBe('D:\\Music');
        expect(settings.temporary_root).toBe('D:\\Music\\.staging');
        expect(settings.concurrent_downloads).toBe(4);
        expect(settings.fallback_action).toBe('keep_both');
        expect(settings.folder_settings?.base_folder).toBe('D:\\Music');
    });

    it('derives safe defaults when the payload is an empty object', async () => {
        mockInvoke((cmd) => (cmd === 'get_download_settings' ? {} : null));

        const settings = await getDownloadSettings();

        expect(settings.download_path).toBe('');
        expect(settings.temporary_root).toBe('');
        expect(settings.concurrent_downloads).toBe(3); // default concurrency
        expect(settings.fallback_action).toBe('try_next');
        expect(settings.folder_settings?.folder_template).toBe('{AlbumArtist}/{Album}');
    });

    it('falls back to KV settings when the command resolves null', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_kv_settings') return { dl_download_path: 'E:\\Lib', dl_concurrent_downloads: '5' };
            if (cmd === 'get_default_download_path') return 'C:\\Fallback';
            return null;
        });

        const settings = await getDownloadSettings();

        expect(settings.download_path).toBe('E:\\Lib');
        expect(settings.temporary_root).toBe(deriveStagingRoot('E:\\Lib'));
        expect(settings.concurrent_downloads).toBe(5);
    });

    it('determinePathStatus maps validation results to renderable statuses', () => {
        expect(determinePathStatus(null)).toBe('valid');
        expect(determinePathStatus({ valid: true, exists: true, is_dir: true, is_writable: true, available_bytes: 1, drive_mounted: true, canonical_path: '' })).toBe('valid');
        expect(determinePathStatus({ valid: false, exists: false, is_dir: false, is_writable: false, available_bytes: 0, drive_mounted: true, canonical_path: '' })).toBe('missing');
        expect(determinePathStatus({ valid: false, exists: true, is_dir: true, is_writable: false, available_bytes: 0, drive_mounted: true, canonical_path: '' })).toBe('not_writable');
        expect(determinePathStatus({ valid: false, exists: true, is_dir: true, is_writable: true, available_bytes: 0, drive_mounted: false, canonical_path: '' })).toBe('unavailable');
    });

    it('deriveStagingRoot tolerates unknown and non-string inputs without throwing TypeError', () => {
        expect(deriveStagingRoot(null)).toBe('');
        expect(deriveStagingRoot(undefined)).toBe('');
        expect(deriveStagingRoot([])).toBe('');
        expect(deriveStagingRoot(['some/path'])).toBe('');
        expect(deriveStagingRoot({})).toBe('');
        expect(deriveStagingRoot(123)).toBe('');
        expect(deriveStagingRoot('   ')).toBe('');
        expect(deriveStagingRoot('/music/library')).toBe('/music/library/.staging');
        expect(deriveStagingRoot('C:\\Music\\Library\\')).toBe('C:\\Music\\Library\\.staging');
    });

    it('reads and persists the preferred account per service', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_service_preferred_accounts') {
                return [{ service_name: 'qobuz', account_id: 7, updated_at: '2026-11-18 10:00:00' }];
            }
            if (cmd === 'set_service_preferred_account') {
                return { service_name: 'qobuz', account_id: 8, updated_at: '2026-11-18 10:05:00' };
            }
            return null;
        });

        const rows = await getServicePreferredAccounts();
        expect(rows).toEqual([
            { service_name: 'qobuz', account_id: 7, updated_at: '2026-11-18 10:00:00' },
        ]);

        const saved = await setServicePreferredAccount('qobuz', 8);
        expect(saved?.account_id).toBe(8);
        expect(invoke).toHaveBeenCalledWith('set_service_preferred_account', {
            serviceName: 'qobuz',
            accountId: 8,
        });

        await setServicePreferredAccount('qobuz', null);
        expect(invoke).toHaveBeenCalledWith('set_service_preferred_account', {
            serviceName: 'qobuz',
            accountId: null,
        });
    });
});

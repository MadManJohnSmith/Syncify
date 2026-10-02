/**
 * tray.spec.ts
 * Regression tests for the tray api surface (IN-5 triage): the tray settings
 * sync, tray download status and tray icon commands must be invoked with the
 * exact backend command names and camelCase payloads.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
    getTraySettings,
    updateTraySettings,
    updateTrayIcon,
    updateTrayStatus,
    trayApi,
} from '@/api/tray';
import { mockInvoke, resetMocks } from '../setup';

describe('tray_api_commands_test', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('getTraySettings invokes get_tray_settings and normalizes a partial payload', async () => {
        mockInvoke((cmd) => (cmd === 'get_tray_settings' ? { closeToTray: false } : null));

        const settings = await getTraySettings();
        expect(settings.closeToTray).toBe(false);
        // Defaults cover the fields the backend omits (matches Rust TraySettings::default()).
        expect(settings.showTrayIcon).toBe(true);
        expect(settings.notificationsEnabled).toBe(true);
        expect(settings.trayIconStyle).toBe('color');
    });

    it('updateTraySettings invokes update_tray_settings with the settings payload', async () => {
        let passedArgs: Record<string, unknown> | null = null;
        mockInvoke((cmd, args) => {
            if (cmd === 'update_tray_settings') {
                passedArgs = args as Record<string, unknown>;
                return null;
            }
            return null;
        });

        await updateTraySettings({
            closeToTray: false,
            startMinimized: false,
            startOnBoot: false,
            notificationsEnabled: true,
            notifyDownloadComplete: true,
            notifySyncComplete: true,
            notifyErrors: true,
            notifyUpdates: true,
            notificationSound: false,
            notifyWhenVisible: false,
            showTrayIcon: true,
            trayIconStyle: 'color',
        });
        expect(passedArgs).not.toBeNull();
        const settings = (passedArgs as unknown as { settings: Record<string, unknown> }).settings;
        expect(settings.closeToTray).toBe(false);
    });

    it('updateTrayIcon sends lowercase TrayState values', async () => {
        let captured: { cmd: string; args?: Record<string, unknown> } | null = null;
        mockInvoke((cmd, args) => {
            captured = { cmd, args };
            return null;
        });

        await updateTrayIcon('downloading');
        expect(captured!.cmd).toBe('update_tray_icon');
        expect(captured!.args).toEqual({ state: 'downloading' });
    });

    it('updateTrayStatus sends isDownloading/downloadCount/syncService in camelCase', async () => {
        let captured: { cmd: string; args?: Record<string, unknown> } | null = null;
        mockInvoke((cmd, args) => {
            captured = { cmd, args };
            return null;
        });

        await updateTrayStatus(true, 3, 'qobuz');
        expect(captured!.cmd).toBe('update_tray_status');
        expect(captured!.args).toEqual({ isDownloading: true, downloadCount: 3, syncService: 'qobuz' });

        await updateTrayStatus(false, 0);
        expect(captured!.args).toEqual({ isDownloading: false, downloadCount: 0, syncService: null });
    });

    it('exposes the full api through the trayApi namespace', () => {
        expect(trayApi.getTraySettings).toBe(getTraySettings);
        expect(trayApi.updateTraySettings).toBe(updateTraySettings);
        expect(trayApi.updateTrayIcon).toBe(updateTrayIcon);
        expect(trayApi.updateTrayStatus).toBe(updateTrayStatus);
    });
});

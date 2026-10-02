/**
 * Tray API
 *
 * Tauri commands for system tray & desktop notification state (TASK-120, IN-5).
 * These are the only UI paths that keep the tray in sync with the app state:
 * - `close_to_tray` toggle (SettingsGeneral) is applied to the tray via
 *   `update_tray_settings`; reading it back uses `get_tray_settings`.
 * - download activity reflects in the tray menu/tooltip via
 *   `update_tray_status` / `update_tray_icon`.
 */

import { invokeCommand } from './tauri';
import { asRecord } from './normalize';

/** Tray and application behavior settings (matches Rust TraySettings, camelCase). */
export interface TraySettings {
    closeToTray: boolean;
    startMinimized: boolean;
    startOnBoot: boolean;
    notificationsEnabled: boolean;
    notifyDownloadComplete: boolean;
    notifySyncComplete: boolean;
    notifyErrors: boolean;
    notifyUpdates: boolean;
    notificationSound: boolean;
    notifyWhenVisible: boolean;
    showTrayIcon: boolean;
    trayIconStyle: string;
}

/** Tray icon states (matches Rust TrayState). */
export type TrayState = 'default' | 'downloading' | 'syncing' | 'error' | 'paused';

const DEFAULT_TRAY_SETTINGS: TraySettings = {
    closeToTray: true,
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
};

function normalizeTraySettings(raw: unknown): TraySettings {
    const rec = asRecord(raw);
    const bool = (v: unknown, fallback: boolean): boolean => (typeof v === 'boolean' ? v : fallback);
    const style = typeof rec?.trayIconStyle === 'string' ? rec.trayIconStyle : DEFAULT_TRAY_SETTINGS.trayIconStyle;
    return {
        closeToTray: bool(rec?.closeToTray, true),
        startMinimized: bool(rec?.startMinimized, false),
        startOnBoot: bool(rec?.startOnBoot, false),
        notificationsEnabled: bool(rec?.notificationsEnabled, true),
        notifyDownloadComplete: bool(rec?.notifyDownloadComplete, true),
        notifySyncComplete: bool(rec?.notifySyncComplete, true),
        notifyErrors: bool(rec?.notifyErrors, true),
        notifyUpdates: bool(rec?.notifyUpdates, true),
        notificationSound: bool(rec?.notificationSound, false),
        notifyWhenVisible: bool(rec?.notifyWhenVisible, false),
        showTrayIcon: bool(rec?.showTrayIcon, true),
        trayIconStyle: style,
    };
}

/**
 * Retrieve the tray behavior settings currently applied in the backend.
 */
export async function getTraySettings(): Promise<TraySettings> {
    const raw = await invokeCommand<unknown>('get_tray_settings');
    return normalizeTraySettings(raw);
}

/**
 * Apply tray behavior settings (close-to-tray, visibility, notifications)
 * to the backend tray state.
 */
export async function updateTraySettings(settings: TraySettings): Promise<void> {
    return invokeCommand<void>('update_tray_settings', { settings });
}

/**
 * Update the tray tooltip/icon state (e.g. "Syncify - Downloading").
 */
export async function updateTrayIcon(state: TrayState): Promise<void> {
    return invokeCommand<void>('update_tray_icon', { state });
}

/**
 * Rebuild the tray menu with the current download activity.
 */
export async function updateTrayStatus(
    isDownloading: boolean,
    downloadCount: number,
    syncService?: string | null
): Promise<void> {
    return invokeCommand<void>('update_tray_status', {
        isDownloading,
        downloadCount,
        syncService: syncService ?? null
    });
}

export const trayApi = {
    getTraySettings,
    updateTraySettings,
    updateTrayIcon,
    updateTrayStatus,
};

import { invokeCommand } from './tauri';

export type NotificationKind = 'info' | 'success' | 'warning' | 'error' | 'progress';
export type NotificationCategory = 'download' | 'enrichment' | 'sync' | 'system' | 'backup';

export interface AppNotification {
    id: string;
    kind: NotificationKind;
    title: string;
    message: string;
    timestamp: string;
    category: NotificationCategory;
    metadata?: Record<string, unknown>;
}

/**
 * Emit a test notification via Tauri backend
 */
export async function emitTestNotification(
    kind: NotificationKind,
    title: string,
    message: string,
    category: NotificationCategory,
    metadata?: Record<string, unknown>
): Promise<AppNotification> {
    return invokeCommand<AppNotification>('emit_test_notification', {
        kind,
        title,
        message,
        category,
        metadata
    });
}

/**
 * Ask the backend to dispatch a desktop notification on the canonical
 * `tray-notification` channel (consumed by useNotificationListener).
 */
export async function showNotification(title: string, body: string): Promise<void> {
    return invokeCommand<void>('show_notification', { title, body });
}

export const notificationsApi = {
    emitTestNotification,
    showNotification,
};

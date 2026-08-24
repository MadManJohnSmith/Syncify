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

export const notificationsApi = {
    emitTestNotification,
};

import { ref, onUnmounted, getCurrentInstance } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { TauriEvents } from '@/api/tauri';
import { useToast } from './useToast';
import type { AppNotification } from '@/api/notifications';

/**
 * Worker lifecycle events emitted by the download supervisor (main.rs).
 */
interface WorkerFatalPayload {
    message: string;
    restart_count: number;
}

interface WorkerRestartedPayload {
    restart_count: number;
    max_restarts: number;
}

/**
 * Startup credential migration events (main.rs).
 */
interface CredentialMigrationPartialPayload {
    failed_count: number;
    failed_ids: number[];
    message: string;
}

interface StaleCredentialsPurgedPayload {
    purged_count: number;
    services: string[];
    message: string;
}

/**
 * Favorites sync completion (favorites.rs).
 */
interface FavoritesSyncCompletedPayload {
    service: string;
    item_type: string;
    total: number;
    imported: number;
}

/**
 * Structured, deduplicated service notification (services/notification.rs).
 * Serialized from Rust with camelCase field names.
 */
interface ServiceNotificationPayload {
    service: string;
    operation: string;
    kind: string;
    severity: 'info' | 'warning' | 'error' | string;
    message: string;
}

/**
 * Desktop notification dispatched by the `show_notification` command (tray.rs).
 */
interface TrayNotificationPayload {
    title: string;
    body: string;
}

export function useNotificationListener() {
    const toast = useToast();
    const isListening = ref(false);
    const unlistens = ref<UnlistenFn[]>([]);

    if (getCurrentInstance()) {
        onUnmounted(stopListening);
    }

    function register(unlisten: UnlistenFn) {
        unlistens.value.push(unlisten);
    }

    async function startListening() {
        if (isListening.value) return;

        try {
            // Listen for general push notifications from backend
            const unlistenNotif = await listen<AppNotification>(TauriEvents.NOTIFICATION, (event) => {
                const notif = event.payload;
                if (!notif) return;

                if (notif.kind === 'error') {
                    toast.error(notif.title, notif.message);
                } else if (notif.kind === 'warning') {
                    toast.warning(notif.title, notif.message);
                } else if (notif.kind === 'success') {
                    toast.success(notif.title, notif.message);
                } else if (notif.kind === 'progress') {
                    toast.progress(notif.title);
                } else {
                    toast.info(notif.title, notif.message);
                }
            });
            register(unlistenNotif);

            // Download worker supervisor events (IN-2): the user must learn
            // when the worker crashed for good or was restarted.
            register(await listen<WorkerFatalPayload>(TauriEvents.WORKER_FATAL, (event) => {
                const p = event.payload;
                toast.error('Download worker crashed', p?.message || 'The download worker stopped unexpectedly.');
            }));

            register(await listen<WorkerRestartedPayload>(TauriEvents.WORKER_RESTARTED, (event) => {
                const p = event.payload;
                const attempt = p ? ` (restart ${p.restart_count}/${p.max_restarts})` : '';
                toast.warning('Download worker restarted', `Recovering from a crash${attempt}.`);
            }));

            // Startup credential migration outcomes (IN-2).
            register(await listen<CredentialMigrationPartialPayload>(TauriEvents.CREDENTIAL_MIGRATION_PARTIAL, (event) => {
                const p = event.payload;
                const count = p?.failed_count ?? 0;
                toast.warning(
                    'Credential migration incomplete',
                    p?.message || `${count} account(s) could not be migrated and require re-authentication.`
                );
            }));

            register(await listen<StaleCredentialsPurgedPayload>(TauriEvents.STALE_CREDENTIALS_PURGED, (event) => {
                const p = event.payload;
                const services = (p?.services || []).join(', ');
                const detail = [p?.message, services ? `Affected services: ${services}.` : '']
                    .filter(Boolean)
                    .join(' ');
                toast.warning(
                    'Accounts require re-authentication',
                    detail || `${p?.purged_count ?? 0} account(s) were removed and require re-authentication.`
                );
            }));

            // Favorites sync completion (IN-2).
            register(await listen<FavoritesSyncCompletedPayload>(TauriEvents.FAVORITES_SYNC_COMPLETED, (event) => {
                const p = event.payload;
                if (!p) return;
                const service = p.service ? p.service.charAt(0).toUpperCase() + p.service.slice(1) : 'favorites';
                toast.success(
                    `${service} favorites synced`,
                    `${p.imported ?? 0} imported of ${p.total ?? 0} found (${p.item_type ?? 'tracks'}).`
                );
            }));

            // Structured per-service notifications, deduplicated in Rust (IN-2).
            register(await listen<ServiceNotificationPayload>(TauriEvents.SERVICE_NOTIFICATION, (event) => {
                const p = event.payload;
                if (!p) return;
                const title = `${p.service ? p.service.charAt(0).toUpperCase() + p.service.slice(1) : 'Service'} ${p.operation || ''}`.trim();
                if (p.severity === 'error') {
                    toast.error(title, p.message);
                } else if (p.severity === 'warning') {
                    toast.warning(title, p.message);
                } else {
                    toast.info(title, p.message);
                }
            }));

            // Desktop notification requests routed by the backend (IN-2).
            register(await listen<TrayNotificationPayload>(TauriEvents.TRAY_NOTIFICATION, (event) => {
                const p = event.payload;
                if (!p) return;
                toast.info(p.title || 'Notification', p.body);
            }));

            isListening.value = true;
        } catch (err) {
            console.warn('[useNotificationListener] Failed to register notification listener:', err);
        }
    }

    function stopListening() {
        unlistens.value.forEach(unlisten => unlisten());
        unlistens.value = [];
        isListening.value = false;
    }

    return {
        startListening,
        stopListening,
        isListening,
    };
}

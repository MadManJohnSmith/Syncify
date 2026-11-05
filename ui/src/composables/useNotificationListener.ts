import { ref, onUnmounted, getCurrentInstance } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { TauriEvents } from '@/api/tauri';
import { useToast, type PopupCategory, type ToastAction } from './useToast';
import { retryFailed } from '@/api/library';
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

/**
 * NT-03 taxonomy. A notification is classified into one of five categories,
 * and each category gets the actions the doc's ACTION rule allows:
 * (a) Go to — a `router.push` to the view where the problem is visible,
 * (b) Retry — the command that undoes the failure,
 * (d) Resolve — open the dialog that settles the matter.
 *
 * The classification lives here, in the frontend, because the backend payloads
 * (`AppNotification.category`, `ServiceNotificationPayload.operation/kind`)
 * already carry everything needed: no Rust change is required.
 */
export type NoticeCategory = PopupCategory;

export interface NoticeActionSpec {
    label: string;
    primary?: boolean;
    /** Router destination for (a) Go to. */
    route?: string;
    /** Payload for (b) Retry. */
    meta?: Record<string, unknown>;
    kind?: 'navigate' | 'retry' | 'resolve';
}

/**
 * Which actions a classified notification offers, and whether it must always
 * interrupt the user (failures do; routine success does not).
 */
export interface NoticePlan {
    category: NoticeCategory;
    actions: NoticeActionSpec[];
    /** Always shown as a popup, even when the category is muted. */
    critical: boolean;
    /** Batch key: everything sharing it collapses into one counted popup. */
    batchKey: string;
}

type NoticeType = 'success' | 'error' | 'warning' | 'info';

/**
 * Map a download `AppNotification` to its plan.
 * worker.rs:1600-1620 emits the per-track success (`"Download Complete"`),
 * :1815-1824 the per-track failure (`"Download Failed"` with the taxonomy
 * verdict in `status`), and :1532-1550 the per-queue manifest warning.
 */
export function planDownloadNotification(
    kind: string,
    metadata?: Record<string, unknown>
): NoticePlan {
    const queueId = typeof metadata?.queue_id === 'number' ? metadata.queue_id : undefined;
    const isShortfall = metadata?.quality_shortfall === true;
    const isFallback = metadata?.quality_fallback === true;

    if (kind === 'error') {
        const actions: NoticeActionSpec[] = [
            { label: 'Go to failed downloads', route: '/downloads?filter=failed', kind: 'navigate' }
        ];
        // worker.rs:1781-1792 stamps the taxonomy verdict in the same payload.
        // `requires_auth` is settled by reconnecting the account and
        // `rejected_quality` is a permanent verdict, so a retry button there
        // would be a lie; offer the destination that actually resolves it.
        const status = typeof metadata?.status === 'string' ? metadata.status : '';
        if (status === 'requires_auth') {
            actions.push({ label: 'Go to accounts', route: '/accounts', kind: 'navigate' });
        } else if (status === 'rejected_quality') {
            actions.push({ label: 'Download settings', route: '/settings', kind: 'navigate' });
        } else if (status === 'failed' && queueId !== undefined) {
            actions.push({ label: 'Retry downloads', primary: true, meta: { queue_id: queueId }, kind: 'retry' });
        }
        return {
            category: 'download',
            critical: true,
            batchKey: 'download:failed',
            actions
        };
    }

    if (kind === 'warning') {
        // B3 "manifest.json could not be written": a disk problem with a
        // concrete folder to look at, so (a) Go to the file system and
        // (a) Go to logs. It is counted with the download batch.
        return {
            category: 'download',
            critical: true,
            batchKey: 'download:warning',
            actions: [
                { label: 'Go to failed downloads', route: '/downloads?filter=failed', kind: 'navigate' },
                { label: 'Open logs', route: '/logs', kind: 'navigate' }
            ]
        };
    }

    // B1 per-track success. Not critical: it is exactly the popup bomb the user
    // reported, and the batch key collapses it into one counted toast.
    const actions: NoticeActionSpec[] = isShortfall || isFallback
        ? [{ label: 'Go to downloads', route: '/downloads', kind: 'navigate' }]
        : [];
    return {
        category: 'download',
        critical: false,
        batchKey: isShortfall || isFallback ? 'download:shortfall' : 'download:completed',
        actions
    };
}

/**
 * Structured service notifications (services/notification.rs) keyed by
 * operation + kind: `auth` (re-authenticate), `expansion` (resolve a
 * disambiguation), anything else (logs / retry).
 *
 * Batch keys deliberately mirror the per-track download keys, because
 * worker.rs:1815-1831 emits BOTH the `AppNotification` and the
 * `ServiceNotification` for the same failure. Sharing the key means the two
 * channels for one track merge into one popup instead of duplicating it.
 */
export function planServiceNotification(
    kind: string,
    operation: string,
    service?: string
): NoticePlan {
    const svc = (service || 'service').toUpperCase();
    if (operation === 'download') {
        switch (kind) {
            case 'auth':
                // (a) Go to the account that must be re-authenticated — the
                // single highest-value action in the whole taxonomy.
                return {
                    category: 'download',
                    critical: true,
                    batchKey: 'download:auth',
                    actions: [{ label: `Reconnect ${svc}`, route: '/accounts', kind: 'navigate' }]
                };
            case 'expansion':
                // (d) Resolve: open DownloadsView with the review dialog, which
                // is what settles the ambiguous track.
                return {
                    category: 'download',
                    critical: true,
                    batchKey: 'download:failed',
                    actions: [{ label: 'Review tracks', route: '/downloads?filter=failed', kind: 'resolve' }]
                };
            default:
                // A failed track: same batch as the AppNotification it rides with.
                return {
                    category: 'download',
                    critical: true,
                    batchKey: 'download:failed',
                    actions: [
                        { label: 'Go to failed downloads', route: '/downloads?filter=failed', kind: 'navigate' },
                        { label: 'Retry downloads', kind: 'retry', meta: { queue_id: undefined } }
                    ]
                };
        }
    }

    if (kind === 'auth') {
        return {
            category: 'system',
            critical: true,
            batchKey: `system:auth:${operation}`,
            actions: [{ label: `Reconnect ${svc}`, route: '/accounts', kind: 'navigate' }]
        };
    }
    return {
        category: 'system',
        critical: false,
        batchKey: `system:notice:${operation || 'notice'}`,
        actions: [{ label: 'Open logs', route: '/logs', kind: 'navigate' }]
    };
}

/**
 * Startup credential events (main.rs). `system:credentials` is one batch for
 * both event names so the two startup paths cannot produce two popups, but
 * they are kept apart from the worker batch, which is a different problem.
 */
export function planCredentialNotification(which: 'migration' | 'purged'): NoticePlan {
    return {
        category: 'system',
        // Always interrupts: without re-authenticating the app is crippled.
        critical: true,
        batchKey: `system:credentials:${which}`,
        actions: [{ label: 'Go to accounts', route: '/accounts', kind: 'navigate' }]
    };
}

/** Sync (favorites / Sync All from tray.rs:766-791). */
export function planSyncNotification(): NoticePlan {
    return {
        // Not critical: the taxonomy puts a successful favorite sync in Logs,
        // and the download category preference does not gate it — it is simply
        // not worth a popup unless something failed.
        category: 'sync',
        critical: false,
        batchKey: 'sync:completed',
        actions: [{ label: 'Go to playlists', route: '/playlists', kind: 'navigate' }]
    };
}

/** Worker supervisor events (main.rs): fatal, restarted, and test notices. */
export function planWorkerNotification(): NoticePlan {
    return {
        category: 'system',
        critical: true,
        batchKey: 'system:worker',
        actions: [
            { label: 'Go to downloads', route: '/downloads', kind: 'navigate' },
            { label: 'Open logs', route: '/logs', kind: 'navigate' }
        ]
    };
}

/** Desktop notifications requested by the backend (tray.rs show_notification). */
export function planTrayNotification(): NoticePlan {
    return {
        category: 'system',
        critical: false,
        batchKey: 'tray:notice',
        actions: [{ label: 'Go to downloads', route: '/downloads', kind: 'navigate' }]
    };
}

/** Enrichment/backup notices: ordinary results go to history, failures to popup. */
export function planBackgroundNotification(category: NoticeCategory, kind: NoticeType): NoticePlan {
    return {
        category,
        critical: kind === 'error' || kind === 'warning',
        batchKey: `${category}:${kind}`,
        actions: [
            { label: category === 'enrichment' ? 'Open metadata' : 'Open settings',
              route: category === 'enrichment' ? '/metadata' : '/settings', kind: 'navigate' }
        ]
    };
}

/**
 * NT-01 navigation bridge. The notification listener runs outside any router
 * context (it is a singleton, and `useRouter()` throws in the shell tests), so
 * a navigation request travels as a `window` CustomEvent. App.vue listens for
 * `syncify:navigate` and pushes the route with the real router — see the
 * "App.vue hook" note in the report.
 */
export const NAVIGATE_EVENT = 'syncify:navigate';

export function emitNavigate(route: string): void {
    if (typeof window === 'undefined' || typeof window.dispatchEvent !== 'function') return;
    window.dispatchEvent(new CustomEvent(NAVIGATE_EVENT, { detail: { route } }));
}

function noticeTypeFromSeverity(severity: string): NoticeType {
    if (severity === 'error' || severity === 'warning') return severity;
    return 'info';
}

export function useNotificationListener() {
    const toast = useToast();
    const isListening = ref(false);
    const unlistens = ref<UnlistenFn[]>([]);

    /** Build the concrete handlers behind a plan's action specs (NT-01). */
    function actionsFor(plan: NoticePlan): ToastAction[] {
        return plan.actions.map((spec) => ({
            label: spec.label,
            primary: spec.primary,
            route: spec.route,
            meta: spec.meta,
            kind: spec.kind,
            handler: async () => {
                if (spec.kind === 'retry') {
                    // (b) Retry — the command that undoes the failure. The
                    // retry outcome re-enters through the same listener, so no
                    // result toast is emitted here.
                    const queueId = typeof spec.meta?.queue_id === 'number' ? spec.meta.queue_id : undefined;
                    try {
                        await retryFailed(queueId);
                    } catch (err) {
                        toast.error('Retry failed', String(err));
                    }
                    return;
                }
                if (spec.route) {
                    // (a) Go to / (d) Resolve — both are plain navigation; the
                    // destination decides what happens next.
                    emitNavigate(spec.route);
                }
            }
        }));
    }

    /** Push one classified notification through the toast system. */
    function pushNotice(plan: NoticePlan, type: NoticeType, title: string, description?: string): void {
        toast.notify({
            type,
            title,
            description,
            category: plan.category,
            batchKey: plan.batchKey,
            critical: plan.critical,
            actions: actionsFor(plan)
        });
    }

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

                // `progress` carries no percentage, so a progress toast created
                // here would never be completed and would stay on screen
                // forever (R16). Downgrade it to info, as before.
                const type = noticeTypeFromSeverity(notif.kind ?? 'info');

                // NT-03: the backend already says which category this is
                // (`AppNotification.category`), so every channel is mapped to
                // its taxonomy entry instead of falling through to the shell.
                if (notif.category === 'download') {
                    pushNotice(planDownloadNotification(type, notif.metadata), type, notif.title, notif.message);
                } else if (notif.category === 'sync') {
                    pushNotice(planSyncNotification(), type, notif.title, notif.message);
                } else if (notif.category === 'enrichment' || notif.category === 'backup') {
                    pushNotice(planBackgroundNotification(notif.category, type), type, notif.title, notif.message);
                } else {
                    pushNotice(planWorkerNotification(), type, notif.title, notif.message);
                }
            });
            register(unlistenNotif);

            // Download worker supervisor events (IN-2): the user must learn
            // when the worker crashed for good or was restarted.
            register(await listen<WorkerFatalPayload>(TauriEvents.WORKER_FATAL, (event) => {
                pushNotice(
                    planWorkerNotification(),
                    'error',
                    'Download worker crashed',
                    event.payload?.message || 'The download worker stopped unexpectedly.'
                );
            }));

            register(await listen<WorkerRestartedPayload>(TauriEvents.WORKER_RESTARTED, (event) => {
                const p = event.payload;
                const attempt = p ? ` (restart ${p.restart_count}/${p.max_restarts})` : '';
                pushNotice(planWorkerNotification(), 'warning', 'Download worker restarted', `Recovering from a crash${attempt}.`);
            }));

            // Startup credential migration outcomes (IN-2).
            register(await listen<CredentialMigrationPartialPayload>(TauriEvents.CREDENTIAL_MIGRATION_PARTIAL, (event) => {
                const p = event.payload;
                const count = p?.failed_count ?? 0;
                pushNotice(
                    planCredentialNotification('migration'),
                    'warning',
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
                pushNotice(
                    planCredentialNotification('purged'),
                    'warning',
                    'Accounts require re-authentication',
                    detail || `${p?.purged_count ?? 0} account(s) were removed and require re-authentication.`
                );
            }));

            // Favorites sync completion (IN-2).
            register(await listen<FavoritesSyncCompletedPayload>(TauriEvents.FAVORITES_SYNC_COMPLETED, (event) => {
                const p = event.payload;
                if (!p) return;
                const service = p.service ? p.service.charAt(0).toUpperCase() + p.service.slice(1) : 'favorites';
                pushNotice(
                    planSyncNotification(),
                    'success',
                    `${service} favorites synced`,
                    `${p.imported ?? 0} imported of ${p.total ?? 0} found (${p.item_type ?? 'tracks'}).`
                );
            }));

            // Structured per-service notifications, deduplicated in Rust (IN-2).
            register(await listen<ServiceNotificationPayload>(TauriEvents.SERVICE_NOTIFICATION, (event) => {
                const p = event.payload;
                if (!p) return;
                const title = `${p.service ? p.service.charAt(0).toUpperCase() + p.service.slice(1) : 'Service'} ${p.operation || ''}`.trim();
                // The worker already emits the same failed track on the
                // AppNotification channel with queue_id/status and an actionable
                // destination. Keep the structured Rust event for other
                // consumers, but never create a second UI notice/history row.
                if (p.operation === 'download') return;
                pushNotice(
                    planServiceNotification(p.kind, p.operation, p.service),
                    noticeTypeFromSeverity(p.severity),
                    title,
                    p.message
                );
            }));

            // Desktop notification requests routed by the backend (IN-2).
            register(await listen<TrayNotificationPayload>(TauriEvents.TRAY_NOTIFICATION, (event) => {
                const p = event.payload;
                if (!p) return;
                pushNotice(planTrayNotification(), 'info', p.title || 'Notification', p.body);
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

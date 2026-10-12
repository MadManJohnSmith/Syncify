/**
 * NT-01..NT-04 regression tests for the notification pipeline:
 *
 * NT-01: a notification carries a semantic action, and the same action is
 *        available from the notification center after the popup disappears.
 * NT-02: per-track popups are aggregated by batch key (one popup, no loss of
 *        the individual history entries, failures never suppressed).
 * NT-03: the taxonomy decides category, criticality, actions and suppression.
 * NT-04: classification is asserted directly on the exported plans.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import ToastNotifications from '@/components/ToastNotifications.vue';
import { useNotificationListener, planDownloadNotification, planServiceNotification, planCredentialNotification, planWorkerNotification, planSyncNotification, planBackgroundNotification, NAVIGATE_EVENT } from '@/composables/useNotificationListener';
import { useToast } from '@/composables/useToast';
import { TauriEvents } from '@/api/tauri';
import { retryFailed } from '@/api/library';
import { resetMocks, emitMockEvent } from '../setup';

vi.mock('@/api/library', () => ({
    retryFailed: vi.fn(async () => 3),
}));

describe('useNotificationListener — Rust events without listeners (IN-2)', () => {
    let toast: ReturnType<typeof useToast>;

    function resetToastState() {
        toast.toasts.value.forEach((t) => toast.dismiss(t.id));
    }

    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        toast = useToast();
        resetToastState();
    });

    it('registers listeners for the canonical notification channel and the 7 unserved Rust events', async () => {
        const listener = useNotificationListener();
        await listener.startListening();

        const requiredEvents = [
            TauriEvents.NOTIFICATION,
            TauriEvents.WORKER_FATAL,
            TauriEvents.WORKER_RESTARTED,
            TauriEvents.CREDENTIAL_MIGRATION_PARTIAL,
            TauriEvents.STALE_CREDENTIALS_PURGED,
            TauriEvents.FAVORITES_SYNC_COMPLETED,
            TauriEvents.SERVICE_NOTIFICATION,
            TauriEvents.TRAY_NOTIFICATION,
        ];

        const { listen } = await import('@tauri-apps/api/event');
        for (const event of requiredEvents) {
            expect(listen, `must listen to "${event}"`).toHaveBeenCalledWith(event, expect.any(Function));
        }
    });

    it('shows an error toast when the download worker dies fatally (worker_fatal)', async () => {
        const listener = useNotificationListener();
        await listener.startListening();

        emitMockEvent(TauriEvents.WORKER_FATAL, {
            message: 'Download worker crashed and could not recover.',
            restart_count: 3,
        });

        expect(toast.toasts.value.length).toBeGreaterThan(0);
        const last = toast.toasts.value[0];
        expect(last.type).toBe('error');
        expect(last.title).toContain('worker crashed');
        expect(last.description).toContain('could not recover');
    });

    it('shows a warning toast when the worker is restarted (worker_restarted)', async () => {
        const listener = useNotificationListener();
        await listener.startListening();

        emitMockEvent(TauriEvents.WORKER_RESTARTED, { restart_count: 1, max_restarts: 3 });

        const last = toast.toasts.value[0];
        expect(last.type).toBe('warning');
        expect(last.title).toContain('restarted');
        expect(last.description).toContain('1/3');
    });

    it('shows a warning toast for a partial credential migration', async () => {
        const listener = useNotificationListener();
        await listener.startListening();

        emitMockEvent(TauriEvents.CREDENTIAL_MIGRATION_PARTIAL, {
            failed_count: 2,
            failed_ids: [4, 9],
            message: 'Some accounts could not be migrated and require re-authentication.',
        });

        const last = toast.toasts.value[0];
        expect(last.type).toBe('warning');
        expect(last.title).toContain('migration incomplete');
        expect(last.description).toContain('re-authentication');
    });

    it('shows a warning toast when stale credentials are purged at startup', async () => {
        const listener = useNotificationListener();
        await listener.startListening();

        emitMockEvent(TauriEvents.STALE_CREDENTIALS_PURGED, {
            purged_count: 2,
            services: ['qobuz', 'tidal'],
            message: 'Some service accounts were encrypted with a different machine key.',
        });

        const last = toast.toasts.value[0];
        expect(last.type).toBe('warning');
        expect(last.title).toContain('re-authentication');
        expect(last.description).toContain('qobuz, tidal');
    });

    it('records a favorites sync in the history without a popup (NT-03)', async () => {
        const listener = useNotificationListener();
        await listener.startListening();

        emitMockEvent(TauriEvents.FAVORITES_SYNC_COMPLETED, {
            service: 'qobuz',
            item_type: 'tracks',
            total: 120,
            imported: 37,
        });

        // The sync category is muted (taxonomy §3.3: "no molesta"), so it must
        // not create a popup — but the information is still recoverable from
        // the notification center, with its action.
        expect(toast.toasts.value.length).toBe(0);
        const entry = toast.history.value[0];
        expect(entry.type).toBe('success');
        expect(entry.title).toContain('Qobuz favorites synced');
        expect(entry.description).toContain('37 imported of 120');
        expect(entry.actionLabel).toBe('Go to playlists');
        expect(entry.actionRoute).toBe('/playlists');
    });

    it('maps service-notification severity to the matching toast type', async () => {
        const listener = useNotificationListener();
        await listener.startListening();

        emitMockEvent(TauriEvents.SERVICE_NOTIFICATION, {
            service: 'tidal',
            accountId: 3,
            operation: 'sync',
            kind: 'auth',
            severity: 'error',
            dedupeKey: 'tidal:3:sync:auth:x',
            message: 'Session expired, please reconnect.',
            occurredAt: new Date().toISOString(),
            resolvedAt: null,
        });

        let last = toast.toasts.value[0];
        expect(last.type).toBe('error');
        expect(last.title).toBe('Tidal sync');
        expect(last.description).toContain('Session expired');

        resetToastState();
        emitMockEvent(TauriEvents.SERVICE_NOTIFICATION, {
            service: 'qobuz',
            operation: 'download',
            kind: 'rate_limit',
            severity: 'warning',
            message: 'Rate limited by provider.',
        });
        // Download failures are already emitted as rich AppNotifications.
        expect(toast.toasts.value).toHaveLength(0);

        resetToastState();
        emitMockEvent(TauriEvents.SERVICE_NOTIFICATION, {
            service: 'deezer',
            operation: 'sync',
            kind: 'network',
            severity: 'info',
            message: 'Retrying shortly.',
        });
        last = toast.toasts.value[0];
        expect(last.type).toBe('info');
    });

    it('shows an info toast for backend tray-notification requests', async () => {
        const listener = useNotificationListener();
        await listener.startListening();

        emitMockEvent(TauriEvents.TRAY_NOTIFICATION, { title: 'Download complete', body: 'Track saved to library.' });

        const last = toast.toasts.value[0];
        expect(last.type).toBe('info');
        expect(last.title).toBe('Download complete');
        expect(last.description).toContain('Track saved');
    });

    it('unlistens every registered event on stopListening', async () => {
        const listener = useNotificationListener();
        await listener.startListening();
        expect(listener.isListening.value).toBe(true);

        listener.stopListening();
        expect(listener.isListening.value).toBe(false);

        // Events emitted after stopListening must not produce toasts.
        const before = toast.toasts.value.length;
        emitMockEvent(TauriEvents.WORKER_FATAL, { message: 'late event', restart_count: 3 });
        expect(toast.toasts.value.length).toBe(before);
    });
});

describe('NT-01..NT-04 — notification taxonomy and actions', () => {
    let toast: ReturnType<typeof useToast>;

    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        toast = useToast();
        toast.toasts.value.forEach(t => toast.dismiss(t.id));
        toast.clearAllHistory();
        toast.setPopupPreference('download', true);
        toast.setPopupPreference('sync', false);
    });

    it('classifies a per-track download success as a noncritical batched activity', () => {
        const plan = planDownloadNotification('success', { queue_id: 8 });
        expect(plan.category).toBe('download');
        expect(plan.critical).toBe(false);
        expect(plan.batchKey).toBe('download:completed');
        expect(plan.actions).toEqual([]);
    });

    it('classifies a shortfall and fallback as actionable download warnings', () => {
        for (const meta of [{ quality_shortfall: true }, { quality_fallback: true }]) {
            const plan = planDownloadNotification('success', meta);
            expect(plan.batchKey).toBe('download:shortfall');
            expect(plan.actions[0].route).toBe('/downloads');
        }
    });

    it('classifies failed tracks as critical and with retry/navigation actions', () => {
        const plan = planDownloadNotification('error', { queue_id: 42, status: 'failed' });
        expect(plan.critical).toBe(true);
        expect(plan.batchKey).toBe('download:failed');
        expect(plan.actions.map(a => a.kind)).toEqual(['navigate', 'retry']);
        expect(plan.actions[0].route).toBe('/downloads?filter=failed');
        expect(plan.actions[1].meta).toEqual({ queue_id: 42 });
    });

    it('does not offer a retry the backend would skip (auth / rejected quality)', () => {
        // worker.rs:1781-1792 stamps the taxonomy verdict; retry_failed skips
        // these, so the action becomes the one that really resolves it.
        const auth = planDownloadNotification('error', { queue_id: 1, status: 'requires_auth' });
        expect(auth.actions.map(a => a.route)).toEqual(['/downloads?filter=failed', '/accounts']);
        const quality = planDownloadNotification('error', { queue_id: 1, status: 'rejected_quality' });
        expect(quality.actions.map(a => a.route)).toEqual(['/downloads?filter=failed', '/settings']);
    });

    it('classifies auth, expansion and network service notifications distinctly', () => {
        // A per-track download failure over the service channel maps to the same
        // batch as the AppNotification it rides with (no duplicate popup).
        expect(planServiceNotification('auth', 'download', 'qobuz').batchKey).toBe('download:auth');
        expect(planServiceNotification('auth', 'download', 'qobuz').actions[0].route).toBe('/accounts');
        expect(planServiceNotification('auth', 'download', 'qobuz').critical).toBe(true);
        expect(planServiceNotification('expansion', 'download').actions[0].kind).toBe('resolve');
        expect(planServiceNotification('expansion', 'download').batchKey).toBe('download:failed');
        expect(planServiceNotification('network', 'download').batchKey).toBe('download:failed');
        // A non-download operation (sync, enrichment) falls back to the system
        // channel and points at logs.
        expect(planServiceNotification('auth', 'sync', 'tidal').batchKey).toBe('system:auth:sync');
        expect(planServiceNotification('network', 'sync').actions[0].route).toBe('/logs');
        expect(planCredentialNotification('purged').actions[0].route).toBe('/accounts');
        expect(planWorkerNotification().actions[0].route).toBe('/downloads');
        expect(planSyncNotification().category).toBe('sync');
    });

    it('routes an enrichment notice to metadata and keeps the enrichment category muted', () => {
        const plan = planBackgroundNotification('enrichment', 'info');
        expect(plan.category).toBe('enrichment');
        expect(plan.critical).toBe(false);
        expect(plan.actions[0].route).toBe('/metadata');

        const failed = planBackgroundNotification('backup', 'error');
        expect(failed.critical).toBe(true);
        expect(failed.actions[0].route).toBe('/settings');

        toast.setPopupPreference('enrichment', false);
        const id = toast.notify({
            type: 'info', title: 'Metadata fetched', category: 'enrichment', batchKey: 'enrichment:info',
        });
        expect(id).toBeNull();
        expect(toast.history.value[0].title).toBe('Metadata fetched');
    });

    it('does not duplicate a failed track: service failure is ignored after AppNotification', async () => {
        const listener = useNotificationListener();
        await listener.startListening();
        // worker.rs:1772-1781 emits BOTH AppNotification(Error) and
        // ServiceNotification(error). Two popups for one failure is the
        // duplicate the taxonomy documents, so both must land in the same batch.
        emitMockEvent(TauriEvents.NOTIFICATION, {
            kind: 'error', title: 'Download Failed', message: 'Track unavailable',
            category: 'download', metadata: { queue_id: 5, status: 'not_found' },
        });
        emitMockEvent(TauriEvents.SERVICE_NOTIFICATION, {
            service: 'qobuz', operation: 'download', kind: 'not_found', severity: 'error',
            message: 'Failed Track - Track unavailable',
        });
        expect(toast.toasts.value.length).toBe(1);
        expect(toast.toasts.value[0].type).toBe('error');
        expect(toast.history.value.length).toBe(1);
        expect(toast.toasts.value[0].batchCount).toBe(1);
    });

    it('aggregates 100 per-track download successes into one popup, preserving 50 recent history entries', async () => {
        const listener = useNotificationListener();
        await listener.startListening();
        for (let i = 0; i < 100; i++) {
            emitMockEvent(TauriEvents.NOTIFICATION, {
                kind: 'success', title: 'Download Complete', message: `Artist - Track ${i}`,
                category: 'download', metadata: { queue_id: i, status: 'failed' },
            });
        }
        expect(toast.toasts.value.length).toBe(1);
        expect(toast.toasts.value[0].batchCount).toBe(100);
        expect(toast.toasts.value[0].description).toContain('100 tracks');
        expect(toast.history.value.length).toBe(50); // Existing center retention cap.
        expect(new Set(toast.history.value.map(h => h.id)).size).toBe(50); // Valid Vue keys.
        expect(toast.history.value[0].description).toContain('Track 99');
    });

    it('aggregates failed tracks separately and never suppresses critical errors', async () => {
        const listener = useNotificationListener();
        await listener.startListening();
        toast.setPopupPreference('download', false);
        for (let i = 0; i < 3; i++) {
            emitMockEvent(TauriEvents.NOTIFICATION, {
                kind: 'error', title: 'Download Failed', message: `Track ${i} unavailable`,
                category: 'download', metadata: { queue_id: i, status: 'not_found' },
            });
        }
        expect(toast.toasts.value.length).toBe(1);
        expect(toast.toasts.value[0].type).toBe('error');
        expect(toast.toasts.value[0].batchCount).toBe(3);
        expect(toast.history.value.length).toBe(3);
        expect(toast.history.value[0].actionRoute).toBe('/downloads?filter=failed');
        // Noncritical successes are muted but still in the center.
        emitMockEvent(TauriEvents.NOTIFICATION, {
            kind: 'success', title: 'Download Complete', message: 'Saved', category: 'download',
        });
        expect(toast.toasts.value.length).toBe(1);
        expect(toast.history.value.length).toBe(4);
        expect(toast.history.value[0].title).toBe('Download Complete');
    });

    it('retains a semantic history action after popup dismissal and dispatches navigation', async () => {
        const listener = useNotificationListener();
        await listener.startListening();
        const nav = vi.fn();
        window.addEventListener(NAVIGATE_EVENT, nav);
        try {
            emitMockEvent(TauriEvents.STALE_CREDENTIALS_PURGED, {
                purged_count: 1, services: ['tidal'], message: 'Please reconnect',
            });
            const entry = toast.history.value[0];
            expect(entry.actionLabel).toBe('Go to accounts');
            expect(entry.actionRoute).toBe('/accounts');
            const toastId = toast.toasts.value[0].id;
            toast.dismiss(toastId);
            expect(toast.runHistoryAction(entry.id)).toBe(true);
            await Promise.resolve();
            expect(nav).toHaveBeenCalledTimes(1);
            expect((nav.mock.calls[0][0] as CustomEvent).detail.route).toBe('/accounts');
        } finally {
            window.removeEventListener(NAVIGATE_EVENT, nav);
        }
    });

    it('runs the retry command for the failed queue item from a popup action', async () => {
        const listener = useNotificationListener();
        await listener.startListening();
        emitMockEvent(TauriEvents.NOTIFICATION, {
            kind: 'error', title: 'Download Failed', message: 'Network error',
            category: 'download', metadata: { queue_id: 27, status: 'failed' },
        });
        const retry = toast.toasts.value[0].actions?.find(a => a.kind === 'retry');
        expect(retry).toBeDefined();
        await retry?.handler();
        expect(retryFailed).toHaveBeenCalledWith(27);
    });

    it('stores category preferences and suppresses only routine activity', () => {
        toast.setPopupPreference('download', false);
        expect(toast.isPopupEnabled('download')).toBe(false);
        expect(JSON.parse(localStorage.getItem('syncify:popup-preferences:v1') || '{}').download).toBe(false);
        const id = toast.notify({
            type: 'success', title: 'Saved', description: 'One track', category: 'download', batchKey: 'download:completed',
        });
        expect(id).toBeNull();
        expect(toast.history.value[0].title).toBe('Saved');
        expect(toast.toasts.value.length).toBe(0);
        toast.notify({
            type: 'error', title: 'Download Failed', category: 'download', batchKey: 'download:failed', critical: true,
        });
        expect(toast.toasts.value[0].type).toBe('error');
    });

    it('reads the real backend tray preference instead of a second store (NT-03)', () => {
        // `TraySettings` is the same KV table worker.rs:1623 already consults
        // through `notification_category_enabled`, so muting "download
        // complete" in Settings must mute the popup here too.
        toast.applyTrayPreferences({
            notify_download_complete: false,
            notify_sync_complete: false,
            notify_updates: false,
        });
        expect(toast.isPopupEnabled('download')).toBe(false);
        expect(toast.isPopupEnabled('enrichment')).toBe(false);
        expect(toast.isPopupEnabled('sync')).toBe(false);
        expect(toast.isPopupEnabled('backup')).toBe(false);
        // `system` is never gated by a per-category toggle.
        expect(toast.isPopupEnabled('system')).toBe(true);

        const id = toast.notify({
            type: 'success', title: 'Saved', category: 'download', batchKey: 'download:completed',
        });
        expect(id).toBeNull();
        expect(toast.toasts.value.length).toBe(0);
        expect(toast.history.value[0].title).toBe('Saved');

        // Re-enabling in the backend turns the popup back on.
        toast.applyTrayPreferences({
            notify_download_complete: true,
            notify_sync_complete: true,
            notify_updates: true,
        });
        expect(toast.isPopupEnabled('download')).toBe(true);
    });
});

describe('ToastNotifications.vue — aggregated popups and action buttons', () => {
    beforeEach(() => {
        document.body.innerHTML = '';
        resetMocks();
        const toast = useToast();
        toast.toasts.value.forEach(t => toast.dismiss(t.id));
        toast.clearAllHistory();
        toast.setPopupPreference('download', true);
    });

    it('renders one toast with the merge counter and both action buttons for a failed batch', async () => {
        const listener = useNotificationListener();
        await listener.startListening();
        for (let i = 0; i < 4; i++) {
            emitMockEvent(TauriEvents.NOTIFICATION, {
                kind: 'error', title: 'Download Failed', message: 'Item not available on provider',
                category: 'download', metadata: { queue_id: i, status: 'failed' },
            });
        }

        const wrapper = mount(ToastNotifications, { attachTo: document.body });
        await flushPromises();

        const cards = wrapper.findAll('.toast');
        expect(cards.length).toBe(1);
        expect(cards[0].text()).toContain('Download Failed');
        expect(cards[0].text()).toContain('4 events merged');
        expect(cards[0].text()).toContain('history keeps each one');

        const buttons = wrapper.findAll('.toast-actions button');
        expect(buttons.map(b => b.text())).toEqual(['Go to failed downloads', 'Retry downloads']);

        // The retry action invokes the command from the popup surface.
        await buttons[1].trigger('click');
        expect(retryFailed).toHaveBeenCalled();
        wrapper.unmount();
    });
});

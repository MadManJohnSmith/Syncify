/**
 * IN-2 regression tests: the seven Rust state/failure events
 * (worker_fatal, worker_restarted, credential_migration_partial,
 * stale_credentials_purged, syncify:favorites_sync_completed,
 * service-notification, tray-notification) must have real UI listeners
 * that surface the information through the existing toast system.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { useNotificationListener } from '@/composables/useNotificationListener';
import { useToast } from '@/composables/useToast';
import { TauriEvents } from '@/api/tauri';
import { resetMocks, emitMockEvent } from '../setup';

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

    it('shows a success toast when a favorites sync completes', async () => {
        const listener = useNotificationListener();
        await listener.startListening();

        emitMockEvent(TauriEvents.FAVORITES_SYNC_COMPLETED, {
            service: 'qobuz',
            item_type: 'tracks',
            total: 120,
            imported: 37,
        });

        const last = toast.toasts.value[0];
        expect(last.type).toBe('success');
        expect(last.title).toContain('Qobuz favorites synced');
        expect(last.description).toContain('37 imported of 120');
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
        last = toast.toasts.value[0];
        expect(last.type).toBe('warning');

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

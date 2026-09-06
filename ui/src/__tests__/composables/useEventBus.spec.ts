import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { useEventBus, resetLocalListeners, TauriEvents } from '@/composables/useEventBus';
import { resetMocks, emitMockEvent } from '../setup';
import * as tauriEventModule from '@tauri-apps/api/event';

describe('useEventBus Composable (TASK-14)', () => {
    let eventBus: ReturnType<typeof useEventBus>;

    beforeEach(() => {
        resetMocks();
        resetLocalListeners();
        // Reset window.__TAURI__ to default (undefined)
        delete (window as any).__TAURI__;
        eventBus = useEventBus();
    });

    afterEach(() => {
        eventBus.offAll();
        resetLocalListeners();
        delete (window as any).__TAURI__;
    });

    describe('Single Invocation & Duplication Prevention', () => {
        it('invokes subscriber exactly once when emitting an event in non-Tauri mode', async () => {
            const handler = vi.fn();
            await eventBus.on('test:single_emit', handler);

            await eventBus.emit('test:single_emit', { data: 42 });

            expect(handler).toHaveBeenCalledTimes(1);
            expect(handler).toHaveBeenCalledWith({ data: 42 });
        });

        it('invokes subscriber exactly once when emitting in Tauri mode (delegates to tauriEmit)', async () => {
            (window as any).__TAURI__ = {};
            const handler = vi.fn();
            await eventBus.on('test:tauri_emit', handler);

            await eventBus.emit('test:tauri_emit', { msg: 'hello tauri' });

            expect(handler).toHaveBeenCalledTimes(1);
            expect(handler).toHaveBeenCalledWith({ msg: 'hello tauri' });
        });

        it('prevents duplicate invocation if subscriber receives event via both local channel and Tauri mock within window', async () => {
            const handler = vi.fn();
            await eventBus.on('test:dual_channel', handler);

            // Simulate immediate dual-channel arrival of the same event and payload
            await eventBus.emit('test:dual_channel', { key: 'val' });
            emitMockEvent('test:dual_channel', { key: 'val' });

            // Must only be called once thanks to 50ms deduplication
            expect(handler).toHaveBeenCalledTimes(1);
            expect(handler).toHaveBeenCalledWith({ key: 'val' });
        });

        it('falls back to local dispatch if tauriEmit throws in Tauri environment', async () => {
            (window as any).__TAURI__ = {};
            const handler = vi.fn();
            await eventBus.on('test:fallback', handler);

            // Force tauriEmit mock to reject once
            const tauriEmitSpy = vi.spyOn(tauriEventModule, 'emit').mockRejectedValueOnce(new Error('IPC disconnected'));

            await eventBus.emit('test:fallback', { retry: true });

            expect(handler).toHaveBeenCalledTimes(1);
            expect(handler).toHaveBeenCalledWith({ retry: true });

            tauriEmitSpy.mockRestore();
        });
    });

    describe('Unregistration (unlisten / off / offAll)', () => {
        it('unregisters listener via unlisten without leaving active handlers', async () => {
            const handler = vi.fn();
            const unlisten = await eventBus.on('test:unlisten', handler);

            expect(eventBus.listenerCount()).toBeGreaterThan(0);
            expect(eventBus.isListening.value).toBe(true);

            unlisten();

            expect(eventBus.listenerCount()).toBe(0);
            expect(eventBus.isListening.value).toBe(false);

            await eventBus.emit('test:unlisten', { afterUnlisten: true });
            expect(handler).not.toHaveBeenCalled();
        });

        it('unregisters listener via off without leaving active handlers', async () => {
            const handler = vi.fn();
            const unlisten = await eventBus.on('test:off', handler);

            expect(eventBus.listenerCount()).toBeGreaterThan(0);

            eventBus.off(unlisten);

            expect(eventBus.listenerCount()).toBe(0);
            expect(eventBus.isListening.value).toBe(false);

            await eventBus.emit('test:off', { off: true });
            expect(handler).not.toHaveBeenCalled();
        });

        it('unregisters all listeners via offAll without leaving active handlers', async () => {
            const handler1 = vi.fn();
            const handler2 = vi.fn();
            await eventBus.on('test:event1', handler1);
            await eventBus.on('test:event2', handler2);

            expect(eventBus.listenerCount()).toBeGreaterThanOrEqual(2);
            expect(eventBus.isListening.value).toBe(true);

            eventBus.offAll();

            expect(eventBus.listenerCount()).toBe(0);
            expect(eventBus.isListening.value).toBe(false);

            await eventBus.emit('test:event1', 'payload1');
            await eventBus.emit('test:event2', 'payload2');

            expect(handler1).not.toHaveBeenCalled();
            expect(handler2).not.toHaveBeenCalled();
        });
    });

    describe('50ms Deduplication Window', () => {
        it('discards duplicate identical events emitted within 50ms', async () => {
            const handler = vi.fn();
            await eventBus.on('test:dedupe_window', handler);

            // Immediate identical emits
            await eventBus.emit('test:dedupe_window', { id: 100 });
            await eventBus.emit('test:dedupe_window', { id: 100 });
            await eventBus.emit('test:dedupe_window', { id: 100 });

            // Only the first one should have been handled
            expect(handler).toHaveBeenCalledTimes(1);
            expect(handler).toHaveBeenCalledWith({ id: 100 });
        });

        it('allows immediate consecutive events if payloads are distinct', async () => {
            const handler = vi.fn();
            await eventBus.on('test:distinct_payloads', handler);

            await eventBus.emit('test:distinct_payloads', { step: 1 });
            await eventBus.emit('test:distinct_payloads', { step: 2 });

            expect(handler).toHaveBeenCalledTimes(2);
            expect(handler).toHaveBeenNthCalledWith(1, { step: 1 });
            expect(handler).toHaveBeenNthCalledWith(2, { step: 2 });
        });

        it('allows identical events after the 50ms deduplication window elapses', async () => {
            const handler = vi.fn();
            await eventBus.on(TauriEvents.SCAN_PROGRESS, handler);

            // First emit
            await eventBus.emit(TauriEvents.SCAN_PROGRESS, { scanned: 10, total: 100 });
            expect(handler).toHaveBeenCalledTimes(1);

            // Wait beyond the 50ms window
            await new Promise((resolve) => setTimeout(resolve, 70));

            // Second identical emit outside the window
            await eventBus.emit(TauriEvents.SCAN_PROGRESS, { scanned: 10, total: 100 });
            expect(handler).toHaveBeenCalledTimes(2);
        });
    });
});

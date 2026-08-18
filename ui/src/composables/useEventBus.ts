import { listen, emit as tauriEmit, type UnlistenFn, type Event } from '@tauri-apps/api/event';
import { ref, onUnmounted, getCurrentInstance } from 'vue';

type LocalHandler = (payload: any) => void | Promise<void>;
const localListeners = new Map<string, Set<LocalHandler>>();

/**
 * Composable for managing Tauri & internal event listeners
 * Automatically cleans up listeners on component unmount
 */
export function useEventBus() {
    const listeners = ref<UnlistenFn[]>([]);
    const registeredLocalHandlers = ref<Array<{ event: string; handler: LocalHandler }>>([]);
    const isListening = ref(false);

    /**
     * Subscribe to a Tauri or local event
     */
    async function on<T>(
        event: string,
        handler: (payload: T) => void | Promise<void>
    ): Promise<UnlistenFn> {
        // Register with local listeners map immediately for fast synchronous communication
        if (!localListeners.has(event)) {
            localListeners.set(event, new Set());
        }
        const set = localListeners.get(event)!;
        set.add(handler as LocalHandler);
        registeredLocalHandlers.value.push({ event, handler: handler as LocalHandler });

        let unlistenTauri: UnlistenFn | null = null;
        try {
            unlistenTauri = await listen<T>(event, (e: Event<T>) => {
                handler(e.payload);
            });
            listeners.value.push(unlistenTauri);
        } catch {
            // In non-tauri or unit-test environments, fallback to local bus
        }

        isListening.value = true;

        const unlisten = () => {
            if (unlistenTauri) {
                unlistenTauri();
                const index = listeners.value.indexOf(unlistenTauri);
                if (index > -1) {
                    listeners.value.splice(index, 1);
                }
            }
            const localSet = localListeners.get(event);
            if (localSet) {
                localSet.delete(handler as LocalHandler);
                if (localSet.size === 0) {
                    localListeners.delete(event);
                }
            }
        };

        return unlisten;
    }

    /**
     * Emit an event locally and across Tauri
     */
    async function emit<T>(event: string, payload?: T): Promise<void> {
        // Dispatch to local subscribers
        const localSet = localListeners.get(event);
        if (localSet) {
            for (const handler of Array.from(localSet)) {
                try {
                    await handler(payload);
                } catch (e) {
                    console.error(`Error in local handler for event ${event}:`, e);
                }
            }
        }

        try {
            await tauriEmit(event, payload);
        } catch {
            // Ignored in unit-tests / non-Tauri contexts
        }
    }

    /**
     * Unsubscribe from a specific listener
     */
    function off(unlisten: UnlistenFn): void {
        unlisten();
        if (listeners.value.length === 0 && registeredLocalHandlers.value.length === 0) {
            isListening.value = false;
        }
    }

    /**
     * Unsubscribe from all listeners
     */
    function offAll(): void {
        listeners.value.forEach(unlisten => unlisten());
        listeners.value = [];
        for (const { event, handler } of registeredLocalHandlers.value) {
            const localSet = localListeners.get(event);
            if (localSet) {
                localSet.delete(handler);
            }
        }
        registeredLocalHandlers.value = [];
        isListening.value = false;
    }

    // Auto-cleanup on component unmount if within active component
    try {
        if (getCurrentInstance()) {
            onUnmounted(() => {
                offAll();
            });
        }
    } catch {
        // Ignored when called outside component context
    }

    return {
        on,
        emit,
        off,
        offAll,
        isListening,
        listenerCount: () => listeners.value.length + registeredLocalHandlers.value.length,
    };
}

/**
 * Pre-defined event names for Tauri events
 */
export const TauriEvents = {
    // Download events (from worker.rs - uses syncify: prefix)
    DOWNLOAD_PROGRESS: 'syncify:download_progress',
    DOWNLOAD_COMPLETE: 'syncify:download_progress', // Uses same event with status field
    DOWNLOAD_FAILED: 'syncify:download_progress',   // Uses same event with status field

    // Scan events
    SCAN_PROGRESS: 'scan-progress',
    SCAN_COMPLETE: 'scan-complete',

    // Import/Sync events
    IMPORT_PROGRESS: 'import-progress',
    IMPORT_COMPLETE: 'import-complete',
    IMPORT_FAILED: 'import-failed',

    // Organize events
    ORGANIZE_PROGRESS: 'organize-progress',
    ORGANIZE_COMPLETE: 'organize-complete',

    // Sync events
    SYNC_PROGRESS: 'sync-progress',
    SYNC_COMPLETE: 'sync-complete',
    SYNC_FAILED: 'sync-failed',

    // Auth events
    AUTH_SESSION_EXPIRED: 'auth-session-expired',

    // Tray events
    TRAY_ACTION: 'tray-action',

    // Background enrichment events
    ENRICHMENT_STATUS: 'background-enrichment-status',
} as const;

export type TauriEventName = typeof TauriEvents[keyof typeof TauriEvents];

/**
 * Event Bus Composable
 * 
 * Manages Tauri event listeners with automatic cleanup.
 */

import { listen, type UnlistenFn, type Event } from '@tauri-apps/api/event';
import { ref, onUnmounted } from 'vue';

/**
 * Composable for managing Tauri event listeners
 * Automatically cleans up listeners on component unmount
 */
export function useEventBus() {
    const listeners = ref<UnlistenFn[]>([]);
    const isListening = ref(false);

    /**
     * Subscribe to a Tauri event
     */
    async function on<T>(
        event: string,
        handler: (payload: T) => void | Promise<void>
    ): Promise<UnlistenFn> {
        const unlisten = await listen<T>(event, (e: Event<T>) => {
            handler(e.payload);
        });

        listeners.value.push(unlisten);
        isListening.value = true;

        return unlisten;
    }

    /**
     * Unsubscribe from a specific listener
     */
    function off(unlisten: UnlistenFn): void {
        unlisten();
        const index = listeners.value.indexOf(unlisten);
        if (index > -1) {
            listeners.value.splice(index, 1);
        }

        if (listeners.value.length === 0) {
            isListening.value = false;
        }
    }

    /**
     * Unsubscribe from all listeners
     */
    function offAll(): void {
        listeners.value.forEach(unlisten => unlisten());
        listeners.value = [];
        isListening.value = false;
    }

    // Auto-cleanup on component unmount
    onUnmounted(() => {
        offAll();
    });

    return {
        on,
        off,
        offAll,
        isListening,
        listenerCount: () => listeners.value.length,
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

    // Sync events (legacy)
    SYNC_PROGRESS: 'sync-progress',
    SYNC_COMPLETE: 'sync-complete',

    // Tray events
    TRAY_ACTION: 'tray-action',

    // Background enrichment events
    ENRICHMENT_STATUS: 'background-enrichment-status',
} as const;

export type TauriEventName = typeof TauriEvents[keyof typeof TauriEvents];

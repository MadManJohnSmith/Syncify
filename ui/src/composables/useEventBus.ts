import { listen, emit as tauriEmit, type UnlistenFn, type Event } from '@tauri-apps/api/event';
import { ref, onUnmounted, getCurrentInstance } from 'vue';
import { isTauri } from '@/api/tauri';

type LocalHandler = (payload: any) => void | Promise<void>;
const localListeners = new Map<string, Set<LocalHandler>>();

const DEDUPE_WINDOW_MS = 50;

function serializePayload(payload: unknown): string {
    if (payload === undefined) {
        return '__undefined__';
    }
    try {
        return JSON.stringify(payload);
    } catch {
        return String(payload);
    }
}

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
        let lastPayloadKey: string | null = null;
        let lastInvocationTime = 0;

        const deduplicatedHandler: LocalHandler = async (payload: any) => {
            const now = Date.now();
            const payloadKey = serializePayload(payload);

            if (
                lastPayloadKey === payloadKey &&
                now - lastInvocationTime < DEDUPE_WINDOW_MS
            ) {
                return;
            }

            lastPayloadKey = payloadKey;
            lastInvocationTime = now;
            await handler(payload);
        };

        // Register with local listeners map immediately for fast synchronous communication
        if (!localListeners.has(event)) {
            localListeners.set(event, new Set());
        }
        const set = localListeners.get(event)!;
        set.add(deduplicatedHandler);
        registeredLocalHandlers.value.push({ event, handler: deduplicatedHandler });

        let unlistenTauri: UnlistenFn | null = null;
        try {
            unlistenTauri = await listen<T>(event, (e: Event<T>) => {
                deduplicatedHandler(e.payload);
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
                localSet.delete(deduplicatedHandler);
                if (localSet.size === 0) {
                    localListeners.delete(event);
                }
            }
            const regIndex = registeredLocalHandlers.value.findIndex(
                entry => entry.event === event && entry.handler === deduplicatedHandler
            );
            if (regIndex > -1) {
                registeredLocalHandlers.value.splice(regIndex, 1);
            }
            if (listeners.value.length === 0 && registeredLocalHandlers.value.length === 0) {
                isListening.value = false;
            }
        };

        return unlisten;
    }

    /**
     * Emit an event locally and across Tauri
     */
    async function emit<T>(event: string, payload?: T): Promise<void> {
        if (isTauri()) {
            try {
                await tauriEmit(event, payload);
                return;
            } catch (err) {
                console.warn(`[useEventBus] tauriEmit failed for event "${event}", falling back to local dispatch:`, err);
            }
        }

        // Dispatch to local subscribers (in non-Tauri / test mode, or if tauriEmit failed)
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
                if (localSet.size === 0) {
                    localListeners.delete(event);
                }
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
 * Utility to reset local listeners map (for testing/cleanup)
 */
export function resetLocalListeners(): void {
    localListeners.clear();
}

export { TauriEvents, type TauriEventName } from '@/api/tauri';


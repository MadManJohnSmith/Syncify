import { ref, onUnmounted, getCurrentInstance } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useToast } from './useToast';
import type { AppNotification } from '@/api/notifications';

export function useNotificationListener() {
    const toast = useToast();
    const isListening = ref(false);
    const unlistens = ref<UnlistenFn[]>([]);

    if (getCurrentInstance()) {
        onUnmounted(stopListening);
    }

    async function startListening() {
        if (isListening.value) return;

        try {
            // Listen for general push notifications from backend
            const unlistenNotif = await listen<AppNotification>('syncify:notification', (event) => {
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
            unlistens.value.push(unlistenNotif);
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

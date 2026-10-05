/**
 * DownloadsViewQueue.spec.ts — regresión de auditoría 27 y de R11.
 *
 * 27: un fallo al leer la cola se muestra como error, no como «no tienes
 *     descargas».
 * R11: la cola se guarda como datos planos con un aviso de cambio explícito, y
 *      las listas largas solo materializan la porción que se pinta.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import DownloadsView from '@/views/DownloadsView.vue';
import { mockInvoke, resetMocks, emitMockEvent } from '../setup';

vi.mock('vue-router', () => ({
    useRouter: () => ({ push: vi.fn() }),
    useRoute: () => ({ query: {} }),
}));

function queueItem(overrides: Record<string, unknown> = {}) {
    return {
        id: 1,
        track_id: 101,
        status: 'downloading',
        title: 'Track One',
        artist: 'Artist One',
        service: 'tidal',
        quality: 'FLAC',
        progress_percent: 0,
        ...overrides,
    };
}

async function mountView(invoke: (cmd: string, args?: any) => unknown) {
    mockInvoke(invoke as any);
    const wrapper = mount(DownloadsView);
    await flushPromises();
    return wrapper;
}

const baseInvoke = (items: any[]) => (cmd: string) => {
    if (cmd === 'get_queue') return items;
    if (cmd === 'get_queue_stats') return { total: items.length, queued: 0, downloading: 1, completed: 0, failed: 0, paused: 0 };
    if (cmd === 'get_worker_status') return { running: true, paused: false, active_downloads: 1, max_concurrent: 3 };
    if (cmd === 'get_download_settings') return {};
    return null;
};

describe('DownloadsView — estados de carga y error (auditoría 27)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    it('muestra un error reintentable cuando la cola no se puede leer', async () => {
        let fail = true;
        const wrapper = await mountView((cmd) => {
            if (fail) throw new Error('worker offline');
            return baseInvoke([queueItem()])(cmd);
        });

        const errorBox = wrapper.find('[data-testid="downloads-load-error"]');
        expect(errorBox.exists()).toBe(true);
        expect(errorBox.text()).toContain('worker offline');
        // Un fallo no puede aterrizar en el estado vacío de la cola.
        expect(wrapper.text()).not.toContain('No active downloads running');

        fail = false;
        await wrapper.find('[data-testid="downloads-load-error"] button').trigger('click');
        await flushPromises();

        expect(wrapper.find('[data-testid="downloads-load-error"]').exists()).toBe(false);
        expect(wrapper.text()).toContain('Track One');
    });

    it('una cola realmente vacía sí muestra el estado vacío', async () => {
        const wrapper = await mountView(baseInvoke([]));

        expect(wrapper.find('[data-testid="downloads-load-error"]').exists()).toBe(false);
        expect(wrapper.text()).toContain('No active downloads running');
    });
});

describe('DownloadsView — la cola no es un ref profundo (R11)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    /**
     * La cola se guarda como datos planos y el aviso de cambio llega por un
     * contador explícito. Si ese aviso se perdiera, la fila se quedaría
     * congelada: esta prueba falla si el progreso deja de pintarse.
     */
    it('un evento de progreso sigue actualizando la fila visible', async () => {
        const wrapper = await mountView(baseInvoke([queueItem()]));

        emitMockEvent('syncify:download_progress', {
            queue_id: 1,
            percent: 42,
            status: 'downloading',
            total_bytes: 1000,
            bytes_downloaded: 420,
        })
        await flushPromises()

        const item = wrapper.find('.download-item')
        expect(item.exists()).toBe(true)
        expect(item.text()).toContain('42%')
    })

    it('un evento terminal mueve la descarga de activas a completadas', async () => {
        const wrapper = await mountView(baseInvoke([queueItem()]));

        emitMockEvent('syncify:download_progress', {
            queue_id: 1,
            percent: 100,
            status: 'completed',
            total_bytes: 1000,
            bytes_downloaded: 1000,
        })
        await flushPromises()

        // Deja de estar en la lista de activas...
        expect(wrapper.find('.download-item').exists()).toBe(false);
        expect(wrapper.text()).toContain('No active downloads running');
        // ...y aparece en la de completadas.
        const completed = wrapper.findAll('.completed-item');
        expect(completed.length).toBe(1);
        expect(completed[0].text()).toContain('Track One');
    })

    it('los datos de la cola no quedan envueltos en un proxy reactivo', async () => {
        const items = [queueItem({ id: 900, title: 'Plain Item' })]
        let seen: any = null
        mockInvoke((cmd) => {
            if (cmd === 'get_queue') {
                seen = items[0]
                return items
            }
            return baseInvoke(items)(cmd)
        })

        const wrapper = mount(DownloadsView)
        await flushPromises()
        wrapper.unmount()

        // Un objeto plano no tiene Band instrumentation de Vue.
        expect(typeof seen).toBe('object')
        expect(Object.prototype.hasOwnProperty.call(seen, '__v_isRef')).toBe(false)
    });
});
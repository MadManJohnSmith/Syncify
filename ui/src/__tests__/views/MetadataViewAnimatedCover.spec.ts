/** MET-03 / LY-04: el barrido S202 vive solo en Metadatos. */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import MetadataView from '@/views/MetadataView.vue';
import { mockInvoke, resetMocks, emitMockEvent } from '../setup';

vi.mock('vue-router', () => ({
    useRoute: () => ({ query: {}, params: {} }),
}));

const sweepResult = {
    scanned_albums: 7, already_animated: 3, downloaded: 2,
    not_found: 1, source_unavailable: 1, failed: 0, cancelled: false,
};

describe('MET-03: portadas animadas en Metadatos', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        document.body.innerHTML = '';
    });

    it('ejecuta el barrido sin cuenta de Apple Music y describe solo el sidecar, sin promesa de CoverFront', async () => {
        const calls: string[] = [];
        mockInvoke((cmd) => {
            calls.push(cmd);
            if (cmd === 'sweep_animated_covers') return sweepResult;
            if (cmd === 'get_library') return { tracks: [], total: 0, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_metadata_stats') return { total_tracks: 0 };
            return null;
        });
        const wrapper = mount(MetadataView);
        await flushPromises();
        // Sin cuenta conectada no hay gate previo: no se pregunta nada de cuentas.
        expect(calls).not.toContain('get_service_auth_status');
        await wrapper.find('[data-testid="open-cover-sweep"]').trigger('click');
        await flushPromises();

        const dialog = document.body.querySelector('.cover-sweep-dialog')!;
        expect(dialog).not.toBeNull();
        // Explicación honesta: la fuente pública puede fallar (source_unavailable).
        expect(dialog.textContent).toContain('Source unavailable');
        // Contrato real del backend: sidecars, nunca retag del FLAC.
        expect(dialog.textContent).toContain('cover.webp');
        expect(dialog.textContent).toContain('cover.animated.webp');
        expect(dialog.textContent).toContain('it does not modify the FLAC CoverFront');
        expect(dialog.textContent).not.toContain('updating the FLAC CoverFront');
        expect(dialog.querySelector('[role="alert"]')).toBeNull();
        expect(dialog.querySelector('[data-testid="cover-sweep-connection-error"]')).toBeNull();
        expect((dialog.querySelector('[data-testid="cover-sweep-run"]') as HTMLButtonElement).disabled).toBe(false);

        (dialog.querySelector('[data-testid="cover-sweep-run"]') as HTMLButtonElement).click();
        await flushPromises();

        const summary = document.body.querySelector('.cover-sweep-summary')!;
        expect(summary.textContent).toContain('Albums: 7');
        expect(summary.textContent).toContain('Already animated: 3');
        expect(summary.textContent).toContain('Downloaded: 2');
        expect(summary.textContent).toContain('No animation available: 1');
        expect(summary.textContent).toContain('Source unavailable: 1');
        expect(summary.textContent).toContain('Failures: 0');
        expect(calls).toContain('sweep_animated_covers');
        expect(calls).not.toContain('get_service_auth_status');
    });

    it('envía force y muestra los recuentos reales del backend', async () => {
        const calls: Array<{ cmd: string; args: unknown }> = [];
        mockInvoke((cmd, args) => {
            calls.push({ cmd, args });
            if (cmd === 'sweep_animated_covers') return sweepResult;
            if (cmd === 'get_library') return { tracks: [], total: 0, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_metadata_stats') return { total_tracks: 0 };
            return null;
        });
        const wrapper = mount(MetadataView);
        await flushPromises();
        await wrapper.find('[data-testid="open-cover-sweep"]').trigger('click');
        await flushPromises();

        const dialog = document.body.querySelector('.cover-sweep-dialog')!;
        (dialog.querySelector('input[type="checkbox"]') as HTMLInputElement).click();
        (dialog.querySelector('[data-testid="cover-sweep-run"]') as HTMLButtonElement).click();
        await flushPromises();

        expect(calls).not.toContain('get_service_auth_status');
        expect(calls.find(c => c.cmd === 'sweep_animated_covers')?.args).toEqual({ force: true });
    });

    it('muestra progreso y cancela por el backend mientras el barrido está activo', async () => {
        let release!: (result: unknown) => void;
        const pending = new Promise(resolve => { release = resolve; });
        const calls: string[] = [];
        mockInvoke((cmd) => {
            calls.push(cmd);
            if (cmd === 'sweep_animated_covers') return pending;
            if (cmd === 'cancel_animated_cover_sweep') return true;
            if (cmd === 'get_library') return { tracks: [], total: 0, offset: 0, limit: 500, has_more: false };
            if (cmd === 'get_metadata_stats') return { total_tracks: 0 };
            return null;
        });
        const wrapper = mount(MetadataView);
        await flushPromises();
        await wrapper.find('[data-testid="open-cover-sweep"]').trigger('click');
        await flushPromises();
        const dialog = document.body.querySelector('.cover-sweep-dialog')!;
        (dialog.querySelector('[data-testid="cover-sweep-run"]') as HTMLButtonElement).click();
        await flushPromises();
        emitMockEvent('animated-cover-sweep-progress', {
            status: 'checking', current: 3, total: 7, album: 'Álbum de prueba', message: 'Buscando...',
        });
        await flushPromises();
        expect(dialog.textContent).toContain('3 / 7');
        expect(dialog.textContent).toContain('Álbum de prueba');
        const cancel = Array.from(dialog.querySelectorAll('button')).find(b => b.textContent?.includes('Cancel Process'))!;
        cancel.click();
        await flushPromises();
        expect(calls).toContain('cancel_animated_cover_sweep');
        release({ ...sweepResult, cancelled: true });
        await flushPromises();
        expect(dialog.querySelector('.cover-sweep-summary')?.textContent).toContain('Sweep cancelled by the user.');
    });
});

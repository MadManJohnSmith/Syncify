/**
 * useVirtualWindow.spec.ts — caracterización del kernel de ventana virtual
 * (ADR-0025, decisión 1: paridad estricta).
 *
 * Congela el comportamiento exacto de las tres configuraciones que hoy usan
 * LibraryView, MetadataView y DownloadsView: índices de ventana, altura total
 * con/sin footer, throttle de scroll sí/no, reset sí/no y overscan. Si una
 * vista real se desvía de lo que aquí se exige, gana la vista: corregir el
 * test, no la vista.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { computed, effectScope, nextTick, ref } from 'vue';
import { useVirtualWindow } from '@/composables/useVirtualWindow';

interface Row {
    id: number;
}

function rows(n: number): Row[] {
    return Array.from({ length: n }, (_, i) => ({ id: i + 1 }));
}

/** Contenedor de scroll simulado: jsdom no hace layout, se fijan las medidas. */
function makeScroller(measures: { scrollHeight?: number; clientHeight?: number; scrollTop?: number } = {}) {
    const el = document.createElement('div');
    Object.defineProperty(el, 'scrollHeight', { value: measures.scrollHeight ?? 0, configurable: true });
    Object.defineProperty(el, 'clientHeight', { value: measures.clientHeight ?? 0, configurable: true });
    Object.defineProperty(el, 'scrollTop', { value: measures.scrollTop ?? 0, writable: true, configurable: true });
    return el;
}

/** Enlaza el contenedor y registra el handler, igual que `ref` + `@scroll` en el template. */
function bindScroller(
    w: ReturnType<typeof useVirtualWindow>,
    measures: { scrollHeight?: number; clientHeight?: number; scrollTop?: number } = {}
) {
    const el = makeScroller(measures);
    el.addEventListener('scroll', w.handleScroll);
    w.containerRef.value = el;
    return el;
}

function scroll(el: HTMLElement) {
    el.dispatchEvent(new Event('scroll'));
}

// Configs congeladas por vista. LibraryView pasa además el footer como getter
// (48 px solo cuando no hay más páginas) y el callback de loadMore.
const LIBRARY_BASE = { rowHeight: 56, overscan: 10, scrollThrottleMs: 120 } as const;
const METADATA_CONFIG = { rowHeight: 64, overscan: 5, emptyViewportHeight: 800 } as const;
const DOWNLOADS_CONFIG = { rowHeight: 60, overscan: 10, initialViewportHeight: 520, emptyViewportHeight: 520 } as const;

afterEach(() => {
    vi.useRealTimers();
});

describe('useVirtualWindow — LibraryView (56 px / overscan 10 / footer 48 / throttle 120)', () => {
    it('ventana en el origen: start 0, end ceil(800/56)+10 y offsetY 0', () => {
        const items = ref(rows(100));
        const w = useVirtualWindow(items, { ...LIBRARY_BASE });

        expect(w.startIndex.value).toBe(0);
        expect(w.endIndex.value).toBe(25); // ceil(800/56) = 15, +10 de overscan
        expect(w.offsetY.value).toBe(0);
        expect(w.visibleItems.value.length).toBe(25);
        expect(w.visibleItems.value[0]).toBe(items.value[0]);
    });

    it('ventana a media lista: scrollTop 1400 recorta 10 filas por lado', () => {
        const items = ref(rows(100));
        const w = useVirtualWindow(items, { ...LIBRARY_BASE });

        w.scrollTop.value = 1400;
        expect(w.startIndex.value).toBe(15); // floor(1400/56)=25, -10
        expect(w.endIndex.value).toBe(50);   // ceil(2200/56)=40, +10
        expect(w.offsetY.value).toBe(15 * 56);
        expect(w.visibleItems.value.map(r => r.id)).toEqual(items.value.slice(15, 50).map(r => r.id));
    });

    it('el overscan no baja de 0 y endIndex se recorta al total', () => {
        const items = ref(rows(30));
        const w = useVirtualWindow(items, { ...LIBRARY_BASE });

        w.scrollTop.value = 100; // floor(100/56)=1, -10 → negativo
        expect(w.startIndex.value).toBe(0);
        expect(w.endIndex.value).toBe(27); // ceil(900/56)=17, +10 = 27 (aún sin recorte)

        w.scrollTop.value = 800; // ceil(1600/56)=29, +10 = 39 → recortado al total
        expect(w.startIndex.value).toBe(4);
        expect(w.endIndex.value).toBe(30);
        expect(w.visibleItems.value.length).toBe(26);
    });

    it('altura total: 48 px de footer solo mientras no haya más páginas', async () => {
        const items = ref(rows(100));
        const hasMore = ref(true);
        const tracks = ref(rows(100));
        const w = useVirtualWindow(items, {
            ...LIBRARY_BASE,
            footerHeight: () => (!hasMore.value && tracks.value.length > 0 ? 48 : 0),
        });

        expect(w.totalHeight.value).toBe(100 * 56); // hasMore: sin footer
        hasMore.value = false;
        await nextTick();
        expect(w.totalHeight.value).toBe(100 * 56 + 48); // fin de lista: con footer
    });

    it('clientHeight 0 conserva la última medición del viewport (sin emptyViewportHeight)', () => {
        const items = ref(rows(100));
        const w = useVirtualWindow(items, { ...LIBRARY_BASE });
        const el = bindScroller(w, { scrollHeight: 5000, clientHeight: 600 });

        scroll(el);
        expect(w.viewportHeight.value).toBe(600);

        Object.defineProperty(el, 'clientHeight', { value: 0, configurable: true });
        scroll(el);
        expect(w.viewportHeight.value).toBe(600); // Library no repone el viewport
    });

    it('throttle 120 ms: primera medición inmediata y la siguiente por el corte, con scrollBottom medido dentro', () => {
        vi.useFakeTimers({ now: 1_000_000 });
        const items = ref(rows(100));
        const measured: number[] = [];
        const w = useVirtualWindow(items, { ...LIBRARY_BASE, onScrollMeasured: (sb) => measured.push(sb) });
        const el = bindScroller(w, { scrollHeight: 2000, clientHeight: 600, scrollTop: 1350 });

        // Primer evento: sin medición previa → leading call inmediata.
        scroll(el);
        expect(measured).toEqual([2000 - 1350 - 600]); // 50, bajo el umbral 300 de la vista

        // Ráfaga dentro de la ventana: una sola medición trailing pendiente.
        el.scrollTop = 1400;
        scroll(el);
        el.scrollTop = 1450;
        scroll(el);
        expect(measured.length).toBe(1);

        vi.advanceTimersByTime(119);
        expect(measured.length).toBe(1);
        vi.advanceTimersByTime(1);
        expect(measured.length).toBe(2);
        expect(measured[1]).toBe(2000 - 1450 - 600);
    });

    it('la posición reactiva se actualiza en cada evento aunque la medición se throttle', () => {
        vi.useFakeTimers({ now: 1_000_000 });
        const items = ref(rows(100));
        const w = useVirtualWindow(items, { ...LIBRARY_BASE, onScrollMeasured: () => {} });
        const el = bindScroller(w, { scrollHeight: 5000, clientHeight: 800, scrollTop: 0 });

        scroll(el); // leading
        el.scrollTop = 1400;
        scroll(el); // dentro del corte
        expect(w.scrollTop.value).toBe(1400);
        expect(w.startIndex.value).toBe(15);
        expect(w.endIndex.value).toBe(50);
    });

    it('el timer de throttle pendiente muere con el scope (paridad del onUnmounted de LibraryView)', () => {
        vi.useFakeTimers({ now: 1_000_000 });
        const items = ref(rows(100));
        const measured: number[] = [];
        const scope = effectScope();
        const w = scope.run(() =>
            useVirtualWindow(items, { ...LIBRARY_BASE, onScrollMeasured: (sb) => measured.push(sb) })
        )!;
        const el = bindScroller(w, { scrollHeight: 2000, clientHeight: 600, scrollTop: 0 });

        scroll(el); // leading
        scroll(el); // trailing pendiente
        scope.stop();
        vi.advanceTimersByTime(500);
        expect(measured.length).toBe(1); // el trailing se canceló
    });
});

describe('useVirtualWindow — MetadataView (64 px / overscan 5 / sin throttle / reset explícito)', () => {
    it('ventana y altura sin footer: scrollTop 1280 recorta 5 filas por lado', () => {
        const items = ref(rows(500));
        const w = useVirtualWindow(items, { ...METADATA_CONFIG });

        expect(w.totalHeight.value).toBe(500 * 64); // sin footer
        w.scrollTop.value = 1280;
        expect(w.startIndex.value).toBe(15); // floor(1280/64)=20, -5
        expect(w.endIndex.value).toBe(38);   // ceil(2080/64)=33, +5
        expect(w.offsetY.value).toBe(15 * 64);
    });

    it('cada evento de scroll actualiza la ventana al instante (sin throttle)', () => {
        const items = ref(rows(500));
        const w = useVirtualWindow(items, { ...METADATA_CONFIG });
        const el = bindScroller(w, { scrollHeight: 32000, clientHeight: 800, scrollTop: 0 });

        el.scrollTop = 1280;
        scroll(el);
        expect(w.startIndex.value).toBe(15);

        el.scrollTop = 2560;
        scroll(el);
        expect(w.startIndex.value).toBe(35); // floor(2560/64)=40, -5
        expect(w.endIndex.value).toBe(58);   // ceil(3360/64)=53, +5
    });

    it('clientHeight 0 repone el viewport a 800 (emptyViewportHeight)', () => {
        const items = ref(rows(500));
        const w = useVirtualWindow(items, { ...METADATA_CONFIG });
        const el = bindScroller(w, { scrollHeight: 32000, clientHeight: 600 });

        scroll(el);
        expect(w.viewportHeight.value).toBe(600);

        Object.defineProperty(el, 'clientHeight', { value: 0, configurable: true });
        scroll(el);
        expect(w.viewportHeight.value).toBe(800);
    });

    it('reset() vuelve a cero el estado y el contenedor; sin contenedor no lanza', () => {
        const items = ref(rows(500));
        const w = useVirtualWindow(items, { ...METADATA_CONFIG });
        const el = bindScroller(w, { scrollHeight: 32000, clientHeight: 800, scrollTop: 0 });

        el.scrollTop = 1280;
        scroll(el);
        expect(w.scrollTop.value).toBe(1280);

        w.reset();
        expect(w.scrollTop.value).toBe(0);
        expect(el.scrollTop).toBe(0);
        expect(w.startIndex.value).toBe(0);

        // El watch de MetadataView puede disparar el reset con la lista vacía
        // (contenedor desmontado): no debe lanzar.
        w.containerRef.value = null;
        w.scrollTop.value = 999;
        expect(() => w.reset()).not.toThrow();
        expect(w.scrollTop.value).toBe(0);
    });
});

describe('useVirtualWindow — DownloadsView (60 px / overscan 10 / viewport 520 / sin reset)', () => {
    it('ventana y altura sin footer: viewport inicial 520', () => {
        const items = ref(rows(1000));
        const w = useVirtualWindow(items, { ...DOWNLOADS_CONFIG });

        expect(w.totalHeight.value).toBe(1000 * 60);
        expect(w.startIndex.value).toBe(0);
        expect(w.endIndex.value).toBe(19); // ceil(520/60)=9, +10 de overscan

        w.scrollTop.value = 3000;
        expect(w.startIndex.value).toBe(40); // floor(3000/60)=50, -10
        expect(w.endIndex.value).toBe(69);   // ceil(3520/60)=59, +10
        expect(w.offsetY.value).toBe(40 * 60);
    });

    it('clientHeight 0 repone el viewport a 520 (emptyViewportHeight)', () => {
        const items = ref(rows(1000));
        const w = useVirtualWindow(items, { ...DOWNLOADS_CONFIG });
        const el = bindScroller(w, { scrollHeight: 60000, clientHeight: 440 });

        scroll(el);
        expect(w.viewportHeight.value).toBe(440);

        Object.defineProperty(el, 'clientHeight', { value: 0, configurable: true });
        scroll(el);
        expect(w.viewportHeight.value).toBe(520);
    });

    it('visibleItems son los mismos objetos del array (R11: sin clonar) y absoluteIndex se adjunta en la vista', () => {
        const items = ref(rows(1000));
        const w = useVirtualWindow(items, { ...DOWNLOADS_CONFIG });
        w.scrollTop.value = 3000;

        // La cola guarda datos planos: el kernel solo recorta, nunca envuelve.
        expect(w.visibleItems.value[0]).toBe(items.value[40]);

        // Patrón de DownloadsView: mapear la ventana adjuntando absoluteIndex.
        const visibleQueueItems = w.visibleItems.value.map((item, idx) => ({
            ...item,
            absoluteIndex: w.startIndex.value + idx,
        }));
        expect(visibleQueueItems.map(i => i.absoluteIndex)).toEqual(
            Array.from({ length: w.visibleItems.value.length }, (_, i) => 40 + i)
        );
    });

    it('cambiar la fuente NO resetea el scroll (el reset no es del kernel)', async () => {
        const items = ref(rows(1000));
        const w = useVirtualWindow(items, { ...DOWNLOADS_CONFIG });
        w.scrollTop.value = 3000;
        expect(w.startIndex.value).toBe(40);

        items.value = rows(50); // nuevo filtro/recorte: la vista decide si resetea
        await nextTick();
        expect(w.scrollTop.value).toBe(3000);
        expect(w.startIndex.value).toBe(40); // paridad: sin clamp superior
        expect(w.endIndex.value).toBe(50);   // recortado al nuevo total
        expect(w.totalHeight.value).toBe(50 * 60);
    });
});

describe('useVirtualWindow — API mínima compartida', () => {
    it('scrollToOffset asigna el valor bruto sin recortar (paridad de jumpToPosition y scrollToTrack)', () => {
        const items = ref(rows(100));
        const w = useVirtualWindow(items, { ...DOWNLOADS_CONFIG });
        const el = bindScroller(w, { scrollHeight: 6000, clientHeight: 520 });

        // (targetIdx - 1) * ROW_HEIGHT puede exceder el scroll máximo real:
        // el elemento lo recorta el navegador, el estado recibe el bruto.
        w.scrollToOffset(5940);
        expect(el.scrollTop).toBe(5940);
        expect(w.scrollTop.value).toBe(5940);
    });

    it('scrollToOffset sin contenedor no lanza ni toca el estado', () => {
        const items = ref(rows(100));
        const w = useVirtualWindow(items, { ...DOWNLOADS_CONFIG });
        expect(() => w.scrollToOffset(120)).not.toThrow();
        expect(w.scrollTop.value).toBe(0);
    });

    it('startIndex solo tiene clamp inferior: por encima del total devuelve ventana vacía (paridad del original)', () => {
        const items = ref(rows(100));
        const w = useVirtualWindow(items, { ...DOWNLOADS_CONFIG });

        w.scrollTop.value = 9999 * 60;
        expect(w.startIndex.value).toBe(9989); // sin min(count)
        expect(w.visibleItems.value).toEqual([]); // slice vacío, como hoy
    });

    it('acepta el array del filtro como computed, igual que las vistas', () => {
        const source = ref(rows(100));
        const filtered = computed(() => source.value.filter(r => r.id % 2 === 0)); // 50 filas
        const w = useVirtualWindow(filtered, { ...METADATA_CONFIG });

        expect(w.totalHeight.value).toBe(50 * 64);
        expect(w.visibleItems.value[0]).toBe(source.value[1]);
    });
});

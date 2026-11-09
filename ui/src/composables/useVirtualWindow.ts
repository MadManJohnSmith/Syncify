/**
 * Kernel de ventana virtual compartido por las tres listas grandes
 * (ADR-0025, decisión 1). Sustituye al triple clon que vivía en
 * LibraryView, MetadataView y DownloadsView con paridad estricta: las
 * divergencias históricas quedan congeladas como configuración explícita.
 *
 * El kernel posee la mecánica y el estado de scroll (posición del
 * contenedor y altura del viewport, con la lectura pesada de layout dentro
 * del corte de throttle). La vista conserva la política: qué array se
 * virtualiza, cuándo se resetea y qué se dispara al acercarse al fondo
 * (callback `onScrollMeasured`). El mapeo de fila y los adornos por ítem
 * (p. ej. el `absoluteIndex` de la cola) siguen en la vista.
 */
import { computed, onScopeDispose, ref, toValue } from 'vue'
import type { MaybeRefOrGetter, Ref } from 'vue'

export interface VirtualWindowOptions {
    /** Altura fija de cada fila, en px. */
    rowHeight: number;
    /** Filas extra renderizadas por encima y por debajo del área visible. */
    overscan: number;
    /**
     * Altura extra del spacer además de las filas. LibraryView reserva aquí
     * el pie de fin de lista (48 px) mientras no haya más páginas; es
     * reactivo porque depende del estado de paginación.
     */
    footerHeight?: MaybeRefOrGetter<number>;
    /** Altura del viewport antes de la primera medición (por defecto 800). */
    initialViewportHeight?: number;
    /**
     * Valor al que se repone el viewport cuando `clientHeight` mide 0.
     * Si se omite, se conserva la última medición (comportamiento de
     * LibraryView).
     */
    emptyViewportHeight?: number;
    /**
     * Corte de throttle de la medición pesada (`scrollHeight`), en ms.
     * 0 u omitido mide en cada evento (MetadataView y DownloadsView no
     * aplican throttle).
     */
    scrollThrottleMs?: number;
    /**
     * Se invoca con la distancia al fondo (`scrollHeight - scrollTop -
     * clientHeight`) medida DENTRO del corte de throttle. LibraryView la usa
     * para disparar loadMore cuando baja de 300 px.
     */
    onScrollMeasured?: (scrollBottom: number) => void;
}

export function useVirtualWindow<T>(
    items: MaybeRefOrGetter<readonly T[]>,
    options: VirtualWindowOptions
) {
    const {
        rowHeight,
        overscan,
        footerHeight = 0,
        initialViewportHeight = 800,
        emptyViewportHeight,
        scrollThrottleMs = 0,
        onScrollMeasured,
    } = options;

    const containerRef: Ref<HTMLElement | null> = ref(null);
    const scrollTop = ref(0);
    const viewportHeight = ref(initialViewportHeight);

    const itemCount = computed(() => toValue(items).length);

    const totalHeight = computed(() => itemCount.value * rowHeight + toValue(footerHeight));

    const startIndex = computed(() => Math.max(0, Math.floor(scrollTop.value / rowHeight) - overscan));

    const endIndex = computed(() => Math.min(
        itemCount.value,
        Math.ceil((scrollTop.value + viewportHeight.value) / rowHeight) + overscan
    ));

    const offsetY = computed(() => startIndex.value * rowHeight);

    const visibleItems = computed(() => toValue(items).slice(startIndex.value, endIndex.value));

    // Scroll: la posición reactiva se actualiza en cada evento sin leer
    // layout pesado (PR-01/PR-02); la lectura de scrollHeight vive solo
    // dentro del corte de throttle.
    function applyMeasurements(target: HTMLElement) {
        scrollTop.value = target.scrollTop;
        const measured = target.clientHeight;
        if (measured) {
            viewportHeight.value = measured;
        } else if (emptyViewportHeight !== undefined) {
            viewportHeight.value = emptyViewportHeight;
        }
    }

    let lastMeasuredAt = 0;
    let measureTimer: ReturnType<typeof setTimeout> | null = null;

    function measure(el: HTMLElement) {
        lastMeasuredAt = Date.now();
        onScrollMeasured?.(el.scrollHeight - el.scrollTop - el.clientHeight);
    }

    function handleScroll(event: Event) {
        const target = event.target as HTMLElement | null;
        if (!target) return;

        applyMeasurements(target);

        if (!onScrollMeasured) return;

        const elapsed = Date.now() - lastMeasuredAt;
        if (elapsed >= scrollThrottleMs) {
            measure(target);
            return;
        }
        if (!measureTimer) {
            measureTimer = setTimeout(() => {
                measureTimer = null;
                // El contenedor puede haber cambiado durante la espera; el
                // target del evento es la alternativa (como en LibraryView).
                const el = containerRef.value || target;
                if (el) measure(el);
            }, scrollThrottleMs - elapsed);
        }
    }

    // Un montaje limpia el throttle pendiente al desmontar (igual que hacía
    // LibraryView en su onUnmounted); fuera de un scope no hay nada que
    // limpiar y se ignora en silencio.
    onScopeDispose(() => {
        if (measureTimer) {
            clearTimeout(measureTimer);
            measureTimer = null;
        }
    }, true);

    /** Reinicia el scroll a 0 (estado y contenedor). Los watches de la vista deciden cuándo. */
    function reset() {
        scrollTop.value = 0;
        if (containerRef.value) {
            containerRef.value.scrollTop = 0;
        }
    }

    /**
     * Coloca el contenedor en un offset absoluto. Sin recorte: el navegador
     * recorta el elemento y el estado recibe el valor bruto, igual que en
     * los clones (jumpToPosition y scrollToTrack no leen de vuelta).
     */
    function scrollToOffset(top: number) {
        const el = containerRef.value;
        if (!el) return;
        el.scrollTop = top;
        scrollTop.value = top;
    }

    return {
        /** Ref del contenedor de scroll; la vista lo enlaza en el template. */
        containerRef,
        /** Posición de scroll en px; escribible para las APIs de scroll de la vista. */
        scrollTop,
        /** Altura medida del viewport en px. */
        viewportHeight,
        /** Altura total del spacer (filas + footer opcional). */
        totalHeight,
        /** Primer índice del array fuente renderizado. */
        startIndex,
        /** Índice exclusivo final del array fuente renderizado. */
        endIndex,
        /** Desplazamiento vertical del bloque de filas visibles, en px. */
        offsetY,
        /** Porción del array fuente renderizada (los mismos objetos, sin clonar). */
        visibleItems,
        /** Handler para `@scroll` del contenedor. */
        handleScroll,
        reset,
        scrollToOffset,
    };
}

/**
 * Composable de navegador de pistas para las listas basadas en biblioteca
 * (ADR-0025, decisiones 2 y 3): fuente declarada por la vista, mapeo inyectado
 * y selección por ids.
 *
 * El composable NO conoce los comandos IPC: cada vista declara su fuente
 *
 *   - paginada (LibraryView): recibe `(offset, limit)` y devuelve la página
 *     con `total` y `has_more`; la triple fuente `get_library` /
 *     `get_favorite_tracks` / `get_duplicate_tracks` queda en la vista.
 *   - de página fija (MetadataView): una tanda por carga, sin `has_more`.
 *
 * y opcionalmente una fuente FTS (LibraryView) con paginador propio; la
 * normalización de la consulta (un `*` por palabra) vive aquí para que el
 * ritmo de la búsqueda sea caracterizable.
 *
 * El mapeo es inyectado: las filas son tipos de la vista (`mapRow`). La
 * selección es por ids con la semántica exacta que tenía MetadataView:
 * `selectOnly` no deselecciona lo ya seleccionado, `toggle` añade/quita,
 * `selectAll` REEMPLAZA con los ids de las filas actuales y `preserve` deja
 * solo los ids vivos entre recargas.
 */
import { computed, ref } from 'vue';
import type { ComputedRef, Ref } from 'vue';

/** Página que devuelve una fuente paginada (misma forma que `LibraryPage`/`SearchResult` de `@/api/library`). */
export interface BrowserSourcePage<TItem> {
    tracks: TItem[];
    total: number;
    offset: number;
    has_more: boolean;
}

/** Tanda única que devuelve una fuente de página fija (sin `has_more`). */
export interface BrowserFixedPage<TItem> {
    tracks: TItem[];
    total: number;
}

/** Fuente paginada de biblioteca: recibe qué cargar por página. */
export type BrowserPagedSource<TItem> = (offset: number, limit: number) => Promise<BrowserSourcePage<TItem>>;

/** Fuente de página fija: una sola tanda por carga; el composable pasa el límite acordado. */
export type BrowserFixedSource<TItem> = (limit: number) => Promise<BrowserFixedPage<TItem>>;

/** Fuente FTS: recibe la consulta YA normalizada (prefijo `*`) y pagina como la biblioteca. */
export type BrowserSearchSource<TItem> = (ftsQuery: string, offset: number, limit: number) => Promise<BrowserSourcePage<TItem>>;

export type BrowserSource<TItem> =
    | { kind: 'paged'; load: BrowserPagedSource<TItem> }
    | { kind: 'fixed'; load: BrowserFixedSource<TItem> };

export interface UseTrackBrowserOptions<TItem, TRow> {
    /** Mapeo inyectado: la fila es un tipo de la vista. */
    mapRow: (item: TItem, index: number) => TRow;
    /** Fuente declarada por la vista; el composable no conoce los comandos. */
    source: BrowserSource<TItem>;
    /** Tamaño de página de la fuente principal (Library: 100; Metadata: 500). */
    pageSize: number;
    /** Fuente FTS declarada (Library). Sin ella `search` es `null`. */
    searchSource?: { load: BrowserSearchSource<TItem> };
    /** Tamaño de página del buscador (Library: 100). Por defecto, `pageSize`. */
    searchPageSize?: number;
    /**
     * Reacción de la vista al fallo de la carga principal. El composable ya
     * registra el error (`loadError` + consola); la vista añade lo suyo
     * (MetadataView muestra su toast local).
     */
    onLoadError?: (error: unknown) => void;
}

/** Selección por ids compartida por las dos vistas (ADR-0025, decisión 3). */
export interface TrackBrowserSelection {
    /** Ids seleccionados, en el orden en que se seleccionaron. */
    selectedIds: Ref<number[]>;
    /** Derivado: el contador no se lleva a mano al estilo L-05. */
    selectedCount: ComputedRef<number>;
    /** Lectura O(1) para las plantillas (clase de fila, `:checked`). */
    isSelected: (id: number) => boolean;
    /** Añade el id si falta, lo quita si está (mutación en el sitio, como el toggle de MetadataView). */
    toggle: (id: number) => void;
    /** Reemplaza la selección con `[id]` salvo que ya esté (semántica de `selectTrack` de MetadataView). */
    selectOnly: (id: number) => void;
    /** REEMPLAZA la selección con los ids de las filas actuales. */
    selectAll: (currentRows: ReadonlyArray<{ id: number }>) => void;
    /** Vacía la selección. */
    clear: () => void;
    /** Deja solo los ids vivos entre recargas (semántica de MetadataView al recargar). */
    preserve: (liveIds: ReadonlyArray<number>) => void;
}

/** Estado y operaciones del buscador FTS (LibraryView). */
export interface TrackBrowserSearch<TRow> {
    /** Resultados mapeados. */
    rows: Ref<TRow[]>;
    /** Última consulta normalizada (con `*`); vacía tras un reset. */
    ftsQuery: Ref<string>;
    offset: Ref<number>;
    total: Ref<number>;
    hasMore: Ref<boolean>;
    isSearching: Ref<boolean>;
    /** Lanza la búsqueda con la consulta en crudo del input (la normaliza aquí). */
    run: (rawQuery: string) => Promise<void>;
    /** Página siguiente del paginador propio del buscador. */
    loadMore: () => Promise<void>;
    /** Vuelve al estado inicial (lo que hacía el watch con la consulta vacía). */
    reset: () => void;
}

export interface TrackBrowser<TRow> {
    /** Filas mapeadas de la fuente principal; la vista las reordena y filtra encima. */
    rows: Ref<TRow[]>;
    /** `total` de la última página leída; `loadMore` NO lo toca (paridad con LibraryView). */
    total: Ref<number>;
    currentOffset: Ref<number>;
    hasMore: Ref<boolean>;
    isLoading: Ref<boolean>;
    isLoadingMore: Ref<boolean>;
    /** Fallo de la última carga principal; `loadMore` y el buscador no lo tocan (paridad). */
    loadError: Ref<string | null>;
    /**
     * Páginas añadidas desde el principio: llave de la optimización de prefijo
     * ordenado de LibraryView (solo un append reutiliza el prefijo).
     */
    pageAppendCount: Ref<number>;
    /** Recarga desde el offset 0 reemplazando las filas. No toca la selección: cada vista decide. */
    load: () => Promise<void>;
    /** Página siguiente (solo fuente paginada; en página fija es un no-op). */
    loadMore: () => Promise<void>;
    selection: TrackBrowserSelection;
    search: TrackBrowserSearch<TRow> | null;
}

/** Normaliza la consulta del input para FTS5: un `*` por palabra, sin símbolos. */
export function toFtsPrefixQuery(raw: string): string {
    return raw
        .trim()
        .split(/\s+/)
        .map(w => w.replace(/[^\w]/g, ''))
        .filter(w => w.length > 0)
        .map(w => w + '*')
        .join(' ');
}

function createSelection(): TrackBrowserSelection {
    const selectedIds = ref<number[]>([]) as Ref<number[]>;
    const selectedSet = computed(() => new Set(selectedIds.value));
    const selectedCount = computed(() => selectedIds.value.length);

    function isSelected(id: number): boolean {
        return selectedSet.value.has(id);
    }

    function toggle(id: number): void {
        const index = selectedIds.value.indexOf(id);
        if (index === -1) {
            selectedIds.value.push(id);
        } else {
            selectedIds.value.splice(index, 1);
        }
    }

    function selectOnly(id: number): void {
        // "Already selected, do nothing": igual que selectTrack de MetadataView.
        if (selectedIds.value.includes(id)) return;
        selectedIds.value = [id];
    }

    function selectAll(currentRows: ReadonlyArray<{ id: number }>): void {
        selectedIds.value = currentRows.map(row => row.id);
    }

    function clear(): void {
        selectedIds.value = [];
    }

    function preserve(liveIds: ReadonlyArray<number>): void {
        const live = new Set(liveIds);
        selectedIds.value = selectedIds.value.filter(id => live.has(id));
    }

    return { selectedIds, selectedCount, isSelected, toggle, selectOnly, selectAll, clear, preserve };
}

function createSearch<TItem, TRow>(
    searchLoad: BrowserSearchSource<TItem>,
    searchPageSize: number,
    mapRow: (item: TItem, index: number) => TRow
): TrackBrowserSearch<TRow> {
    const rows = ref<TRow[]>([]) as Ref<TRow[]>;
    const ftsQuery = ref('');
    const offset = ref(0);
    const total = ref(0);
    const hasMore = ref(false);
    const isSearching = ref(false);

    function reset(): void {
        rows.value = [];
        offset.value = 0;
        total.value = 0;
        hasMore.value = false;
        ftsQuery.value = '';
        isSearching.value = false;
    }

    async function run(rawQuery: string): Promise<void> {
        if (!rawQuery.trim()) {
            reset();
            return;
        }

        isSearching.value = true;
        try {
            const normalized = toFtsPrefixQuery(rawQuery);
            if (!normalized) {
                // Consulta solo de símbolos: se vacían los resultados y se corta
                // el giro, sin tocar el resto del estado (paridad exacta).
                rows.value = [];
                return;
            }

            offset.value = 0;
            ftsQuery.value = normalized;

            const result = await searchLoad(normalized, 0, searchPageSize);
            rows.value = result.tracks.map((item, index) => mapRow(item, index));
            total.value = result.total;
            hasMore.value = result.has_more;
            offset.value = result.offset + result.tracks.length;
        } catch (error) {
            console.error('useTrackBrowser: database search failed:', error);
            // En fallo solo mueren los resultados; total/hasMore/offset/consulta
            // quedan como estaban (paridad con el catch original).
            rows.value = [];
        } finally {
            isSearching.value = false;
        }
    }

    async function loadMore(): Promise<void> {
        if (isSearching.value || !hasMore.value || !ftsQuery.value) return;

        isSearching.value = true;
        try {
            const result = await searchLoad(ftsQuery.value, offset.value, searchPageSize);
            const newRows = result.tracks.map((item, index) => mapRow(item, index));
            rows.value = [...rows.value, ...newRows];
            hasMore.value = result.has_more;
            offset.value = result.offset + result.tracks.length;
        } catch (error) {
            console.error('useTrackBrowser: failed to load more search results:', error);
        } finally {
            isSearching.value = false;
        }
    }

    return { rows, ftsQuery, offset, total, hasMore, isSearching, run, loadMore, reset };
}

export function useTrackBrowser<TItem, TRow>(
    options: UseTrackBrowserOptions<TItem, TRow> & { searchSource: { load: BrowserSearchSource<TItem> } }
): TrackBrowser<TRow> & { search: TrackBrowserSearch<TRow> };
export function useTrackBrowser<TItem, TRow>(
    options: UseTrackBrowserOptions<TItem, TRow>
): TrackBrowser<TRow>;
export function useTrackBrowser<TItem, TRow>(
    options: UseTrackBrowserOptions<TItem, TRow>
): TrackBrowser<TRow> {
    const { mapRow, source, pageSize, searchSource, searchPageSize = pageSize, onLoadError } = options;

    const rows = ref<TRow[]>([]) as Ref<TRow[]>;
    const total = ref(0);
    const currentOffset = ref(0);
    const hasMore = ref(false);
    const isLoading = ref(false);
    const isLoadingMore = ref(false);
    const loadError = ref<string | null>(null);
    const pageAppendCount = ref(0);

    async function load(): Promise<void> {
        isLoading.value = true;
        loadError.value = null;
        currentOffset.value = 0;
        try {
            if (source.kind === 'fixed') {
                const result = await source.load(pageSize);
                rows.value = result.tracks.map((item, index) => mapRow(item, index));
                total.value = result.total;
                hasMore.value = false;
                currentOffset.value = rows.value.length;
            } else {
                const page = await source.load(0, pageSize);
                rows.value = page.tracks.map((item, index) => mapRow(item, index));
                total.value = page.total;
                hasMore.value = page.has_more;
                currentOffset.value = page.offset + page.tracks.length;
            }
        } catch (error) {
            loadError.value = String(error);
            console.error('useTrackBrowser: failed to load source:', error);
            onLoadError?.(error);
        } finally {
            isLoading.value = false;
        }
    }

    async function loadMore(): Promise<void> {
        if (source.kind !== 'paged') return;
        if (isLoadingMore.value || !hasMore.value) return;

        isLoadingMore.value = true;
        try {
            const page = await source.load(currentOffset.value, pageSize);
            const newRows = page.tracks.map((item, index) => mapRow(item, index));
            // Push sobre el MISMO array: la identidad del array es la prueba de
            // "solo hubo un append" de la optimización de prefijo ordenado.
            rows.value.push(...newRows);
            pageAppendCount.value++;
            hasMore.value = page.has_more;
            currentOffset.value = page.offset + page.tracks.length;
        } catch (error) {
            console.error('useTrackBrowser: failed to load more tracks:', error);
        } finally {
            isLoadingMore.value = false;
        }
    }

    const selection = createSelection();
    const search = searchSource ? createSearch(searchSource.load, searchPageSize, mapRow) : null;

    return {
        rows,
        total,
        currentOffset,
        hasMore,
        isLoading,
        isLoadingMore,
        loadError,
        pageAppendCount,
        load,
        loadMore,
        selection,
        search,
    };
}

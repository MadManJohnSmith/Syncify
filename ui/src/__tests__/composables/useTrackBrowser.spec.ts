/**
 * useTrackBrowser.spec.ts — caracterización del composable de navegador de
 * pistas (ADR-0025, decisiones 2 y 3: paridad estricta con lo que hoy viven
 * LibraryView y MetadataView).
 *
 * Congela: paginación offset/has_more de la fuente declarada (y su cambio en
 * caliente), la página fija de MetadataView, la búsqueda FTS con query
 * prefijado y loadMore, la aplicación del mapeo inyectado y la selección por
 * ids con la semántica exacta de MetadataView. Si un test exige algo que la
 * vista real no hace, gana la vista: corregir el test, no la vista.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { useTrackBrowser, toFtsPrefixQuery } from '@/composables/useTrackBrowser';
import type { BrowserSourcePage } from '@/composables/useTrackBrowser';

interface Item {
    id: number;
    name: string;
}

interface Row {
    id: number;
    label: string;
}

function items(...ids: number[]): Item[] {
    return ids.map(id => ({ id, name: `item-${id}` }));
}

function mapRow(item: Item, index: number): Row {
    return { id: item.id, label: `${item.name}#${index}` };
}

function page(ids: number[], total: number, offset: number, hasMore: boolean): BrowserSourcePage<Item> {
    return { tracks: items(...ids), total, offset, has_more: hasMore };
}

afterEach(() => {
    vi.restoreAllMocks();
});

describe('useTrackBrowser — fuente paginada (LibraryView: PAGE_SIZE 100)', () => {
    it('load pide la página 0 con el pageSize declarado, mapea y registra total/hasMore/offset', async () => {
        const source = vi.fn(async (offset: number, limit: number) => {
            expect([offset, limit]).toEqual([0, 100]);
            return page([1, 2, 3], 250, 0, true);
        });
        const browser = useTrackBrowser<Item, Row>({ source: { kind: 'paged', load: source }, mapRow, pageSize: 100 });

        expect(browser.isLoading.value).toBe(false);
        const loading = browser.load();
        expect(browser.isLoading.value).toBe(true); // se enciende antes del await
        await loading;

        expect(source).toHaveBeenCalledTimes(1);
        expect(browser.rows.value.map(r => r.id)).toEqual([1, 2, 3]);
        expect(browser.rows.value[0].label).toBe('item-1#0'); // mapeo con índice de página
        expect(browser.total.value).toBe(250);
        expect(browser.hasMore.value).toBe(true);
        expect(browser.currentOffset.value).toBe(3); // page.offset + tracks.length
        expect(browser.isLoading.value).toBe(false);
        expect(browser.loadError.value).toBeNull();
    });

    it('loadMore solo avanza con hasMore y fuera de una carga; añade al MISMO array (invariante del prefijo ordenado)', async () => {
        const source = vi.fn(async (offset: number) =>
            offset === 0 ? page([1, 2], 4, 0, true) : page([3, 4], 4, 2, false)
        );
        const browser = useTrackBrowser<Item, Row>({ source: { kind: 'paged', load: source }, mapRow, pageSize: 100 });
        await browser.load();

        // Guardas: sin hasMore o con carga en vuelo no se pide página.
        browser.hasMore.value = false;
        await browser.loadMore();
        expect(source).toHaveBeenCalledTimes(1);
        browser.hasMore.value = true;
        browser.isLoadingMore.value = true;
        await browser.loadMore();
        expect(source).toHaveBeenCalledTimes(1);
        browser.isLoadingMore.value = false;

        const sameArray = browser.rows.value;
        await browser.loadMore();

        expect(source).toHaveBeenLastCalledWith(2, 100);
        expect(browser.rows.value).toBe(sameArray); // push en sitio: la identidad prueba el append
        expect(browser.rows.value.map(r => r.id)).toEqual([1, 2, 3, 4]);
        expect(browser.pageAppendCount.value).toBe(1);
        expect(browser.currentOffset.value).toBe(4);
        expect(browser.hasMore.value).toBe(false);
        expect(browser.isLoadingMore.value).toBe(false);
        expect(browser.total.value).toBe(4); // loadMore no toca el total (paridad con loadLibrary/loadMore)
    });

    it('load reemplaza el array (identidad nueva): la vista debe reordenar completo tras una recarga', async () => {
        let generation = 0;
        const source = vi.fn(async () => {
            const id = ++generation;
            return page([id], 1, 0, false);
        });
        const browser = useTrackBrowser<Item, Row>({ source: { kind: 'paged', load: source }, mapRow, pageSize: 100 });
        await browser.load();
        const first = browser.rows.value;
        await browser.load();

        expect(browser.rows.value).not.toBe(first);
        expect(browser.rows.value.map(r => r.id)).toEqual([2]);
    });

    it('la fuente se invoca de nuevo en cada loadMore con el offset vigente: el cambio en caliente lo decide su closure', async () => {
        // La triple fuente de LibraryView lee los filtros en cada llamada. Aquí
        // se congela el contrato: el composable no cachea la fuente ni el
        // offset, así un closure que cambia de criterio cambia la fuente al
        // vuelo (quirk preservado: una página en curso puede venir del otro
        // criterio, como hoy).
        let useFavorites = false;
        const source = vi.fn(async (offset: number) =>
            useFavorites ? page([100 + offset], 300, offset, offset < 200) : page([offset + 1], 3, offset, offset < 2)
        );
        const browser = useTrackBrowser<Item, Row>({ source: { kind: 'paged', load: source }, mapRow, pageSize: 100 });
        await browser.load();

        useFavorites = true;
        await browser.loadMore();

        expect(browser.rows.value.map(r => r.id)).toEqual([1, 101]);
        expect(browser.hasMore.value).toBe(true);
    });

    it('fallo de load: loadError = String(error), NO limpia las filas y avisa a la vista vía onLoadError', async () => {
        const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
        const onLoadError = vi.fn();
        let fail = false;
        const source = vi.fn(async () => {
            if (fail) throw new Error('library table locked');
            return page([1, 2], 2, 0, false);
        });
        const browser = useTrackBrowser<Item, Row>({
            source: { kind: 'paged', load: source },
            mapRow,
            pageSize: 100,
            onLoadError,
        });
        await browser.load();
        const stale = browser.rows.value;

        fail = true;
        await browser.load();

        expect(browser.loadError.value).toBe('Error: library table locked');
        expect(browser.isLoading.value).toBe(false);
        expect(browser.rows.value).toBe(stale); // paridad: el fallo no vacía la lista
        expect(onLoadError).toHaveBeenCalledTimes(1);
        expect(consoleSpy).toHaveBeenCalled();
    });

    it('fallo de loadMore: solo consola; ni loadError ni el estado de paginación cambian', async () => {
        const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
        let fail = false;
        const source = vi.fn(async (offset: number) => {
            if (fail) throw new Error('boom');
            return offset === 0 ? page([1], 3, 0, true) : page([2], 3, 1, true);
        });
        const browser = useTrackBrowser<Item, Row>({ source: { kind: 'paged', load: source }, mapRow, pageSize: 100 });
        await browser.load();

        fail = true;
        await browser.loadMore();

        expect(browser.loadError.value).toBeNull();
        expect(browser.rows.value.map(r => r.id)).toEqual([1]);
        expect(browser.currentOffset.value).toBe(1);
        expect(browser.hasMore.value).toBe(true);
        expect(browser.isLoadingMore.value).toBe(false);
        expect(consoleSpy).toHaveBeenCalled();
    });
});

describe('useTrackBrowser — fuente de página fija (MetadataView: 500 sin has_more)', () => {
    it('load recibe el límite declarado, mapea y cierra sin hasMore; loadMore es un no-op', async () => {
        const source = vi.fn(async (limit: number) => {
            expect(limit).toBe(500);
            return { tracks: items(7, 8), total: 2 };
        });
        const browser = useTrackBrowser<Item, Row>({ source: { kind: 'fixed', load: source }, mapRow, pageSize: 500 });
        await browser.load();

        expect(browser.rows.value.map(r => r.id)).toEqual([7, 8]);
        expect(browser.total.value).toBe(2);
        expect(browser.hasMore.value).toBe(false);
        expect(browser.currentOffset.value).toBe(2);

        await browser.loadMore();
        expect(source).toHaveBeenCalledTimes(1);
        expect(browser.pageAppendCount.value).toBe(0);
    });

    it('el límite llega en cada load: la vista decide la fuente según filterType en el momento de cargar', async () => {
        let filterType = 'all';
        const source = vi.fn(async (limit: number) =>
            filterType === 'missing'
                ? { tracks: items(50), total: 1 } // estimación: total = crudo leído
                : { tracks: items(1, 2, 3), total: 900 }
        );
        const browser = useTrackBrowser<Item, Row>({ source: { kind: 'fixed', load: source }, mapRow, pageSize: 500 });

        await browser.load();
        expect(browser.total.value).toBe(900);

        filterType = 'missing';
        await browser.load();
        expect(browser.total.value).toBe(1);
        expect(browser.rows.value.map(r => r.id)).toEqual([50]);
    });
});

describe('useTrackBrowser — búsqueda FTS (LibraryView: paginador propio de 100)', () => {
    it('toFtsPrefixQuery prefija * por palabra y tira símbolos', () => {
        expect(toFtsPrefixQuery('  Neon   Nights! ')).toBe('Neon* Nights*');
        expect(toFtsPrefixQuery('AC/DC')).toBe('ACDC*');
        expect(toFtsPrefixQuery('!!!')).toBe('');
        expect(toFtsPrefixQuery('   ')).toBe('');
    });

    it('run normaliza la consulta, pagina desde 0 y mapea los resultados', async () => {
        const searchSource = vi.fn(async (ftsQuery: string, offset: number, limit: number) => {
            expect([ftsQuery, offset, limit]).toEqual(['Neon* Nights*', 0, 100]);
            return page([9], 40, 0, true);
        });
        const browser = useTrackBrowser<Item, Row>({
            source: { kind: 'paged', load: async () => page([], 0, 0, false) },
            searchSource: { load: searchSource },
            mapRow,
            pageSize: 100,
            searchPageSize: 100,
        });

        const run = browser.search!.run('Neon Nights!');
        expect(browser.search!.isSearching.value).toBe(true);
        await run;

        expect(browser.search!.rows.value.map(r => r.id)).toEqual([9]);
        expect(browser.search!.ftsQuery.value).toBe('Neon* Nights*');
        expect(browser.search!.total.value).toBe(40);
        expect(browser.search!.hasMore.value).toBe(true);
        expect(browser.search!.offset.value).toBe(1);
        expect(browser.search!.isSearching.value).toBe(false);
    });

    it('run con consulta vacía resetea todo el estado del buscador', async () => {
        const searchSource = vi.fn(async () => page([9], 40, 0, true));
        const browser = useTrackBrowser<Item, Row>({
            source: { kind: 'paged', load: async () => page([], 0, 0, false) },
            searchSource: { load: searchSource },
            mapRow,
            pageSize: 100,
        });
        await browser.search!.run('neon');
        expect(browser.search!.rows.value).toHaveLength(1);

        await browser.search!.run('   ');

        expect(browser.search!.rows.value).toEqual([]);
        expect(browser.search!.ftsQuery.value).toBe('');
        expect(browser.search!.offset.value).toBe(0);
        expect(browser.search!.total.value).toBe(0);
        expect(browser.search!.hasMore.value).toBe(false);
        expect(browser.search!.isSearching.value).toBe(false);
    });

    it('consulta solo de símbolos: vacía resultados y corta el giro sin tocar el resto (paridad)', async () => {
        const searchSource = vi.fn(async () => page([9], 40, 0, true));
        const browser = useTrackBrowser<Item, Row>({
            source: { kind: 'paged', load: async () => page([], 0, 0, false) },
            searchSource: { load: searchSource },
            mapRow,
            pageSize: 100,
        });
        await browser.search!.run('neon');
        searchSource.mockClear();

        await browser.search!.run('!!!');

        expect(searchSource).not.toHaveBeenCalled();
        expect(browser.search!.rows.value).toEqual([]);
        expect(browser.search!.ftsQuery.value).toBe('neon*');
        expect(browser.search!.total.value).toBe(40);
        expect(browser.search!.hasMore.value).toBe(true);
        expect(browser.search!.isSearching.value).toBe(false);
    });

    it('fallo de la búsqueda: mueren los resultados, el resto del estado queda como estaba (paridad)', async () => {
        const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
        const searchSource = vi.fn(async (q: string) => {
            if (q === 'neon*') return page([9], 40, 0, true);
            throw new Error('fts exploded');
        });
        const browser = useTrackBrowser<Item, Row>({
            source: { kind: 'paged', load: async () => page([], 0, 0, false) },
            searchSource: { load: searchSource },
            mapRow,
            pageSize: 100,
        });
        await browser.search!.run('neon');

        await browser.search!.run('badger');

        expect(browser.search!.rows.value).toEqual([]);
        expect(browser.search!.ftsQuery.value).toBe('badger*'); // se fijó antes del fetch
        expect(browser.search!.offset.value).toBe(0);
        expect(browser.search!.total.value).toBe(40); // del intento anterior
        expect(browser.search!.hasMore.value).toBe(true);
        expect(browser.search!.isSearching.value).toBe(false);
        expect(consoleSpy).toHaveBeenCalled();
    });

    it('loadMore del buscador: guarda por consulta/hasMore/isSearching, añade y avanza el offset', async () => {
        const searchSource = vi.fn(async (q: string, offset: number) =>
            offset === 0 ? page([1, 2], 3, 0, true) : page([3], 3, 2, false)
        );
        const browser = useTrackBrowser<Item, Row>({
            source: { kind: 'paged', load: async () => page([], 0, 0, false) },
            searchSource: { load: searchSource },
            mapRow,
            pageSize: 100,
            searchPageSize: 100,
        });

        // Sin consulta previa: no hay nada que paginar.
        await browser.search!.loadMore();
        expect(searchSource).not.toHaveBeenCalled();

        await browser.search!.run('neon');
        await browser.search!.loadMore();

        expect(searchSource).toHaveBeenLastCalledWith('neon*', 2, 100);
        expect(browser.search!.rows.value.map(r => r.id)).toEqual([1, 2, 3]);
        expect(browser.search!.offset.value).toBe(3);
        expect(browser.search!.hasMore.value).toBe(false);

        await browser.search!.loadMore();
        expect(searchSource).toHaveBeenCalledTimes(2); // sin hasMore, no vuelve a pedir
    });

    it('fallo del loadMore del buscador conserva los resultados ya listados', async () => {
        const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
        const searchSource = vi.fn(async (_q: string, offset: number) => {
            if (offset === 0) return page([1], 2, 0, true);
            throw new Error('page 2 failed');
        });
        const browser = useTrackBrowser<Item, Row>({
            source: { kind: 'paged', load: async () => page([], 0, 0, false) },
            searchSource: { load: searchSource },
            mapRow,
            pageSize: 100,
        });
        await browser.search!.run('neon');

        await browser.search!.loadMore();

        expect(browser.search!.rows.value.map(r => r.id)).toEqual([1]);
        expect(browser.search!.isSearching.value).toBe(false);
    });

    it('sin searchSource declarada, search es null y el resto del composable funciona (MetadataView)', async () => {
        const browser = useTrackBrowser<Item, Row>({
            source: { kind: 'fixed', load: async () => ({ tracks: items(1), total: 1 }) },
            mapRow,
            pageSize: 500,
        });
        await browser.load();
        expect(browser.search).toBeNull();
        expect(browser.rows.value.map(r => r.id)).toEqual([1]);
    });
});

describe('useTrackBrowser — selección por ids (semántica exacta de MetadataView)', () => {
    function makeBrowser(rowsIds: number[]) {
        return useTrackBrowser<Item, Row>({
            source: { kind: 'fixed', load: async () => ({ tracks: items(...rowsIds), total: rowsIds.length }) },
            mapRow,
            pageSize: 500,
        });
    }

    it('toggle añade y quita; selectedCount se deriva (nada de contador incremental)', async () => {
        const browser = makeBrowser([1, 2, 3]);
        await browser.load();
        const selection = browser.selection;

        expect(selection.selectedCount.value).toBe(0);
        selection.toggle(2);
        expect(selection.selectedIds.value).toEqual([2]);
        expect(selection.selectedCount.value).toBe(1);
        expect(selection.isSelected(2)).toBe(true);

        selection.toggle(2);
        expect(selection.selectedIds.value).toEqual([]);
        expect(selection.selectedCount.value).toBe(0);
    });

    it('selectOnly reemplaza con [id], salvo que ya esté: entonces no hace nada', async () => {
        const browser = makeBrowser([1, 2, 3]);
        await browser.load();
        const selection = browser.selection;

        selection.toggle(3); // [3]
        selection.selectOnly(1);
        expect(selection.selectedIds.value).toEqual([1]);

        selection.selectOnly(1);
        expect(selection.selectedIds.value).toEqual([1]); // "Already selected, do nothing"
        expect(selection.selectedCount.value).toBe(1);
    });

    it('selectAll REEMPLAZA con los ids de las filas actuales; clear vacía', async () => {
        const browser = makeBrowser([1, 2, 3]);
        await browser.load();
        const selection = browser.selection;

        selection.toggle(99); // id que ya no está en filas
        selection.selectAll(browser.rows.value);
        expect(selection.selectedIds.value).toEqual([1, 2, 3]);
        expect(selection.selectedCount.value).toBe(3);

        selection.clear();
        expect(selection.selectedIds.value).toEqual([]);
        expect(selection.selectedCount.value).toBe(0);
    });

    it('preserve deja solo los ids vivos, conservando el orden de selección', async () => {
        const browser = makeBrowser([1, 2, 3]);
        await browser.load();
        const selection = browser.selection;

        selection.toggle(5);
        selection.toggle(2);
        selection.toggle(9);
        expect(selection.selectedIds.value).toEqual([5, 2, 9]);

        selection.preserve([2, 9, 12]); // 5 murió en la recarga; 2 y 9 siguen vivos
        expect(selection.selectedIds.value).toEqual([2, 9]);
        expect(selection.selectedCount.value).toBe(2);
    });

    it('isSelected es la lectura de plantilla y responde a la selección vigente', async () => {
        const browser = makeBrowser([1, 2]);
        await browser.load();
        const selection = browser.selection;

        expect(selection.isSelected(1)).toBe(false);
        selection.toggle(1);
        selection.toggle(2);
        expect([1, 2].every(id => selection.isSelected(id))).toBe(true);
        selection.toggle(2);
        expect(selection.isSelected(2)).toBe(false);
    });
});

describe('useTrackBrowser — mapeo inyectado', () => {
    it('el mapper se aplica a filas paginadas, fijas y de búsqueda, con su índice de página', async () => {
        const searchSource = vi.fn(async () => page([30, 31], 2, 0, false));
        const browser = useTrackBrowser<Item, Row>({
            source: { kind: 'fixed', load: async () => ({ tracks: items(10, 11), total: 2 }) },
            searchSource: { load: searchSource },
            mapRow,
            pageSize: 500,
        });
        await browser.load();
        expect(browser.rows.value).toEqual([
            { id: 10, label: 'item-10#0' },
            { id: 11, label: 'item-11#1' },
        ]);

        await browser.search!.run('x');
        expect(browser.search!.rows.value).toEqual([
            { id: 30, label: 'item-30#0' },
            { id: 31, label: 'item-31#1' },
        ]);
    });
});

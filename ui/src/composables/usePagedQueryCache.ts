/**
 * Caché de páginas de consultas paginadas (L13, issue #110).
 *
 * Tres garantías, todas deterministas y sin reloj:
 *
 *  1. **Caché por clave**: una página ya leída se sirve sin volver a llamar a
 *     la fuente. La clave la declara la vista (`keyOf`) y debe describir la
 *     consulta sin el `offset`; el `offset` identifica la página.
 *  2. **Deduplicación en vuelo**: dos peticiones simultáneas de la misma
 *     clave y página comparten la misma promesa; la fuente se invoca una vez.
 *  3. **Token de secuencia**: `invalidate()` incrementa la generación y vacía
 *     la caché. Una respuesta que empezó antes de la invalidación no se
 *     guarda y llega con la generación vieja, para que la vista la descarte
 *     en vez de pintar datos anteriores al evento.
 *
 * No hay TTL: la invalidación es explícita (evento de sync, importación,
 * favorito, playlist, descarga, metadatos o cuarentena; ver
 * `libraryCacheInvalidation.ts`).
 */
import { ref } from 'vue'
import type { Ref } from 'vue'

/** Toda consulta paginada identifica su página por `offset`. */
export interface PagedQuery {
    offset: number
}

export interface PagedQueryCacheOptions<TQuery extends PagedQuery, TPage> {
    /** Clave estable de la consulta SIN el offset (texto, orden, filtros, límite). */
    keyOf: (query: TQuery) => string
    /** Ejecuta la consulta en la fuente real (IPC, HTTP, …). */
    fetchPage: (query: TQuery) => Promise<TPage>
    /**
     * Claves de consulta retenidas a la vez. Al superar el límite se descarta
     * la más antigua sin usar (LRU aproximada por orden de inserción); la
     * clave activa acaba de insertarse y nunca es la víctima.
     */
    maxKeys?: number
}

export interface CachedPage<TPage> {
    page: TPage
    /** Generación que produjo la página; comparar con `generation` antes de pintar. */
    generation: number
    /** `true` si se sirvió de caché sin llamar a la fuente. */
    fromCache: boolean
}

/** Contadores de instrumentación (tests y diagnóstico); no son reactivos. */
export interface PagedQueryCacheStats {
    requests: number
    cacheHits: number
    deduped: number
    discarded: number
}

export interface PagedQueryCache<TQuery extends PagedQuery, TPage> {
    load: (query: TQuery) => Promise<CachedPage<TPage>>
    /** Página ya cacheada, sin lanzar la fuente. */
    peek: (query: TQuery) => TPage | undefined
    invalidate: () => void
    generation: Ref<number>
    stats: PagedQueryCacheStats
}

/**
 * Clave canónica de una consulta paginada: JSON de la consulta sin `offset`.
 * Las vistas construyen la consulta con orden de claves estable; el `limit`
 * forma parte de la identidad porque dos tamaños de página no comparten
 * páginas.
 */
export function pageQueryKey<TQuery extends PagedQuery>(query: TQuery): string {
    const { offset: _offset, ...rest } = query as Record<string, unknown>
    return JSON.stringify(rest)
}

export function usePagedQueryCache<TQuery extends PagedQuery, TPage>(
    options: PagedQueryCacheOptions<TQuery, TPage>
): PagedQueryCache<TQuery, TPage> {
    const { keyOf, fetchPage, maxKeys = 8 } = options

    const generation = ref(0)
    const pages = new Map<string, Map<number, TPage>>()
    const inflight = new Map<string, Promise<CachedPage<TPage>>>()
    const stats: PagedQueryCacheStats = { requests: 0, cacheHits: 0, deduped: 0, discarded: 0 }

    function slotKey(key: string, offset: number): string {
        return `${key}\u0000${offset}`
    }

    function touch(key: string): Map<number, TPage> {
        // LRU aproximada: reinsertar mueve la clave al final del Map.
        const existing = pages.get(key)
        if (existing) {
            pages.delete(key)
            pages.set(key, existing)
            return existing
        }
        const bucket = new Map<number, TPage>()
        pages.set(key, bucket)
        if (pages.size > maxKeys) {
            const oldest = pages.keys().next().value
            if (oldest !== undefined) pages.delete(oldest)
        }
        return bucket
    }

    function peek(query: TQuery): TPage | undefined {
        const bucket = pages.get(keyOf(query))
        return bucket?.get(query.offset)
    }

    async function load(query: TQuery): Promise<CachedPage<TPage>> {
        const key = keyOf(query)
        const cached = pages.get(key)?.get(query.offset)
        if (cached !== undefined) {
            stats.cacheHits++
            touch(key)
            return { page: cached, generation: generation.value, fromCache: true }
        }

        const pendingKey = slotKey(key, query.offset)
        const pending = inflight.get(pendingKey)
        if (pending) {
            stats.deduped++
            return pending
        }

        const startedAt = generation.value
        stats.requests++
        let request: Promise<CachedPage<TPage>>
        request = fetchPage(query)
            .then((page) => {
                if (startedAt !== generation.value) {
                    // La invalidación llegó mientras la petición estaba en
                    // vuelo: no se guarda. La vista compara `generation`.
                    stats.discarded++
                    return { page, generation: startedAt, fromCache: false }
                }
                touch(key).set(query.offset, page)
                return { page, generation: startedAt, fromCache: false }
            })
            .finally(() => {
                if (inflight.get(pendingKey) === request) {
                    inflight.delete(pendingKey)
                }
            })

        inflight.set(pendingKey, request)
        return request
    }

    function invalidate(): void {
        generation.value++
        pages.clear()
        // Vaciar también las peticiones en vuelo: un `load` posterior al
        // evento debe pedir datos frescos, no engancharse a la respuesta
        // anterior al evento.
        inflight.clear()
    }

    return { load, peek, invalidate, generation, stats }
}

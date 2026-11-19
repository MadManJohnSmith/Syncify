/**
 * usePagedQueryCache.spec.ts — L13 (#110): caché de páginas por clave,
 * deduplicación de peticiones en vuelo y token de secuencia.
 *
 * Todo determinista: los resolvers se controlan a mano, sin red ni reloj.
 */
import { describe, it, expect, vi } from 'vitest'
import { pageQueryKey, usePagedQueryCache } from '@/composables/usePagedQueryCache'

interface Query {
    text: string | null
    offset: number
    limit: number
}

interface Page {
    items: number[]
    offset: number
}

function query(text: string | null, offset: number, limit = 100): Query {
    return { text, offset, limit }
}

function deferred<T>() {
    let resolve!: (value: T) => void
    let reject!: (error: unknown) => void
    const promise = new Promise<T>((res, rej) => {
        resolve = res
        reject = rej
    })
    return { promise, resolve, reject }
}

describe('pageQueryKey — identidad de la consulta sin offset', () => {
    it('el offset no forma parte de la clave; el límite sí', () => {
        expect(pageQueryKey(query('daft', 0))).toBe(pageQueryKey(query('daft', 500)))
        expect(pageQueryKey(query('daft', 0, 100))).not.toBe(pageQueryKey(query('daft', 0, 200)))
        expect(pageQueryKey(query('daft', 0))).not.toBe(pageQueryKey(query(null, 0)))
    })
})

describe('usePagedQueryCache', () => {
    it('sirve una página ya leída sin volver a llamar a la fuente', async () => {
        const fetchPage = vi.fn(async (q: Query) => ({ items: [q.offset], offset: q.offset }))
        const cache = usePagedQueryCache<Query, Page>({ keyOf: pageQueryKey, fetchPage })

        const first = await cache.load(query('a', 0))
        const second = await cache.load(query('a', 0))

        expect(fetchPage).toHaveBeenCalledTimes(1)
        expect(first.fromCache).toBe(false)
        expect(second.fromCache).toBe(true)
        expect(second.page).toEqual({ items: [0], offset: 0 })
        expect(cache.stats).toMatchObject({ requests: 1, cacheHits: 1 })
    })

    it('deduplica peticiones simultáneas de la misma clave y página', async () => {
        const pending = deferred<Page>()
        const fetchPage = vi.fn(() => pending.promise)
        const cache = usePagedQueryCache<Query, Page>({ keyOf: pageQueryKey, fetchPage })

        const first = cache.load(query('a', 0))
        const second = cache.load(query('a', 0))
        expect(fetchPage).toHaveBeenCalledTimes(1)
        expect(cache.stats.deduped).toBe(1)

        pending.resolve({ items: [1, 2], offset: 0 })
        await expect(first).resolves.toMatchObject({ page: { items: [1, 2] }, fromCache: false })
        await expect(second).resolves.toMatchObject({ page: { items: [1, 2] }, fromCache: false })
    })

    it('claves distintas no comparten páginas', async () => {
        const fetchPage = vi.fn(async (q: Query) => ({ items: [q.text === 'a' ? 1 : 2], offset: q.offset }))
        const cache = usePagedQueryCache<Query, Page>({ keyOf: pageQueryKey, fetchPage })

        const a = await cache.load(query('a', 0))
        const b = await cache.load(query('b', 0))

        expect(fetchPage).toHaveBeenCalledTimes(2)
        expect(a.page.items).toEqual([1])
        expect(b.page.items).toEqual([2])
    })

    it('invalidate descarta las respuestas en vuelo con la generación vieja', async () => {
        const firstRequest = deferred<Page>()
        const fetchPage = vi.fn()
            .mockImplementationOnce(() => firstRequest.promise)
            .mockResolvedValueOnce({ items: [9], offset: 0 })
        const cache = usePagedQueryCache<Query, Page>({ keyOf: pageQueryKey, fetchPage })

        const staleLoad = cache.load(query('a', 0))
        cache.invalidate()
        const freshLoad = cache.load(query('a', 0))

        firstRequest.resolve({ items: [1], offset: 0 })
        const stale = await staleLoad
        const fresh = await freshLoad

        expect(stale.generation).toBeLessThan(fresh.generation)
        expect(stale.generation).not.toBe(cache.generation.value)
        expect(fresh.page.items).toEqual([9])
        expect(cache.stats.discarded).toBe(1)
        // La respuesta vieja no quedó cacheada: pedir la página vuelve a la fuente
        // solo si no es la misma generación ya guardada.
        expect(cache.peek(query('a', 0))).toEqual({ items: [9], offset: 0 })
    })

    it('invalidate vacía la caché: la siguiente carga vuelve a la fuente', async () => {
        const fetchPage = vi.fn(async (q: Query) => ({ items: [q.offset], offset: q.offset }))
        const cache = usePagedQueryCache<Query, Page>({ keyOf: pageQueryKey, fetchPage })

        await cache.load(query('a', 0))
        cache.invalidate()
        expect(cache.peek(query('a', 0))).toBeUndefined()

        await cache.load(query('a', 0))
        expect(fetchPage).toHaveBeenCalledTimes(2)
    })

    it('retiene como máximo maxKeys claves (la activa nunca se descarta)', async () => {
        const fetchPage = vi.fn(async (q: Query) => ({ items: [q.text === 'a' ? 1 : 2], offset: q.offset }))
        const cache = usePagedQueryCache<Query, Page>({ keyOf: pageQueryKey, fetchPage, maxKeys: 2 })

        await cache.load(query('a', 0))
        await cache.load(query('b', 0))
        await cache.load(query('c', 0))
        expect(cache.peek(query('a', 0))).toBeUndefined()
        expect(cache.peek(query('c', 0))).toBeDefined()

        const hit = await cache.load(query('c', 0))
        expect(hit.fromCache).toBe(true)
    })
})

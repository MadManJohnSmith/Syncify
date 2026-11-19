/**
 * Señal de invalidación de las cachés de páginas de biblioteca (L13, #110).
 *
 * La caché no caduca por tiempo: cada mutación que cambia lo que devuelve
 * `query_library` avisa aquí. Los wrappers de la API (`toggle_favorite`,
 * `add_to_playlist`, `update_track_metadata`, cola de descargas, acciones por
 * lotes) llaman a `invalidateLibraryPages` DESPUÉS de que el backend confirme;
 * los eventos de la app (sync, importación, favoritos sincronizados, acciones
 * y descargas terminales) los traduce `useLibraryCacheInvalidation`.
 *
 * Es un emisor síncrono y sin Vue a propósito: la invalidación ocurre antes
 * de que el `await` del wrapper devuelva el control a la vista, así la
 * recarga que la vista dispare después siempre encuentra la caché vacía.
 */

export type LibraryCacheInvalidationReason =
    | 'sync'
    | 'import'
    | 'favorite'
    | 'playlist'
    | 'download'
    | 'metadata'
    | 'library-action'
    | 'quarantine'
    | 'manual'

type Listener = (reason: LibraryCacheInvalidationReason) => void

const listeners = new Set<Listener>()

/** Invalida todas las cachés de páginas de biblioteca montadas. */
export function invalidateLibraryPages(reason: LibraryCacheInvalidationReason = 'manual'): void {
    for (const listener of Array.from(listeners)) {
        try {
            listener(reason)
        } catch (error) {
            console.error('[library-cache] invalidación falló:', error)
        }
    }
}

/** Suscribe una caché a la señal. Devuelve la baja. */
export function onLibraryPagesInvalidated(listener: Listener): () => void {
    listeners.add(listener)
    return () => {
        listeners.delete(listener)
    }
}

/** Solo para tests: deja la señal sin suscriptores entre casos. */
export function resetLibraryPageInvalidationForTests(): void {
    listeners.clear()
}

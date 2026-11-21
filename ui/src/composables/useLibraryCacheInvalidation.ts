/**
 * Traduce los eventos de la app a invalidaciones de las cachés de páginas de
 * biblioteca (L13, #110). Matriz cubierta:
 *
 *  - sync: SYNC_COMPLETE y SYNC_RUN_FINISHED (#65)
 *  - importación: IMPORT_COMPLETE
 *  - favorito: FAVORITES_SYNC_COMPLETED (los toggles locales avisan desde el
 *    wrapper de la API, que conoce el resultado)
 *  - playlist: mutaciones locales desde `api/playlists.ts`
 *  - descarga: `DOWNLOAD_PROGRESS` solo cuando el estado es terminal; un
 *    evento de progreso cada pocos píxeles no invalida nada
 *  - metadatos: `update_track_metadata` y los enriquecimientos, desde el
 *    wrapper de la API
 *  - cuarentena: #72/L09 todavía no existe. Cuando exista debe emitir
 *    `library-updated` o llamar a `invalidateLibraryPages('quarantine')`; no
 *    se inventa un evento ahora.
 */
import { TauriEvents, useEventBus } from './useEventBus'
import { invalidateLibraryPages } from './libraryCacheInvalidation'

const INVALIDATION_EVENTS: Array<[string, Parameters<typeof invalidateLibraryPages>[0]]> = [
    [TauriEvents.LIBRARY_UPDATED, 'sync'],
    [TauriEvents.IMPORT_COMPLETE, 'import'],
    [TauriEvents.SYNC_COMPLETE, 'sync'],
    [TauriEvents.SYNC_RUN_FINISHED, 'sync'],
    [TauriEvents.FAVORITES_SYNC_COMPLETED, 'favorite'],
    [TauriEvents.LIBRARY_ACTION_FINISHED, 'library-action'],
]

const TERMINAL_DOWNLOAD_STATUSES = new Set([
    'complete',
    'completed',
    'failed',
    'cancelled',
    'canceled',
])

/**
 * Registra la matriz de eventos mientras viva el componente actual (el bus
 * limpia sus listeners al desmontar). Llamarlo desde una vista de biblioteca.
 */
export function useLibraryCacheInvalidation(): void {
    const eventBus = useEventBus()

    for (const [event, reason] of INVALIDATION_EVENTS) {
        void eventBus.on(event, () => {
            invalidateLibraryPages(reason)
        })
    }

    void eventBus.on(TauriEvents.DOWNLOAD_PROGRESS, (payload: { status?: unknown; terminal?: unknown }) => {
        // El evento se emite por tick de bytes: solo un final cambia la fila.
        const status = typeof payload?.status === 'string' ? payload.status : ''
        const terminal = payload?.terminal === true
        if (terminal || TERMINAL_DOWNLOAD_STATUSES.has(status)) {
            invalidateLibraryPages('download')
        }
    })
}

/**
 * FE-PERF-01 (#49): conjunto ACOTADO de vistas cacheadas por <KeepAlive>.
 *
 * Nunca global (criterio de aceptación de #49 y user story 8 de #34): retener
 * todas las vistas degradaría la memoria a medida que se navega. El límite de
 * retención es doble y estructural:
 *   - esta lista FIJA de nombres (el `include` de KeepAlive filtra por el
 *     `name` del componente, que cada vista incluida declara con
 *     `defineOptions` para que el contrato sea explícito y verificable);
 *   - `:max` en App.vue igual a la longitud de esta lista: aunque algo
 *     entrara por error en el include, el total retenido no puede crecer
 *     (KeepAlive descarta por LRU).
 *
 * Justificación del conjunto — los cuatro hubs del sidebar, revisados una y
 * otra vez dentro de una sesión:
 *   - DashboardView: su `onMounted` lanza `createLibrarySnapshot` + 11
 *     llamadas en `Promise.all` + `getVersion`; volver repetía las 13.
 *   - LibraryView: la vista con más estado (lista acumulada, filtros, scroll
 *     virtualizado); reconstruirla es lo más caro y perder el scroll es la
 *     regresión más perceptible.
 *   - DownloadsView: la cola de descargas se consulta en cada visita mientras
 *     hay actividad de descargas.
 *   - SettingsView: el chunk de vista más grande (~200 kB raw); revisitarla
 *     repetía su parseo entero.
 *
 * Excluidas a propósito:
 *   - AlbumDetailView / ArtistDetailView: KeepAlive cachea por nombre de
 *     componente, así que navegando de un álbum a otro les serviría la
 *     instancia del anterior sin remontar (los params cambian y la vista
 *     mostraría el álbum equivocado). No deben entrar nunca en el include.
 *   - MetadataView, LyricsView, MigrationView, AccountsView: visitas menos
 *     frecuentes; se excluyen para mantener pequeño el límite de retención
 *     (más instances = más DOM y estado retenidos).
 */
// El array no es `readonly` porque la prop `include` de KeepAlive tipa
// `MatchPattern` como `(string | RegExp)[]` mutable; el módulo es la única
// fuente del conjunto y los tests fijan su contenido.
export const KEEP_ALIVE_INCLUDE: string[] = [
  'DashboardView',
  'LibraryView',
  'DownloadsView',
  'SettingsView',
]

/** Techo duro de instancias retenidas (LRU dentro del include). */
export const KEEP_ALIVE_MAX: number = KEEP_ALIVE_INCLUDE.length

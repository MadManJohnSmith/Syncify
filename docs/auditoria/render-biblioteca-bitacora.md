# Bitácora de Auditoría y Ajustes — Carril render-biblioteca

- **Carril**: `render-biblioteca`
- **Fecha**: 2026-10-08
- **IDs asignados**: PR-01, PR-02, L-11, L-12
- **Ficheros intervenidos**:
  - `ui/src/views/LibraryView.vue`
  - `ui/src/__tests__/views/LibraryView.spec.ts`
  - `docs/auditoria/matriz-ajustes.md`

---

## 1. Qué se cambió (con rutas y líneas exactas)

### PR-02: Virtualización manual de la lista de canciones (~23.000 pistas)
- **Problema previo**: En `ui/src/views/LibraryView.vue`, la vista de lista iteraba `filteredTracks` íntegramente (`v-for="track in filteredTracks"`). Con `PAGE_SIZE=100` y concatenación en `loadMore()`, la acumulación de pistas en el DOM provocaba un colapso de rendimiento en Linux al superar los millares de filas.
- **Solución aplicada**: Se portó el patrón de virtualización manual probado en `DownloadsView.vue:549-564, 1768-1807` sin dependencias externas:
  - Contenedor con `contain: strict;` en el elemento de scroll (`ui/src/views/LibraryView.vue:555`).
  - Espaciador de altura total reactivo `totalVirtualHeight` (`ui/src/views/LibraryView.vue:557, 2337-2339`) calculado a `ROW_HEIGHT = 56px` por fila (`ui/src/views/LibraryView.vue:1578`) más padding al final de lista.
  - Ventana deslizante `visibleTracks` con `virtualStartIndex` y `virtualEndIndex` amortiguados por `OVERSCAN = 10` filas (`ui/src/views/LibraryView.vue:1579, 2341-2357`).
  - Posicionamiento absoluto y desplazamiento mediante `transform: translateY(${virtualOffsetY}px)` (`ui/src/views/LibraryView.vue:559`).
  - Coexistencia completa preservada: selección múltiple (reactividad por mutación del objeto `track.isSelected`), menú contextual (`openContextMenu`), doble clic para reproducir, atajos de teclado y deep-link scrolling (`focusTrackFromQuery`, que posiciona el scroll de la ventana virtual antes de ejecutar `scrollIntoView`, `ui/src/views/LibraryView.vue:2691-2696`).

### PR-01: Calibración del scroll y eliminación de reflow sincrónico forzado
- **Problema previo**: En `ui/src/views/LibraryView.vue:1466-1485` (anterior), `handleScroll` leía `target.scrollHeight`, `target.scrollTop` y `target.clientHeight` **antes** del corte de `SCROLL_THROTTLE_MS = 120ms`, forzando recálculos de layout/reflow en cada frame de evento de rueda o inercia.
- **Solución aplicada**:
  - Se reestructuró `handleScroll` (`ui/src/views/LibraryView.vue:1632-1678`).
  - La actualización de `libraryScrollTop` y `libraryViewportHeight` para la ventana virtual se realiza sin consultar propiedades pesadas de layout de árbol completo.
  - La lectura de `scrollHeight`, `scrollTop` y `clientHeight` para evaluar el umbral de carga infinita (`scrollBottom < 300`) fue trasladada **estrictamente al interior del corte throttled** (`checkAndDispatchLoadMore`), tanto en el disparo inmediato (si `elapsed >= SCROLL_THROTTLE_MS`) como en el temporizador diferido de cola (`scrollThrottleTimer`).

### L-11: Barra de acciones por lote como overlay flotante y retirada de "Check Availability"
- **Problema previo**: La barra de selección por lote se renderizaba como bloque inline `shrink-0` dentro del flujo flex vertical (`LibraryView.vue:222-244`), robando espacio útil al contenedor de la tabla y reduciendo el área visible de datos. Además, conservaba la acción obsoleta "Check Availability".
- **Solución aplicada**:
  - Se convirtió la barra en un overlay flotante con `fixed bottom-20 inset-x-8 max-w-xl mx-auto z-40` dentro de `<Transition name="slide-down">` (`ui/src/views/LibraryView.vue:299-317`).
  - Se eliminó el botón "Check Availability" del menú de selección por lote, dejando exclusivamente las acciones "Download", "Remove" y "Clear Selection".
  - Se añadió la animación de transición `.slide-down` en `<style scoped>` (`ui/src/views/LibraryView.vue:2796-2804`).

### L-12: Indicadores de descarga, concurrencia y reconciliación en la cabecera
- **Problema previo**: El encabezado de la biblioteca desaprovechaba el espacio libre junto al título y no exponía el estado de la cola de descargas ni el control de hilos de trabajo.
- **Solución aplicada**:
  - Se replicaron los componentes de telemetría de `DownloadsView.vue:13-127` en la cabecera de la biblioteca (`ui/src/views/LibraryView.vue:15-98`):
    - Píldoras de estado reactivas: `Active`, `Queued`, `Completed`, `Failed` y `Selected`.
    - Selector directo de hilos concurrentes (1, 2, 3, 4, 5, 6, 8, 10) que invoca `set_max_concurrency` por IPC Tauri (`ui/src/views/LibraryView.vue:69-91, 1600-1610`).
    - Botón de alternancia para el panel de reconciliación de cola (`showQueueDetails`).
  - Se implementó la tira desplegable de reconciliación (`ui/src/views/LibraryView.vue:100-149`) con desglose de seleccionadas, elegibles, excluidas preflight, pendientes, activas, completadas, fallidas y canceladas.
  - Se integró la consulta reactiva y cíclica en `fetchQueueStats` y la suscripción a eventos `TauriEvents.DOWNLOAD_PROGRESS` en el ciclo de vida (`ui/src/views/LibraryView.vue:1612-1623, 2715-2747`).
  - Se definió la animación `.animate-spin-slow` en `<style scoped>` (`ui/src/views/LibraryView.vue:2778-2788`).

---

## 2. Evidencia y Pruebas Ejecutadas

- **Comando ejecutado**:
  ```bash
  cd ui && npx vitest run src/__tests__/views/LibraryView.spec.ts
  ```
- **Resultado obtenido**:
  - `Test Files: 1 passed (1)`
  - `Tests: 38 passed (38)` (34 preexistentes + 4 nuevos dedicados para PR-01, PR-02, L-11 y L-12)
  - Cobertura de las 4 pruebas nuevas:
    1. `PR-02: renders virtual scroller with contain: strict, spacer height and window slicing`: Verifica que con 60 elementos el contenedor posea `contain: strict;`, la altura del spacer sea exacta (`3408px`), y el DOM contenga únicamente la ventana recortada (`< 60` filas).
    2. `PR-01: handleScroll updates virtual position and throttles loadMore calls`: Verifica que ráfagas de 10 eventos de scroll no disparen lecturas continuas ni llamadas descontroladas a `get_library`, respetando el corte de throttle.
    3. `L-11: batch selection bar is rendered as fixed overlay without Check Availability`: Verifica que al seleccionar pistas la barra sea `fixed bottom-20`, no tenga `shrink-0`, contenga `Download` y `Remove`, y no contenga `Check Availability`.
    4. `L-12: renders download telemetry pills, concurrency selector, and queue reconciliation details`: Verifica el renderizado de píldoras de telemetría (Active, Queued, Completed, Failed), el cambio de concurrencia mediante `set_max_concurrency` al pulsar los botones de hilos, y el despliegue del panel de reconciliación.

---

## 3. Qué queda abierto

- Ninguna tarea pendiente en el carril `render-biblioteca`. Todos los requisitos para PR-01, PR-02, L-11 y L-12 quedaron completamente implementados y validados.

---

## 4. Riesgos evaluados y mitigaciones

1. **Desalineación visual en vistas agrupadas o modo grid**:
   - *Riesgo*: El cálculo de virtualización (`ROW_HEIGHT = 56px`) aplica a la vista de tabla/lista estándar (`viewMode === 'list' && groupBy === 'none'`).
   - *Mitigación*: Las vistas `grid` y `groupedBy...` conservan sus contenedores específicos con scroll nativo y grupos expandibles independientes, sin interferencia del virtualizador de tabla.
2. **Navegación por deep-link a filas virtuales no renderizadas**:
   - *Riesgo*: `focusTrackFromQuery` buscando `data-track-id` podría fallar si la fila se encuentra fuera del rango `[virtualStartIndex, virtualEndIndex]`.
   - *Mitigación*: Se ajustó `focusTrackFromQuery` para pre-calcular el índice de la pista y posicionar `listContainerRef.scrollTop` en la coordenada calculada antes de invocar `scrollIntoView`.
3. **Consumo de IPC por actualización de estadísticas de cola**:
   - *Riesgo*: Sobrecargar el backend Tauri con llamadas periódicas a `get_queue_stats` y `get_worker_status`.
   - *Mitigación*: Intervalo espaciado (4 segundos) limpiado en `onUnmounted`, y recargas guiadas por eventos `DOWNLOAD_PROGRESS` en lugar de polling agresivo.

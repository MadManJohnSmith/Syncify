# Bitácora — Carril `render-metadata` (MT-02, MET-01, MET-02)

**Fecha:** 2026-10-08  
**Carril:** `render-metadata` (Ola 2)  
**IDs asignados:** MT-02, MET-01, MET-02  
**Ficheros poseídos y modificados:**  
- `ui/src/views/MetadataView.vue`
- `ui/src/__tests__/views/MetadataView.spec.ts`
- `docs/auditoria/matriz-ajustes.md` (registro de los 3 IDs previo a cambios según R1)

---

## 1. Verificación previa y diagnóstico contra el código

### MT-02: Ciclo de vida y pantalla en negro por timer/listener zombi
- **Diagnóstico:**
  En `MetadataView.vue:1551` (original), la llamada `onUnmounted(...)` se registraba dentro de la función asíncrona pasada a `onMounted(async () => { ... })`, situada tras múltiples `await` (`loadTracks()`, `loadMetadataStats()`, `loadScoreWeights()`, `import('@tauri-apps/api/event')`).
  En Vue 3, las llamadas a `onUnmounted()` ejecutadas fuera de la ejecución sincrónica inicial de `setup()` pierden la instancia activa del componente y son descartadas silenciosamente con warning de Vue en consola.
  Como consecuencia:
  1. La desuscripción de `unlistenBg` nunca se ejecutaba al navegar fuera de la vista.
  2. El temporizador `bgRefreshTimer = setTimeout(..., 3000)` continuaba activo tras el desmontaje.
  3. Los eventos emitidos por el backend (`BACKGROUND_ENRICHMENT_STATUS`) disparaban `loadTracks()` sobre un componente ya desmontado.
  4. Los temporizadores de auto-remoción de toasts (`setTimeout`) tampoco se cancelaban.
- **Relación con "Ajustes" vs "Metadatos":**
  El reporte en `Errores-Pruebas-2026-10-08.md:419` indicaba: *"MT-02 — La pantalla se queda en negro y deja de mostrar los ajustes. Ocurre cada ~20 segundos aproximadamente"*.
  Al auditar el código: la pestaña de Ajustes de Metadatos es `ui/src/views/settings/SettingsMetadata.vue` (`SettingsView.vue:70`), la cual no tiene ese ciclo de vida defectuoso ni listeners zombis. El temporizador zombi superviviente de `MetadataView.vue` que llamaba periódicamente a `loadTracks()` cada vez que llegaban eventos de enriquecimiento generaba conflictos de concurrencia e invalidaba el estado al navegar entre vistas.
- **Veredicto:** Hallazgo **REAL** en `MetadataView.vue`.

### MET-01: Parpadeo/refresco agresivo y falta de virtualización
- **Diagnóstico:**
  1. `loadTracks()` ponía incondicionalmente `isLoading.value = true` (`MetadataView.vue:1455` original).
  2. En el template, `<div v-if="isLoading" class="flex-1 flex items-center justify-center">` sustituía el listado completo por un spinner de carga centrado a pantalla completa. En cada refresco periódico por enriquecimiento, toda la lista de canciones desaparecía y se volvía a montar.
  3. El worker en segundo plano `src-tauri/src/enrichment_worker.rs:133-145` emite `background-enrichment-status` **por cada pista** individual procesada, no por lote. El debounce previo de 3 s provocaba continuos repintados ante ráfagas de enriquecimiento.
  4. La vista solicitaba `getLibrary(0, 500)` y montaba hasta 500 elementos `.track-row` de forma síncrona en el DOM (`v-for="track in filteredTracks"`), sin scroller virtual.
- **Veredicto:** Hallazgo **REAL** en `MetadataView.vue`.

### MET-02: Banner redundante de enriquecimiento en segundo plano
- **Diagnóstico:**
  En `MetadataView.vue:88-111` original existía un banner:
  `<div v-if="backgroundEnrichment || enrichProgress">` que mostraba el texto *"MusicBrainz Enrichment: <mensaje>"*.
  Ese proceso en segundo plano corresponde a la barra de estado global (`StatusBar.vue:8-33`, pestaña "Sync Details" / "Background Processes"), tal como estipulan BP-01 y BP-02. Mantenerlo en `MetadataView` duplicaba información e introducía ruido visual en la cabecera del selector de pistas.
- **Veredicto:** Hallazgo **REAL** en `MetadataView.vue`.

---

## 2. Cambios realizados

### A. Corrección del ciclo de vida y timers zombi (MT-02)
- En `ui/src/views/MetadataView.vue`:
  - Se añadió la guarda booleana síncrona `let isMounted = true` y el set `activeToastTimers = new Set<ReturnType<typeof setTimeout>>()`.
  - Se movió el registro de `onUnmounted` a nivel raíz del bloque `<script setup>` de forma síncrona.
  - Al desmontar:
    - Se marca `isMounted = false`.
    - Se cancela `bgRefreshTimer` si existe.
    - Se cancelan y desuscriben `unlistenBg` y `unlistenEnrichment`.
    - Se cancelan todos los temporizadores pendientes en `activeToastTimers`.
  - En todas las funciones asíncronas (`loadTracks`, `loadMetadataStats`, `loadScoreWeights`, listeners de eventos), se verifica `if (!isMounted) return` antes y después de cada punto de suspensión `await`.

### B. Distinción entre carga inicial y refresco silencioso, consolidación de eventos y virtualización (MET-01)
- En `ui/src/views/MetadataView.vue`:
  - Se añadió la bandera reactiva `isRefreshing` y la variable de exclusión mutua `isLoadingTracks` para evitar peticiones concurrentes de `loadTracks`.
  - `loadTracks(isRefresh = false)`:
    - Si `isRefresh && tracks.value.length > 0`, se activa `isRefreshing.value = true` y se muestra un pequeño icono giratorio en la caja de búsqueda (`<span v-if="isRefreshing" class="material-symbols-outlined ... animate-spin">sync</span>`). No se desmonta el listado ni se activa el overlay a pantalla completa.
    - El overlay completo `<div v-if="isLoading && tracks.length === 0">` solo se muestra durante la primera carga cuando no hay datos en memoria.
  - Consolidación por lotes (`scheduleBatchRefresh`):
    - Si el backend emite `status === 'waiting'` (indicando cola vacía / lote terminado emitido en `enrichment_worker.rs:186-197`), se programa un refresco rápido a 500 ms.
    - Si llegan eventos por pista individual con `status === 'completed'`, se consolida la actualización espaciándola a 15 s para evitar refrescos continuos durante ráfagas de enriquecimiento.
  - **Virtualización de listas** (siguiendo el patrón de `DownloadsView.vue:549-564` y `:1768-1807`):
    - Constantes `ROW_HEIGHT = 64` px y `OVERSCAN = 5`.
    - Contenedor con `ref="trackContainerRef"` y `@scroll="onTrackScroll"`.
    - Computadas reactivas: `totalVirtualHeight`, `virtualStartIndex`, `virtualEndIndex`, `virtualOffsetY`, y `visibleTracks`.
    - En el template, se sustituyó el bucle directo `v-for="track in filteredTracks"` por el contenedor virtualizado con spacer `height: totalVirtualHeight` y ventana con `transform: translateY(virtualOffsetY)`.
    - Se añadió scroll automático a la posición de la pista (`scrollToTrack`) al buscar o saltar vía parámetro de URL `trackId`.

### C. Retirada de la píldora de enriquecimiento de fondo y preservación de métricas de completitud (MET-02)
- En `ui/src/views/MetadataView.vue`:
  - Se eliminó el banner de estado de enriquecimiento en segundo plano (`v-if="backgroundEnrichment || enrichProgress"` en el encabezado izquierdo).
  - Se preservó intacta la tira de completitud de biblioteca (`metadataStats`: total pistas, % ISRC, % MBID, % carátulas, % género, % año, completitud media; `MetadataView.vue:113-124`).
  - Para la acción manual de auto-reparación iniciada explícitamente por el usuario ("Fix Selected"), la barra de progreso se reubicó de manera compacta dentro de la tarjeta de la herramienta MusicBrainz en el panel desplegable de Auto-Fix.

### D. Pruebas unitarias actualizadas y ampliadas (R2)
- En `ui/src/__tests__/views/MetadataView.spec.ts`:
  - Se adaptó la prueba existente que antes exigía que `background-enrichment-status` mostrara un banner en la vista, verificando ahora que dicho banner no se renderiza en `MetadataView` conforme a MET-02 (R2 cumplida y documentada).
  - Se añadieron 3 nuevas pruebas unitarias:
    1. `MET-01: does not show full-screen loader on background refresh when tracks are loaded`.
    2. `MT-02: unmounting cleans up without throwing or firing on dead component`.
    3. `MET-01 / PR-03: virtualizes track rows for large lists` (verifica que con 60 pistas solo se renderiza en el DOM una ventana acotada de filas).

---

## 3. Evidencia de verificación ejecutada

Comando de pruebas unitarias ejecutado:
```bash
npm --prefix ui test -- src/__tests__/views/MetadataView.spec.ts
```
Resultado:
- **15 tests ejecutados, 15 passed (100% exitosos)** en 2.03s.

Comando de verificación de tipos TypeScript / Vue:
```bash
npx --prefix ui vue-tsc -p ui/tsconfig.json --noEmit
```
Resultado:
- **0 errores de compilación de tipos.**

---

## 4. Lo que queda abierto

- **PR-03 en `LyricsView.vue` y `LibraryView.vue`:**
  Este carril solo cubre `MetadataView.vue`. La virtualización de listas en `LyricsView.vue` correspondió al carril `render-letras` y en `LibraryView.vue` al carril de la biblioteca.
- **Enriquecimiento en lote en backend (`src-tauri`):**
  El backend emite eventos individuales por cada pista (`enrichment_worker.rs:133-145`). El frontend ahora los amortigua y consolida mediante `status === 'waiting'`, pero una futura optimización del backend podría emitir eventos con contadores agrupados para reducir el tráfico IPC.

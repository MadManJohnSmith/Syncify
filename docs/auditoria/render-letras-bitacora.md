# Bitácora — carril `render-letras` (PR-03)

**Fecha:** 2026-10-08  
**Carril:** `render-letras` (Ola 2 — render)  
**ID asignado:** PR-03  
**Fichero poseído y modificado:** `ui/src/views/LyricsView.vue` (únicamente la sección de renderizado de la lista izquierda de canciones)  
**Ficheros no tocados:** `ui/src/views/MetadataView.vue` (reservado para el carril `2-metadata`), pruebas y componentes de otros dominios.

---

## 1. Verificación previa de PR-03

- **Diagnóstico del problema:**
  En `LyricsView.vue:1525-1530`, la carga inicial invoca `libraryApi.getLibrary(0, 500)`, guardando hasta 500 canciones en `tracks.value`. En el template original (`:183-193`), el listado utilizaba un bucle directo `v-for="track in filteredTracks"` sobre un contenedor con scroll ordinario (`class="flex-1 overflow-y-auto custom-scrollbar"`).
- **Impacto:**
  Con 500 elementos cargados, se instanciaban y mantenían en el DOM 500 nodos de fila completos de forma simultánea, incluyendo botones, checkboxes, imágenes/portadas y etiquetas de estado de letra, provocando sobrecarga de memoria y caídas de fluidez al desplazarse en la lista.
- **Veredicto:**
  **PR-03 es un hallazgo REAL y verificado** en `ui/src/views/LyricsView.vue`.

---

## 2. Cambios realizados

Siguiendo estrictamente el patrón de virtualización manual existente en `ui/src/views/DownloadsView.vue:549-564` y `:1768-1807`:
1. **Contenedor virtualizado (`ui/src/views/LyricsView.vue:180-186`):**
   - Se añadió `ref="trackListRef"` y listener reactivo `@scroll="onTrackListScroll"`.
   - Se aplicó la clase `flex-1 min-h-0 overflow-y-auto custom-scrollbar relative` con `style="contain: strict;"` para aislar el reflow y layout del resto del panel.
2. **Spacer y ventana flotante (`ui/src/views/LyricsView.vue:188-190`):**
   - **Spacer virtual:** `height: totalTrackHeight + 'px'`, manteniendo la barra de scroll y el rango de desplazamiento natural de todas las pistas coincidentes.
   - **Ventana con desplazamiento absoluto:** `transform: translateY(${trackOffsetY}px)` posicionado en `top: 0, left: 0, right: 0`.
   - **Filas renderizadas:** iteración restringida a `v-for="track in visibleTracks"`, con altura fija `TRACK_ROW_HEIGHT` (64px: portada de 40px + padding vertical `py-3` de 24px).
3. **Cálculo de ventana reactiva y overscan (`ui/src/views/LyricsView.vue:1347-1389`):**
   - Constantes: `TRACK_ROW_HEIGHT = 64`, `TRACK_OVERSCAN = 10`.
   - `totalTrackHeight`: `filteredTracks.value.length * TRACK_ROW_HEIGHT`.
   - `trackStartIndex`: `Math.max(0, Math.floor(trackScrollTop / TRACK_ROW_HEIGHT) - TRACK_OVERSCAN)`.
   - `trackEndIndex`: `Math.min(filteredTracks.value.length, Math.ceil((trackScrollTop + trackViewportHeight) / TRACK_ROW_HEIGHT) + TRACK_OVERSCAN)`.
   - `trackOffsetY`: `trackStartIndex * TRACK_ROW_HEIGHT`.
   - `visibleTracks`: slice de `filteredTracks` entre `trackStartIndex` y `trackEndIndex`.
4. **Manejo de dimensionamiento y cambios de filtro:**
   - Soporte para redimensionamiento del viewport mediante `ResizeObserver` sobre `trackListRef` (con fallback de 520px).
   - `watch([searchQuery, filterType])`: reinicia `scrollTop` a 0 al filtrar o buscar.
   - `watch(filteredTracks)`: acota `scrollTop` si la lista filtrada se reduce y el scroll previo quedaba por encima del nuevo límite superior.

---

## 3. Observaciones de otros dominios (no modificados para evitar colisiones)

En cumplimiento de las reglas del carril, **no se tocó el comportamiento funcional de las letras**:
- **LY-01:** En `ui/src/views/LyricsView.vue:413` se observan marcas de tiempo en el contenedor de letras sincronizadas (`<span class="timestamp ...">{{ line.label }}</span>`) que se muestran con independencia del modo de escucha. Corresponde al carril de letras/reproductor.
- **LY-04:** En `ui/src/views/LyricsView.vue:67-76`, el botón de barrido de portadas animadas (`sweepAnimatedCovers`) no comprueba la conexión de Apple Music de forma preventiva. Corresponde al carril respectivo.
- **PR-03 en MetadataView:** La virtualización de `ui/src/views/MetadataView.vue:146` corresponde a `2-metadata` y no se ha tocado aquí.

---

## 4. Evidencia y comprobaciones ejecutadas

1. **Compilación de TypeScript y empaquetado de producción:**
   - Comando: `npm --prefix ui run build`
   - Resultado: Salida exitosa con `vue-tsc --noEmit` y `vite build` generando todos los chunks de producción sin errores tipográficos ni de sintaxis (`dist/assets/LyricsView-*.css` y bundles generados).
2. **Suite de pruebas unitarias de LyricsView:**
   - Comando: `npm --prefix ui run test:run -- src/__tests__/views/LyricsView.spec.ts src/__tests__/views/LyricsViewS202.spec.ts --silent`
   - Resultado: 2 archivos pasados, 16 pruebas superadas (100% éxito).
3. **Comprobación de destino Windows (`cargo check --target x86_64-pc-windows-msvc`):**
   - Ejecutado según regla R4 para validar el entorno global. No se modificó ningún archivo en Rust en este carril.
4. **Inspección de diferencias:**
   - Comando: `git diff -- ui/src/views/LyricsView.vue`
   - Se verificó que solo se añadieron las reactividades y elementos de virtualización requeridos, preservando intactos todos los eventos `@click="selectTrack"`, checkboxes de selección múltiple, enlaces de portada y estado de letras.

---

## 5. Qué queda abierto

- La virtualización análoga de `MetadataView.vue` (también mencionada bajo el ID PR-03 en `docs/auditoria/matriz-ajustes.md:18`) queda para el carril propietario `2-metadata`.

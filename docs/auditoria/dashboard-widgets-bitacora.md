# Bitácora — dashboard-widgets (Carril 2-dashboard)

**Fecha:** 2026-10-08  
**Carril:** `2-dashboard` (`dashboard-widgets`)  
**IDs asignados:** D-01, D-02, D-03, D-04, D-05, D-06, D-07  
**Ficheros modificados:**
- `ui/src/views/DashboardView.vue`
- `ui/src/__tests__/views/DashboardNoScroll.spec.ts`
- `ui/src/__tests__/views/DashboardView.spec.ts`
- `docs/auditoria/matriz-ajustes.md`

---

## 1. Verificación previa y análisis de evidencias

Antes de aplicar cualquier modificación, se inspeccionó el código fuente y las referencias cruzadas del plan v2 (`docs/PLAN-Correccion-168.md`) y la auditoría (`docs/Errores-Pruebas-2026-10-08.md`):

1. **D-01 (System Diagnostics):** `DashboardView.vue:241-264` montaba un widget que comprobaba binarios locales (`FFmpeg` y `Chromaprint`) mediante `toolsApi.checkFfmpeg()` y `toolsApi.checkFingerprint()` (`DashboardView.vue:579-594`). Dado que el instalador empaqueta ambos binarios, esta información es redundante e irrelevante para el usuario final.
2. **D-02 (Library Growth):** Se verificó que los datos **sí existen** en la base de datos: la tabla `library_snapshots` (`src-tauri/migrations/0015_historical_snapshots.sql:4`) y el comando `create_library_snapshot` (`src-tauri/src/commands/dashboard.rs:36-127`) persisten registros reales. Si no hay histórico acumulado, `DashboardView.vue:665-669` deriva un punto `'Current'` con las descargas y el total de la biblioteca. El motivo por el cual el widget no mostraba nada era un **fallo de maquetación CSS Flexbox**: el contenedor de barras (`DashboardView.vue:87-95`) utilizaba `items-end` sin altura definida (`height: auto`). En la especificación CSS, un elemento con `h-full` cuyo padre no tiene altura explícita ni estiramiento evalúa a `0px`. Las barras hijas con porcentaje (`:style="{ height: entry.total + '%' }"`) se calculaban sobre 0px, colapsando su altura a 0 píxeles visibles.
3. **D-03 (Storage Use) y D-04 (Lyrics Coverage):** En la grilla `xl` de 12 columnas (`DashboardView.vue:40`), `storage-usage` (`:117`) y `lyrics-coverage` (`:303`) ocupaban `xl:col-span-3` cada una. Esto era desproporcionado respecto a la información que muestran (un total de almacenamiento con desglose breve y una barra de porcentaje con dos números respectivamente).
4. **D-05 (Sources mal formateado):** La evidencia en `DashboardView.vue:501-502` confirmaba que el código realizaba una capitalización ingenua (`s.name.charAt(0).toUpperCase() + s.name.slice(1)`), dejando el identificador de base de datos `'apple_music'` como `'Apple_music'`. En lugar de parches locales, el repositorio ya cuenta con una función centralizada probada en `ui/src/composables/useGlobalTasks.ts:38-50` (`formatServiceName`) que mapea canónicamente `apple_music` a `'Apple Music'`.
5. **D-06 (Barrido del dashboard):** Varios contadores carecían de separadores de miles (`.toLocaleString()`), Sources no ofrecía un estado vacío si no había servicios conectados (`stats.services.length === 0`), y el cálculo del ancho de calidad de audio no tenía salvaguarda ante `totalTracks === 0` (pudiendo generar `NaN%`).
6. **D-07 (Solapamiento en pantallas pequeñas):** En `library-overview` (`DashboardView.vue:41-74`), la rejilla de 6 cajas métricas (`grid-cols-3 content-start`) empujaba verticalmente y en anchos estrechos montaba sus textos encima de la línea divisoria del pie (`:70-73`). El pie carecía de `mt-auto` y las métricas carecían de tipografías responsivas y truncamiento defensivo.

---

## 2. Ajustes realizados

### D-01: Eliminación de System Diagnostics
- **Código:** Se retiró el bloque `<div class="stat-card system-diagnostics ...">` de `DashboardView.vue:241-264`.
- **Script:** Se eliminaron las referencias reactivas `ffmpegStatus` y `fpcalcStatus`, la función `fetchSystemDiagnostics()`, su invocación en `fetchData()`, y el import no utilizado de `toolsApi`.
- **Regla R2:** En `ui/src/__tests__/views/DashboardNoScroll.spec.ts:98,105-126`, se actualizó la aserción de cantidad total de paneles visibles de 13 a 12 y se retiró `'system-diagnostics'` de la lista esperada de paneles. La prueba no fue eliminada, sino actualizada para reflejar el cambio intencional de diseño.

### D-02: Corrección del widget Library Growth
- En `DashboardView.vue`, se refactorizó la jerarquía flexbox del gráfico:
  - El contenedor principal utiliza `items-stretch` con espaciado vertical.
  - La columna de barras cuenta con `h-full min-h-[64px]` y cada barra posee un riel con `w-full flex-1 min-h-[48px] max-h-[140px] flex items-end justify-center gap-1`.
  - El ancho de las barras se fijó en `w-2.5 sm:w-3.5` con esquinas redondeadas (`rounded-t`).
  - Se añadió `Math.max(entry.total, 6) + '%'` y `Math.max(entry.downloaded, 6) + '%'` cuando el valor es mayor a 0, garantizando que incluso porcentajes mínimos tengan representación visible de al menos 4–6 px en pantalla.
  - Se añadieron tooltips descriptivos (`:title`) con los porcentajes reales.

### D-03: Reducción proporcional de Storage Use
- Se redujo el ancho en grilla `xl` de `xl:col-span-3` a `xl:col-span-2`.
- Se compactó la barra de formatos (`h-1.5`) y la lista de formatos con `overflow-y-auto custom-scrollbar`.
- Se añadió `truncate` y tooltip (`:title`) al texto inferior de espacio libre y tamaño por pista para evitar desbordes.

### D-04: Reducción proporcional de Lyrics Coverage
- Se redujo el ancho en grilla `xl` de `xl:col-span-3` a `xl:col-span-2`.
- Se optimizó la disposición vertical mediante `flex flex-col justify-between overflow-hidden`.
- Se envolvieron las etiquetas de synced / missing con `flex-wrap` y se compactó el botón de recuperación de letras faltantes (`Fetch Missing Lyrics`).

### D-05: Formateador centralizado para nombres de servicios
- Se importó `formatServiceName` desde `@/composables/useGlobalTasks` en `DashboardView.vue`.
- Se aplicó en la propiedad computada `stats.services`:
  ```typescript
  services: serviceStatuses.value.map((s, i) => ({
    name: formatServiceName(s.name),
    percent: s.library_count > 0 ? Math.round((s.library_count / (lib?.total_tracks || 1)) * 100) : 0,
    color: ['bg-blue-500', 'bg-green-500', 'bg-gray-900', 'bg-gray-400'][i] || 'bg-gray-400'
  }))
  ```
- Se aplicó igualmente en `exportReport()` tanto en el objeto JSON exportado como en el texto copiado al portapapeles.
- Convierte de forma uniforme `'apple_music'` en `'Apple Music'`, `'spotify'` en `'Spotify'`, etc.

### D-06: Barrido integral de formato en el Dashboard
- Formateo numérico consistente: se aplicó `.toLocaleString()` a `totalAlbums`, `totalArtists`, `totalPlaylists`, `activeDownloads`, `queueStats.queued`, `queueStats.downloading`, `queueStats.completed`, `queueStats.failed`, `queueStats.total`, `lyricsStats.with_lyrics`, `lyricsStats.total_tracks`, `lyricsStats.synced_lyrics`, `lyricsStats.missing_lyrics`, `qualityData[].count`, `topArtists[].tracks`, `topGenres[].tracks` y `duplicateStats`.
- Sources: se agregó un bloque de estado vacío explícito (`<div v-else class="... text-gray-500 italic text-xs">No sources connected</div>`) cuando no hay cuentas ni servicios configurados.
- Audio Quality: protección contra división por 0 (`stats.totalTracks > 0 ? ... : 0`).

### D-07: Prevención de desbordes y solapamientos en pantallas estrechas
- Rebalanceo general de la grilla de 12 columnas (3 filas exactas en `xl`):
  - **Fila 1 (12 cols):** `library-overview` (5 cols), `growth-chart` (5 cols), `storage-usage` (2 cols).
  - **Fila 2 (12 cols):** `quality-distribution` (3 cols), `service-distribution` (3 cols), `queue-stats` (2 cols), `recent-activity` (4 cols, absorbiendo las 2 columnas liberadas por System Diagnostics).
  - **Fila 3 (12 cols):** `metadata-quality` (3 cols), `lyrics-coverage` (2 cols), `top-artists` (2 cols), `top-genres` (2 cols), `duplicates` (3 cols).
- En `library-overview`:
  - Se fijó `mt-auto` en el pie de tarjeta (`.border-t`) para anclarlo en la parte inferior e impedir que el contenido superior se monte sobre la línea divisoria.
  - La cuadrícula de 6 métricas utiliza `grid-cols-3 gap-1.5 sm:gap-2 content-start overflow-y-auto custom-scrollbar`.
  - Tamaños tipográficos adaptativos (`text-base sm:text-lg` para cifras, `text-[10px] sm:text-[11px]` para etiquetas) con `truncate` y atributos `title`.
  - El pie utiliza `flex items-center justify-between gap-2` con `truncate` en ambas secciones.

---

## 3. Pruebas y verificaciones ejecutadas

1. **Pruebas unitarias de Vitest:**
   Comando ejecutado:
   ```bash
   npm --prefix ui run test:run -- src/__tests__/views/Dashboard
   ```
   Resultado:
   - `src/__tests__/views/DashboardNoScroll.spec.ts`: 6/6 pruebas pasadas.
   - `src/__tests__/views/DashboardView.spec.ts`: 10/10 pruebas pasadas.
   - Total: **16/16 pruebas exitosas (0 fallos)**.

2. **Verificación de tipos TypeScript:**
   Comando ejecutado:
   ```bash
   cd ui && npx vue-tsc --noEmit
   ```
   Resultado:
   - 0 errores de compilación de tipos en TypeScript.

---

## 4. Elementos que quedan abiertos

- **Ninguno dentro del alcance del carril `2-dashboard`:** Los 7 IDs (D-01 a D-07) han sido completados, verificados y cubiertos con pruebas unitarias.

---

## 5. Riesgos analizados

- **Mantenimiento de la grilla de una pantalla:** La grilla en pantallas `xl` suma estrictamente 12 columnas por fila (5+5+2, 3+3+2+4, 3+2+2+2+3). Cualquier incorporación futura de widgets debe reajustar los spans para respetar la regla R12 (diseño en una sola pantalla sin scroll en monitores grandes).
- **Dependencia de `library_snapshots`:** Si SQLite falla al crear el snapshot diario, `DashboardView.vue` mantiene el fallback honesto basado en las estadísticas globales de la biblioteca (`Current`), evitando mostrar gráficos vacíos o ficticios.

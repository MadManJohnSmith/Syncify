# Bitácora de Auditoría y Corrección — Reproductor Completo (Ola 7)

**Carril:** `7-reproductor`  
**Subagente:** `reproductor-completo`  
**Fecha:** 2026-10-09  
**IDs asignados:** PL-04, PL-05, PL-08, PL-09, PL-10  

---

## 1. Diagnóstico y Verificación de los Hallazgos

Se verificó el estado previo del código fuente para cada ID antes de aplicar modificaciones:

- **PL-08:** `ui/src/components/StatusBar.vue:7-260` tenía los elementos de estado agrupados a la izquierda (`sync-status`, `storage-status`, `network-status`, `settings-link`), sin ningún elemento en el centro. La reproducción activa solo se mostraba en una barra flotante superpuesta de 64 px (`NowPlayingBar.vue:7-8`) a `bottom-8`.
- **PL-09:** Las carátulas de las pistas en curso (`NowPlayingBar.vue:22-27` y `StatusBar.vue`) no tenían ninguna señal de hover que indicara interactividad ni soporte de click para expandir la vista a un reproductor grande.
- **PL-10:** En `ui/src/App.vue:152-158` y `App.vue:481-484` existía el botón de navegación "Now Playing" en el drawer lateral emitiendo el evento global `CustomEvent('syncify:open-fullscreen-player')`, pero ningún componente escuchaba dicho evento (estaba documentado como enganche inerte a la espera de la Ola 7).
- **PL-04:** La aplicación carecía de una vista grande de "reproduciendo ahora" que permitiera apreciar:
  - Letras sincronizadas en tiempo real (LRC con timestamps) o letras en texto plano.
  - Portadas animadas generadas/descargadas por los pipelines (`cover.webp`, `cover.animated.webp`).
  - Portada estática en alta definición (`cover.jpg` / URL remota original).
  - Metadatos enriquecidos de la pista (formato, bit depth, sample rate, calidad Hi-Res, ISRC, MusicBrainz ID, BPM, tonalidad, género, ruta de almacenamiento).
- **PL-05:** Los álbumes descargados con digital booklets (`booklet.pdf` en servicios como Qobuz) no contaban con ningún visualizador ni acceso en la interfaz de usuario.

---

## 2. Ajustes Realizados

Conforme a la regla **R1**, se añadieron previamente las filas correspondientes en `docs/auditoria/matriz-ajustes.md` antes de modificar el código.

### A. Composable `usePlayer.ts` (`ui/src/composables/usePlayer.ts`)
- Se introdujo el estado reactivo `isFullscreenOpen = ref(false)` y los métodos `openFullscreen()`, `closeFullscreen()`, `toggleFullscreen()`.
- Se registró el listener global en `window` para el evento `syncify:open-fullscreen-player` emitido desde el menú lateral (`App.vue:483`).
- Se expuso la API en el retorno del composable de manera retrocompatible y tolerante a mocks parciales en pruebas previas.

### B. Barra de Estado `StatusBar.vue` (`ui/src/components/StatusBar.vue`)
- **PL-08:** Se reubicó la información de reproducción en el **centro** de la barra inferior con `class="status-center-player flex-1 flex items-center justify-center gap-2.5 min-w-0 mx-2 text-xs"` (`data-testid="status-bar-player"`), visible cuando `current` tiene una pista activa.
- **PL-09:** Se incorporó la carátula en miniatura con señal visual al hacer hover (`open_in_full` en overlay, cursor pointer y tooltip/aria-label descriptivo) y click para abrir el reproductor completo (`data-testid="status-bar-track-cover"`).
- Se añadieron controles de transporte rápido (prev, toggle play/pause, next), tiempo transcurrido/total y botón dedicado para maximizar a pantalla completa (`data-testid="status-bar-expand-btn"`).
- Se preservó el espaciado y alineación hacia la derecha de Settings (`settings-link ml-auto`), manteniendo intacta la regla PL-03 y las pruebas de `ShellCascaron.spec.ts`.
- Se montó el componente global `<FullscreenPlayer />` como parte del shell de la aplicación.

### C. Barra Flotante `NowPlayingBar.vue` (`ui/src/components/NowPlayingBar.vue`)
- **PL-09:** Se dotó a la carátula de la pista (`TrackCover`) de interactividad: cursor pointer, señal visual al hover (`open_in_full` con opacidad suave al grupo), y click para abrir el reproductor de pantalla completa (`data-testid="player-cover-expand"`).
- Se añadió un botón explícito de pantalla completa en la barra de transporte (`data-testid="player-fullscreen-btn"`).

### D. Componente Nuevo `FullscreenPlayer.vue` (`ui/src/components/player/FullscreenPlayer.vue`)
- **PL-04 & PL-10:** Vista de pantalla completa inmersiva con fondo desenfocado y auras cromáticas (`data-testid="fullscreen-player"`).
  - **Letras Sincronizadas (PL-04):** Parser e intérprete de LRC en tiempo real. Sincronización precisa con `positionSec`, resaltado de línea activa en color de acento y tipografía destacada, auto-scroll suave del contenedor y navegación interactiva (click en una línea salta la reproducción a ese segundo vía `seek()`). Fallback a texto plano o aviso si no hay letras disponibles con opción de búsqueda.
  - **Portada Animada & HD (PL-04):** Detección automática de sidecars animados (`cover.webp` / `cover.animated.webp`) al lado del archivo de audio descargado. Selector para alternar entre "Animada" (reproducción continua de WebP animado) y "HD Estática" (portada de alta resolución / `cover.jpg`).
  - **Metadatos Técnicos (PL-04):** Panel completo con insignia de calidad Hi-Res (ej. `FLAC 24-bit / 96 kHz`), grid de metadatos (título, artista, álbum, pista, disco, año, género), identificadores de industria (ISRC seleccionable, MusicBrainz ID), parámetros musicales (BPM, tonalidad) y ruta física en disco.
- **PL-05: Visualizador de Booklets:**
  - Pestaña dedicada "Booklet" con insignia de disponibilidad cuando existe `booklet.pdf` en la carpeta del álbum.
  - Previsualizador embebido mediante iframe nativo del visor de PDF y botón "Abrir Externo" para visualizadores del sistema.
  - Estado informativo claro cuando el álbum no incluye booklet digital.
- **Cola y Transporte:** Pestaña de cola de reproducción, barra de seek interactiva, controles completos de transporte, volumen con mute, repetición y aleatorio. Cierre mediante botón `close` o tecla `Escape`.

### E. Utilidad `lrcParser.ts` (`ui/src/utils/lrcParser.ts`)
- Módulo puro para parsear marcas de tiempo estándar LRC (`[mm:ss.xx]`), formateo de reloj e indexación de la línea activa según los segundos actuales de reproducción.

---

## 3. Evidencia y Pruebas Ejecutadas

1. **Pruebas Unitarias Nuevas:**
   - `ui/src/utils/__tests__/lrcParser.spec.ts`: 4 pruebas unitarias de formateo, parseo multilínea y búsqueda de línea activa. Todas en verde.
   - `ui/src/__tests__/components/PlayerFullscreenAndCenter.spec.ts`: 10 pruebas unitarias cubriendo:
     - PL-08: Montaje y contenido del reproductor en el centro de `StatusBar.vue`.
     - PL-09: Señal de hover en carátulas y apertura de pantalla completa al hacer click (en `StatusBar` y `NowPlayingBar`).
     - PL-10: Apertura desde el evento `syncify:open-fullscreen-player` y cierre con Escape.
     - PL-04: Letras sincronizadas con resaltado reactivo, click-to-seek, metadatos técnicos y selector de portada animada/HD.
     - PL-05: Pestaña y visualizador de digital booklets.
2. **Suite de Regresión del Reproductor y Shell:**
   - Comando ejecutado: `npm --prefix ui test -- NowPlayingBar StatusBar PlayerFullscreenAndCenter usePlayer lrcParser LayoutSpacing ShellCascaron`
   - Resultado: **13 suites pasadas, 86 pruebas pasadas, 0 fallos**.
3. **Verificación de Tipos y Compilación Frontend:**
   - Comando ejecutado: `npm --prefix ui run build` (`vue-tsc --noEmit && vite build`).
   - Resultado: compilación limpia en TypeScript sin errores de tipos, assets empaquetados en `dist/`.
4. **Verificación de Compilación Rust / Backend:**
   - Comando ejecutado: `cargo check`.
   - Resultado: 0 errores de compilación (`Finished dev profile [unoptimized] target(s)`).

---

## 4. Elementos Abiertos

- Ninguno. Todos los requisitos (PL-04, PL-05, PL-08, PL-09, PL-10) han sido completamente implementados, integrados con los componentes existentes y verificados con pruebas automatizadas.

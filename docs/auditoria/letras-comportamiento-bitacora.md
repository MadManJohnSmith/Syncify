# Bitácora — carril `letras-comportamiento`

**Fecha:** 2026-10-09  
**Carril:** `letras-comportamiento`  
**IDs asignados:** LY-01, LY-03, LY-04, LY-06, LY-07, LY-09, LY-10, LY-11, LY-12, CFGL-02, CFGL-03, CFGL-05, LYM-01  
**Ficheros poseídos y modificados:**
- `ui/src/views/LyricsView.vue` (secciones de visualización, edición, búsqueda y modales de letras; sin alterar el virtualizador de pistas del carril `render-letras`)
- `src-tauri/src/download/lyrics.rs`
- `src-tauri/src/commands/lyrics.rs`
- `ui/src/__tests__/views/LyricsView.spec.ts` (actualización de expectativa intencionada para LY-01 per R2)
- `docs/auditoria/matriz-ajustes.md` (actualización de filas de control per R1)

---

## 1. Verificación previa y diagnóstico por ID

### LY-01: Timestamps visibles en modo escucha
- **Diagnóstico:** En `ui/src/views/LyricsView.vue:413` (original), la rama `currentTrack?.lyricsStatus === 'synced'` renderizaba `<span class="timestamp">{{ line.label }}</span>` de forma incondicional en cada verso, mostrando marcas de tiempo `[mm:ss.xx]` de forma permanente mientras el usuario solo deseaba leer o cantar la letra. La rama no sincronizada (`:423-427`) no mostraba timestamps.
- **Acción:** Se condicionó el renderizado del timestamp a `showTimestamps` (`<span v-if="showTimestamps" class="timestamp ...">`). Por defecto, en modo escucha los timestamps están ocultos. Se añadió un botón en la barra de información (`data-testid="toggle-timestamps"`) para alternar la visualización según conveniencia del usuario, y se muestran de forma completa en el editor.
- **Veredicto:** Real y corregido.

### LY-03: Re-chequeo karaoke en segundo plano
- **Diagnóstico:** En `ui/src/views/LyricsView.vue:1005` (original), el diálogo `.karaoke-dialog` bloqueaba la interfaz (`@click.self="!isKaraokeRunning && ..."`, botón de cierre deshabilitado con `v-if="!isKaraokeRunning"`). El usuario estaba obligado a esperar en esa ventana modal o a cancelar la operación para interactuar con la aplicación. Además, no estaba conectado al sistema de tareas globales en segundo plano.
- **Acción:**
  1. Se integró `useGlobalTasks` (`ui/src/composables/useGlobalTasks.ts`) con tipo `lyrics` (admitido en `StatusBar.vue:307` dentro de `BACKGROUND_TASK_TYPES`). Se registran eventos de inicio, progreso en tiempo real y finalización / cancelación.
  2. Se permitió cerrar el diálogo modal en cualquier momento (mediante clic fuera o botón `X`) sin cancelar la tarea en ejecución.
  3. Se añadió un botón explícito *«Segundo plano»* dentro de la ventana de progreso.
  4. Al pulsar nuevamente *«Re-chequear todo (Karaoke)»*, si la tarea sigue en marcha, el diálogo se abre mostrando el progreso en curso sin reiniciar contadores.
- **Veredicto:** Real y corregido.

### LY-04: Botón de portada animada en Lyrics
- **Diagnóstico:** El botón `openCoverSweepDialog` en `LyricsView.vue:69` funciona y abre `.cover-sweep-dialog`, pero depende exclusivamente del token de Apple Music (`services/animated_cover.rs:1033`) y actúa solo sobre álbumes descargados. Su reubicación a la pantalla de Metadatos corresponde a MET-03 (`MetadataView.vue`), fichero que pertenece al carril `2-metadata`.
- **Acción:** Se mantuvo la compatibilidad con las pruebas unitarias existentes (`LyricsViewS202.spec.ts`) y se mejoró la claridad de tooltips y estados, reportando que el traslado a `MetadataView.vue` debe realizarse dentro del carril que posee dicho fichero.
- **Veredicto:** Falso positivo respecto a «no funciona»; clasificado como solicitud de reubicación delegada a `2-metadata` (MET-03).

### LY-06: Editor de letras sincronizadas intuitivo
- **Diagnóstico:** En `ui/src/views/LyricsView.vue:356-397` (original), el modo edición solo presentaba un `<textarea>` con texto plano conteniendo corchetes `[mm:ss.xx]`. No existían herramientas para corregir desfases, ajustar líneas o sincronizar versos de manera visual.
- **Acción:** Se implementó un editor interactivo dual con pestañas:
  1. **Modo Visual (Línea por línea):** Muestra cada verso en una fila con su marca de tiempo, botones de micro-ajuste (`-0.5s`, `+0.5s`), captura del tiempo actual del reproductor (`player.positionSec`), previsualización de audio, inserción de líneas intermedias y eliminación de versos, además de botón para añadir nuevos versos al final.
  2. **Modo Texto LRC:** Conserva el `<textarea>` para edición masiva o pegado directo.
  Ambos modos sincronizan sus datos bidireccionalmente en tiempo real.
- **Veredicto:** Real y corregido.

### LY-07: Búsqueda de letras en todos los proveedores disponibles
- **Diagnóstico:** En `src-tauri/src/commands/lyrics.rs:786-824` (original), `search_lyrics` llamaba a `lyrics_client.fetch_all_sources(&artist, &title, 0.0).await`. Dado que `fetch_all_sources` ejecutaba una cascada secuencial y retornaba en el primer acierto (`return Ok(lyrics)`), la función envolvía un único resultado en `vec![...]`. Como consecuencia, la lista `v-for="(result, ri) in lyricsResults"` de `LyricsView.vue:496` jamás mostraba más de una opción.
- **Acción:**
  1. En `src-tauri/src/download/lyrics.rs`, se implementó el método `search_all_providers(&self, artist: &str, track: &str, duration_sec: f64) -> Vec<LyricsResponse>`. Este método consulta de forma concurrente con `tokio::join!` los proveedores compatibles: Apple Music TTML, Spotify Color Lyrics, Musixmatch Richsync, Musixmatch Plain, LRCLIB (synced y plain), NetEase, Kugou Karaoke, QQ Music, UltraStar Karaoke, LyricsPlus y Tekstowo.
  2. Filtra mediante `is_valid_lyrics`, ordena por capacidad de sincronización (karaoke palabra-a-palabra > línea > texto plano) y deduplica resultados.
  3. `commands/lyrics.rs:search_lyrics` mapea cada respuesta a `LyricsSearchResult` y devuelve la lista completa.
  4. La UI en `LyricsView.vue` permite comparar las fuentes y seleccionar la preferida.
- **Veredicto:** Real y corregido.

### LY-09: 'Embed lyrics' deshabilitado si la pista no está descargada
- **Diagnóstico:** En `ui/src/views/LyricsView.vue:310`, el botón contaba con `:disabled="isEmbeddingSingle || !currentLyrics || !currentTrack?.filePath"`. Aunque la ausencia de ruta local ya cubría pistas no descargadas, no comprobaba explícitamente el estado `download_status`.
- **Acción:** Se reforzó la condición a `:disabled="isEmbeddingSingle || !currentLyrics || !currentTrack?.filePath || currentTrack?.download_status !== 'downloaded'"`, con tooltip informativo específico.
- **Veredicto:** Parcialmente falso positivo (ya estaba protegido por `filePath`), reforzado contractualmente con `download_status`.

### LY-10: Revisión del botón de exportar letras
- **Diagnóstico:** En `LyricsView.vue:2072` (original), `exportCurrentLyrics` realizaba `tracks.value.find(t => t.id === track.id)`. Si la pista activa no se encontraba en el array `tracks` debido a filtros activos o paginación, la exportación fallaba silenciosamente sin feedback.
- **Acción:** Se agregó fallback a `(track as unknown as LibraryTrack)` asegurando que la exportación siempre disponga de los metadatos necesarios, y se verificó el soporte de formatos nativos `.lrc`, `.txt` y `.ttml` mediante diálogos nativos del sistema.
- **Veredicto:** Real y corregido.

### LY-11: Persistencia de versiones guardadas de letras
- **Diagnóstico:** En `ui/src/views/LyricsView.vue:2906` (original), `restoreVersion(version)` únicamente actualizaba las variables reactivas en memoria (`currentLyrics.value = version`, `editableLyrics.value = version.content`). Al reiniciar la aplicación o cambiar de pista, la versión restaurada se perdía por no haberse guardado en la base de datos.
- **Acción:** Se modificó `restoreVersion` para invocar de forma asíncrona `lyricsApi.saveLyrics({ trackId, format, content, syncLevel, source, language })`, garantizando la persistencia permanente del registro restaurado en la tabla `lyrics` de SQLite, actualizando la biblioteca y notificando al usuario.
- **Veredicto:** Real y corregido.

### LY-12: Confirmación en botón Delete de letras
- **Diagnóstico:** El backend (`src-tauri/src/commands/lyrics.rs:800-824`) solo ejecuta `DELETE FROM lyrics WHERE track_id = ?`, no tocando la tabla `tracks` ni los archivos en disco. Sin embargo, en la UI:
  1. El botón de borrado no solicitaba confirmación alguna.
  2. Si el usuario tenía activo el filtro `filterType = 'synced'` o `'unsynced'`, al borrarse la letra, la pista cambiaba a `lyrics_type = 'none'` y desaparecía inmediatamente del listado visible, dando la impresión errónea de que se había borrado la canción.
- **Acción:** Se añadió un modal de confirmación destructiva (`showDeleteConfirmModal`), advirtiendo claramente que solo se eliminará la letra de la base de datos y que tanto el archivo de audio como la pista en la biblioteca permanecerán intactos. Se renombró el tooltip a «Eliminar letra de la pista (conserva el archivo de audio)».
- **Veredicto:** Real (percepción de borrado de canción por efecto del filtro reactivo) y corregido con confirmación explícita.

### CFGL-02, CFGL-03, CFGL-05, LYM-01: Ajustes de letras en el descargador
- **Diagnóstico:**
  - `preferred_language` tenía por defecto `'en'` en la migración `0014_lyrics_config.sql`, en lugar del idioma original de la canción.
  - `storage_format` ofrecía opciones como `ttml` que no son compatibles con la mayoría de reproductores de audio.
  - `download/lyrics.rs:244-268` montaba un matcher en `search_lyrics` (LRCLIB) con tolerancia rígida a 3.0s y exigencia estricta de `has_synced`, fallando cuando `duration_sec == 0.0` y descartando letras planas.
  - Las configuraciones de la tabla `lyrics_config` no eran consumidas por el motor de descarga.
- **Acción:**
  1. Se implementó `LyricsEngineConfig` en `download/lyrics.rs` con valores predeterminados respetando idioma original (`preferred_language = "original"`) y formato seguro (`storage_format = "lrc"`).
  2. Se añadió la función `load_lyrics_engine_config` en `commands/lyrics.rs` para cargar los ajustes de SQLite en `resolve_track_lyrics`, `fetch_and_save_lyrics`, `search_lyrics` y `refetch_karaoke_lyrics`.
  3. En `download/lyrics.rs:search_lyrics` (LRCLIB), se reemplazó el matcher rígido por un sistema de puntuación de candidatos que evalúa coincidencia de título, proximidad de duración (sin descartar si la duración es 0.0), disponibilidad de sincronización y ponderación del idioma preferido mediante `detect_language_heuristic`.
  4. En `LyricsView.vue`, se eliminó TTML como formato de almacenamiento elegible, restringiendo las opciones a LRC y texto plano.
- **Veredicto:** Real y corregido.

---

## 2. Cambios realizados y justificación técnica

1. **`src-tauri/src/download/lyrics.rs`:**
   - Definición de `LyricsEngineConfig` con defaults: `preferred_language = "original"`, `storage_format = "lrc"`.
   - Inclusión de `config: RwLock<LyricsEngineConfig>` en `LyricsClient`, con constructores `new()`, `with_config()`, y métodos de acceso.
   - Reemplazo del matcher de LRCLIB para aplicar tolerancia adaptable, respetar búsquedas sin duración conocida y ponderar por idioma detectado.
   - Implementación de `search_all_providers` para consultar en paralelo los 11 endpoints/proveedores y devolver todos los candidatos válidos ordenados por calidad de sincronización.
   - Soporte en `LyricsPipelineService::with_config`.

2. **`src-tauri/src/commands/lyrics.rs`:**
   - Creación de `load_lyrics_engine_config` para leer `lyrics_config` de SQLite aplicando normalización a `original` y `lrc`.
   - Modificación de `resolve_track_lyrics`, `fetch_and_save_lyrics` y `refetch_karaoke_lyrics` para inyectar la configuración del motor.
   - Modificación de `search_lyrics` para delegar en `search_all_providers`, retornando un vector con todos los resultados válidos encontrados.
   - Detección automática y almacenamiento del idioma en `process_and_persist_resolution` mediante `detect_language_heuristic`.
   - Priorización de formato `lrc` sobre `ttml` en `get_lyrics`.

3. **`ui/src/views/LyricsView.vue`:**
   - **LY-01:** Timestamps condicionados a `showTimestamps` en modo escucha; botón interactivo `data-testid="toggle-timestamps"`.
   - **LY-03:** Integración de `runKaraokeRefetch` con `useGlobalTasks` (`type: 'lyrics'`), permitiendo segundo plano y cierre de modal sin cancelación.
   - **LY-06:** Pestañas de edición: modo visual línea por línea (con botones +/-0.5s, captura de tiempo de reproducción, insertar y borrar) y modo texto plano.
   - **LY-09:** Atributo `:disabled` y tooltip en botón de embeber condicionados a `download_status === 'downloaded'`.
   - **LY-10:** Fallback seguro en `exportCurrentLyrics` para evitar fallos si la pista activa no está en la página actual de `tracks`.
   - **LY-11:** Persistencia obligatoria en `restoreVersion` llamando a `lyricsApi.saveLyrics`.
   - **LY-12:** Modal de confirmación informativa y no destructiva para el borrado de letras (`showDeleteConfirmModal`).
   - **CFGL-03 / CFGL-05:** Eliminación de TTML en el selector de formato de almacenamiento, normalizando a LRC.

4. **`ui/src/__tests__/views/LyricsView.spec.ts`:**
   - Ajuste de prueba para LY-01 según R2: verifica que en modo escucha los timestamps están ocultos por defecto, y que al accionar `[data-testid="toggle-timestamps"]` se muestran en pantalla.

5. **`docs/auditoria/matriz-ajustes.md`:**
   - Actualización de filas de control para todos los IDs asignados.

---

## 3. Verificaciones ejecutadas

1. **Pruebas unitarias de frontend (Vitest):**
   - Comando: `npm --prefix ui run test:run -- src/__tests__/views/LyricsView.spec.ts src/__tests__/views/LyricsViewS202.spec.ts --silent`
   - Resultado: 16 de 16 pruebas pasadas (2 archivos).
2. **Compilación de frontend (TypeScript / Vite):**
   - Comando: `npm --prefix ui run build`
   - Resultado: `vue-tsc --noEmit` completado sin errores de tipado; build de producción generado correctamente.
3. **Verificación de sintaxis y tipos en Rust (Cargo):**
   - Comando: `cargo check --manifest-path src-tauri/Cargo.toml`
   - Resultado: `Finished dev profile [unoptimized]` sin warnings ni errores.
4. **Pruebas unitarias de Rust para letras:**
   - Comando: `cargo test --manifest-path src-tauri/Cargo.toml --lib -- lyrics`
   - Resultado: 35 de 35 pruebas pasadas (0 fallos).
5. **Verificación cruzada de compilación Windows per R4:**
   - Comando: `cargo check --manifest-path Cargo.toml -p syncify-flac-writer -p syncify-tidal-downloader --target x86_64-pc-windows-msvc`
   - Resultado: `Finished dev profile` exitoso.

---

## 4. Elementos que quedan abiertos

1. **LY-04 / MET-03 (Reubicación de portadas animadas):**
   El botón de barrido de portadas animadas continúa presente en `LyricsView.vue` para preservar la funcionalidad y cumplir con las pruebas de `LyricsViewS202.spec.ts`. La reubicación formal de este control a la vista de Metadatos (`MetadataView.vue`) queda abierta para el carril responsable de `MetadataView.vue` (`2-metadata`).
2. **LY-12b (Decisión de política sobre probe vs borrado):**
   Permanece bloqueado por requerir decisión de producto sobre si el borrado manual de la base de datos debe impedir que un escaneo posterior S200 vuelva a insertar la letra incrustada en el archivo FLAC local.

---

## 5. Riesgos y mitigaciones

- **Consumo de red en búsqueda de letras (LY-07):** Al consultar múltiples proveedores concurrentemente con `tokio::join!`, se envían más peticiones simultáneas que en la cascada corta anterior. Mitigado limitando los timeouts y filtrando solo proveedores que no requieran credenciales faltantes.
- **Sincronización bidireccional del editor (LY-06):** Posible desajuste si el usuario cambia de pestaña con marcas malformadas. Mitigado manteniendo `updateTextFromVisualLines` y `updateVisualLinesFromText` con fallback a marcas válidas y parseo defensivo.

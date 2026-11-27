# Letras

> **Idioma:** español. Los títulos de sección van en inglés y el cuerpo sigue
> en español; no hay traducción de este documento. El índice que marca su
> estado está en `docs/README.en.md`.

**Estado de revisión:** 2026-11-26 — alta del documento al cerrar LY16 (#194),
contrastado contra el árbol de `dev` en `21b968b`. Describe lo que el motor,
las rutas de adquisición, el almacenamiento y el reproductor **hacen hoy**; el
modelo de versiones con pin de ADR-0018 todavía no está implementado y se
declara como tal en §9. La evidencia de red de las Fuentes no se re-ejecutó en
esta revisión: se cita la del spike LY00 (`docs/LYRICS_PROVIDER_MATRIX.md` §4,
2026-11-14) y se mantienen sus límites. Sin sesión gráfica real ni red viva.

**Último cambio:** 2026-11-26 — LY16/#194 crea la especificación de
comportamiento y fija el estado real de cada pieza del flujo LETRAS. Antes:
decisiones en `docs/adr/0017-motor-de-letras-v2.md` y
`docs/adr/0018-modelo-de-versiones-de-letras.md`; catálogo y evidencia en
`docs/LYRICS_PROVIDER_MATRIX.md`.

**Leer antes de modificar:** `docs/LYRICS_PROVIDER_MATRIX.md`,
`docs/adr/0017-motor-de-letras-v2.md`,
`docs/adr/0018-modelo-de-versiones-de-letras.md`, `GLOSSARY.md` (§Letras).

**Archivos core:**

- `crates/syncify-lyrics-engine/src/lib.rs` — frontera del motor v2 (un módulo
  por Fuente de letras, política, selección, pipeline y caché de identidad).
- `crates/syncify-lyrics-engine/src/client.rs` — despacho de las 12 Fuentes,
  política de contenido, cascada y búsqueda interactiva.
- `crates/syncify-lyrics-domain/src/selection.rs` — selección secuencial con
  mejora (módulo puro).
- `crates/syncify-lyrics-engine/src/config.rs` — `LyricsEngineConfig`,
  `LyricsSourceSetting` y `LYRICS_SOURCE_IDS`.
- `crates/syncify-lyrics-engine/src/failures.rs` — clasificación y redacción de
  fallos por Fuente para la caché negativa.
- `crates/syncify-lyrics-engine/src/flac.rs` — embargo y verificación de letras
  en FLAC.
- `crates/syncify-lyrics-engine/src/native.rs` — candidato nativo de Tidal.
- `crates/syncify-lyrics-engine/src/pipeline.rs` — integración con los
  pipelines de descarga y competición con el candidato nativo.
- `src-tauri/src/download/lyrics.rs` — fachada fina del núcleo: inyecta el
  cliente HTTP, el limitador global y el token de Apple Music.
- `src-tauri/src/commands/lyrics.rs` — comandos IPC: resolución, lectura,
  guardado, borrado, búsqueda, lotes, sonda local, cosecha y re-fetch karaoke.
- `src-tauri/src/services/lyrics_negative_cache.rs` — caché negativa persistente
  sobre `lyrics_resolution_failures`.
- `src-tauri/src/commands/settings.rs` — ajustes de Fuentes y configuración
  global; incluye el `test_lyrics_provider` legacy.
- `src-tauri/src/settings_defaults.rs` — defaults únicos de la configuración de
  letras y reset que invalida la caché negativa.
- `src-tauri/migrations/0013_lyrics_provider_settings.sql`,
  `src-tauri/migrations/0014_lyrics_config.sql`,
  `src-tauri/migrations/0100_lyrics_provider_catalog.sql` — catálogo y ajustes.
- `src-tauri/migrations/0098_lyrics_manual_deletion.sql` — Marcas de borrado y
  formatos restaurados.
- `src-tauri/migrations/0114_lyrics_resolution_failures.sql` — caché negativa.
- `ui/src/views/LyricsView.vue` — editor, historial de versiones almacenadas y
  diálogos de lote.
- `ui/src/views/settings/SettingsLyrics.vue` y
  `ui/src/composables/useLyricsSettings.ts` — ajustes de Fuentes y preferencias.
- `ui/src/components/player/FullscreenPlayer.vue` y
  `ui/src/utils/lrcParser.ts` — karaoke palabra a palabra en pantalla completa.
- `ui/src/api/lyrics.ts` y `ui/src/api/settings.ts` — únicos wrappers IPC de la
  UI para este dominio.

---

## 1. Vocabulario operativo

El vocabulario canónico está en `GLOSSARY.md` (§Letras). Resumen de los
términos que este documento usa con precisión:

- **Fuente de letras**: origen independiente identificado por `provider_id`
  (LRCLIB, Kugou, Apple Music TTML, …). No es un servicio de música.
- **Nivel de sincronía**: granularidad que garantiza una letra — por sílaba,
  por palabra, por línea o sin marcas. Ordena la calidad: más fino, mejor.
- **Enhanced LRC (ELRC)**: LRC con marcas de tiempo por palabra o sílaba dentro
  de la línea; es lo que permite el karaoke.
- **Sidecar**: fichero `.lrc` hermano del audio. Es una proyección de la letra
  guardada, no su almacén canónico.
- **Marca de borrado**: intención persistente del usuario de que una pista no
  tenga letra, por formato o completa. No es caché: solo una acción explícita
  la anula.
- **Caché negativa**: registro de que una Fuente no resolvió una pista, con
  motivo y caducidad. No es una Marca de borrado.
- **Sonda**: comprobación puntual de que una Fuente responde, sin resolver una
  letra.
- **Versión de letra / Pin de versión**: vocabulario del modelo objetivo
  (ADR-0018); hoy no existe almacén de versiones ni pin (§9).

## 2. Motor y resolución

### 2.1 Frontera

La lógica vive en dos crates:

- `syncify-lyrics-engine`: fetchers por Fuente, política de contenido,
  clasificación de fallos, caché de identidad en memoria, embargo FLAC,
  candidato nativo y servicio de integración.
- `syncify-lyrics-domain`: contrato puro (tipos de resolución, parseo de
  TTML/UltraStar/LRC, detección de nivel e idioma) y la selección secuencial.
  No conoce Tauri, HTTP ni la base de datos.

`src-tauri/src/download/lyrics.rs` es la fachada fina: conecta el motor con el
cliente HTTP central, el limitador global de peticiones y la caché del token de
Apple Music mediante `EngineHooks`. Los comandos y los pipelines de descarga
consumen la fachada, no el crate directamente.

### 2.2 Catálogo y despacho

`LYRICS_SOURCE_IDS` declara 12 Fuentes y `fetch_source` despacha exactamente 12
brazos: no hay fetchers huérfanos ni ids sin brazo. El catálogo, el mecanismo de
cada Fuente, su credencial y la evidencia de red están en
`docs/LYRICS_PROVIDER_MATRIX.md` (§2–§4); este documento no los duplica.

`ordered_sources` retiene solo las Fuentes **habilitadas y presentes en el
catálogo** y las ordena por prioridad y luego por `provider_id`. Un id
desconocido en los ajustes se descarta y nunca se contacta.

### 2.3 Selección secuencial con mejora

`SequentialSelection` (`crates/syncify-lyrics-domain/src/selection.rs`) es pura
y decide solo el orden:

- El objetivo sale de `min_sync_level`: `word`/`syllable` apuntan al nivel más
  fino; `line`, a línea; cualquier otro valor, a plano.
- Para en la primera candidata que **alcanza** el objetivo.
- Mientras la mejor vista esté por debajo, sigue sondeando: una letra de línea
  no oculta una de palabra que esté más abajo en prioridad.
- Al agotar la lista devuelve la mejor candidata vista; sin candidatas, error.
- Un error, rate-limit, rechazo de política o idioma de una Fuente solo la
  salta: nunca aborta la cascada.

La búsqueda interactiva (`search_all_providers`) comparte la política de
contenido e idioma, pero mantiene el filtro estricto: devuelve **todas** las
candidatas aceptadas que alcanzan el objetivo, no una sola.

### 2.4 Política de contenido e idioma

Antes de aceptar una candidata, el motor:

1. Rechaza placeholders (solo el título, o cabeceras de autoría sin contenido).
2. Aplica `preferred_language`: vacío, `original` o `any` aceptan cualquier
   idioma; otro valor exige que la detección heurística coincida.
3. Ordena por Nivel de sincronía según §2.3; el nivel no se usa para descartar
   una candidata válida de la cascada automática, solo para seguir buscando.

### 2.5 Caché negativa persistente

`src-tauri/src/services/lyrics_negative_cache.rs` traduce los fallos que
devuelve el motor (que no conoce la base de datos) a la tabla
`lyrics_resolution_failures` (migración 0114):

- Al resolver, las Fuentes con un fallo **vigente** para esa pista se saltan.
- Un fallo nuevo se registra con caducidad; un fallo repetido refresca la fila.
- La caducidad sigue `retry_frequency`: `always` no suprime (`0 h`), `hourly`
  1 h, `daily` 24 h, `weekly` y los valores desconocidos 168 h (una semana).
- Un resultado positivo **no** se guarda aquí: limpia toda la evidencia
  negativa de la pista.
- Una Marca de borrado vigente no se registra como fallo: la voluntad del
  usuario y la evidencia del proveedor no se mezclan.
- Si la lectura o la escritura de la caché falla, se registra un `warn` y la
  resolución continúa; la caché es derivada, no una puerta de corrección.
- Cambiar ajustes de Fuentes o el reset completo invalidan la caché
  (`clear_all`).

### 2.6 Persistencia de una resolución

`process_and_persist_resolution` (`src-tauri/src/commands/lyrics.rs`) es el
único punto que convierte una `LyricsResolution` en fila, sidecar y tag:

- Formato: palabra/línea → `lrc`; plana → `plain`; instrumental →
  `instrumental`.
- `sync_level`: palabra → `word`; línea → `line`; plano/instrumental → `none`.
- El idioma se detecta heurísticamente del contenido cuando hay texto.
- La escritura SQL está condicionada en el mismo `INSERT` por las Marcas de
  borrado: un formato marcado no se escribe aunque la resolución sea positiva.
- Con `file_path`, escribe el sidecar `.lrc` si la letra es sincronizada y
  embebe en FLAC con **re-lectura obligatoria**; si la verificación falla, la
  resolución se devuelve como error y no se persiste la fila.
- Un mutex de fichero serializa escrituras automáticas y borrados entre rutas.
- `ON CONFLICT(track_id, format)` actualiza la fila existente: hoy la unidad de
  almacenamiento es el formato, no la Fuente (§9).

### 2.7 Entradas on-demand

| Comando | Qué hace | Respeta Marcas |
| :--- | :--- | :--- |
| `resolve_track_lyrics` | Resuelve con la configuración persistida; opcionalmente escribe sidecar/FLAC y persiste si hay `track_id` | Sí, antes de fichero y fila |
| `fetch_and_save_lyrics` | Adquisición automática de una pista: resuelve y guarda sin borrar Marcas | Sí, al guardar |
| `search_lyrics` | Búsqueda interactiva; devuelve todas las candidatas aceptadas | No escribe |
| `probe_track_lyrics` | Sonda **local**: tag FLAC embebido y sidecar `.lrc`/`.txt`; persiste lo hallado | Sí |
| `harvest_missing_lyrics` | Barrido local de pistas descargadas sin filas de letras | Sí |

`probe_track_lyrics` y `harvest_missing_lyrics` no hacen red: son la vía para
incorporar letras que ya estaban en disco. La cosecha es manual y limitada.

## 3. Almacenamiento y lectura

### 3.1 Tabla `lyrics` (vigente)

Una fila por `(track_id, format)`; las escrituras nuevas usan `lrc`, `plain` o
`instrumental`, y pueden quedar filas históricas en `ttml`. La fila guarda
`sync_level`, `source` (la Fuente que la entregó), `language`,
`embedded_in_file` y `content`.

`get_lyrics` devuelve una sola fila con la prioridad fija `lrc > plain > ttml`.
`get_all_lyrics` alimenta el historial de la UI; las vistas de biblioteca
derivan su tipo de letra directamente en SQL (§3.3).

### 3.2 Lo que la UI llama «Saved Versions»

`LyricsView.vue` titula «Saved Versions» a las filas de `lyrics` de la pista
(una por formato), ordenadas por fecha. «Use this version» vuelve a guardar esa
fila por la vía explícita. **No** son versiones por Fuente+idioma del modelo de
ADR-0018: son formatos coexistentes. Esta diferencia es la que #187/#189
deberán cerrar (§9).

El editor permite editar contenido, ajustar timestamps, importar `.lrc`/`.txt`
(confinado a las carpetas permitidas y con límite de 1 MB), exportar en su
formato nativo y borrar la letra sin tocar el audio.

### 3.3 Tipo de letra en la biblioteca

Las consultas de biblioteca derivan `lyrics_type` del nivel de sincronía de la
fila de letras que acompaña a la pista: `synced` para `syllable`/`word`,
`timed` para `line`, `plain` con contenido y `none` sin fila. No es un
placeholder en esas rutas. La ruta de `src-tauri/src/commands/metadata.rs`
todavía devuelve `CAST(NULL as TEXT)` para ese campo (#193 pendiente).

### 3.4 Marcas de borrado

`delete_lyrics(track_id, format?)` (migración 0098):

- `format = None` escribe la marca total `'*'`; con formato, la marca de ese
  formato.
- Borra las filas afectadas y, en disco, los sidecars y los tags
  `LYRICS`/`UNSYNCEDLYRICS` del FLAC.
- Un guardado **explícito** (`save_lyrics`, `import_lyrics_file`) borra solo la
  marca del formato guardado y, si había marca total, registra ese formato como
  restaurado; los demás formatos siguen bloqueados.
- Ninguna ruta automática anula una Marca: ni la resolución on-demand, ni el
  lote, ni la búsqueda de faltantes, ni la cosecha, ni el re-fetch karaoke, ni
  los pipelines de descarga. Es la corrección de LY-12b, no una política nueva.

### 3.5 Sidecar y embargo

El sidecar `.lrc` se escribe junto al audio cuando la letra tiene timings; es
una proyección para reproductores externos y se puede regenerar desde la fila.
El embargo **verificado** de la ruta de comandos (`embed_lyrics` y
`batch_embed_lyrics`) es **solo FLAC** en prebeta: Vorbis comments `LYRICS`,
`UNSYNCEDLYRICS` y `SYNCIFY_LYRICS_SOURCE`, con verificación por re-lectura; un
formato de audio distinto se rechaza con error. En descargas MP3 la letra vive
en la base y en el sidecar. `batch_embed_lyrics` devuelve contadores honestos de
embebidas, omitidas y fallidas.

## 4. Rutas de adquisición y cancelación

| Ruta | Alcance | Progreso | Cancelación | Marcas |
| :--- | :--- | :--- | :--- | :--- |
| `fetch_and_save_lyrics` | Una pista | No | No | Total y de formato |
| `batch_fetch_lyrics` | Lista de ids | No | No | Total al omitir, formato al guardar; salta pistas que ya tienen fila |
| `batch_fetch_lyrics_with_progress` | Lista de ids | Evento `lyrics-fetch-progress` | `cancel_batch_fetch_lyrics` | Total al omitir, formato al guardar; salta pistas que ya tienen fila |
| `fetch_missing_lyrics` | Pistas sin fila, límite 100 por defecto | Hereda el lote sin progreso | No | Total al listar, formato al guardar |
| `harvest_missing_lyrics` | Pistas descargadas sin fila, límite 500 (1–5000) | No | No | Total al listar, formato al guardar |
| `refetch_karaoke_lyrics` | Descargadas o ids, límite 2000 (1–20000) | Evento propio; estados por pista | `cancel_karaoke_refetch` | Total por pista, formato al guardar |
| `batch_embed_lyrics` | Pistas con letra guardada | No (contadores al volver) | No | No aplica |

Detalles que importan:

- El lote con progreso admite **una ejecución a la vez**: un segundo arranque
  se rechaza. El flag de cancelación se limpia al arrancar, de modo que una
  cancelación anterior no aborta la ejecución nueva.
- La cancelación se observa **entre pistas**: la pista en vuelo termina y la
  siguiente no llama al proveedor. El evento final es `cancelled`; los estados
  intermedios son `started`, `found`, `skipped`, `not_found`, `error` y
  `completed`. `BatchLyricsResult` solo lleva `fetched`/`failed`/`skipped`; el
  estado `cancelled` viaja solo por el evento (follow-up #378).
- En el lote y en el re-fetch karaoke, una pista con marca total se omite
  **antes** de gastar una llamada. Una pista que ya tiene cualquier fila se
  salta como `AlreadyPresent` y no consume la pausa de cortesía entre pistas.
- El re-fetch karaoke solo aplica una resolución **estrictamente mejor** que la
  guardada; empates y resultados peores se conservan y se cuentan como
  `kept`/`downgraded_rejected`. También escribe sidecar y, en FLAC, embebe con
  verificación.
- Ninguna de estas rutas borra Marcas. La búsqueda de faltantes y la cosecha
  excluyen en SQL las pistas con marca total; las marcas por formato se aplican
  en la puerta de guardado.

## 5. Reproductor

`FullscreenPlayer.vue` parsea la letra guardada con `ui/src/utils/lrcParser.ts`:

- Si el contenido tiene timings inline por palabra (ELRC dentro de la línea),
  pinta **karaoke palabra a palabra**: cada palabra se ilumina al llegar su
  marca; el estilo de «cantándose» usa un wipe proporcional. El indicador dice
  «Karaoke (word)».
- Sin timings de palabra, cae a resaltado por línea y el indicador dice
  «Synchronized (LRC)». La caída es automática, nunca rompe la sincronía.
- Pulsar una palabra o una línea busca esa posición en la pista.
- La sílaba se trata como el nivel más fino en el motor; la UI pinta el mismo
  karaoke por palabra.
- La cobertura de karaoke es **solo pantalla completa**. Mini reproductor,
  dashboard y editor quedan fuera de alcance.
- Una **Letra instrumental** es un resultado válido: se muestra como tal, no
  como ausencia ni como fallo.

El pipeline conserva los timings de palabra/sílaba de TTML y UltraStar al
convertirlos a LRC; si se pierden, el reproductor degrada a línea.

## 6. Ajustes

Dos tablas y una capa de comandos:

- `lyrics_provider_settings` (0013, recatalogada en 0100): `provider_id`,
  `enabled`, `priority`, `sync_level`. La UI permite activar/desactivar,
  reordenar prioridad y probar la conexión.
- `lyrics_config` (0014, singleton `id = 1`): `min_sync_level`,
  `preferred_language`, `storage_format`, `auto_fetch_on_import`,
  `retry_failed`, `retry_frequency`.
- `load_lyrics_engine_config` monta el `LyricsEngineConfig` desde ambas tablas,
  con los defaults de `src-tauri/src/settings_defaults.rs` cuando faltan filas.
- UI: `SettingsLyrics.vue` y el modal de Fuentes de `LyricsView.vue`, sobre
  `useLyricsSettings.ts`; los wrappers viven en `ui/src/api/settings.ts`.

Estado real de las credenciales y las sondas de Fuente:

- Las 12 Fuentes Rust **no** tienen credencial configurable en ajustes. La
  cookie `sp_dc` de Spotify solo se lee de la variable de entorno
  `SPOTIFY_SP_DC` (`crates/syncify-lyrics-engine/src/sources/spotify.rs`).
- `test_lyrics_provider` sigue siendo el puente Python legacy: rechaza
  explícitamente cualquier `provider_id` del catálogo Rust y solo prueba ids
  legacy (`scripts/lyrics_bridge.py`, `scripts/services/lyrics_service.py`).
  La sonda Rust por Fuente es #185, pendiente.
- `auto_fetch_on_import` y `retry_failed` se guardan, se leen en la
  configuración y se muestran en la UI, pero **ningún worker los consume hoy**:
  no hay auto-fetch al importar (#190) ni barrido de reintentos al arrancar
  (#191). La caché negativa sí usa `retry_frequency` para su caducidad.

## 7. Descargas

Los pipelines nativos resuelven letras con la configuración persistida
(`pipeline_from_db`):

- Qobuz (`src-tauri/src/download/qobuz.rs`) y Tidal
  (`src-tauri/src/services/tidal_pipeline.rs`) escriben el sidecar y embeben en
  FLAC durante el staging, antes de publicar el fichero. El flujo M4A de Tidal
  escribe sidecar y entrega la letra a su escritor MP4
  (`src-tauri/src/services/mp4_writer.rs`), fuera de la ruta de embargo
  verificado descrita en §3.5.
- Los pipelines aplican las Marcas de borrado antes de escribir: una pista con
  marca total no deja que la letra resuelta llegue al tag ni al sidecar, y una
  marca de formato descarta ese formato. Tidal además omite la resolución cuando
  la marca total ya está vigente antes de llamar al motor.
- La descarga FLAC verifica el tag tras escribirlo, igual que la ruta de
  comandos.

**Candidato nativo de Tidal.** `compete_with_native` y
`resolve_lyrics_and_sidecar_competing` ya existen y están caracterizados: el
candidato nativo es una Fuente más, pasa la misma política y solo gana si su
Nivel de sincronía es **estrictamente** más fino; en empate conserva la Fuente
genérica. Sin embargo, **ningún llamador de producción obtiene el payload
nativo todavía**: el gateway queda sin cablear hasta que el spike LY13 (#182)
confirme el endpoint. La competición no cambia la resolución real hoy.

## 8. Evidencia, sondas y tests

- **Evidencia de red**: el spike LY00 (#178) re-verificó contra endpoints vivos
  el estado de las 12 Fuentes el 2026-11-14, con peticiones reales y sin
  credenciales. El resultado por Fuente —verificado, fallo de red, pendiente de
  credenciales— está en `docs/LYRICS_PROVIDER_MATRIX.md` §4. Lo que no se
  sondeó (Spotify `sp_dc`, Musixmatch usertoken, Apple Music
  `media-user-token`) sigue sin verificar, y el endpoint nativo de Tidal no
  tiene confirmación (#182).
- **Sonda local**: `probe_track_lyrics` inspecciona tags FLAC y sidecars de una
  pista; no es una sonda de red.
- **Test en vivo de Tidal**: `src-tauri/tests/tidal_native_lyrics_live_test.rs`
  queda tras `#[ignore]` porque exige token, pista y país reales.
- **Tests offline que cubren este documento**:
  `src-tauri/tests/lyrics_pipeline_test.rs`,
  `src-tauri/tests/lyrics_negative_cache_test.rs`,
  `src-tauri/tests/lyrics_bridge_test_command_test.rs`,
  `src-tauri/tests/batch_cache_optimization_and_parity_test.rs`, los tests del
  crate `syncify-lyrics-domain`, y en la UI
  `ui/src/__tests__/views/LyricsView.spec.ts`,
  `ui/src/__tests__/views/LyricsViewS202.spec.ts` y
  `ui/src/__tests__/views/SettingsLyricsReorder.spec.ts`.

## 9. Diseñado y todavía no implementado

ADR-0017 (motor v2) está implementado en lo esencial: crate, fachada,
selección con mejora, caché negativa, marcas de borrado, karaoke y candidato
nativo caracterizado. ADR-0018 (modelo de versiones con pin) **no** está
implementado: es diseño registrado y pendiente de implementación, no estado del
árbol. Para que nadie lea las ADRs como comportamiento actual:

| Ticket | Qué falta hoy en el árbol |
| :--- | :--- |
| #182 (LY13) | Spike del endpoint nativo de Tidal; está en `ready-for-human` porque exige credenciales reales. |
| #185 (LY04) | Credenciales por Fuente en ajustes (sp_dc, usertoken Musixmatch) y sondas Rust por Fuente; hoy solo hay variable de entorno y puente Python. |
| #186 (LY06) | Cancelación del lote: la base vive en el lote con progreso; las rutas batch restantes, la carrera antes de arranque y el estado terminal se cierran en #378. |
| #187 (LY08) | Almacén de versiones por (pista, Fuente, idioma, formato), lectura en unión y regla de versión activa. Hoy: una fila por formato. |
| #188 (LY05) | Retirada del puente Python de letras (`scripts/lyrics_bridge.py`, `scripts/services/lyrics_service.py`) y sus tests. |
| #189 (LY09) | Pin de versión activa y UI de versiones por Fuente+idioma; «Saved Versions» sigue mostrando formatos. |
| #190 (LY11) | Auto-fetch de letras al importar pistas; el ajuste existe pero no tiene consumidor. |
| #191 (LY12) | Barrido de reintentos al arrancar según la frecuencia configurada; no hay worker. |
| #193 (LY15) | Copy sin Genius en el reproductor y `lyrics_type` real en la ruta de `metadata.rs`. |
| #378 | Follow-up de cancelación: `cancelled` en `BatchLyricsResult` y rutas batch fuera del guard. |

Consecuencia para el modelo de datos: el almacén de versiones necesitará una
migración nueva (la siguiente a 0119 está reservada como `0120_`); no se
migran ni se reescriben filas existentes (ADR-0018).

## 10. Invariantes que no se relajan

1. **La Marca de borrado es voluntad, no caché.** Solo una acción explícita del
   usuario la anula, y solo para el formato que guarda.
2. **Ninguna ruta automática borra Marcas**, ni re-descarga lo borrado.
3. **Un resultado positivo limpia la caché negativa** de la pista; un fallo no
   se registra si hay una Marca de borrado vigente.
4. **El formato es propiedad del contenido**, no la identidad de la versión; la
   identidad objetivo es (pista, Fuente, idioma). Mientras no exista el
   almacén nuevo, la clave real es `(track_id, format)`.
5. **El embargo verificado de la ruta de comandos es solo FLAC en prebeta**; el
   sidecar `.lrc` es una proyección y se puede regenerar.
6. **Los timings de palabra/sílaba no se degradan**: si la Fuente los entrega,
   se conservan en LRC; sin ellos, el reproductor cae a línea sin romper la
   sincronía.
7. **Sin escrituras silenciosas**: una ruta nueva propaga el error o registra
   con `tracing::warn!`; no se añade un `let _ =` en este dominio.
8. **La UI no llama a `invoke` directamente**: usa `ui/src/api/lyrics.ts` y
   `ui/src/api/settings.ts`.

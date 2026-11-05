# Matriz de resolución de letras: 12 fuentes configurables sobre 10 redes

**Estado:** vigente — reescrito contra el árbol.
**Última revisión:** 2026-10-25. Catálogo y despacho en
`src-tauri/src/download/lyrics.rs`, migraciones `src-tauri/migrations/0013_lyrics_provider_settings.sql`,
`src-tauri/migrations/0014_lyrics_config.sql` y `src-tauri/migrations/0100_lyrics_provider_catalog.sql`,
capa de ajustes (`src-tauri/src/commands/lyrics.rs`, `src-tauri/src/commands/settings.rs`) y UI
(`ui/src/composables/useLyricsSettings.ts`). Resumen: el motor ya no es una cascada fija de 16
estrategias sino un catálogo de **12 fuentes configurables por el usuario**; la prioridad y el
habilitado/deshabilitado por fuente se documentan como capa propia (§2.3); se corrige la cita de un
test que no existía (§5); los endpoints citados se contrastan con el código vigente y las
discrepancias quedan anotadas (§4); la evidencia de auditoría por red se conserva como **medición
heredada, no re-verificada**. Nota de trazabilidad: este documento se llamaba
LYRICS_16_PROVIDER_MATRIX.md; el «16» del título antiguo describía una arquitectura que ya no
existe (§6).

## 1. Alcance

Este documento describe el motor de letras del núcleo Rust (`src-tauri/src/download/lyrics.rs`).
El prototipo experimental que la auditoría original citaba junto al motor no está en este
repositorio: vivía bajo `workspace/`, que `.gitignore` excluye, de modo que ningún checkout ni
ningún job de CI lo ha contenido nunca; ese directorio no existe hoy en el árbol y nada aquí debe
leerse como describiéndolo. Tampoco existe la ruta legacy/syncify-cli que mencionaron documentos
tempranos (§6).

La unidad del motor ya no es la «estrategia numerada» de la versión anterior sino la **fuente**:
una variante identificable por `provider_id`, con un fetcher concreto, que el usuario puede
activar/desactivar y reordenar. La fuente de verdad del catálogo es la constante
`LYRICS_SOURCE_IDS` (12 entradas) y `fetch_source` despacha exactamente 12 brazos: no hay fetchers
huérfanos ni ids sin brazo. Las 12 fuentes cubren 10 redes: Apple Music, Spotify, Musixmatch,
UltraStar USDB, Kugou, QQ Music, NetEase, LyricsPlus, LRCLIB y Tekstowo.pl.

## 2. Arquitectura vigente

### 2.1 Catálogo y despacho

- `LYRICS_SOURCE_IDS`, en este orden: `apple_music_ttml`, `spotify_color`,
  `musixmatch_richsync`, `ultrastar`, `kugou`, `qqmusic`, `netease_synced`, `lyricsplus`,
  `lrclib_synced`, `musixmatch_plain`, `lrclib_plain`, `tekstowo_plain`.
- `LyricsSourceSetting` (`provider_id`, `enabled`, `priority`): estado por fuente que llega de la
  configuración (§2.3).
- `ordered_sources`: retiene solo las fuentes habilitadas **y** presentes en el catálogo (un id
  desconocido en los ajustes se descarta y nunca se contacta) y ordena por
  (`priority`, `provider_id`).
- `fetch_source(id, artist, track, duration)`: match de los 12 `provider_id` contra su fetcher;
  un id desconocido devuelve error.
- `fetch_all_sources`: descarga automática — recorre las fuentes ordenadas y devuelve la primera
  que la política acepta; si ninguna la acepta, error.
- `search_all_providers`: búsqueda interactiva — misma política, devuelve **todas** las aceptadas
  (líneas ordenadas por `start_time_ms`). Ambas entradas se invocan desde comandos de
  `src-tauri/src/commands/lyrics.rs`.
- `orchestrate_resolution` envuelve `fetch_all_sources` y mide tiempos. Su doc-comment dice
  «(16 tiers)»: es un comentario residual erróneo; el despacho real tiene 12 brazos.

### 2.2 Política de aceptación (`policy_accepts`)

1. `is_valid_lyrics`: rechaza placeholders (solo el título, o cabeceras 作词/作曲 sin contenido).
2. `min_sync_level` del usuario: `word`/`syllable` exige rango 3 (elrc no vacío o sync type
   `KARAOKE_WORD_SYNCED`/`WORD_SYNCED`); `line` exige rango 2 (`LINE_SYNCED` o líneas con
   contenido); cualquier otro valor acepta desde rango 1.
3. `preferred_language`: vacío, `original` o `any` aceptan cualquier idioma; otro valor exige que
   `detect_language_heuristic` detecte ese idioma.

### 2.3 Configuración de usuario (prioridad y habilitado por fuente)

La prioridad y el habilitado/deshabilitado son **ajustes de usuario**, no constantes del código:

- Tabla `lyrics_provider_settings` (creada en 0013, recatalogada en 0100): `provider_id` único,
  `enabled`, `priority`, `sync_level`.
- Tabla `lyrics_config` singleton (0014): `min_sync_level`, `preferred_language`,
  `storage_format`, `auto_fetch_on_import`, `retry_failed`, `retry_frequency`.
- `load_lyrics_engine_config` (`src-tauri/src/commands/lyrics.rs`) monta el `LyricsEngineConfig`
  desde ambas tablas: el formato se normaliza a `lrc` (CFGL-03) y el idioma vacío u «original» se
  trata como original (CFGL-02). Si no hay fila, rige `Default for LyricsEngineConfig`, que usa el
  catálogo completo habilitado con prioridades 1..12 en el orden de `LYRICS_SOURCE_IDS`.
- Persistencia por fuente: comandos en `src-tauri/src/commands/settings.rs` (listado ordenado por
  prioridad, actualización de `enabled`/`priority`, reordenación).
- UI: `ui/src/composables/useLyricsSettings.ts` (cargar ajustes, activar/desactivar fuente,
  subir/bajar prioridad, probar fuente, editar la configuración global) consumido por la vista de
  ajustes `ui/src/views/settings/SettingsLyrics.vue` (y `ui/src/views/SettingsView.vue`); los
  wrappers IPC viven en `ui/src/api/settings.ts`. Una fuente deshabilitada nunca se contacta.

### 2.4 Catálogo por defecto en la base de datos

| `provider_id` | priority | sync_level | Origen |
| :--- | :---: | :---: | :--- |
| `apple_music_ttml` | 1 | syllable | 0013 (como `apple_music`), renombrado en 0100 |
| `lrclib_synced` | 2 | line | 0013 (como `lrclib`), renombrado en 0100 |
| `netease_synced` | 3 | line | 0013 (como `netease`), renombrado en 0100 |
| — | 4 | — | hueco: era `genius`, retirado en 0100 |
| `spotify_color` | 5 | word | 0100 |
| `musixmatch_richsync` | 6 | word | 0100 |
| `ultrastar` | 7 | syllable | 0100 |
| `kugou` | 8 | word | 0100 |
| `qqmusic` | 9 | line | 0100 |
| `lyricsplus` | 10 | line | 0100 |
| `musixmatch_plain` | 11 | none | 0100 |
| `lrclib_plain` | 12 | none | 0100 |
| `tekstowo_plain` | 13 | none | 0100 |

La migración 0100 preserva las decisiones del usuario en las filas existentes (los renombres son
`UPDATE`, no borrados) y declara su criterio en la cabecera: «retire Genius (no fetcher)».

## 3. Las 12 fuentes

Mecanismos verificados en el código vigente (2026-10-25):

| Fuente (`provider_id`) | Fetcher | Red | Mecanismo | Credencial | Entrega |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `apple_music_ttml` | `fetch_apple_music_ttml` | Apple Music | búsqueda por storefront (`gb`, `us`, `de`, `fr`, `mx`) en `https://amp-api.music.apple.com/v1/catalog/{sf}/search` (`types=songs`) y letra en `/v1/catalog/{sf}/songs/{id}/lyrics`; TTML → elrc (`parse_ttml_to_elrc`) | token Bearer WebPlayKid extraído dinámicamente (`extract_apple_music_token`, caché de sesión centralizada en `src-tauri/src/services/animated_cover.rs`) | `KARAOKE_WORD_SYNCED` + elrc |
| `spotify_color` | `fetch_spotify_lyrics` | Spotify | cookie sp_dc (variable de entorno `SPOTIFY_SP_DC`, leída al construir el cliente) → token web player vía `open.spotify.com` → `https://spclient.wg.spotify.com/color-lyrics/v2/track/{id}?format=json&market=from_token` | `SPOTIFY_SP_DC` | `KARAOKE_WORD_SYNCED` si hay sílabas; si no `LINE_SYNCED`; elrc si sílabas |
| `musixmatch_richsync` | `fetch_musixmatch_richsync` | Musixmatch | usertoken desktop auto (`token.get`, caché de 10 min) + `track.richsync.get` en `apic-desktop.musixmatch.com` (por `commontrack_id`) | usertoken auto | `KARAOKE_WORD_SYNCED` + elrc |
| `ultrastar` | `fetch_ultrastar_karaoke` | UltraStar USDB | `https://usdb.animux.de/api/v1/songs` (búsqueda) + `index.php?link=gettxt` (TXT); TXT → elrc (`parse_ultrastar_to_elrc`) | ninguna | `KARAOKE_WORD_SYNCED` + elrc |
| `kugou` | `fetch_kugou_karaoke` | Kugou | `http://lyrics.kugou.com/search` + `http://lyrics.kugou.com/download` (`fmt=krc`); KRC descifrado con clave embebida + zlib | ninguna | `KARAOKE_WORD_SYNCED` + elrc |
| `qqmusic` | `fetch_qqmusic_lyrics` | QQ Music | búsqueda en `https://c.y.qq.com/soso/fcgi-bin/client_search_cp` + letra en `https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg` (por `songmid`) | ninguna | `KARAOKE_WORD_SYNCED` si karaoke; si no `LINE_SYNCED`; elrc si karaoke |
| `netease_synced` | `fetch_netease_lyrics` | NetEase | búsqueda en `https://music.163.com/api/search/get` + **una sola** llamada a `https://music.163.com/api/song/lyric?id={}&lv=-1&kv=-1&tv=-1`; prefiere klyric y si no lrc; karaoke si la letra trae marcaje `<mm:ss.xx>` | ninguna (Referer `music.163.com`) | `KARAOKE_WORD_SYNCED` si karaoke; si no `LINE_SYNCED`; elrc si karaoke |
| `lyricsplus` | `fetch_lyricsplus` | LyricsPlus | `https://lyricsplus-api.vercel.app/v1/search?q=`; exige ≥ 4 líneas | ninguna | `KARAOKE_WORD_SYNCED` si karaoke; si no `LINE_SYNCED`; elrc si karaoke |
| `lrclib_synced` | `search_lyrics` | LRCLIB | **una sola búsqueda puntuada** en `https://lrclib.net/api/search?q=` (rate-limit `LRCLIB_LIMITER`, definido en `src-tauri/src/download/http_client.rs`): +100 sincronizada, +30 título exacto, duración (±3 s fuerte, ±8 s débil, fuera de tolerancia descartada), +60 idioma preferido; elrc si la letra sincronizada trae marcaje por palabra | ninguna | `KARAOKE_WORD_SYNCED` / `LINE_SYNCED` (o `UNSYNCED` si solo hay texto) |
| `musixmatch_plain` | `fetch_musixmatch_plain` | Musixmatch | `track.lyrics.get` en `apic-desktop.musixmatch.com`; una llamada | usertoken auto | `UNSYNCED` |
| `lrclib_plain` | `fetch_lrclib_plain` | LRCLIB | `https://lrclib.net/api/get?artist_name={}&track_name={}`; una llamada | ninguna | `UNSYNCED` |
| `tekstowo_plain` | `fetch_tekstowo_plain` | Tekstowo.pl | `https://www.tekstowo.pl/szukaj,wykonawca,{},tytul,{}.html` + página de la canción (scraping) | ninguna | `UNSYNCED` |

Notas de despacho: las «estrategias dobles» de la versión anterior ya no son pasos separados.
`netease_synced`, `lyricsplus` y `qqmusic` deciden palabra/línea dentro de una sola respuesta;
`lrclib_synced` es una búsqueda puntuada (no «exact + simplified»); `musixmatch_plain` y
`lrclib_plain` son una llamada cada uno.

## 4. Evidencia de auditoría por red (medición heredada, no re-verificada)

Estados heredados de la revisión anterior de este documento (última edición versionada: commit
f2bfb8a, 2026-10-05). **No** se han re-verificado en la revisión de 2026-10-25: la suite offline
del repositorio no toca la red, de modo que estos valores son una fotografía de aquella medición,
no una garantía actual. Vocabulario heredado: `available` (endpoint público devolviendo letras),
`requires_auth` (necesita credencial del usuario), `source_unavailable` (endpoint caído o
limitado); `parser_tested`/`fixture_tested` (test offline de parseo) y
`network_tested`/`untested_network` (endpoint ejercitado o no en pruebas con red).

| Fuente (catálogo actual) | Estado heredado | Red heredada | Filas de la matriz antigua que absorbe |
| :--- | :--- | :--- | :--- |
| `apple_music_ttml` | `available` | `network_tested` | 1 |
| `spotify_color` | `requires_auth` (cookie sp_dc) | `untested_network`, `fixture_tested` | 2 |
| `musixmatch_richsync` | `available` | `network_tested` | 3 |
| `ultrastar` | `available` | `network_tested` | 4 |
| `kugou` | `available` | `network_tested` | 5 |
| `qqmusic` | `available` | `network_tested` | 6 |
| `netease_synced` | `available` | `network_tested` | 7 (klyric) y 11 (línea) |
| `lyricsplus` | `available` | `network_tested` | 8 (palabra) y 8b (línea) |
| `lrclib_synced` | `available` | `network_tested` | 9 y 10 («exact»/«simplified») |
| `musixmatch_plain` | `available` | `network_tested` | 12 y 14 («exact»/«simplified») |
| `lrclib_plain` | `available` | `network_tested` | 13 y 15 («exact»/«simplified») |
| `tekstowo_plain` | `available` | `network_tested` | 16 |

Correcciones que esta revisión introduce sobre esa medición (contraste con el código vigente):

- **UltraStar**: el mecanismo real es `usdb.animux.de/api/v1/songs` + `index.php?link=gettxt`; la
  fila antigua citaba `/api/v1/txt`.
- **Kugou**: el mecanismo real es `lyrics.kugou.com` (search/download); la fila antigua citaba
  `krcs.kugou.com`.
- **NetEase**: los parámetros reales del endpoint de letra son `lv=-1&kv=-1&tv=-1`; la fila
  antigua citaba `os=pc`.
- **Apple Music**: el endpoint de letra real es `/v1/catalog/{sf}/songs/{id}/lyrics`; la fila
  antigua citaba `syllable-lyrics`.
- **LRCLIB synced**: no hay variante «simplified»: un brazo (`lrclib_synced`), una búsqueda
  puntuada, una llamada.

### 4.1 Caso de validación con descarga real (heredado)

Prueba de descarga real contra Qobuz registrada en la revisión anterior:

- **Pista**: Gloria Gaynor — I Will Survive
- **Fuente que respondió**: NetEase (hoy `netease_synced`), `fetch_netease_lyrics`
- **Formato**: `LINE_SYNCED` (53 líneas)
- **Fichero**: `downloads_real_test\Gloria Gaynor\[1978] Love Tracks\05 - I Will Survive.flac`
  (346,55 MB, FLAC 192 kHz/24 bit, verificado con `ffprobe`)
- **Carátula**: bloque `CoverFront`, JPEG 600x600 embebido en el flujo FLAC
- **Layout**: almacenado plano según `LibraryLayout`, sin subdirectorios artificiales
  (`raw`, `enriched`, `processed`, `final`)

Esta validación puntual demuestra la ejecución end-to-end de esa fuente en el pipeline vivo en su
momento; **no** es prueba de disponibilidad de red de las 12 fuentes.

## 5. Preservación de Enhanced LRC (sección corregida)

Cuando `elrc_content` está presente (marcaje por palabra tipo `<00:10.00>I <00:10.50>wish
<00:11.00>you`), el motor impone preservación estricta:

1. **Sidecar `.lrc`**: se escribe junto al audio con el contenido exacto
   (`resolution.generate_sidecar_lrc()` en `src-tauri/src/commands/lyrics.rs`; un fallo de
   escritura se registra con `tracing::warn!`, no aborta).
2. **FLAC VorbisComment `LYRICS`**: si el contenido es LRC (formato `lrc` o contiene `[`), la
   cadena Enhanced LRC se escribe literal (vía `metaflac`).
3. **FLAC VorbisComment `UNSYNCEDLYRICS`**: texto plano derivado con `strip_lrc_timestamps` y,
   encima, `strip_lyrics_header` (regla LY-02: la cabecera de créditos se limpia solo de la copia
   plana; `LYRICS` queda verbatim). Nunca se escribe un `UNSYNCEDLYRICS` vacío: en la rama LRC la
   etiqueta se omite si el limpiador no deja nada; en la rama de texto plano se conserva el
   contenido original como último recurso.
4. **Verificación por relectura**: tras escribir las etiquetas, el fichero se relee para
   confirmarlas.
5. **Test real**: `test_enhanced_lrc_word_synced_preservation_strict`
   (`src-tauri/src/download/lyrics.rs`): una respuesta con elrc conserva
   `sync_type = KaraokeWordSynced` y el contenido sincronizado literal. Lo complementan
   `test_cascade_priority_karaoke_over_linesynced` y `test_cascade_priority_linesynced_over_plain`
   (el orden karaoke > línea > plano que aplica la política de aceptación).

Cita errónea corregida: la revisión anterior citaba
`test_enhanced_lrc_word_level_degradation_negative`, que no existe en el árbol (0 coincidencias).
`strip_lrc_timestamps` está definida en `crates/syncify-lyrics-domain/src/lib.rs` (con una copia en
`crates/syncify-flac-writer/src/lib.rs`); `src-tauri/src/download/lyrics.rs` la re-exporta junto
al resto del contrato de dominio.

## 6. Lo que ya no existe

- **La cascada fija de 16 estrategias.** No hay lista de 16: hay un catálogo de 12 fuentes
  (`LYRICS_SOURCE_IDS`) y un despacho de 12 brazos. Las 16 filas antiguas se contraen así: las
  variantes palabra/línea de NetEase (7 y 11), LyricsPlus (8 y 8b) y LRCLIB synced (9 y 10), y las
  variantes «exact/simplified» de Musixmatch plain (12 y 14) y LRCLIB plain (13 y 15) son hoy una
  fuente cada una; las filas 1-6 y 16 se corresponden una a una. El comentario «(16 tiers)» del
  doc-comment de `orchestrate_resolution` es residual y erróneo frente a los 12 brazos.
- **Genius.** Retirado explícitamente en `src-tauri/migrations/0100_lyrics_provider_catalog.sql`
  («retire Genius (no fetcher)»): sin brazo en `fetch_source`, sin id en `LYRICS_SOURCE_IDS`,
  `DELETE` de su fila y hueco en la prioridad 4 del catálogo por defecto.
- **La ruta del prototipo `workspace/`.** La revisión anterior ya advertía que el prototipo citado
  vivía bajo `workspace/`, excluido por `.gitignore` y jamás versionado. El directorio no existe
  hoy en el árbol.
- **Rutas tipo legacy/syncify-cli.** No existe ningún directorio `legacy/` ni fichero versionado
  con esa ruta (0 coincidencias en el índice de git). Cualquier fuente externa que la cite está
  desactualizada.
- **El test `test_enhanced_lrc_word_level_degradation_negative`.** No existe; el test real es el
  citado en §5.

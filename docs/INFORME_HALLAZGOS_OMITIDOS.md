# Hallazgos verificados que NO se implementaron

Documento para el propietario. No es una lista de pendientes: es el registro de
lo que se dejó intacto y **por qué**, con el coste de hacerlo mal medido o
razonado. Todo lo que aquí aparece fue marcado CONFIRMADO por la verificación y
descartado deliberadamente por las reglas del encargo (REFUTADO, o cambio de
comportamiento observable).

Nada de esto está en el diff. No hay commit.

---

## 1. Cambios de comportamiento observable — requieren decisión tuya

### 1.1 #14 — Enrutar el tráfico por `execute_with_retry` (`http_client.rs:192`)

**Qué hace hoy:** `execute_with_retry` tiene **0 call sites de producción**
(`grep -rn execute_with_retry src-tauri` → sólo la definición y sus tests). El
código lo admite en `http_client.rs:25-26`.

**Por qué NO lo implementé (corrección al plan):** el plan lo marcaba riesgo
"alto" por el motivo *"toda la resiliencia del repo no aplica al tráfico
real"*. **Eso es falso.** El criterio de reintento **sí** está en producción, por
otra vía: `http_retry::is_transient_status` tiene **10 usos de producción**
(`tidal.rs:498,1033,1344,1515,1612,1685,2496`, `qobuz.rs:1565`,
`soundcloud.rs:448`, `apple_music.rs:367`, `deezer.rs:1061`,
`spotify.rs:429`) y `docs/MATRIZ_PARIDAD_IMPORTACION.md:53` documenta la decisión:
*"un único `ErrorTaxonomy` (`services/http_retry.rs`) en vez de tres criterios
escritos a mano"*.

Es decir: **criterio compartido, aplicado por cada cliente** — una decisión
deliberada y documentada, no código muerto. Enrutar todo por `execute_with_retry`
sería un cambio de arquitectura, no una corrección de bug.

**Lo que sí es higiene:** hay **dos** funciones realmente muertas —
`execute_with_retry` y `download_stream_to_file` (0 call sites de producción).
Borrarlas es de bajo riesgo. **No lo hice**: borrarlas es una decisión tuya, y
`network_resilience_test.rs` las ejercita (21 tests), así que habría que
reescribir esos tests.

**Trade-off si lo quieres igual:** un endpoint que hoy falla al primer 503 pasa a
reintentarse → **más tráfico saliente** con servicios ya al límite de cuota, y
latencia añadida donde hoy no la hay (los timeouts de 30 s en `spotify.rs:461` y
`qobuz.rs:482` se multiplican por los intentos). Empezar por **un** endpoint,
verde en cada commit, y **medir** la tasa de error por operación antes/después:
un test verde no demuestra que no se degradó el cuota global.

---

### 1.2 #16 — Endurecer el matching de pistas

Cuatro sitios confirmados:

| Sitio | Problema |
|---|---|
| `track_matcher.rs:46-49` | `(Some,Some) => tolerancia; _ => true` — si falta `duration`, **matchea** |
| `crates/syncify-core-domain/src/metadata.rs:563-567` | `title_matches` por **contención pura**: `"Love"` ⊂ `"Love Story"` → match |
| `metadata.rs:569-573` | `artist_matches` idéntico, sin normalizar separadores |
| `crates/syncify-tidal-downloader/src/lib.rs:1190-1195` | `candidate_tracks.first()` **sin gate**, tras un `info!` de log |

**Por qué NO:** endurecer cambia **qué pistas se funden**. El riesgo real es el
contrario del que citaba el plan: hoy `"Love"` y `"Love Story"` **sí se funden**.
Apretar reduce fusiones erróneas pero **introduce** la California's, y con
proveedores sin `duration` hoy se fusionan *todas* las coincidencias de título.

**Hallazgo extra que la verificación no cerró:** `src-tauri/src/download/qobuz.rs:2203-2241`
tiene un **segundo** par `title_matches`/`artist_matches`, y su `artist_matches`
además parte por separadores. **Dos implementaciones divergentes de la misma
comparación.** Eso sí es deuda real y es lo primero que unificaría.

**Si lo quieres:** `lib.rs:1189` debería quedar como
`provenance: "fallback_first_candidate"` para que el usuario distinga un match
real de un primero-de-la-lista. Verificar **campo a campo**, no "no falla".

---

### 1.3 #13 — Rate limiter con dimensión de cuenta

**Hecho confirmado:** la clave es `String` de servicio
(`rate_limiter.rs:146-148`, `HashMap<String, BucketState>`) y `accounts`
permite N cuentas por servicio (`migrations/0002_normalize_schema.sql:37-47`,
`UNIQUE(service_id, email)`).

**Por qué NO — y por qué el plan se equivocaba de signo:** el plan argumenta que
"2+ cuentas se congelan mutuamente" y que separarlas "cambia el ritmo global ⇒
más 429". Falso: `penalize_on_rate_limit` (`rate_limiter.rs:361-362`) dice
literalmente *"so every other caller of the shared limiter slows down with it —
not just the request that happened to be rejected"*, y
`test_penalize_on_rate_limit_suspends_the_shared_limiter`
(`rate_limiter.rs:460`, hoy verde) fija ese comportamiento **por diseño**.

Congelar a las demás cuentas tras un 429 **es correcto**: el 429 es del
servicio, no de la cuenta. Separar por cuenta **empeoraría** el problema de
cuota. Además la app es local, un usuario, un proceso: no compra nada medible.

**Test que rompería si lo implementas:** `rate_limiter.rs:460`. Que rompa
confirma diseño intencional.

---

### 1.4 #18 — Ventana de scroll en `MetadataView` / `LyricsView`

**Cifras confirmadas** (conteo estático de plantilla, no navegador real):
`MetadataView.vue:145-205` son **19 elementos por fila**
(7 div, 4 span, 2 p, 2 circle, 1 input, 1 img, 1 svg, 1 button).
500 filas × 19 ≈ **9 500** nodos (`MetadataView.vue:1464` limita a 500).
5 000 × 19 ≈ **95 000**.

**Matiz que el plan no dice:** son **dos measurements distintas, no uno
escalado**. 9 500 es MetadataView@500; los ~95 000 mezclan `LyricsView.vue:1495`
(limita a 5000) con la densidad de MetadataView. Vistas distintas, dominios
distintos.

**Por qué NO:** cambiar el límite **amplía lo que el usuario ve**. Hoy con >500
pistas no ve nada más; mañana sí. Es cambio de alcance, no de bug. Y la
ventanizada mueve `scrollTop`: hay que fijar altura total o la lista salta.

---

### 1.5 #3 — `Ctrl+H` abre dos modales

**Bug confirmado empíricamente** (no por lectura): montando ambos componentes
como en `App.vue:373-374` y despachando un `keydown` real →
`showShortcutsHelp = true` y `HelpPanel emitted = [[true]]`. Un keypress, dos
overlays. Registrado en `KeyboardShortcuts.vue:277` y `HelpPanel.vue:792-793`.

**Por qué NO:** elegir cuál gana es **decisión de producto**, y el plan la
trató como refactor. Hoy el modal de shortcuts tapa el panel de ayuda
(`z-[150]` vs `z-[200]`), así que el usuario nunca ve lo que
`KeyboardShortcuts.vue:277` declara.

**Corrección al plan (su riesgo estaba mal medido):** el plan decía que
`KeyboardShortcutsConsolidated.spec.ts`, `KeyboardShortcutsPrint.spec.ts` y
`HelpPanel.test.ts` "afirman el comportamiento actual y fallarían". **Falso**:
grep de `'h'`, `"h"`, `Ctrl+H` en `ui/src/__tests__/` y
`src/components/__tests__/` → **cero coincidencias**. Las teclas assertadas son
`'?'`, `'/'`, `'k'`, `'f'`, `'r'`, `','`, `Escape` — no `h`. **Ningún test
rompe**; el riesgo real es bajo, no medio.

**Lo que sí hay que arreglar sí o sí (independiente del perdedor):**
`KeyboardShortcuts.vue:274,277` registra bindings sin `when:`, y `Ctrl+?` se
dispara escribiendo en un `<input>`. Eso no lo verifiqué de nuevo — es un
SOSPECHA del barrido frontend, sin re-ejecución.

**Detalle que el plan se equivocó al revés:** `:1468-1694` urge comparar
`Ctrl+/` y `Ctrl+?` y concluye que "un test verde prueba el shortcut
equivocado". Ambos están registrados y son funcionalmente **idénticos**
(`registerShortcut('?')` y `registerShortcut('Ctrl+?')` abren el mismo panel).
No es un error.

---

### 1.6 #12 — Cachear `resolve_tool`

`cmd_utils.rs:73-83`, `apply_bundled_tool_env` ×4 por comando. **No verificado**
(no medí los ~8 syscalls). Es Higiene, pero cachear **sí** cambia
comportamiento: si el usuario cambia `FFMPEG_PATH` en caliente, el valor cacheado
gana. Requiere decisión explícita, no un refactor.

---

### 1.7 #17 — MusicBrainz en lotes

**REFUTADO.** `library.rs:1013` itera `tracks.chunks(BATCH_SIZE)` con
`BATCH_SIZE = 20` (`:970`), duerme 1100 ms **entre lotes** (`:1029-1032`, que
respeta el rate limit de MusicBrainz) y llama `batch_lookup_by_isrc` (`:1038`)
con los 20 ISRCs del lote. **Ya está en lotes.** El "1-a-1" citado
(`enrichment.rs:1487-1496`) es el motor por pista con *fallback a texto* para
pistas sin ISRC — un problema distinto que el batch no cubre. El plan
presentaba lo existente como lo que faltaba.

---

### 1.8 #4 — Reconciliación de arranque con `SyncWriterLock`

**REFUTADO como trampa.** El plan dice que la reconciliación "destruye
descargas en vuelo de otra instancia". **No puede**: el filtro es de estado y
tiempo — `operation_recovery.rs:1374-1378` sólo toca `status = 'downloading'`
con `started_at`/`created_at` de **más de 1 hora** (doc en `:1348`: "stuck in
'downloading' for more than 1 hour (TASK-84)"). Una descarga en vuelo de una
instancia viva no llega a 1 h en el momento del arranque.

Añadir el lock **no protege contra nada real** e introduce un fallo nuevo que el
propio plan admite: hacer que el arranque espere y falle donde hoy el borrado es
best-effort (`let _ = std::fs::remove_file`, `:1392`, `:1409`) y nunca bloquea.

---

## 2. Higiene confirmada, no implementada

Sin cambio de comportamiento, pero cada una necesita una decisión tuya y/o un
test propio. No las toqué para mantener el diff acotado a lo que estaba
verificado con test rojo/verde.

| # | Hallazgo | Dónde | Nota |
|---|---|---|---|
| 7 | `operation_journal` sin podar | `migrations/0059:4`, `tidal_pipeline.rs:737` | MB/año siguen siendo **extrapolación**: nadie midió la tasa real de escrituras |
| 8 | 3 `loadAlbum()` por sync | `AlbumDetailView.vue:296-307` vs `progress.rs:14-25` | Riesgo bajo; merece medición antes de tocar |
| 9 | `is_valid_audio_file` lee el archivo entero para mirar 4 bytes | `operation_recovery.rs:1165-1166` | Bloquea el runtime en una ruta async |
| 10 | N+1 en `reconcile_canonical_download_records` | `operation_recovery.rs:1536-1556` | 2n round-trips, `folder_settings` releído n veces |
| 11/#19 | 3 `#[allow(dead_code)]` + `track_matcher.rs` muerto en producción | `enrichment.rs:89,106,146`; `track_matcher.rs:11-13` | **#11 y #19 son el mismo hecho contado dos veces.** El comentario **no miente**: documenta exactamente que sólo lo ejercitan tests |
| 20 | `MB_QUERY_CACHE` sin expulsión | `musicbrainz.rs:223-224, 345-348` | `static` sin `MAX_`/`clear`. No verifiqué el tamaño de entrada |

---

## 3. Lo que este encargo NO midió

- **No ejecuté la suite Rust completa** como puerta de este diff más allá de
  `--lib` + las suites tocadas (ver el reporte principal).
- **La medición de #1 es con el CLI de `sqlite3`**, no con el pool `sqlx` de la
  app. El SQL es idéntico y la diferencia de I/O es despreciable frente a 100 s,
  pero no es la misma ruta de ejecución.
- **No reproduje los 9 054 nodos DOM en navegador real**; es conteo estático de
  plantilla, que ignora nodos de texto y `<template>`.
- **No medí** la tasa real de escrituras a `operation_journal` (#7) ni los ~8
  syscalls de `resolve_tool` (#12).
# Download Pipeline

> **Idioma:** español. Los títulos de sección están en inglés y el cuerpo
> sigue en español; no hay traducción de este documento todavía. El índice
> que marca su estado está en `docs/README.en.md`.

**Estado de revisión:** 2026-11-14 — re-derivación contra el árbol (verificación dirigida de símbolos y comportamiento; D-04b, issue #59). Re-deriva y absorbe la re-revisión del 2026-10-25 (cadena de encolado `queue_downloads` → `commands/queue.rs::add_to_queue`; bucle del worker event-driven con `slot_available_notify`; claim atómico vía `claim_next_item`; `process_download_claimed` + `process_download_internal`; estado `cancelled`; payload de `syncify:download_progress`) y las entradas del 2026-11-06/07 (puerto de proveedores ADR-0023, invalidación de caché de streaming ADR-0024). Ajustado ahora al árbol: `worker.rs` ronda hoy las 3.400 líneas; el arranque del worker ya no es un reset simple — ejecuta en orden `sanitize_stuck_downloads` (TASK-84: descargas `downloading` de más de 1 h a `failed` y staging purgado), `reconcile_downloads_on_startup` (`services/operation_recovery.rs`), `recover_interrupted_jobs` (`download/job.rs`) para el reset restante a `queued`, `repair_unresolved_queue_sources` y la restauración de la concurrencia desde `sync_settings` (SB01: superficie canónica; la KV `dl_concurrent_downloads` queda como lectura tolerante de legado). El barrido periódico cada 60 s aplica la misma sanitización TASK-84. Sigue sin verificar en escritorio real la ejecución de descargas de punta a punta (GUI, red viva): véase README §"Verification and limitations".

**Último cambio:** 2026-11-21 — #338 (follow-up de D05/#117): el gate falla
cerrado ante FLAC/M4A ilegible (lector ausente ≠ lectura fallida), el recheck
estricto no confía en la calidad anunciada, la duración de cabecera documenta
su límite frente al truncado de cola, Deezer free queda declarado para
D11, Tidal ya no duplica el `320` y un rechazo no deja fichero promovido
sin dueño — archivos: `download/verification.rs`, `worker.rs`,
`download/quality_reconcile.rs`, `services/tidal_pipeline.rs`,
`crates/syncify-core-domain/src/quality.rs`. Previo: 2026-11-19 — L14 (issue #109): la biblioteca expone los
cinco estados de descarga y la calidad efectiva en la consulta de página,
filtra por estado y calidad, comparte la regla de estados con la cobertura de
álbum (L07) y reutiliza `add_to_queue` / `force_redownload_tracks` /
`delete_track_audio`. Previo: 2026-11-19 — D05 (issue #117): gate de verificación de entrega antes de `complete` (`download/verification.rs`), `fallback_action` aplicado por el worker, piso de bitrate por proveedor y `downloads.account_id` — archivos: `worker.rs`, `download/verification.rs`, `download/quality_reconcile.rs`, `download/provider_gateway.rs`, `download/progress.rs`, `download/orchestrator.rs`, `crates/syncify-core-domain/src/quality.rs`. Previo: 2026-11-14 — re-derivación completa contra el árbol (issue #59) — archivos: `worker.rs`, `download/job.rs`, `services/operation_recovery.rs`. Previo: 2026-11-07 — P6 Fase C (ADR-0024): la descarga completada invalida la caché de streaming de la pista — archivos: `worker.rs`, `services/stream_cache.rs`. Previo: 2026-11-06 — P5: el despacho de proveedores pasa por el puerto `download/provider_gateway.rs` (ADR-0023) — archivos: `download/orchestrator.rs`, `download/provider_gateway.rs`, `download/providers/*`, `download/quality_reconcile.rs`, `worker.rs`. Previo: 2026-10-25 — re-revisión de comportamiento contra el árbol. Previo: 2026-08-25 — eliminación del módulo legacy `downloader.rs` (QBDLX/streamrip) y del bundle `resources/qbdlx-mod/`. Previo a ese: S40 — 2026-03-30.
**Leer antes de modificar:** `worker.rs`, `commands/queue.rs`, `commands/download.rs`, `services/stream_cache.rs`
**Archivos core:**
- `src-tauri/src/worker.rs` (~3.400 líneas — bucle del worker, claim atómico, eventos de progreso, supervisor auxiliar)
- `src-tauri/src/download/mod.rs` (module index)
- `src-tauri/src/download/orchestrator.rs` (DownloadOrchestrator)
- `src-tauri/src/download/provider_gateway.rs` (puerto `DownloadProvider`, registro, `ProviderRequest`/`ProviderDelivery`/`ProviderError`)
- `src-tauri/src/download/qobuz.rs` (credential-free Qobuz download)
- `src-tauri/src/download/tidal.rs` (credential-free Tidal download)
- `src-tauri/src/download/providers/` (adaptadores por proveedor: llamada al motor, normalización, provenance y reconciliación de calidad)
- `src-tauri/src/download/quality_reconcile.rs` (reconciliación de calidad física compartida)
- `src-tauri/src/download/amazon.rs` (Amazon Music download)
- `src-tauri/src/download/http_client.rs` (shared HTTP client)
- `src-tauri/src/download/progress.rs` (DownloadRequest, progress types)
- `src-tauri/src/download/lyrics.rs` (LyricsClient for embed)
- `src-tauri/src/download/songlink.rs` (Songlink/Odesli cross-service)
- `src-tauri/src/download/identity.rs` (reglas de acuerdo de identidad título/duración compartidas por preflight y verificación en descarga)
- `src-tauri/src/download/resolver.rs` (resolve_download_plan: una única decisión de pista/edición, servicio, cuenta y calidad)
- `src-tauri/src/download/audio_inspector.rs` (métricas reales del fichero descargado: metaflac/mp4ameta/cabeceras)
- `src-tauri/src/download/verification.rs` (D05: gate de verificación de entrega — duración, tags, portada — antes de `complete`)

---

## Flujo Completo

```
┌─────────────────────────────────────────────────────────┐
│  FRONTEND: invoke("queue_downloads", { track_ids })     │
│  → commands/download.rs::queue_downloads                │
│  → commands/queue.rs::add_to_queue                      │
│    → perform_add_to_queue: INSERT INTO download_queue   │
│      status='queued', priority, position, created_at    │
│      (+ identidad de fuente: service_*, target_*)       │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│  BACKGROUND WORKER (worker.rs — spawn supervisado en    │
│  el setup de main.rs)                                   │
│                                                         │
│  1. On startup, en orden:                               │
│     a. sanitize_stuck_downloads (TASK-84): las filas    │
│        'downloading' de más de 1 h pasan a 'failed'     │
│        y se purga su staging                            │
│     b. reconcile_downloads_on_startup                   │
│        (services/operation_recovery.rs)                 │
│     c. recover_interrupted_jobs (download/job.rs):      │
│        el resto de 'downloading' → 'queued'             │
│     d. repair_unresolved_queue_sources                  │
│     e. Concurrencia restaurada de sync_settings (SB01)  │
│  2. Loop:                                               │
│     a. Cada 60 s: sanitize_stuck_downloads (TASK-84,    │
│        >1 h → 'failed')                                 │
│     b. Check stopped flag → break                       │
│     c. Wait if paused (Notify-based)                    │
│     d. Si no hay hueco de concurrencia: espera en       │
│        slot_available_notify.notified() (fallback 50 ms)│
│     e. claim_next_item(): SELECT WHERE status='queued'  │
│        ORDER BY priority DESC, position ASC,            │
│        created_at ASC + UPDATE atómico a 'downloading'  │
│        (WHERE id=? AND status='queued')                 │
│     f. process_download_claimed(...) en tokio::spawn    │
│     g. Si no hay items: espera en slot_available_notify │
│        (fallback 500 ms) — sin sondeo con sleep fijo    │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│  process_download_claimed() → process_download_internal(): │
│  1. increment_active() (en el bucle, antes del spawn);  │
│     el decremento lo hace ActiveDownloadGuard al caer   │
│  2. Emit DownloadProgressEvent { status: "started" }    │
│  3. mark_downloading(queue_id)                          │
│  4. Fetch TrackMeta from DB (title, isrc, album, etc.)  │
│  5. resolve_download_output_dir() + preferencias de     │
│     embed/artefacto; build DownloadRequest              │
│  6. DownloadOrchestrator::download_track(request)       │
│  7. On success: mark_complete (con métricas reales de   │
│     download/audio_inspector.rs; si el ledger quedó     │
│     escrito, invalida la caché de streaming de la pista │
│     vía stream_cache::invalidate_track, ADR-0024),      │
│     emit "complete"                                     │
│  8. On failure: mark_failed, emit "failed"              │
└─────────────────────────────────────────────────────────┘
```

### Queue States

| Status        | Meaning                                  | Transition          |
|---------------|------------------------------------------|---------------------|
| `queued`      | Waiting to be picked up by worker        | → downloading       |
| `downloading` | Active download in progress              | → complete / failed |
| `complete`    | Successfully downloaded                  | terminal            |
| `failed`      | Download failed, retry_count incremented | → queued (manual)   |
| `cancelled`   | Cancelled by user                        | terminal            |

El CHECK de la tabla (`migrations/0004_production.sql`) admite exactamente estos
cinco valores. `cancel_download` / `cancel_queue_item` (`commands/queue.rs`)
mueven una fila `queued` o `downloading` a `cancelled`, y `get_queue_stats` lo
cuenta como estado aparte.

### Worker State (DownloadWorkerState)

```rust
struct DownloadWorkerState {
    paused: Arc<AtomicBool>,          // pause/resume control
    stopped: Arc<AtomicBool>,         // permanent stop
    active_count: Arc<AtomicUsize>,   // current active downloads
    max_concurrent: Arc<AtomicUsize>, // default: 2
    unpause_notify: Arc<Notify>,      // wake waiters on resume
    slot_available_notify: Arc<Notify>, // wake loop on freed slot or new item
}
```

Methods: `pause()`, `resume()`, `stop()`, `wait_if_paused()`, `notify_available()`,
`active_downloads()`, `increment_active()` / `decrement_active()`,
`set_max_concurrent()`, `is_paused()` / `is_stopped()`.

### Estado en la biblioteca (L14 — issue #109)

La biblioteca no ejecuta descargas ni decide calidad: lee el estado de la cola
y del ledger. `commands/library_query.rs` proyecta en la MISMA consulta de
página los cinco estados del badge —`downloaded` si `downloads.file_path`
existe; si no, `downloading`, `queued` o `failed` según la fila de
`download_queue`; `not_downloaded` en el resto— y la calidad efectiva del
fichero (`FLAC 24/96`). `library_albums.rs` agrega esos mismos estados para la
cobertura del álbum con la expresión compartida; no hay una regla duplicada
ni una consulta por pista.

Las acciones reutilizan la única cola: encolar llama a `add_to_queue` (con su
guardrail de deduplicación) y re-descargar a `force_redownload_tracks` (borra
el Download y las filas de cola de esa pista y la reencola por
`perform_add_to_queue`). «Borrar audio» llama a `delete_track_audio` (L04):
elimina el Download, sus entradas de cola y el fichero, y conserva Track,
TrackSource, pertenencia y evidencia de edición.

### Download Sources (pipeline Rust-nativo: `download/orchestrator.rs`)

| Service | Implementación | Método |
|---------|----------------|--------|
| Qobuz | `download/qobuz.rs` (`QobuzDownloader`) | API oficial de Qobuz con firma de requests (`build_request_signature`), resolución de token y proxy fallback — HTTP nativo, sin subprocess ni binarios externos |
| Tidal | `download/tidal.rs` (re-export del crate externo `syncify-tidal-downloader`) | Cliente HTTP nativo con progreso |
| Amazon | `download/amazon.rs` (`AmazonDownloader`) | Vía servicio DoubleDouble |

El despacho de descarga pasa por el puerto de proveedores
(`download/provider_gateway.rs`, ADR-0023): el orquestador resuelve
Qobuz/Tidal/Amazon por nombre y cada adaptador (`download/providers/*.rs`)
encapsula la llamada al motor, la normalización de provenance y la
reconciliación de calidad física. La política de fallback, auth, progreso y
SongLink sigue en el orquestador; SongLink sigue siendo un resolvedor, no un
proveedor.

El worker instancia `crate::download::DownloadOrchestrator` dentro de
`process_download_internal` (`worker.rs`). El directorio de salida lo resuelve
`resolve_download_output_dir()` (método de `DownloadWorker` en `worker.rs`):
`folder_settings.base_folder` → `settings.dl_download_path|download_path` →
fallback `dirs::audio_dir()/Syncify`. Único proceso externo en `download/`:
`ffmpeg` del sistema, usado por `download/lyrics.rs` para conversión (no forma
parte de las descargas).

La decisión de pista/edición, servicio, cuenta y calidad se calcula una sola vez
en `download/resolver.rs` (`resolve_download_plan`), y las reglas de identidad
compartidas (título + duración) viven en `download/identity.rs`; el preflight
que decide qué se puede encolar está en `commands/queue.rs`
(`evaluate_track_preflight`, `preflight_download_batch`,
`enqueue_eligible_batch`).

### Verificación de entrega antes de `complete` (D05 — issue #117)

Antes de promocionar una fila a `complete`, el worker corre el gate
`verify_delivery_before_complete` (`download/verification.rs`, ejercitado por
`tests/delivery_verification_test.rs`) sobre el fichero ya promovido y
recortado (TASK-76), a través de `verify_then_complete` (`worker.rs`), que es
la costura única gate → `mark_complete`:

- **Duración física** dentro de la tolerancia fijada de ±2 s frente a la del
  catálogo (`DURATION_TOLERANCE_SECS`); el silencio recortado por el trimmer
  amplía la tolerancia en lo recortado. Sin evidencia en alguno de los dos
  lados (MP3 sin lector de duración, catálogo sin `duration_ms`) no se compara.
- **Tags releídos** (título, artista, álbum, disco, pista) corroboran el
  catálogo: un tag ausente o distinto cuando el catálogo lo declara es fallo.
  FLAC vía metaflac, M4A vía mp4ameta; MP3 se salta hasta que aterrice el
  escritor ID3 de D16 (falta de lector no es fallo).
- **Portada válida** (datos presentes con magia JPEG/PNG) solo cuando el
  usuario pidió incrustarla (`embed_artwork`).
- **Calidad física manda**: una decisión `RejectedQuality` de la política
  compartida, o una entrega por debajo de lo pedido bajo política estricta,
  rechaza la entrega. La política estricta es `fallback_action = 'skip'` de
  Ajustes **o** el job con `allow_fallback = false`; con fallback permitido la
  entrega completa declarando la caída (`CompletedWithQualityShortfall` /
  `CompletedWithQualityFallback`, `quality_fallback_used = 1` y motivo).

Si el gate falla, la fila se marca `failed` con el prefijo
`DeliveryVerificationFailed:` y **no queda fila de esa entrega en el ledger de
`downloads`**. `mark_complete` conserva además una defensa: una decisión
`RejectedQuality` nunca se promociona a `complete`. Un rechazo después de
`checkpoint_promoted` retira el rastro de la propia entrega: algunas rutas
(Tidal, `tidal_pipeline` §9) escriben su fila de `downloads` y hacen commit
antes del gate, así que la limpieza distingue la fila de esta entrega —mismo
`track_id` que la cola— de una fila ajena que ocupa el path. La propia se
borra y el fichero se mueve a una cuarentena explícita
(`<directorio de la entrega>/.syncify-quarantine/`, con el `queue_id` en el
nombre) en vez de destruirse: el audio válido de un rechazo por portada, tags
o calidad se conserva para inspección y la biblioteca no lo indexa (tampoco lo
hace con el fichero original, que ya no está). El motivo del fallo incluye la
ruta de cuarentena. Una fila de otra pista conserva fila y fichero, y un fallo
al consultar o borrar el ledger no autoriza a tocar el fichero. El orden
gate → `mark_complete` se prueba por comportamiento con `verify_then_complete`,
la misma costura que usa el worker, no buscando cadenas en el fuente; en
producción `mark_complete` no tiene otra llamada, así que no hay ruta de
promoción que esquive el gate.

**Límites de evidencia (#338).** «Falta de lector» y «lectura fallida» son
cosas distintas: MP3 sigue sin lector hasta D16 y sus checks se saltan, pero un
FLAC/M4A no vacío que metaflac/mp4ameta no puede parsear **falla cerrado**. Bajo
política estricta, una reinspección física fallida tampoco cae a los valores
anunciados por el proveedor. La duración verificada sale de la cabecera
(STREAMINFO/mvhd): detecta un preview o un fichero equivocado con cabecera más
corta; si no hay ninguna duración física, el check se salta por falta de
evidencia y solo la política estricta rechaza la reinspección fallida; y **no**
detecta un truncado por la cola que conserva la cabecera completa; la
corroboración con longitud decodificable queda fuera porque
duplicaría un decode completo por pista. El verificador existe
(`verify_flac_integrity_stream` en `crates/syncify-flac-writer`: MD5 PCM de
STREAMINFO o decode-check) y cablearlo exige medir antes ese coste por pista,
así que el hito documenta el límite en vez de improvisarlo. Ambos lados están
fijados en `tests/delivery_verification_test.rs` y en los tests unitarios del
módulo.

**Piso de bitrate por proveedor** (matriz en
`syncify_core_domain::quality::provider_lossy_floor_kbps`, consumida por
`quality_reconcile` y, cuando aterricen, por los adaptadores D11/D12):
SoundCloud 128, Deezer 320
(128 en cuentas free), Qobuz/Tidal/Amazon 320 por su matriz. El piso global de
320 dejó de aplicarse en duro; la reconciliación y el orquestador consultan la
matriz del servicio que entrega. Un MP3 128 se declara como lo que es
(`lossy`), nunca se etiqueta lossless. Ningún adaptador repite el `320` a
mano: Tidal también resuelve su piso por la matriz. La mitad `account_free`
del piso de Deezer **sigue abierta** y queda como entregable de D11 (el
adaptador nativo de Deezer; D12 es su gemelo de SoundCloud): el registro de
`download/providers/` solo tiene Qobuz, Tidal y Amazon, así que ninguna ruta
de descarga entrega con `service = "deezer"`, y `accounts` tampoco persiste el
plan; cuando el adaptador aterrice debe pasar la capa de la cuenta en vez del
`false` fijo de `quality_reconcile`.

**Atribución de cuenta**: `DownloadResult.account_id` viaja desde
`ProviderDelivery.account_id` y el ledger escribe `downloads.account_id` con
la cuenta que entregó (fallback: `resolved_account_id` de la fila), como
evidencia de enrutado para el panel de jobs.

### Interacción con la sesión de reproducción (P6 — ADR-0024)

La reproducción prefiere el fichero local: `PlayerState::open`
(`services/playback_session.rs`) consulta `downloads` antes de streamear. Dos
consecuencias directas sobre el pipeline de descarga:

- **Invalidación de caché al completar** (corrección 4 de la Fase C): cuando
  `mark_complete` escribe el ledger de `downloads`, invoca
  `stream_cache::invalidate_track` con el app handle del worker, que borra las
  entradas de caché de streaming de la pista (fichero, `.done` y `.partial` de
  todas las calidades; el sufijo `-` del prefijo evita colisiones
  `track-1-`/`track-11-`). Así la siguiente reproducción no sirve contenido
  viejo: o manda el fichero recién descargado, o se vuelve a streamear. Si el
  ledger no llegó a escribirse, la caché se conserva (fallo blando; una
  invalidación fallida nunca falla la descarga y la LRU la recoge después).
- **Parciales reanudables y re-resolución única en streaming** (correcciones 2
  y 3 de la Fase C): el streaming progresivo reanuda por HTTP Range desde el
  byte recibido (un servidor sin soporte de Range degrada a descarga desde
  cero) y DASH trunca a la frontera del último segmento completo y continúa en
  el siguiente (marcador `.partial`); la cabecera FLAC se valida antes de dar
  un parcial por bueno. Un fallo de catálogo 401/403/410 re-resuelve UNA vez
  (ADR-0010.enm.4). La reanudación de parciales de la propia cola de descargas
  (staging de `download_queue`) queda fuera del alcance (deuda declarada en
  ADR-0024).

### Worker Supervisor (en `src-tauri/src/main.rs`, dentro del `setup`)

- Spawns worker in `tokio::task::spawn`
- Catches panics via `JoinHandle`
- Auto-restarts up to 3 times with backoff (5s first, 30s subsequent)
- Emits `worker_restarted` / `worker_fatal` events

---

## Campos críticos en DB

| Table | Column | Notes |
|-------|--------|-------|
| `download_queue` | `id`, `track_id`, `status`, `priority` | Queue state machine |
| `download_queue` | `position` | Orden de reclamación (`priority DESC, position ASC, created_at ASC`); añadida por `migrations/0046_download_queue_resume.sql` junto a `resumable`/`staging_path` |
| `download_queue` | `started_at`, `completed_at` | Timing |
| `download_queue` | `progress_percent` | Updated during download |
| `download_queue` | `error_message`, `retry_count` | Failure tracking |
| `download_queue` | `service_id`, `service_name`, `service_track_id`, `service_album_id`, `target_title`, `target_artist`, `target_album`, `target_isrc`, `smart_studio_origin`, `allow_fallback` | Identidad de fuente bloqueada en la cola (`migrations/0050_download_queue_source_identity.sql`) |
| `download_queue` / `downloads` | `requested_quality`, `effective_quality`, `requested_format`, `effective_format`, `quality_decision`, `provider_fallback_used`, `quality_fallback_used`, `decision_reason` | Quality policy y decision tracking (`migrations/0060_quality_policy_and_decision_tracking.sql`) |
| `downloads` | `track_id`, `file_path`, `downloaded_at` | Completed download records |

---

## Eventos Tauri emitidos

| Event | Payload |
|-------|---------|
| `syncify:download_progress` | `DownloadProgressEvent` (struct en `worker.rs`): `{ queue_id, track_id, title, artist, status, progress_percent, message, bytes_downloaded, total_bytes, percent, instant_kbps, average_kbps, phase, terminal, phase_timings }` |
| `worker_restarted` | `{ restart_count, max_restarts }` |
| `worker_fatal` | `{ message, restart_count }` |

Status values: `"started"`, `"downloading"`, `"complete"`, `"failed"`

---

## Puntos de ruptura conocidos

1. **Worker restart limit**: After 3 panics, worker stops permanently until app restart. No self-healing beyond that.
2. **Interrupted downloads**: el arranque primero sanciona (TASK-84: `downloading` de más de 1 h → `failed`, staging purgado) y después resetea el resto de `downloading` a `'queued'` (`recover_interrupted_jobs`). Las descargas interrumpidas se reintentan desde cero (la reanudación de parciales de `download_queue` es deuda declarada en ADR-0024).
3. **ffmpeg en PATH**: `download/lyrics.rs` requiere `ffmpeg` del sistema para conversión de audio; no hay binario empaquetado ni fallback.

### Resueltos / eliminados (2026-08-25)

- ~~**qbdlx-mod path**~~ — ELIMINADO: la ruta `get_qbdlx_path()` → `<resource_dir>/qbdlx-mod/QobuzDownloaderX-MOD.exe` vivía solo en el módulo muerto `src-tauri/src/downloader.rs`, que fabricaba `{output}/{track_id}.flac` sin verificar el archivo. Se borró el módulo, el vendor `resources/qbdlx-mod/` (~3,1 MB de fuente C# de terceros, GPL-3.0) y la clave `"resources"` de `tauri.conf.json`. El pipeline Qobuz vivo es Rust-nativo y nunca consumió ese binario.
- ~~**streamrip (`rip`) availability**~~ — OBSOLETO: solo existía en `downloader.rs` eliminado; Tidal/Amazon usan clientes HTTP nativos.
- ~~**Output directory no configurable**~~ — RESUELTO: `resolve_download_output_dir()` (método de `DownloadWorker` en `worker.rs`) resuelve el output-dir dinámicamente desde settings.
- ~~**Dos orquestadores**~~ — RESUELTO: `downloader.rs` (DB+subprocess) eliminado; queda un único `download::DownloadOrchestrator` (`download/orchestrator.rs`).

### Legacy QBDLX (por si se quiere reintroducir)

La arquitectura histórica delegaba las descargas de Qobuz al binario Windows QobuzDownloaderX-MOD vía subprocess. Se eliminó el 2026-08-25 (commit pendiente): el módulo que la invocaba llevaba meses muerto (cero referencias `crate::downloader` / `downloader::` fuera de sí mismo) y cada instalador arrastraba 3,1 MB de fuente C# ajena que jamás se compilaba en esta máquina (no hay dotnet).

Referencias upstream pineadas para una eventual reintroducción:

- Upstream (fork MOD): https://github.com/DJDoubleD/QobuzDownloaderX-MOD
- Proyecto original (AiiR): https://github.com/ImAiiR/QobuzDownloaderX
- Librería de API aislada: https://github.com/DJDoubleD/QobuzApiSharp
- Licencia del vendor: GPL-3.0 (LICENSE en el historial git). La copia vendida NO traía versión/commit pineados (el csproj solo pinea dependencias NuGet: Newtonsoft.Json 13.0.3, QobuzApiSharp 0.0.8, TagLibSharp 2.3.0); entró al repo en el commit inicial `cf23c01`.

Para reintroducir un binario real: recuperarlo del historial (`git log -- src-tauri/resources/qbdlx-mod`), compilarlo con dotnet FUERA de este repo, publicarlo como artefacto descargable, volver a añadir `"resources"` al `bundle` de `tauri.conf.json` y apuntar la resolución de ruta al nuevo layout.

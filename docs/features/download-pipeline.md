# Download Pipeline

> **Idioma:** español. Los títulos de sección están en inglés y el cuerpo
> sigue en español; no hay traducción de este documento todavía. El índice
> que marca su estado está en `docs/README.en.md`.

**Estado de revisión:** 2026-10-25 — re-revisión contra el árbol (verificación dirigida de símbolos y comportamiento): corregida la cadena de encolado (`queue_downloads` vive en `commands/download.rs` y delega en `commands/queue.rs::add_to_queue`), el bucle del worker ya es event-driven (`slot_available_notify`, sin sondeo de 3 s), el claim es atómico vía `claim_next_item`, `process_download` hoy son `process_download_claimed` + `process_download_internal`, añadido el estado `cancelled`, payload de `syncify:download_progress` completado y citas de línea sustituidas por símbolos. Sigue sin verificar en escritorio real la ejecución de descargas de punta a punta (GUI, red viva): véase README §"Verification and limitations".

**Último cambio:** 2026-11-06 — P5: el despacho de proveedores pasa por el puerto `download/provider_gateway.rs` (ADR-0023) — archivos: `download/orchestrator.rs`, `download/provider_gateway.rs`, `download/providers/*`, `download/quality_reconcile.rs`, `worker.rs`. Previo: 2026-10-25 — re-revisión de comportamiento contra el árbol — archivos: `worker.rs`, `commands/download.rs`, `commands/queue.rs`, `main.rs`, `migrations/0004_production.sql`, `migrations/0046_download_queue_resume.sql`, `migrations/0050_download_queue_source_identity.sql`, `migrations/0060_quality_policy_and_decision_tracking.sql`. Previo: 2026-08-25 — eliminación del módulo legacy `downloader.rs` (QBDLX/streamrip) y del bundle `resources/qbdlx-mod/`. Previo a ese: S40 — 2026-03-30.
**Leer antes de modificar:** `worker.rs`, `commands/queue.rs`, `commands/download.rs`
**Archivos core:**
- `src-tauri/src/worker.rs` (3.184 líneas — bucle del worker, claim atómico, eventos de progreso, supervisor auxiliar)
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
│  1. On startup: reset status='downloading' → 'queued'   │
│  2. Loop:                                               │
│     a. Cada 60 s: barrido de descargas atascadas (>1 h) │
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
│     download/audio_inspector.rs), emit "complete"       │
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
2. **Interrupted downloads**: On startup, all `status='downloading'` rows are reset to `'queued'`. This means partial downloads get retried from scratch (no resume).
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

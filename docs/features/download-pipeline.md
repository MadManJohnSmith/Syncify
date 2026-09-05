# Download Pipeline

**Estado de revisión:** 2026-09-05 — pendiente de revalidación contra la implementación actual. Úsalo como referencia de diseño y verifica el código fuente antes de confiar en los detalles.

**Último cambio:** 2026-08-25 — eliminación del módulo legacy `downloader.rs` (QBDLX/streamrip) y del bundle `resources/qbdlx-mod/` — archivos: `main.rs`, `lib.rs`, `downloader.rs` (eliminado), `tauri.conf.json`. Previo: S40 — 2026-03-30.
**Leer antes de modificar:** `commands/queue.rs`, `commands/download.rs`
**Archivos core:**
- `src-tauri/src/worker.rs` (428 lines — background worker loop)
- `src-tauri/src/download/mod.rs` (module index)
- `src-tauri/src/download/orchestrator.rs` (DownloadOrchestrator)
- `src-tauri/src/download/qobuz.rs` (credential-free Qobuz download)
- `src-tauri/src/download/tidal.rs` (credential-free Tidal download)
- `src-tauri/src/download/amazon.rs` (Amazon Music download)
- `src-tauri/src/download/http_client.rs` (shared HTTP client)
- `src-tauri/src/download/progress.rs` (DownloadRequest, progress types)
- `src-tauri/src/download/lyrics.rs` (LyricsClient for embed)
- `src-tauri/src/download/songlink.rs` (Songlink/Odesli cross-service)

---

## Flujo Completo

```
┌─────────────────────────────────────────────────────────┐
│  FRONTEND: invoke("queue_downloads", { track_ids })     │
│  → commands/queue.rs: INSERT INTO download_queue        │
│    status='queued', priority, created_at                │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│  BACKGROUND WORKER (worker.rs — runs in main.rs setup) │
│                                                         │
│  1. On startup: reset status='downloading' → 'queued'   │
│  2. Loop:                                               │
│     a. Check stopped flag → break                       │
│     b. Wait if paused (Notify-based)                    │
│     c. Check concurrency < max_concurrent               │
│     d. get_next_item(): SELECT WHERE status='queued'    │
│        ORDER BY priority DESC, created_at ASC           │
│     e. process_download(queue_id, track_id, ...)        │
│     f. If no items → sleep 3s                           │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│  process_download():                                    │
│  1. increment_active()                                  │
│  2. Emit DownloadProgressEvent { status: "started" }    │
│  3. mark_downloading(queue_id)                          │
│  4. Fetch TrackMeta from DB (title, isrc, album, etc.) │
│  5. Build DownloadRequest with output_dir, quality      │
│  6. DownloadOrchestrator::download_track(request)       │
│  7. On success: mark_complete, emit "complete"          │
│  8. On failure: mark_failed, emit "failed"              │
│  9. decrement_active()                                  │
└─────────────────────────────────────────────────────────┘
```

### Queue States

| Status        | Meaning                                  | Transition          |
|---------------|------------------------------------------|---------------------|
| `queued`      | Waiting to be picked up by worker        | → downloading       |
| `downloading` | Active download in progress              | → complete / failed |
| `complete`    | Successfully downloaded                  | terminal            |
| `failed`      | Download failed, retry_count incremented | → queued (manual)   |

### Worker State (DownloadWorkerState)

```rust
struct DownloadWorkerState {
    paused: Arc<AtomicBool>,         // pause/resume control
    stopped: Arc<AtomicBool>,        // permanent stop
    active_count: Arc<AtomicUsize>,  // current active downloads
    max_concurrent: Arc<AtomicUsize>, // default: 2
    unpause_notify: Arc<Notify>,     // wake waiters on resume
}
```

Methods: `pause()`, `resume()`, `stop()`, `wait_if_paused()`, `status()`

### Download Sources (pipeline Rust-nativo: `download/orchestrator.rs`)

| Service | Implementación | Método |
|---------|----------------|--------|
| Qobuz | `download/qobuz.rs` (`QobuzDownloader`) | API oficial de Qobuz con firma de requests (`build_request_signature`), resolución de token y proxy fallback — HTTP nativo, sin subprocess ni binarios externos |
| Tidal | `download/tidal.rs` (re-export del crate externo `syncify-tidal-downloader`) | Cliente HTTP nativo con progreso |
| Amazon | `download/amazon.rs` (`AmazonDownloader`) | Vía servicio DoubleDouble |

El worker instancia `crate::download::DownloadOrchestrator` sobre `download/orchestrator.rs` (worker.rs:833 y :876). El directorio de salida lo resuelve `resolve_download_output_dir()` (worker.rs:592–625): `folder_settings.base_folder` → `settings.dl_download_path|download_path` → fallback `dirs::audio_dir()/Syncify`. Único proceso externo en `download/`: `ffmpeg` del sistema, usado por `download/lyrics.rs` para conversión (no forma parte de las descargas).

### Worker Supervisor (main.rs L312–387)

- Spawns worker in `tokio::task::spawn`
- Catches panics via `JoinHandle`
- Auto-restarts up to 3 times with backoff (5s first, 30s subsequent)
- Emits `worker_restarted` / `worker_fatal` events

---

## Campos críticos en DB

| Table | Column | Notes |
|-------|--------|-------|
| `download_queue` | `id`, `track_id`, `status`, `priority` | Queue state machine |
| `download_queue` | `started_at`, `completed_at` | Timing |
| `download_queue` | `progress_percent` | Updated during download |
| `download_queue` | `error_message`, `retry_count` | Failure tracking |
| `downloads` | `track_id`, `file_path`, `downloaded_at` | Completed download records |

---

## Eventos Tauri emitidos

| Event | Payload |
|-------|---------|
| `syncify:download_progress` | `{ queue_id, track_id, title, artist, status, progress_percent, message }` |
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
- ~~**Output directory no configurable**~~ — RESUELTO: `resolve_download_output_dir()` (worker.rs:592–625) resuelve el output-dir dinámicamente desde settings.
- ~~**Dos orquestadores**~~ — RESUELTO: `downloader.rs` (DB+subprocess) eliminado; queda un único `download::DownloadOrchestrator` (`download/orchestrator.rs`).

### Legacy QBDLX (por si se quiere reintroducir)

La arquitectura histórica delegaba las descargas de Qobuz al binario Windows QobuzDownloaderX-MOD vía subprocess. Se eliminó el 2026-08-25 (commit pendiente): el módulo que la invocaba llevaba meses muerto (cero referencias `crate::downloader` / `downloader::` fuera de sí mismo) y cada instalador arrastraba 3,1 MB de fuente C# ajena que jamás se compilaba en esta máquina (no hay dotnet).

Referencias upstream pineadas para una eventual reintroducción:

- Upstream (fork MOD): https://github.com/DJDoubleD/QobuzDownloaderX-MOD
- Proyecto original (AiiR): https://github.com/ImAiiR/QobuzDownloaderX
- Librería de API aislada: https://github.com/DJDoubleD/QobuzApiSharp
- Licencia del vendor: GPL-3.0 (LICENSE en el historial git). La copia vendida NO traía versión/commit pineados (el csproj solo pinea dependencias NuGet: Newtonsoft.Json 13.0.3, QobuzApiSharp 0.0.8, TagLibSharp 2.3.0); entró al repo en el commit inicial `cf23c01`.

Para reintroducir un binario real: recuperarlo del historial (`git log -- src-tauri/resources/qbdlx-mod`), compilarlo con dotnet FUERA de este repo, publicarlo como artefacto descargable, volver a añadir `"resources"` al `bundle` de `tauri.conf.json` y apuntar la resolución de ruta al nuevo layout.

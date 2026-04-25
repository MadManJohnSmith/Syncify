# Download Pipeline

**Último cambio:** S40 — 2026-03-30 — archivos: `worker.rs`, `downloader.rs`, `download/*.rs`
**Leer antes de modificar:** `commands/queue.rs`, `commands/download.rs`
**Archivos core:**
- `src-tauri/src/worker.rs` (428 lines — background worker loop)
- `src-tauri/src/downloader.rs` (361 lines — orchestrator with qbdlx/streamrip)
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

### Download Sources (downloader.rs)

| Service | Tool | Method |
|---------|------|--------|
| Qobuz | QobuzDownloaderX-MOD | subprocess `-t <id> -o <path> -q <quality>` |
| Tidal | streamrip (`rip`) | `rip url https://tidal.com/track/<id> -d <path>` |
| Deezer | streamrip (`rip`) | `rip url https://deezer.com/track/<id> -d <path>` |

Source priority: configurable via `DownloadConfig.service_priority` (default: qobuz → tidal → deezer)

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

1. **qbdlx-mod path**: `get_qbdlx_path()` expects binary at `resources/qbdlx-mod/QobuzDownloaderX-MOD.exe`. Missing binary → all Qobuz downloads fail silently.
2. **streamrip availability**: Tidal/Deezer downloads require `rip` CLI in PATH. No fallback.
3. **Worker restart limit**: After 3 panics, worker stops permanently until app restart. No self-healing beyond that.
4. **Interrupted downloads**: On startup, all `status='downloading'` rows are reset to `'queued'`. This means partial downloads get retried from scratch (no resume).
5. **Output directory**: Defaults to `dirs::audio_dir()/Syncify` or `C:\Music\Syncify`. Not user-configurable from the worker (hardcoded in `process_download`).
6. **Two orchestrator implementations**: `downloader.rs` (DownloadOrchestrator with DB+subprocess) and `download/orchestrator.rs` (DownloadOrchestrator credential-free). The worker uses `download::DownloadOrchestrator`, not `downloader.rs`.

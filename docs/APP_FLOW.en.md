# Syncify: the complete journey, from account to file

Syncify does not have an account of its own. The “account” is the session for
each service (Qobuz, Tidal, Spotify, Deezer, SoundCloud or Apple Music). The app
is a **Tauri 2** desktop application with a **Vue 3** UI, a **Rust** core,
**Python** bridges for logins, and a local **SQLite** database.

```
┌──────────────────────────── Vue 3 UI ────────────────────────────┐
│ Onboarding · Accounts · Library · Downloads · Player             │
└──────────────────────────────┬───────────────────────────────────┘
                               │ Tauri IPC (invoke / events)
┌──────────────────────────────▼───────────────────────────────────┐
│ Rust core                                                        │
│  auth → sync/import → EnrichmentEngine → SQLite (WAL)            │
│  queue → worker → resolver → provider → verify/tag               │
└───────────────┬──────────────────────────────┬───────────────────┘
                │                              │
     Python bridges (Playwright,          HTTP APIs
     metadata, lyrics)                    Qobuz · Tidal · Spotify · …
```

The whole journey fits in one line:

**connect account → `sync_service` → provider APIs → `EnrichmentEngine` →
SQLite → queue → worker → Qobuz/Tidal/Amazon → staging → verification and tags
→ disk → `downloads`.**

---

## 1. Account: from the wizard to `accounts`

### 1.1 What the user sees

The first-run wizard (`ui/src/components/OnboardingWizard.vue`) has these steps:

1. Welcome.
2. Choose a music folder.
3. Connect one or more services.
4. Choose download quality.
5. Choose what to import (favorites, albums, playlists, artists…).
6. Finish.

The folder and quality are saved in settings; import preferences are saved per
service. **Connecting an account does not start the import**: the wizard only
records what should be imported later. The actual import is triggered from the
**Accounts** view button or by the internal scheduler.

### 1.2 Login under the hood

The UI calls `start_auth_and_save` (`ui/src/api/accounts.ts`), except for
Spotify, which uses the native `spotify_auth_webview` flow.

On the Rust side (`src-tauri/src/commands/auth.rs`):

1. It runs `python scripts/auth_bridge.py <service> login` with the bundled
   Python runtime.
2. The bridge returns JSON on stdout using the `{ success, data, error }`
   contract.
3. Rust normalizes identity: `display_name`, `email`, `user_id`.
4. Credentials are encrypted with **AES-256-GCM** (`src-tauri/src/crypto.rs`).
5. The key lives in the **OS keyring**; if unavailable, a `.crypto_key` file
   with `0600` permissions in the app profile directory is used.
6. It UPSERTs into `accounts`: the library of other accounts for the same
   service is preserved, this one is marked as the only active account, and the
   encrypted `credentials_json` is stored.

| Service | Login mechanism | Credentials stored |
|---|---|---|
| **Qobuz** | Playwright: browser login and session capture | `user_auth_token`, `username`, `password` |
| **Tidal** | Device code + PKCE: opens a verification URL | `access_token`, `refresh_token`, `token_expiry`, `user_id` |
| **Spotify** | Native OAuth PKCE in a Tauri webview; refresh via `sp_dc` | tokens and expiry |
| **Deezer** | Playwright: captures the ARL cookie | `arl` (also used as `access_token`) |
| **SoundCloud** | Playwright: OAuth token and `/me` lookup | `oauth_token`, `access_token`, profile |
| **Apple Music** | Playwright: Music User Token + developer token | `music_user_token`, `developer_token`, `storefront` |

### 1.3 What happens at startup and before using an account

- At startup, `main.rs` tries to decrypt every credential. If the keyring key
  changed or is unavailable, it marks the account as `credentials_invalid` and
  emits `stale_credentials_purged`; if the key becomes usable again, it
  recovers the account.
- Before importing or downloading, the active account is resolved and the token
  is refreshed when needed (Spotify, Tidal, Qobuz auto-login).
- If the provider rejects the session, `auth-session-expired`/
  `auth-state-updated` is emitted, the UI asks the user to reconnect, and
  reconnecting re-queues that service's failed downloads.

---

## 2. Import: from the provider API to the local library

### 2.1 Trigger

The **Import/Sync** button in `AccountsView` calls `sync_service`. The Command
Palette and the internal scheduler (`sync_scheduler.rs` +
`tray::sync_all_services`) use it too; the scheduler syncs active services at
the configured interval when `auto_sync_enabled` is on.

### 2.2 Preparation

`perform_sync_service_with_emitter` (`src-tauri/src/commands/service.rs`) does
the following, in order:

1. Opens an `operation_journal` entry so an unexpected shutdown can be detected.
2. Resolves the effective account.
3. Acquires a write lock per service+account (`SyncWriterLock`, based on
   `flock`) and an in-memory lock per account (`AccountSync`).
4. Checks authentication status; if it is not valid, it emits `requires_auth`
   and aborts.
5. Loads import preferences (favorites, albums, artists, playlists, purchases,
   history…).
6. Writes the `Transfer` checkpoint before fetching any data.

### 2.3 Remote traversal

Each service has its own import arm in `service.rs`. It traverses favorites,
saved albums, playlists, artists and, where the provider exposes them, purchases
or listening history. Pagination is not one universal helper:

- `import_pagination.rs` defines the **offset** and **cursor** contracts.
- Qobuz and Spotify use offset + short page; Tidal has its own helper; Deezer
  uses `next_offset`; SoundCloud uses `next_href`; Apple Music uses its own
  offset.
- Page sizes range from 50 to 100 depending on the service.
- `http_retry.rs` treats 408, 429 and 5xx as transient, honors `Retry-After`,
  and applies backoff with jitter. Deezer, Apple and SoundCloud also use the
  global rate limiter; Qobuz, Tidal and Spotify have their own policies.

### 2.4 Per-track persistence

For Qobuz, Tidal, Spotify, Deezer and SoundCloud, every track goes through
`EnrichmentEngine::enrich_and_persist_sync_track`
(`src-tauri/src/services/enrichment.rs`), inside a `BEGIN IMMEDIATE`
transaction:

1. Resolves artist and album, creating them if they do not exist.
2. Looks up the canonical track identity in this order:
   - **A**: `track_sources(service_id, service_track_id)`.
   - **B**: unique ISRC.
   - **C**: legacy columns (`qobuz_id`, `spotify_id`).
3. On collisions, it flags `needs_review` instead of merging blindly.
4. UPSERTs into `tracks`, `artists`, `albums`, `album_artists`, `track_artists`,
   `track_credits`, `track_sources` and `library_entries`.
5. Records the edition/provenance in the edition tables.
6. Retries the transaction up to 5 times if SQLite reports “locked”.

Playlists are stored separately: `playlists` + `playlist_sources`, tracks with
`INSERT OR IGNORE` into `playlist_tracks`, then positions are recompacted to
remove duplicates and renumber.

> During import, the unified engine **does not query MusicBrainz**: its callers
> pass `query_musicbrainz: false`. External enrichment happens later.

### 2.5 Completion, progress and recovery

- Updates `accounts.last_synced` and `service_sync_settings.last_synced`.
- Stores counters in `account_import_counts`.
- Emits `sync-progress` and `import-progress`; on completion,
  `sync-complete`/`import-complete`, or `sync-failed`/`import-failed`.
- Closes the journal with `commit` or `fail`.
- At startup, `reconcile_startup_operations` detects interrupted operations and
  marks/audits them. **There is no per-page resume**: a new sync walks the whole
  library from the beginning.
- In the background, the MusicBrainz worker (`enrichment_worker.rs`) fills in
  `musicbrainz_id` and `enrichment_status`; Last.fm and local analysis add
  genre, BPM, key and energy depending on the path.

---

## 3. Downloads: from the queue to a verified FLAC

### 3.1 Enqueueing

From Library, Search, Album, Artist or Playlist, the UI calls:

- `add_to_queue` for one track,
- `enqueue_tracks` for a multi-selection,
- `add_batch_to_queue`/`enqueue_eligible_batch` for batches.

The backend (`src-tauri/src/commands/queue.rs`):

1. Checks for duplicates: in practice the guardrail mostly compares `track_id`.
2. Picks the source: explicit identity (`service_name` + `service_track_id`) or
   candidates from `track_sources` with an active account and the best quality.
3. Fills in title, artist, album and ISRC from `tracks`.
4. Inserts into `download_queue` with `queued` status, priority, requested
   quality and `resumable=1`.

Batches also run a **preflight**: non-downloadable sources are discarded, a
missing active account is detected, quality is evaluated and resolution is
attempted by ISRC/MusicBrainz/title. Ambiguous matches are blocked with
`AmbiguousSource`.

### 3.2 The worker

`src-tauri/src/worker.rs` runs under a supervisor that restarts it up to 3 times
if it panics.

- Default concurrency: **2 downloads**; configurable in settings
  (`dl_concurrent_downloads`).
- Claims work with an atomic `UPDATE ... WHERE status='queued'`, ordered by
  priority, position and date.
- When there is no work, waits 500 ms or for a notification.
- Supports pause/resume.
- At startup, reconciles the journal, cleans staging and returns interrupted
  rows to `queued`.

States persisted in `download_queue`:

| State | Meaning |
|---|---|
| `queued` | Waiting for a slot |
| `downloading` | Claimed by the worker |
| `complete` | File promoted and ledger updated |
| `failed` | Failure; manual retry |
| `cancelled` | Cancellation requested |

The UI receives `syncify:download_progress` (and also `syncify:progress`) with
transient states such as `searching`, `downloading` and `finalizing`, plus
`complete`, `failed`, `requires_auth`, `rejected_quality` or `not_found`.

### 3.3 Source resolution

The `DownloadOrchestrator` builds a `DownloadPlan` with ordered candidates. The
account is chosen by validity, entitlement, activity and age; the plan respects
the locked source and can re-resolve up to 4 times if a candidate fails.

- **Qobuz** and **Tidal** have native engines.
- **Spotify, Deezer, SoundCloud and Apple Music** do not serve audio directly:
  they are resolved via **SongLink/Odesli**, which looks for equivalents on
  Tidal/Qobuz and, as a fallback, Amazon.
- In strict mode, lower quality is not accepted; in fallback mode it may step
  down, but **never below 320 kbps**.

### 3.4 Quality and transfer

The policy (`crates/syncify-core-domain/src/quality.rs`) prioritizes FLAC/Hi-Res
and enforces a 320 kbps floor for lossy audio. After downloading, the file is
physically re-inspected with `audio_inspector` (STREAMINFO for FLAC, metadata
for MP4) and compared against what the provider promised.

- **Qobuz**: requests signed with the account token; direct FLAC download;
  rejects 30-second previews and streams that do not cover the catalogued
  duration.
- **Tidal**: receives a segmented stream and the pipeline assembles it.
- **Amazon**: fallback via DoubleDouble, only in the non-lossless pass.
- Transfers use staging (`.part` or a temporary directory), retries, and
  classification of transient vs. terminal errors.

### 3.5 Post-processing and promotion

Before moving the file to its final destination, the pipeline:

1. Writes **tags** and verifies the FLAC.
2. Embeds **cover art**, static and, when applicable, animated.
3. Resolves and writes **lyrics** `.lrc`.
4. Copies the booklet, `cover.jpg`, `cover.webp`, etc.
5. Trims leading/trailing silence (fail-soft).
6. Renames to the library layout: by default
   `{AlbumArtist}/[{Year}] {Album}/NN - {Title}.flac`, with collision
   disambiguation.
7. Records the file in `downloads`, updates `tracks.audio_quality` and
   `track_sources`, and optionally queues BPM analysis.
8. Generates a `manifest.json` in the output folder.

If something fails, the error is classified: transient failures stay in
`failed` with `retry_count+1`; terminal failures are marked so they are not
retried automatically. Retrying is manual from the Downloads view.

---

## 4. The tables that tell the story

| Table | Role |
|---|---|
| `accounts` | Encrypted credentials, identity and active account per service |
| `tracks`, `albums`, `artists` | Canonical catalog |
| `track_sources` | Link between a canonical track and its ID on each service |
| `library_entries` | Which track belongs to which account |
| `playlists`, `playlist_tracks` | Imported playlists and their order |
| `download_queue` | State of each download |
| `downloads` | Ledger of downloaded files |
| `operation_journal` | Checkpoints and recovery after an unexpected shutdown |

---

## 5. Current-state caveats worth keeping in mind

- **Apple Music is the exception**: its import uses raw SQL and does not go
  through `EnrichmentEngine`; it also does not emit per-track progress.
- **Only Qobuz and Tidal download natively**; Amazon is the fallback via
  SongLink. Spotify, Deezer, SoundCloud and Apple Music are for
  importing/migrating, not for direct downloads.
- **Transient download failures are not retried automatically**: they stay in
  `failed` and must be retried manually.
- **Cancelling a download does not abort the in-flight transfer**: it changes
  the state, but the pipeline may still promote the file; the `mark_complete`
  guard prevents marking it complete in the queue.
- **Imports cannot resume per page or be cancelled**: the journal detects the
  interruption, but the next sync walks everything again.
- The documents in `docs/features/` are **design references**, not a verified
  description of current behavior; the code has moved ahead of those texts.

### Following the code

| Phase | Entry points |
|---|---|
| Account | `ui/src/components/OnboardingWizard.vue`, `ui/src/api/accounts.ts`, `src-tauri/src/commands/auth.rs`, `scripts/auth_bridge.py`, `src-tauri/src/crypto.rs` |
| Import | `src-tauri/src/commands/service.rs`, `src-tauri/src/services/import_pagination.rs`, `http_retry.rs`, `operation_recovery.rs`, `services/enrichment.rs`, `sync_scheduler.rs` |
| Download | `src-tauri/src/commands/queue.rs`, `src-tauri/src/worker.rs`, `src-tauri/src/download/{orchestrator,resolver,qobuz,tidal,amazon,songlink,audio_inspector}.rs`, `crates/syncify-core-domain/src/{quality,layout}.rs` |
| UI/IPC | `ui/src/api/*`, `ui/src/views/{LibraryView,DownloadsView,AccountsView}.vue`, `ui/src/App.vue` |

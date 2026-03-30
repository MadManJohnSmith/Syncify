# Syncify

**Desktop-first music library orchestrator** — Import, compare, download, and organize your music from multiple streaming services.

## Features

- 🎵 **Multi-Service Import** — Spotify, Qobuz, Tidal, Deezer, SoundCloud
- 🔄 **Smart Matching** — ISRC-based track matching across services
- ⬇️ **Quality Downloads** — Hi-res FLAC from Qobuz/Tidal, MP3/AAC fallback
- 📝 **Synced Lyrics** — Apple Music word-sync, line-sync fallback
- 🏷️ **Metadata Enrichment** — MusicBrainz, Last.fm, Spotify
- 🔍 **Audio Fingerprinting** — AcoustID identification & duplicate detection
- 📁 **Auto-Organization** — Artist/Album folder structure
- 🎚️ **Format Conversion** — FLAC → MP3/AAC/OGG via FFmpeg

## Quick Start

### Prerequisites

- **Rust** 1.70+ with `cargo`
- **Node.js** 18+ with `npm`
- **Python** 3.10+ with `pip`

### Installation

```bash
# Clone the repository
git clone https://github.com/yourname/syncify.git
cd syncify

# Install Python dependencies
python -m venv .venv
.venv\Scripts\activate  # Windows
# source .venv/bin/activate  # Linux/Mac
pip install -r requirements.txt

# Install frontend dependencies
cd ui
npm install
cd ..

# Copy environment template
copy .env.example .env  # Windows
# cp .env.example .env  # Linux/Mac
```

### Configuration

Edit `.env` with your API credentials:

```env
# Spotify (required for Spotify import)
SPOTIFY_CLIENT_ID=your_client_id
SPOTIFY_CLIENT_SECRET=your_client_secret
SPOTIFY_REDIRECT_URI=http://127.0.0.1:8888/callback

# Qobuz (required for Qobuz downloads)
QOBUZ_APP_ID=your_app_id
QOBUZ_APP_SECRET=your_app_secret

# Tidal (OAuth - no credentials needed)
# Deezer (browser login - no credentials needed)

# Optional: AcoustID for fingerprinting
ACOUSTID_API_KEY=your_key  # Get free key at https://acoustid.org/api-key

# Optional: Last.fm for metadata
LASTFM_API_KEY=your_key
```

### Run

```bash
# Development mode
cargo tauri dev

# Build for production
cargo tauri build
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Tauri Desktop App                     │
├─────────────────────────────────────────────────────────┤
│  Vue/React UI  ←→  Tauri Commands (55+)  ←→  SQLite DB  │
├─────────────────────────────────────────────────────────┤
│                   Python Bridges (9)                     │
│  auth │ lyrics │ download │ metadata │ fingerprint      │
│  conversion │ scanner │ organizer │ playlist            │
├─────────────────────────────────────────────────────────┤
│              External Tools (auto-downloaded)            │
│              FFmpeg  │  Chromaprint/fpcalc              │
└─────────────────────────────────────────────────────────┘
```

## Usage

### Import Library

1. Click **Add Service** → Select Spotify/Qobuz/Tidal
2. Complete OAuth authentication
3. Library imports automatically to local database

### Download Tracks

1. Select tracks in library view
2. Click **Download** → Choose quality preference
3. Background worker processes queue automatically

### Sync Playlists

1. Go to **Playlists** tab
2. Select source service and playlist
3. Export or match to another service

## API Reference

### Frontend Commands (TypeScript)

```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Library
const tracks = await invoke('get_library');
const stats = await invoke('get_library_stats');

// Queue Management
await invoke('add_to_queue', { trackId: 123, priority: 80 });
await invoke('add_batch_to_queue', { trackIds: [1, 2, 3] });
const queue = await invoke('get_queue');
const queueStats = await invoke('get_queue_stats');

// Worker Control
await invoke('pause_downloads');
await invoke('resume_downloads');
const status = await invoke('get_worker_status');

// Listen for progress
await listen('syncify:download_progress', (event) => {
  console.log(event.payload); // {title, artist, status, progress_percent}
});
```

## Development

### Project Structure

```
syncify/
├── src-tauri/           # Rust backend
│   ├── src/
│   │   ├── main.rs      # App entry point
│   │   ├── commands/    # Tauri commands (14 files, 6800+ lines)
│   │   │   ├── mod.rs, types.rs, library.rs
│   │   │   ├── service.rs, auth.rs, download.rs
│   │   │   ├── queue.rs, accounts.rs, settings.rs
│   │   │   ├── tools.rs, dashboard.rs, migration.rs
│   │   │   └── handlers.rs, url_import.rs
│   │   ├── download/    # Rust-native downloaders
│   │   ├── worker.rs    # Background download worker
│   │   └── db.rs        # SQLite connection
│   └── Cargo.toml
├── ui/                  # Frontend (Vue 3 + TypeScript)
├── scripts/             # Python bridges
│   ├── auth_bridge.py
│   ├── lyrics_bridge.py
│   ├── download_bridge.py
│   └── ...
├── migrations/          # SQLite schema
└── .env                 # API credentials
```

### Running Tests

```bash
# Rust tests
cd src-tauri && cargo test

# Python bridge tests  
python -m pytest tests/

# Health check
python scripts/health_check.py
```

## External Dependencies

| Tool | Purpose | Installation |
|------|---------|--------------|
| FFmpeg | Audio conversion | Auto-downloaded on first use |
| Chromaprint | Audio fingerprinting | Auto-downloaded on first use |

Or install manually:
```bash
# Windows (winget)
winget install Gyan.FFmpeg
winget install AcoustID.Chromaprint

# macOS (brew)
brew install ffmpeg chromaprint
```

## License

MIT License - See [LICENSE](LICENSE)

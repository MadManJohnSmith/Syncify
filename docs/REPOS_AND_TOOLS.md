# External Repositories & Tools

This document catalogs all external repositories and tools used by Syncify, their purposes, and how they integrate with the project.

---

## Adjacent Tools (`adjacent_tools/`)

All external repos are cloned into `adjacent_tools/` for reference and subprocess usage.

### Downloading & API References

| Repo | Source | Purpose |
|------|--------|---------|
| `qbdlx-mod` | [DJDoubleD/QobuzDownloaderX-MOD](https://github.com/DJDoubleD/QobuzDownloaderX-MOD) | **Primary** Qobuz downloader + API reference (C# → Rust patterns) |
| `qbdlx-original` | [ImAiiR/QobuzDownloaderX](https://github.com/ImAiiR/QobuzDownloaderX) | Backup Qobuz reference |
| `streamrip` | [nathom/streamrip](https://github.com/nathom/streamrip) | Multi-service downloading (Qobuz, Tidal, Deezer, SoundCloud) |
| `ez-migrate-qbz` | [FacuM/EzMigrateQBZ](https://github.com/FacuM/EzMigrateQBZ) | Qobuz library migration patterns |

### Lyrics & Metadata

| Repo | Source | Purpose |
|------|--------|---------|
| `db-ttml-id` | [Ramadani1t/DB-TTML-ID](https://github.com/Ramadani1t/DB-TTML-ID) | TTML lyrics data structure reference |
| `mx-lrc` | [fashni/MxLRC](https://github.com/fashni/MxLRC) | LRC lyrics parsing patterns |
| `uta` | [SDLMoe/Uta](https://github.com/SDLMoe/Uta) | Lyrics provider integration |
| `beets` | [beetbox/beets](https://github.com/beetbox/beets) | Metadata enrichment, AcoustID fingerprinting, tagging reference |

### Syncify (Old)

| Repo | Source | Purpose |
|------|--------|---------|
| `syncify-old` | [MadManJohnSmith/Syncify](https://github.com/MadManJohnSmith/Syncify) | Previous Python Syncify project (reference for fuzzy matching, not reused directly) |
### Experimental / Legacy

| Repo              | Source | Purpose |
|-------------------|--------|---------|
| `syncify-test-2024` | local folder | Old experimental version of the app; kept only for reference, no direct code reuse. Check it out if youre struggling with an implementation issue. |

---

## MCP Servers

These servers are used for agent-driven development instead of raw shell commands.

| Server | Purpose | Usage |
|--------|---------|-------|
| **Filesystem (Reference)** | Read/write project files | Local edits, config changes |
| **Docker** | Build/run containers | beets, music tools, dev environment |
| **SQLite (QLite)** | Database operations | Inspect/modify `syncify.db`, migrations |
| **GitHub Official** | Remote repo access | Browse QobuzDownloaderX-MOD, streamrip, beets |
| **Playwright** | Browser automation | Spotify/Qobuz OAuth, lyrics scraping |
| **Context7 / Shodan** | External intelligence | Network checks (rarely needed) |
| **Perplexity** | Web search | Research support |

---

## Tool Selection Guidelines

```
When you need to:
• Edit or create files     → Filesystem MCP
• Run builds/tests         → Docker MCP (not raw shell)
• Database operations      → SQLite MCP
• Read remote repos        → GitHub MCP
• Browser automation       → Playwright MCP
```

### Priority Order

1. **Filesystem + SQLite** for local edits and DB work
2. **Docker** for running builds/tests in containers
3. **GitHub Official** only for reading remote repositories
4. **Playwright** only when normal HTTP/API access is insufficient

---

## Service Connectors (Implementation Status)

| Service | Status | Auth Method | Notes |
|---------|--------|-------------|-------|
| Spotify | 🟢 Complete | OAuth 2.0 | Import working, auto-refresh pending |
| Qobuz | 🟢 Complete | Username/Password + API | Full import with progress events |
| Tidal | 🟢 Complete | OAuth Browser Flow | Full import with progress events |
| Deezer | 🟢 Complete | ARL Token | Full import with progress events |
| SoundCloud | 🟡 Auth Only | OAuth 2.0 | Auth works, import pending |
| Apple Music | 🟡 Planned | MusicKit / Browser | Not started |

---

## Related Documentation

- [PROJECT_CONTEXT.md](./PROJECT_CONTEXT.md) - Full project overview
- [SETUP_STEPS.md](./SETUP_STEPS.md) - Environment setup guide
- [SYSTEM_PROMPT_SYNCIFY.md](./SYSTEM_PROMPT_SYNCIFY.md) - Agent system prompt

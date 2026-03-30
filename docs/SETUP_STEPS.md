# Syncify Setup Steps

This guide walks through setting up the Syncify development environment on Windows.

---

## Prerequisites

### Required Software

| Tool | Version | Purpose |
|------|---------|---------|
| **Rust** | 1.75+ | Core development |
| **Node.js** | 18+ | Tauri frontend |
| **Python** | 3.10+ | Subprocess fallbacks |
| **Docker Desktop** | Latest | Containerized builds/tests |
| **Git** | Latest | Version control |

### Recommended Tools

- **VS Code** with rust-analyzer extension
- **SQLite Browser** for database inspection
- **Postman** for API testing

---

## Step 1: Clone Repository

```powershell
git clone https://github.com/MadManJohnSmith/Syncify.git
cd Syncify
```

---

## Step 2: Install Rust Toolchain

```powershell
# Install rustup if not present
winget install Rustlang.Rustup

# Install stable toolchain
rustup default stable

# Add Windows target
rustup target add x86_64-pc-windows-msvc

# Verify installation
cargo --version
```

---

## Step 3: Clone Adjacent Tools

```powershell
mkdir adjacent_tools
cd syncify/adjacent_tools

git clone https://github.com/DJDoubleD/QobuzDownloaderX-MOD qbdlx-mod
git clone https://github.com/ImAiiR/QobuzDownloaderX qbdlx-original
git clone https://github.com/nathom/streamrip
git clone https://github.com/FacuM/EzMigrateQBZ ez-migrate-qbz
git clone https://github.com/Ramadani1t/DB-TTML-ID db-ttml-id
git clone https://github.com/fashni/MxLRC mx-lrc
git clone https://github.com/SDLMoe/Uta uta
git clone https://github.com/beetbox/beets beets
git clone https://github.com/MadManJohnSmith/Syncify syncify-old

cd ..
```
> Note: `syncify-old` is a legacy reference project only. Do not copy code directly; use it as inspiration for matching and comparison logic.

---

## Step 4: Setup SQLite Database

```powershell
# Create database directory
mkdir -p data

# Run initial migration (once Rust core is ready)
cargo sqlx database create
cargo sqlx migrate run
```

---

## Step 5: Install Node Dependencies (UI)

```powershell
cd ui
npm install
cd ..
```

---

## Step 6: Setup Python Environment (Fallbacks)

```powershell
# Create virtual environment
python -m venv .venv

# Activate (PowerShell)
.\.venv\Scripts\Activate.ps1

# Install dependencies
pip install -r requirements.txt
```

---

## Step 7: Configure MCP Servers

Ensure the following MCP servers are configured in your agent environment:

1. **Filesystem** - Point to Syncify repo root
2. **Docker** - Ensure Docker Desktop is running
3. **SQLite** - Configure path to `syncify.db`
4. **GitHub** - Add personal access token
5. **Playwright** - Install browser dependencies

```powershell
# Install Playwright browsers
npx playwright install chromium
```

---

## Step 8: Build & Run

### Development Mode

```powershell
# Build Rust core
cargo build

# Run Tauri dev server
npm run tauri dev
```

### Production Build

```powershell
# Build release
cargo build --release

# Package Tauri app
npm run tauri build
```

---

## Verification Checklist

- [ ] `cargo check` passes without errors
- [ ] `npm run dev` starts UI server
- [ ] SQLite database created in `data/syncify.db`
- [ ] Adjacent tools cloned in `adjacent_tools/`
- [ ] Docker containers can be started
- [ ] MCP servers respond correctly

---

## Troubleshooting

### Common Issues

| Issue | Solution |
|-------|----------|
| `sqlx` not found | Run `cargo install sqlx-cli` |
| Node modules missing | Run `npm install` in `ui/` |
| Docker not running | Start Docker Desktop |
| Rust build fails | Run `rustup update` |

### Getting Help

1. Check [PROJECT_CONTEXT.md](./PROJECT_CONTEXT.md) for architecture details
2. Review [REPOS_AND_TOOLS.md](./REPOS_AND_TOOLS.md) for tool references
3. Consult adjacent_tools/ for implementation patterns

---

## Next Steps

After setup is complete:

1. Run `/bootstrap-core` workflow to initialize Rust core
2. Run `/hook-ui` workflow to wire frontend to backend
3. Review `SYSTEM_PROMPT_SYNCIFY.md` for agent workflow rules

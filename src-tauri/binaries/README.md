# Bundled external binaries (ffmpeg / ffprobe / fpcalc)

`tauri.conf.json` declares these as `bundle.externalBin`, so the NSIS
installer, the Linux DEB and the AppImage ship them next to the application
executable. At build time each file must exist with its Rust target-triple
suffix:

- `ffmpeg-x86_64-pc-windows-msvc.exe`, `ffprobe-x86_64-pc-windows-msvc.exe`, `fpcalc-x86_64-pc-windows-msvc.exe`
- `ffmpeg-x86_64-unknown-linux-gnu`, `ffprobe-x86_64-unknown-linux-gnu`, `fpcalc-x86_64-unknown-linux-gnu`

The CI workflows (`build-linux.yml`, `build-windows.yml`) call the provider to
download the pinned builds, verify them and place them here automatically. For
local production builds (`tauri build`), the same command does everything
(download, SHA-256 verification against the pin table, placement):

```bash
python3 scripts/dependency_manager.py place-external          # host triple
python3 scripts/dependency_manager.py place-external --triple x86_64-unknown-linux-gnu
```

ffprobe has no download of its own: it travels inside the pinned ffmpeg
archive as a byproduct and is placed next to ffmpeg.

Without these files `tauri build` fails; `tauri dev` and `cargo check/test`
do not need them (runtime resolution falls back to `PATH`).

The contents of this directory are git-ignored (these text placeholders are
the only tracked files; the provider overwrites them with the real binaries).

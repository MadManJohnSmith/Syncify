# Bundled external binaries (ffmpeg / ffprobe / fpcalc)

`tauri.conf.json` declares these as `bundle.externalBin`, so the NSIS
installer, the Linux DEB and the AppImage ship them next to the application
executable. At build time each file must exist with its Rust target-triple
suffix:

- `ffmpeg-x86_64-pc-windows-msvc.exe`, `ffprobe-x86_64-pc-windows-msvc.exe`, `fpcalc-x86_64-pc-windows-msvc.exe`
- `ffmpeg-x86_64-unknown-linux-gnu`, `ffprobe-x86_64-unknown-linux-gnu`, `fpcalc-x86_64-unknown-linux-gnu`

The CI workflows (`build-linux.yml`, `build-windows.yml`) download the pinned
static builds and place them here automatically. For local production builds
(`tauri build`), fetch them first:

```bash
python3 scripts/dependency_manager.py   # downloads to bin/
bash scripts/prepare_bundled_binaries.sh
```

Without these files `tauri build` fails; `tauri dev` and `cargo check/test`
do not need them (runtime resolution falls back to `PATH`).

The contents of this directory are git-ignored.

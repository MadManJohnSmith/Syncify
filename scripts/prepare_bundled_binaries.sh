#!/usr/bin/env bash
# Copia los binarios externos de bin/ a src-tauri/binaries/ con el sufijo del
# target triple, tal y como exige bundle.externalBin en tauri.conf.json.
# Los workflows de CI hacen esto automáticamente; este script es para builds
# de producción locales (`tauri build`). Requiere bin/{ffmpeg,ffprobe,fpcalc}
# (descárgalos antes con: python3 scripts/dependency_manager.py).
set -euo pipefail
cd "$(dirname "$0")/.."

TRIPLE="${TARGET_TRIPLE:-$(rustc -vV | awk '/host:/ {print $2}')}"
case "$TRIPLE" in
  *windows*) EXT=".exe" ;;
  *) EXT="" ;;
esac

mkdir -p src-tauri/binaries
missing=0
for tool in ffmpeg ffprobe fpcalc; do
  src="bin/$tool$EXT"
  dst="src-tauri/binaries/$tool-$TRIPLE$EXT"
  if [ -f "$src" ]; then
    cp -f "$src" "$dst"
    chmod +x "$dst" 2>/dev/null || true
    echo "OK: $dst"
  else
    echo "FALTA: $src — ejecuta antes: python3 scripts/dependency_manager.py" >&2
    missing=1
  fi
done
exit "$missing"

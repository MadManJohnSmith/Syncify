#!/usr/bin/env bash
# FE-PERF-01 (#49): harness de medición de navegación en la ventana REAL.
#
# Arranca la app de escritorio (tauri dev) con la sonda de navegación
# (ui/src/perf/navigationProbe.ts) activada, espera a que la sonda complete su
# secuencia programática de rutas y guarda el JSON de resultados en
# ~/.local/state/syncify-fperf01/. La app corre con un perfil de datos
# DESECHABLE (XDG_DATA_HOME redirigido): no toca ~/.local/share/com.syncify.app
# ni la biblioteca real.
#
# Uso: tools/nav-perf.sh <etiqueta>   (p. ej. before / after)
#
# Requisitos: binario debug compilado (cargo build -p syncify-tauri con el
# mismo CARGO_TARGET_DIR), DISPLAY con XWayland, y ninguna otra instancia de
# Syncify corriendo.

set -euo pipefail

LABEL="${1:?Uso: tools/nav-perf.sh <etiqueta>}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE_DIR="${SYNCIFY_NAVPERF_DIR:-$HOME/.local/state/syncify-fperf01}"
TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/syncify-target-fperf01}"
RESULTS_DIR="$STATE_DIR/results"
PROFILE_DIR="$STATE_DIR/profile-$LABEL"

mkdir -p "$RESULTS_DIR" "$PROFILE_DIR"

if [[ -z "${DISPLAY:-}" ]]; then
  echo "ERROR: DISPLAY no está definido: la medición es en la ventana real, no headless." >&2
  exit 1
fi
if pgrep -f 'syncify-tauri|vite' >/dev/null 2>&1; then
  echo "ERROR: hay una instancia de Syncify o vite corriendo; ciérrala antes de medir." >&2
  pgrep -af 'syncify-tauri|vite' >&2
  exit 1
fi

RESULT_FILE="$RESULTS_DIR/nav-$LABEL.json"
rm -f "$RESULT_FILE"

echo "[nav-perf] etiqueta=$LABEL target=$TARGET_DIR perfil=$PROFILE_DIR"

# `npm run dev` ya fuerza GDK_BACKEND=x11 y WEBKIT_DISABLE_DMABUF_RENDERER=1.
# La app usa el perfil desechable vía XDG_DATA_HOME (Tauri resuelve
# app_local_data_dir con XDG, y el storage del webview también).
env -u VITE_NAVPERF \
  VITE_NAVPERF=1 \
  VITE_NAVPERF_COLLECT="$RESULT_FILE" \
  CARGO_TARGET_DIR="$TARGET_DIR" \
  PKG_CONFIG_PATH="${PKG_CONFIG_PATH:-/usr/lib/openssl-3/pkgconfig}" \
  XDG_DATA_HOME="$PROFILE_DIR/xdg-data" \
  npm --prefix "$REPO_ROOT" run dev >"$STATE_DIR/dev-$LABEL.log" 2>&1 &
APP_PID=$!

cleanup() {
  # tauri dev queda como proceso padre: matar el grupo completo.
  kill -- -"$APP_PID" 2>/dev/null || true
  sleep 1
  pkill -f 'syncify-tauri' 2>/dev/null || true
  pkill -f 'vite' 2>/dev/null || true
}
trap cleanup EXIT

echo "[nav-perf] esperando resultados (secuencia: 2 warmup + 10 rondas x 4 vistas)..."
# La sonda hace POST al collector de vite al terminar; hitos intermedios
# (probe-installed, first-view-ready...) van quedando en el mismo fichero.
# Timeout generoso por si hay que compilar el binario en el primer arranque.
for i in $(seq 1 240); do
  if [[ -s "$RESULT_FILE" ]] && grep -q '"finishedAt"' "$RESULT_FILE"; then
    sleep 2  # deja que el POST completo aterrice
    break
  fi
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    echo "ERROR: tauri dev terminó sin producir resultados. Ver $STATE_DIR/dev-$LABEL.log" >&2
    exit 1
  fi
  sleep 5
done

if [[ ! -s "$RESULT_FILE" ]] || ! grep -q '"finishedAt"' "$RESULT_FILE"; then
  echo "ERROR: la sonda no publicó resultados (timeout). Ver $STATE_DIR/dev-$LABEL.log" >&2
  exit 1
fi

echo "[nav-perf] resultados en $RESULT_FILE"
node -e "
const fs = require('fs');
const lines = fs.readFileSync(process.argv[1], 'utf8').trim().split('\n');
for (const l of lines) {
  const o = JSON.parse(l);
  if (o.stage) console.log('[trace]', o.stage, o.message !== undefined ? String(o.message) : '');
}
const finalLine = [...lines].reverse().find((l) => l.includes('finishedAt'));
const data = JSON.parse(finalLine);
console.log('label:', data.label, 'finishedAt:', data.finishedAt);
for (const s of data.summary) {
  console.log(\`\${s.pair}  kind=\${s.kind}  n=\${s.n}  p50=\${s.p50.toFixed(1)}ms  p95=\${s.p95.toFixed(1)}ms  max=\${s.max.toFixed(1)}ms\`);
}
const ipc = Object.entries(data.ipc).filter(([c]) => /create_library_snapshot|get_library_stats|get_library|get_queue_stats/.test(c));
console.log('IPC clave:', JSON.stringify(Object.fromEntries(ipc)));
" "$RESULT_FILE"

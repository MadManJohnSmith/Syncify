#!/usr/bin/env bash
# WK-01 (#52): experimento aislado y medido de composición WebKitGTK.
#
# Lanza un webview mínimo (GTK 3 + WebKitGTK 4.1, la misma pila que el webview de
# Tauri) en un proceso hijo por corrida, con el mismo backend gráfico que la app
# y un timeout máximo, y compara la ruta de compatibilidad (`main.rs`) contra la
# acelerada. Registra, por corrida: código/señal de salida, si la ventana está
# visible y con píxeles de la pantalla de prueba, frames del web process
# (`requestAnimationFrame`) y frames pintados por GTK (`after-paint`).
#
# Qué NO hace, por diseño:
#   * No arranca, toca, reinicia ni mata la instancia de Syncify en uso. Puede
#     convivir con ella; solo aparece una ventana de prueba de 480x360 durante
#     ~5 s por corrida.
#   * No escribe en ~/.local/share/com.syncify.app: cada corrida usa un perfil
#     WebKit/XDG efímero propio bajo un directorio temporal.
#   * No cambia el producto: no hay Rust, UI, migraciones ni arranque de la app.
#
# Rutas medidas (todas con el MISMO backend, distinto entorno de WebKit):
#   compatible  WEBKIT_DISABLE_DMABUF_RENDERER=1 + WEBKIT_DISABLE_COMPOSITING_MODE=1
#               (exactamente lo que fija src-tauri/src/main.rs)
#   acelerada   sin esas dos variables (composición/EGL disponibles)
#   dmabuf-off  solo WEBKIT_DISABLE_DMABUF_RENDERER=1 (composición activa, sin
#               renderer DMABuf): el punto medio que discute la cola.
#
# Uso:
#   tools/wk01-composicion.sh [--backends x11,wayland] [--routes compatible,acelerada]
#                             [--reps N] [--timeout S] [--label ETIQUETA] [--keep]
#
# Requisitos: sesión gráfica (X11/XWayland o Wayland), PyGObject del sistema con
# GTK 3 y WebKit2 4.1 (`/usr/bin/python3`), y `pkg-config` para registrar
# versiones. Los resultados van a ~/.local/state/syncify-wk01/<etiqueta>/ (nunca
# a /tmp: ver docs/agents/wiki/entorno.md) salvo --out.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROBE="$REPO_ROOT/tools/wk01/webkit_probe.py"
SUMMARIZE="$REPO_ROOT/tools/wk01/summarize.py"
FIXTURE="$REPO_ROOT/tools/wk01/fixture.html"
PYTHON="${WK01_PYTHON:-/usr/bin/python3}"

BACKENDS="auto"
ROUTES="compatible"
REPS=1
TIMEOUT_S=30
KEEP=0
LABEL="$(date +%Y%m%dT%H%M%S)"
OUT_DIR=""

ALL_ROUTES=("compatible")
BACKEND_ORDER=("x11" "wayland")

ROUTE_ENV=(
  "compatible:WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1"
  "acelerada:"
  "dmabuf-off:WEBKIT_DISABLE_DMABUF_RENDERER=1"
)

usage() {
  sed -n '2,40p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --backends) BACKENDS="${2:?falta valor para --backends}"; shift 2 ;;
    --backends=*) BACKENDS="${1#*=}"; shift ;;
    --routes) ROUTES="${2:?falta valor para --routes}"; shift 2 ;;
    --routes=*) ROUTES="${1#*=}"; shift ;;
    --reps) REPS="${2:?falta valor para --reps}"; shift 2 ;;
    --reps=*) REPS="${1#*=}"; shift ;;
    --timeout) TIMEOUT_S="${2:?falta valor para --timeout}"; shift 2 ;;
    --timeout=*) TIMEOUT_S="${1#*=}"; shift ;;
    --label) LABEL="${2:?falta valor para --label}"; shift 2 ;;
    --label=*) LABEL="${1#*=}"; shift ;;
    --out) OUT_DIR="${2:?falta valor para --out}"; shift 2 ;;
    --out=*) OUT_DIR="${1#*=}"; shift ;;
    --keep) KEEP=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "opción desconocida: $1 (usa --help)" >&2; exit 2 ;;
  esac
done

OUT_DIR="${OUT_DIR:-$HOME/.local/state/syncify-wk01/$LABEL}"
mkdir -p "$OUT_DIR"

# ── Comprobaciones de entorno ────────────────────────────────────────────────

if [[ ! -x "$PYTHON" ]]; then
  echo "ERROR: no existe el intérprete $PYTHON" >&2
  exit 2
fi

if ! command -v pkg-config >/dev/null 2>&1; then
  echo "ERROR: falta pkg-config (solo para registrar versiones)" >&2
  exit 2
fi

if ! "$PYTHON" - <<'PY' >/dev/null 2>&1
import gi
gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
gi.require_version("WebKit2", "4.1")
from gi.repository import Gtk, Gdk, WebKit2  # noqa: F401
PY
then
  echo "ERROR: $PYTHON no tiene PyGObject con GTK 3 + WebKit2 4.1." >&2
  echo "       Instala python3-gobject / python-gobject del sistema." >&2
  exit 2
fi

resolve_backends() {
  local requested="$1"
  local resolved=()
  local backend
  if [[ "$requested" == "auto" || "$requested" == "both" ]]; then
    for backend in "${BACKEND_ORDER[@]}"; do
      case "$backend" in
        x11) [[ -n "${DISPLAY:-}" ]] && resolved+=("$backend") ;;
        wayland) [[ -n "${WAYLAND_DISPLAY:-}" ]] && resolved+=("$backend") ;;
      esac
    done
  else
    IFS=',' read -r -a resolved <<<"$requested"
  fi
  if [[ ${#resolved[@]} -eq 0 ]]; then
    echo "ERROR: no hay backend gráfico disponible (DISPLAY/WAYLAND_DISPLAY vacíos)." >&2
    exit 2
  fi
  for backend in "${resolved[@]}"; do
    case "$backend" in
      x11) [[ -n "${DISPLAY:-}" ]] || { echo "ERROR: backend x11 pedido sin DISPLAY" >&2; exit 2; } ;;
      wayland) [[ -n "${WAYLAND_DISPLAY:-}" ]] || { echo "ERROR: backend wayland pedido sin WAYLAND_DISPLAY" >&2; exit 2; } ;;
      *) echo "ERROR: backend desconocido '$backend' (x11|wayland)" >&2; exit 2 ;;
    esac
  done
  printf '%s\n' "${resolved[@]}"
}

ROUTE_ENV_FOR() {
  local route="$1" pair
  for pair in "${ROUTE_ENV[@]}"; do
    if [[ "${pair%%:*}" == "$route" ]]; then
      printf '%s' "${pair#*:}"
      return 0
    fi
  done
  echo "ERROR: ruta desconocida '$route'" >&2
  return 1
}

mapfile -t SELECTED_BACKENDS < <(resolve_backends "$BACKENDS")

if [[ "$ROUTES" == "all" ]]; then
  SELECTED_ROUTES=("${ALL_ROUTES[@]}")
else
  IFS=',' read -r -a SELECTED_ROUTES <<<"$ROUTES"
fi
for route in "${SELECTED_ROUTES[@]}"; do
  ROUTE_ENV_FOR "$route" >/dev/null
done

for value in "$REPS" "$TIMEOUT_S"; do
  if ! [[ "$value" =~ ^[1-9][0-9]*$ ]]; then
    echo "ERROR: --reps/--timeout deben ser enteros > 0 (recibido '$value')" >&2
    exit 2
  fi
done

# ── Metadatos de la máquina (sin datos personales) ──────────────────────────

SYNCIFY_RUNNING="no"
if pgrep -f 'syncify-tauri' >/dev/null 2>&1; then
  SYNCIFY_RUNNING="sí"
fi

{
  printf 'label=%s\n' "$LABEL"
  printf 'date=%s\n' "$(date -Is)"
  printf 'kernel=%s\n' "$(uname -sr)"
  printf 'session_type=%s\n' "${XDG_SESSION_TYPE:-desconocido}"
  printf 'desktop=%s\n' "${XDG_CURRENT_DESKTOP:-desconocido}"
  printf 'display=%s\n' "${DISPLAY:-}"
  printf 'wayland_display=%s\n' "${WAYLAND_DISPLAY:-}"
  printf 'syncify_instance_running=%s\n' "$SYNCIFY_RUNNING"
  printf 'loadavg=%s\n' "$(cut -d' ' -f1-3 /proc/loadavg)"
  printf 'gtk=%s\n' "$(pkg-config --modversion gtk+-3.0 2>/dev/null || echo '?')"
  printf 'webkit2gtk=%s\n' "$(pkg-config --modversion webkit2gtk-4.1 2>/dev/null || echo '?')"
  printf 'python=%s\n' "$("$PYTHON" --version 2>&1)"
  printf 'nvidia_driver=%s\n' "$(sed -n 's/^NVRM version: *//p' /proc/driver/nvidia/version 2>/dev/null | head -1)"
  printf 'mesa=%s\n' "$(pkg-config --modversion gl 2>/dev/null || echo '?')"
} >"$OUT_DIR/entorno.txt"

echo "[wk01] etiqueta=$LABEL"
echo "[wk01] resultados en $OUT_DIR"
echo "[wk01] backends=${SELECTED_BACKENDS[*]} rutas=${SELECTED_ROUTES[*]} reps=$REPS timeout=${TIMEOUT_S}s"
echo "[wk01] instancia de Syncify en uso: $SYNCIFY_RUNNING (no se toca)"

# ── Corridas ────────────────────────────────────────────────────────────────

TMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/syncify-wk01-XXXXXX")"
cleanup() {
  if [[ "$KEEP" -eq 1 ]]; then
    echo "[wk01] perfiles temporales conservados en $TMP_ROOT"
  else
    rm -rf "$TMP_ROOT"
  fi
}
trap cleanup EXIT

run_one() {
  local backend="$1" route="$2" rep="$3"
  local run_dir="$OUT_DIR/$backend/$route/r$rep"
  local profile="$TMP_ROOT/$backend-$route-r$rep"
  mkdir -p "$run_dir" "$profile/data" "$profile/config" "$profile/cache"

  local route_env
  route_env="$(ROUTE_ENV_FOR "$route")"

  local load_before
  load_before="$(cut -d' ' -f1-3 /proc/loadavg)"

  # El perfil es efímero: XDG_DATA_HOME/XDG_CONFIG_HOME/XDG_CACHE_HOME apuntan a
  # $TMP_ROOT, así que WebKit no escribe en ~/.local/share ni en la BD de la app.
  # shellcheck disable=SC2086  # route_env es una lista controlada de KEY=VALUE
  set +e
  env \
    GDK_BACKEND="$backend" \
    GSETTINGS_BACKEND=memory \
    NO_AT_BRIDGE=1 \
    XDG_DATA_HOME="$profile/data" \
    XDG_CONFIG_HOME="$profile/config" \
    XDG_CACHE_HOME="$profile/cache" \
    $route_env \
    timeout --signal=TERM --kill-after=2 "$TIMEOUT_S" \
    "$PYTHON" "$PROBE" \
      --fixture "$FIXTURE" \
      --out "$run_dir/events.jsonl" \
      --png-dir "$run_dir" \
      >"$run_dir/cmd.log" 2>&1
  local rc=$?
  set -e

  local load_after
  load_after="$(cut -d' ' -f1-3 /proc/loadavg)"

  "$PYTHON" - "$run_dir/run.json" "$backend" "$route" "$rep" "$rc" "$load_before" "$load_after" <<'PY'
import json
import sys

path, backend, route, rep, rc, load_before, load_after = sys.argv[1:8]
rc = int(rc)
payload = {
    "backend": backend,
    "route": route,
    "rep": int(rep),
    "exit_code": rc,
    "signal": rc - 128 if rc > 128 else None,
    "timed_out": rc in (124, 137),
    "loadavg_before": [float(x) for x in load_before.split()],
    "loadavg_after": [float(x) for x in load_after.split()],
}
with open(path, "w", encoding="utf-8") as handle:
    json.dump(payload, handle, indent=2)
    handle.write("\n")
PY

  echo "[wk01] $backend/$route r$rep → salida=$rc$([[ $rc -gt 128 ]] && echo " (señal $((rc - 128)))")"
  return 0
}

for backend in "${SELECTED_BACKENDS[@]}"; do
  for route in "${SELECTED_ROUTES[@]}"; do
    for ((rep = 1; rep <= REPS; rep++)); do
      run_one "$backend" "$route" "$rep"
    done
  done
done

# ── Resumen ─────────────────────────────────────────────────────────────────

"$PYTHON" "$SUMMARIZE" "$OUT_DIR" --label "$LABEL"

echo "[wk01] listo: $OUT_DIR/resumen.md y $OUT_DIR/results.json"

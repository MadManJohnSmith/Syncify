#!/usr/bin/env bash
# Captura a PNG las variantes de un prototipo de UI desechable.
#
#   scripts/prototype_shots.sh <tab> [variante...]
#   scripts/prototype_shots.sh dashboard mando maquina
#
# Requiere el dev server de Vite vivo (npm --prefix ui run dev) y un Chromium.
# Las capturas van fuera del repo, a ../.dsh-scratch/proto-shots/<tab>/.
# Este script se borra junto con ui/src/views/__prototype__.
set -euo pipefail

TAB="${1:?uso: prototype_shots.sh <tab> [variante...]}"
shift || true
VARIANTS=("$@")

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Sin variantes explícitas: se leen del propio Prototype.vue del tab.
if [ ${#VARIANTS[@]} -eq 0 ]; then
  HOST="$ROOT/ui/src/views/__prototype__/$TAB/Prototype.vue"
  [ -f "$HOST" ] || HOST="$ROOT/ui/src/views/__prototype__/${TAB^}Prototype.vue"
  if [ -f "$HOST" ]; then
    mapfile -t VARIANTS < <(grep -oP "\{ id: '\K[^']+" "$HOST" | awk '!seen[$0]++')
  fi
  if [ ${#VARIANTS[@]} -eq 0 ]; then
    echo "No pude descubrir variantes para '$TAB'; pásalas a mano." >&2
    exit 1
  fi
fi
OUT="${PROTO_SHOTS_DIR:-$ROOT/../.dsh-scratch/proto-shots}/$TAB"
BROWSER="${PROTO_BROWSER:-chromium}"
BASE="${PROTO_BASE_URL:-http://localhost:5173}"
mkdir -p "$OUT"

if ! curl -sf -o /dev/null "$BASE/"; then
  echo "El dev server no responde en $BASE — arráncalo con: npm --prefix ui run dev" >&2
  exit 1
fi

for variant in "${VARIANTS[@]}"; do
  target="$OUT/$variant.png"
  profile="$(mktemp -d)"
  "$BROWSER" --headless=new --disable-gpu --no-sandbox --hide-scrollbars \
    --user-data-dir="$profile" --window-size=1440,1000 --virtual-time-budget=5000 \
    --screenshot="$target" "$BASE/__prototype/$TAB?variant=$variant" >/dev/null 2>&1 || true
  rm -rf "$profile"
  if [ -s "$target" ]; then
    echo "$(basename "$target")  $(du -h "$target" | cut -f1)"
  else
    echo "FALLO $variant" >&2
    exit 1
  fi
done

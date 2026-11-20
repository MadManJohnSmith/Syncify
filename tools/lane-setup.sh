#!/usr/bin/env bash
# lane-setup.sh — prepara el entorno de un worktree de carril.
#
#   tools/lane-setup.sh <worktree>
#
# Enlaza desde el checkout principal el entorno que git no versiona
# (`ui/node_modules`, `.venv` y, si existe, `node_modules`), construye `ui/dist`
# si falta (sin él `tauri::generate_context!` panickea y parece un fallo de
# clippy) y recuerda los exports del target de Rust de este carril.
#
# No toca ficheros versionados. Los symlinks no están cubiertos por los
# patrones de .gitignore con barra final: haz staging explícito por fichero,
# nunca `git add -A`.
set -euo pipefail

usage() {
    sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

if [ "${1:-}" = "" ] || [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
    usage
    exit 0
fi

lane="$1"
[ -d "$lane" ] || { echo "No existe el worktree: $lane" >&2; exit 1; }
lane="$(cd "$lane" && pwd -P)"
git -C "$lane" rev-parse --is-inside-work-tree >/dev/null 2>&1 \
    || { echo "No es un checkout git: $lane" >&2; exit 1; }

main_worktree="$(git -C "$lane" worktree list --porcelain | sed -n 's/^worktree //p' | head -n 1)"
[ -n "$main_worktree" ] || { echo "No puedo resolver el checkout principal." >&2; exit 1; }
main_worktree="$(cd "$main_worktree" && pwd -P)"

if [ "$lane" = "$main_worktree" ]; then
    echo "$lane ya es el checkout principal: no hay nada que enlazar."
    exit 0
fi

link_from_main() {
    local relative="$1" source="$main_worktree/$1" destination="$lane/$1"
    if [ -e "$destination" ] || [ -L "$destination" ]; then
        echo "  ok     $relative ya existe"
        return 0
    fi
    if [ ! -e "$source" ] && [ ! -L "$source" ]; then
        echo "  FALTA  $relative en $main_worktree: créalo allí y repite" >&2
        return 1
    fi
    mkdir -p "$(dirname "$destination")"
    ln -s "$source" "$destination"
    echo "  enlazado $relative -> $source"
}

echo "Preparando $lane desde $main_worktree"
missing=0
link_from_main "ui/node_modules" || missing=1
link_from_main ".venv" || missing=1
# La raíz la necesita `npm run dev` (tauri CLI), no las puertas de cargo.
link_from_main "node_modules" || true

if [ ! -d "$lane/ui/dist" ]; then
    echo "==> ui/dist ausente: npm --prefix ui run build"
    (cd "$lane" && npm --prefix ui run build)
else
    echo "  ok     ui/dist ya existe"
fi

target="$HOME/.cache/syncify-target-$(basename "$lane")"
cat <<EOF

Target de Rust de este carril (fuera de tmpfs, no lo compartas):
  export CARGO_TARGET_DIR="$target"
  export TMPDIR="\$HOME/.cache/tmp"

Staging: añade solo los ficheros del cambio; los symlinks de entorno no están
versionados.
EOF

exit "$missing"

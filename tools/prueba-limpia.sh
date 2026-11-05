#!/usr/bin/env bash
# prueba-limpia.sh — arranca Syncify en desarrollo para verificarla a mano.
#
#   tools/prueba-limpia.sh
#
# Deja dos rastros en ~/.local/state/syncify-manual-test/:
#   - terminal-<fecha>.log: todo lo que imprime `tauri dev` (y la terminal).
#   - los logs nativos de la app, porque SYNCIFY_LOG_DIR apunta ahí.
# Cerrar la app termina la captura.
#
# No compartas esos logs: pueden contener cuentas y detalles de la biblioteca.
# Para apuntar a otro clon: SYNCIFY_REPO=/ruta/al/clon tools/prueba-limpia.sh
set -o pipefail
umask 077

repo="${SYNCIFY_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
[ -d "$repo" ] || { echo "No encuentro el repo en $repo. Exporta SYNCIFY_REPO para apuntar a otro clon." >&2; exit 1; }

log_dir="$HOME/.local/state/syncify-manual-test"
mkdir -p "$log_dir"
terminal_log="$log_dir/terminal-$(date +%Y%m%d-%H%M%S).log"

export RUST_LOG="info"
export RUST_BACKTRACE="1"
export SYNCIFY_LOG_DIR="$log_dir"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/syncify-rust-target}"

printf 'Syncify manual test log: %s\n' "$terminal_log"
printf 'Native application logs: %s\n' "$log_dir"
printf 'Closing the app ends this capture. Do not share the raw logs publicly; they may contain account or library details.\n'

cd "$repo" || exit 1
npm run dev 2>&1 | tee -a "$terminal_log"
exit "${PIPESTATUS[0]}"

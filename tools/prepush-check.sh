#!/usr/bin/env bash
# prepush-check.sh — puerta previa al push: las banderas de CI, en caliente.
#
#   tools/prepush-check.sh               fmt + clippy (~2 min con caché caliente)
#   tools/prepush-check.sh --with-tests  añade cargo test completo (~15 min)
#
# Sale 1 al primer fallo. Un push que pasa aquí no gasta un ciclo de CI de
# 33 min por un warning; las banderas son exactamente las de CI, así que no
# sustituye nada: adelanta el mismo veredicto a la máquina local.
#
# Variables que respeta del entorno: CARGO_TARGET_DIR (por defecto
# /tmp/syncify-target, el de AGENTS.md) y cualquier export específico de la
# máquina (p. ej. PKG_CONFIG_PATH para OpenSSL): ese pegamento va en el
# entorno o en el hook, nunca en este script.
#
# Hook local opcional (un fichero por clon, no versionado):
#   printf '#!/usr/bin/env bash\nexec "$(git rev-parse --show-toplevel)/tools/prepush-check.sh"\n' \
#     > .git/hooks/pre-push && chmod +x .git/hooks/pre-push
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/syncify-target}"

case "${1:-}" in
    "") ;;
    --with-tests) run_tests=1 ;;
    *) echo "Uso: tools/prepush-check.sh [--with-tests]" >&2; exit 2 ;;
esac

echo "==> cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "==> cargo clippy --workspace --all-targets --locked -- -D warnings"
cargo clippy --workspace --all-targets --locked -- -D warnings

if [ "${run_tests:-0}" = 1 ]; then
    echo "==> cargo test --locked --no-fail-fast"
    cargo test --locked --no-fail-fast
fi

echo "Puerta pre-push en verde."

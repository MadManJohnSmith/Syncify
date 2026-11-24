#!/usr/bin/env bash
# prepush-check.sh — puerta previa al push: las banderas de CI, en caliente.
#
#   tools/prepush-check.sh               fmt + clippy (~2 min con caché caliente)
#   tools/prepush-check.sh --with-tests  añade cargo test completo (~15 min)
#   tools/prepush-check.sh --force       no salta cargo aunque el diff sea solo docs
#
# Si el diff contra el upstream (o contra origin/dev si la rama no tiene
# upstream) solo toca *.md, docs/**, AGENTS.md o GLOSSARY.md, se saltan fmt,
# clippy y la suite: se ejecutan las tres puertas de docs y se dice qué se
# saltó. --force anula el salto para el caso raro.
#
# Sale 1 al primer fallo. Un push que pasa aquí no gasta un ciclo de CI de
# 33 min por un warning; las banderas son exactamente las de CI, así que no
# sustituye nada: adelanta el mismo veredicto a la máquina local.
#
# Variables que respeta del entorno: CARGO_TARGET_DIR (por defecto un target
# por checkout bajo ~/.cache, fuera de tmpfs; ver AGENTS.md) y cualquier
# export específico de la máquina (p. ej. PKG_CONFIG_PATH para OpenSSL): ese
# pegamento va en el entorno o en el hook, nunca en este script.
#
# Hook local opcional (un fichero por clon, no versionado):
#   printf '#!/usr/bin/env bash\nexec "$(git rev-parse --show-toplevel)/tools/prepush-check.sh"\n' \
#     > .git/hooks/pre-push && chmod +x .git/hooks/pre-push
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/syncify-target-$(basename "$PWD")}"

run_tests=0
force=0
for arg in "$@"; do
    case "$arg" in
        --with-tests) run_tests=1 ;;
        --force) force=1 ;;
        -h|--help)
            sed -n '2,12p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "Uso: tools/prepush-check.sh [--with-tests] [--force]" >&2
            exit 2
            ;;
    esac
done

run_docs_gates() {
    local python_bin="python3"
    if [ -x .venv/bin/python ]; then
        python_bin=".venv/bin/python"
    else
        echo "AVISO: falta .venv/bin/python; las puertas corren con python3 y pueden perder imports." >&2
    fi
    for gate in test_docs_consistency test_readme_claims test_agents_claims; do
        echo "==> $python_bin scripts/tests/$gate.py"
        "$python_bin" "scripts/tests/$gate.py"
    done
}

upstream="$(git rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null || true)"
base="${upstream:-origin/dev}"
docs_only=0
if git rev-parse --verify --quiet "$base" >/dev/null; then
    changed="$(git diff --name-only "$base...HEAD" 2>/dev/null || true)"
    if [ -n "$changed" ]; then
        docs_only=1
        while IFS= read -r path; do
            case "$path" in
                *.md|docs/*|AGENTS.md|GLOSSARY.md) ;;
                *)
                    docs_only=0
                    break
                    ;;
            esac
        done <<< "$changed"
    fi
else
    echo "AVISO: no encuentro $base; se ejecuta la puerta completa." >&2
fi

if [ "$docs_only" -eq 1 ] && [ "$force" -eq 0 ]; then
    echo "==> diff contra $base solo toca docs: se saltan cargo fmt, clippy y la suite"
    echo "    (usa --force para forzarlos; el hook real no los necesita para un push de docs)"
    run_docs_gates
    echo "Puerta pre-push de docs en verde."
    exit 0
fi

echo "==> cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "==> cargo clippy --workspace --all-targets --locked -- -D warnings"
cargo clippy --workspace --all-targets --locked -- -D warnings

if [ "$run_tests" -eq 1 ]; then
    echo "==> cargo test --workspace --locked --no-fail-fast"
    cargo test --workspace --locked --no-fail-fast
fi

echo "Puerta pre-push en verde."

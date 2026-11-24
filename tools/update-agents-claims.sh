#!/usr/bin/env bash
# update-agents-claims.sh — deja las cifras del AGENTS.md en lo medido.
#
#   tools/update-agents-claims.sh            # reescribe AGENTS.md
#   tools/update-agents-claims.sh --check    # no escribe; sale 1 si hay cifras obsoletas
#
# Cual es el momento (#368): un carril lo corre DESPUES de mergear la rama del
# lote (`git merge origin/wave/ola-0-batch-N`) y ANTES del push final; asi el
# docs job del PR mide el arbol del head y no el merge con la base. El
# integrador lo corre antes del consolidado a main.
#
# La medida y la reescritura viven en `scripts/agents_claims.py`, el mismo
# modulo que usa la puerta `scripts/tests/test_agents_claims.py`: no hay dos
# implementaciones que se puedan separar. Si una cifra no se puede medir, el
# helper sale 2 y no escribe nada.
#
# Prefiere `.venv/bin/python` porque la discovery de la suite Python cuenta
# imports: sin las dependencias de scripts/ pierde modulos y la cifra falla por
# el entorno, no por el arbol. `PYTHON=/ruta/bin/python` lo anula.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [ -n "${PYTHON:-}" ]; then
  python="$PYTHON"
elif [ -x "$root/.venv/bin/python" ]; then
  python="$root/.venv/bin/python"
else
  python="python3"
fi

exec "$python" "$root/scripts/agents_claims.py" "$@"

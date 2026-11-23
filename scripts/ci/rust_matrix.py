#!/usr/bin/env python3
"""Patas del job Rust de ci.yml: Windows solo donde su contexto es requerido.

La proteccion de `main` exige seis contextos con sufijo de matriz
(`gh api repos/MadManJohnSmith/Syncify/branches/main/protection/required_status_checks`),
tres de ellos `(windows-latest)`: Rust, Frontend y Python. Un PR a `main`
tiene que producir la pata Windows de Rust o el merge queda bloqueado por un
check que nunca aparece. Un PR de carril a `wave/*` no: no hay proteccion que
la exija y solo paga el tiempo de la pata.

La decision vive aqui, y no en una expresion del workflow, por el mismo motivo
que la deteccion de areas (scripts/ci/resolve_areas.py): una expresion de
GitHub no se puede probar en local, y una errata en el `matrix:` deja el job
Rust entero sin ejecutar (con `strict=true`, main sin su check). Esta version
tiene tests de comportamiento en scripts/tests/test_rust_matrix.py.

Contrato con el workflow:

  Entrada  GITHUB_EVENT_NAME, GITHUB_BASE_REF (base del pull_request) y
           GITHUB_REF (rama del push), tal cual los expone Actions.
  Salida   UN output `rust_os`: lista JSON con las patas a ejecutar. Se añade
           a $GITHUB_OUTPUT cuando existe y siempre se imprime en stdout.
  Regla    Las dos patas en `push` a `refs/heads/main` y en `pull_request`
           con base `main` (el consolidado, red final). En el resto —PRs a
           `wave/*`, `workflow_dispatch`— solo Ubuntu.

Consumo:

    matrix:
      os: ${{ fromJSON(needs.changed.outputs.rust_os) }}
"""

from __future__ import annotations

import json
import os
import sys

UBUNTU = "ubuntu-latest"
WINDOWS = "windows-latest"
TRUNK = "main"
BOTH_LEGS = (UBUNTU, WINDOWS)


def rust_matrix(event_name: str | None, base_ref: str | None, ref: str | None) -> list[str]:
    """Patas del job Rust para el evento de GitHub que las pide."""
    if event_name == "pull_request":
        return list(BOTH_LEGS) if base_ref == TRUNK else [UBUNTU]
    if event_name == "push" and ref == f"refs/heads/{TRUNK}":
        return list(BOTH_LEGS)
    return [UBUNTU]


def emit_rust_os(os_list: list[str]) -> None:
    """Escribe el output `rust_os` y lo deja tambien en el log."""
    line = f"rust_os={json.dumps(os_list)}"
    output_path = os.environ.get("GITHUB_OUTPUT")
    if output_path:
        with open(output_path, "a", encoding="utf-8") as output_file:
            output_file.write(line + "\n")
    print(line, flush=True)


def main(argv: list[str] | None = None) -> int:
    del argv  # sin banderas: la entrada es el entorno, la salida un output
    emit_rust_os(
        rust_matrix(
            os.environ.get("GITHUB_EVENT_NAME"),
            os.environ.get("GITHUB_BASE_REF"),
            os.environ.get("GITHUB_REF"),
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

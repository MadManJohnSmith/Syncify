#!/usr/bin/env python3
"""Resolucion de areas afectadas para ci.yml: un unico output, fallback incluido.

El job `changed` de ci.yml usaba tres outputs (rust/frontend/python) calculados
con expresiones de GitHub dentro de un ternario, y ahi vivia la trampa que
costo 5 commits de reparacion: dorny/paths-filter emite los STRINGS
"true"/"false", y en las expresiones de GitHub cualquier string no vacio
(incluido "false") es truthy, de modo que un `||` enmascaraba al fallback y una
PR de solo .github/ o docs/ quedaba en verde sin ejecutar nada.

Este script se queda esa decision: la deteccion es una comparacion exacta
contra "true" (nunca semantica truthy) y el fallback — si nada detectado,
probarlas todas — es codigo probado, no una clausula repetida. La trampa ya no
es un comentario del workflow: es el caso de test de
scripts/tests/test_resolve_areas.py.

Contrato con el workflow:

  Entrada  Tres variables de entorno con los outputs crudos del filtro de
           rutas de dorny: FILTER_RUST, FILTER_FRONTEND, FILTER_PYTHON.
           Se pasan por `env:` y no por interpolacion en `run:` para no
           incrustar expresiones de GitHub dentro de un shell.
  Salida   UN unico output `areas`, una lista JSON con los nombres de las
           areas a ejecutar, en orden fijo (rust, frontend, python). La linea
           `areas=[...]` se añade a $GITHUB_OUTPUT cuando existe (dentro de
           Actions) y siempre se imprime en stdout (log y uso manual).
  Fallback Sin ninguna area detectada, la lista contiene las tres: una PR
           solo-de-.github/ o docs/ ejecuta todo en vez de nada. El fake-green
           de los jobs (aparecer en verde sin ejecutar pasos) se conserva por
           semantica de required checks, intacto.

Consumo (pertenencia exacta, sin trampas truthy ni colisiones de substring:
`contains` sobre la lista parseada compara ELEMENTOS completos):

    if: contains(fromJSON(needs.changed.outputs.areas), 'rust')
"""

from __future__ import annotations

import json
import os
import sys

# Orden fijo de la lista emitida: determinista e independiente del orden en
# que lleguen las variables de entorno.
AREAS = ("rust", "frontend", "python")

# Outputs crudos de dorny, tal cual llegan del paso `filter` de ci.yml.
INPUT_VARS = {
    "rust": "FILTER_RUST",
    "frontend": "FILTER_FRONTEND",
    "python": "FILTER_PYTHON",
}

FALLBACK_NOTICE = "::notice::El filtro de rutas no detecto areas; se ejecutan todas."


def detected(raw_value: str | None) -> bool:
    """True solo cuando el filtro dijo explicitamente "true".

    Comparacion exacta, nunca truthy: los strings "false", "0" o vacio no
    detectan nada (ese era el bug original). Se recortan espacios y se ignora
    el caso por robustez, no por semantica: dorny emite "true" en minusculas.
    """
    return (raw_value or "").strip().lower() == "true"


def resolve_areas(raw_outputs: dict[str, str | None]) -> list[str]:
    """Areas a ejecutar para los outputs crudos del filtro de rutas."""
    areas = [area for area in AREAS if detected(raw_outputs.get(area))]
    if not areas:
        # Fallback: si nada detectado, probarlas todas. Es el caso de una PR
        # cuyos ficheros no caen en ningun filtro (o de un filtro que no dijo
        # nada): correr todo es el fallo seguro; no correr nada, el bug.
        return list(AREAS)
    return areas


def emit_areas(areas: list[str]) -> None:
    """Escribe el unico output `areas` y lo deja tambien en el log."""
    line = f"areas={json.dumps(areas)}"
    output_path = os.environ.get("GITHUB_OUTPUT")
    if output_path:
        with open(output_path, "a", encoding="utf-8") as output_file:
            output_file.write(line + "\n")
    print(line, flush=True)


def main(argv: list[str] | None = None) -> int:
    del argv  # sin banderas: la entrada es el entorno, la salida un output
    raw_outputs = {area: os.environ.get(var) for area, var in INPUT_VARS.items()}
    if not any(detected(value) for value in raw_outputs.values()):
        print(FALLBACK_NOTICE, flush=True)
    emit_areas(resolve_areas(raw_outputs))
    return 0


if __name__ == "__main__":
    sys.exit(main())

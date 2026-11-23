"""Puerta de cifras del AGENTS.md.

El AGENTS.md anota recuentos a mano ("362 objetivos de test", "501 tests",
"158 ficheros de test UI", "los 13 tests #[ignore]", "~19 escrituras
silenciosas") y eran la unica cifra del repo que ningun gate verificaba: en
la sesion de cierres del 2026-11-11 cuatro PRs chocaron en rebase por esas
lineas. Esta puerta mide el arbol y compara; si el arbol cambia, la cifra del
AGENTS.md se actualiza con el numero medido, no con el recordado.

La medida y los patrones viven en `scripts/agents_claims.py`, porque desde
#368 los comparte `tools/update-agents-claims.sh`: el comando que reescribe las
cifras de un carril y esta puerta no pueden derivar en dos copias de la logica.

Cubre lo derivable de forma hermetica. Quedan fuera, a proposito:
- El conteo de tests de la suite UI: vitest materializa `it.each` en tiempo de
  ejecucion y no hay conteo hermetico que no sea correr la suite. Desde la
  retro del 2026-11-19 (R4) el AGENTS.md solo publica el numero de ficheros,
  que si es medible.
- "(2 skipped)" de la suite Python: los skips son condicionales
  (skipIf/skipUnless) y dependen de la plataforma.
- La cifra "~19" se lee del BASELINE de `test_silent_write_ratchet.py` (una
  sola fuente de verdad); esta puerta verifica que el AGENTS.md la cite igual,
  para que no se separen.

El conteo Python usa el interprete que ejecuta la suite: fuera de `.venv` la
discovery ya pierde modulos (4 import errors) y esta puerta falla con el
mismo arbol que el resto, no antes.
"""

import importlib.util
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
AGENTS_MD = (REPO_ROOT / "AGENTS.md").read_text(encoding="utf-8")


def _claims_module():
    spec = importlib.util.spec_from_file_location(
        "agents_claims", REPO_ROOT / "scripts" / "agents_claims.py"
    )
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


CLAIMS = _claims_module()


def _claimed(key: str) -> int:
    spec = CLAIMS.SPEC_BY_KEY[key]
    value = CLAIMS.claimed(AGENTS_MD, spec)
    if value is None:
        raise AssertionError(
            f"AGENTS.md ya no cifra {spec.label}: actualiza esta puerta y el helper"
        )
    return value


class AgentsCountTests(unittest.TestCase):
    """Cada cifra del AGENTS.md, medida contra el arbol."""

    def test_python_suite_count_matches_discovery(self):
        claimed = _claimed("python_tests")
        loaded = CLAIMS.measure_python_tests(REPO_ROOT)
        self.assertEqual(
            claimed,
            loaded,
            f"AGENTS.md cifra {claimed} tests de Python; la discovery carga {loaded}. "
            "Corre tools/update-agents-claims.sh o explica por que el arbol cambio",
        )

    def test_cargo_objectives_match_the_tree(self):
        claimed = _claimed("cargo_targets")
        try:
            measured = CLAIMS.measure_cargo_targets(REPO_ROOT)
        except CLAIMS.CargoUnavailable:
            self.skipTest("cargo no esta en PATH: la cifra de objetivos no se puede medir aqui")
        self.assertEqual(
            claimed,
            measured,
            f"AGENTS.md cifra {claimed} objetivos de test; el arbol tiene {measured}. "
            "Corre tools/update-agents-claims.sh o explica por que el arbol cambio",
        )

    def test_ignore_count_matches_the_tree(self):
        claimed = _claimed("ignore_tests")
        measured = CLAIMS.measure_ignore_sites(REPO_ROOT)
        self.assertEqual(
            claimed,
            measured,
            f"AGENTS.md cifra {claimed} tests #[ignore]; el arbol tiene {measured}. "
            "Corre tools/update-agents-claims.sh o explica por que el arbol cambio",
        )

    def test_ui_file_count_matches_the_tree(self):
        claimed = _claimed("ui_files")
        measured = CLAIMS.measure_ui_files(REPO_ROOT)
        self.assertEqual(
            claimed,
            measured,
            f"AGENTS.md cifra {claimed} ficheros de test de UI; el arbol tiene {measured}. "
            "Corre tools/update-agents-claims.sh o explica por que el arbol cambio",
        )

    def test_silent_write_figure_matches_the_ratchet_baseline(self):
        claimed = _claimed("silent_writes")
        measured = CLAIMS.measure_silent_writes(REPO_ROOT)
        self.assertEqual(
            claimed,
            measured,
            "la cifra de escrituras silenciosas del AGENTS.md se separo del "
            f"BASELINE del ratchet ({measured}); una sola fuente de verdad",
        )


if __name__ == "__main__":
    unittest.main()

"""Puerta de cifras del AGENTS.md.

El AGENTS.md anota recuentos a mano ("318 objetivos de test", "428 tests",
"131 ficheros", "los 13 tests #[ignore]", "~113 escrituras silenciosas") y
eran la unica cifra del repo que ningun gate verificaba: en la sesion de
cierres del 2026-11-11 cuatro PRs chocaron en rebase por esas lineas. Esta
puerta mide el arbol y compara; si el arbol cambia, la cifra del AGENTS.md se
actualiza con el numero medido, no con el recordado.

Cubre lo derivable de forma hermetica. Quedan fuera, a proposito:
- "1139 tests" de la suite UI: vitest materializa `it.each` en tiempo de
  ejecucion y no hay conteo hermetico que no sea correr la suite.
- "(2 skipped)" de la suite Python: los skips son condicionales
  (skipIf/skipUnless) y dependen de la plataforma.
- La cifra "~113" no se mide aqui: su fuente es BASELINE en
  `test_silent_write_ratchet.py`; esta puerta solo verifica que el AGENTS.md
  la cite igual, para que no se separen.

El conteo Python usa el interprete que ejecuta la suite: fuera de `.venv` la
discovery ya pierde modulos (4 import errors) y esta puerta falla con el
mismo arbol que el resto, no antes.
"""

import importlib.util
import json
import re
import subprocess
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
AGENTS_MD = (REPO_ROOT / "AGENTS.md").read_text(encoding="utf-8")
IGNORE_GLOBS = ("src-tauri/tests/**/*.rs", "crates/*/tests/**/*.rs")


def _agents_count(pattern: str, label: str) -> int:
    match = re.search(pattern, AGENTS_MD)
    if match is None:
        raise AssertionError(f"AGENTS.md ya no cifra {label}: actualiza esta puerta")
    return int(match.group(1))


def _cargo_test_targets() -> int:
    """Objetivos de test del workspace: `cargo metadata` mas grep de unit tests.

    Cada paquete aporta sus targets de integracion (kind "test"); lib y bins
    aportan un objetivo unitario solo si algun `.rs` bajo `src/` lleva
    atributo de test. Esta derivacion reproduce el recuento original de 318.
    """
    out = subprocess.run(
        ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
        capture_output=True,
        text=True,
        check=True,
        cwd=REPO_ROOT,
    ).stdout
    unit_attr = re.compile(r"#\[(?:tokio::test|test|sqlx::test)\b")
    total = 0
    for pkg in json.loads(out)["packages"]:
        kinds = [t.get("kind") or [] for t in pkg["targets"]]
        total += sum(1 for kind in kinds if "test" in kind)
        pkg_dir = Path(pkg["manifest_path"]).parent
        if any(
            unit_attr.search(p.read_text(encoding="utf-8", errors="ignore"))
            for p in (pkg_dir / "src").rglob("*.rs")
        ):
            total += sum(1 for kind in kinds if "lib" in kind or "bin" in kind)
    return total


class AgentsCountTests(unittest.TestCase):
    """Cada cifra del AGENTS.md, medida contra el arbol."""

    def test_python_suite_count_matches_discovery(self):
        claimed = _agents_count(r"(\d+) tests \(\d+ skipped\)", "la suite Python")
        suite = unittest.defaultTestLoader.discover(str(REPO_ROOT / "scripts" / "tests"))
        loaded = suite.countTestCases()
        self.assertEqual(
            claimed,
            loaded,
            f"AGENTS.md cifra {claimed} tests de Python; la discovery carga {loaded}. "
            "Actualiza el AGENTS.md con el numero medido",
        )

    def test_cargo_objectives_match_the_tree(self):
        claimed = _agents_count(r"(\d+) objetivos de test", "los objetivos de cargo")
        try:
            measured = _cargo_test_targets()
        except FileNotFoundError:
            self.skipTest("cargo no esta en PATH: la cifra de objetivos no se puede medir aqui")
        except subprocess.CalledProcessError as error:
            self.fail(f"cargo metadata fallo: {error.stderr.strip()}")
        self.assertEqual(
            claimed,
            measured,
            f"AGENTS.md cifra {claimed} objetivos de test; el arbol tiene {measured}",
        )

    def test_ignore_count_matches_the_tree(self):
        claimed = _agents_count(r"los (\d+) tests `#\[ignore\]`", "los tests ignorados")
        ignore_sites = sum(
            path.read_text(encoding="utf-8").count("#[ignore")
            for pattern in IGNORE_GLOBS
            for path in REPO_ROOT.glob(pattern)
        )
        self.assertGreater(ignore_sites, 0, "no hay sitios #[ignore]: el conteo seria vacuo")
        self.assertEqual(
            claimed,
            ignore_sites,
            f"AGENTS.md cifra {claimed} tests #[ignore]; el arbol tiene {ignore_sites}",
        )

    def test_ui_file_count_matches_the_tree(self):
        claimed = _agents_count(r"(\d+) ficheros / \d+ tests", "los ficheros de la UI")
        ui_src = REPO_ROOT / "ui" / "src"
        measured = len(list(ui_src.rglob("*.spec.ts"))) + len(list(ui_src.rglob("*.test.ts")))
        self.assertEqual(
            claimed,
            measured,
            f"AGENTS.md cifra {claimed} ficheros de test de UI; el arbol tiene {measured}",
        )

    def test_silent_write_figure_matches_the_ratchet_baseline(self):
        claimed = _agents_count(r"~(\d+) `let _ = sqlx::query", "las escrituras silenciosas")
        spec = importlib.util.spec_from_file_location(
            "test_silent_write_ratchet",
            Path(__file__).with_name("test_silent_write_ratchet.py"),
        )
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        self.assertEqual(
            claimed,
            module.BASELINE,
            "la cifra de escrituras silenciosas del AGENTS.md se separo del "
            f"BASELINE del ratchet ({module.BASELINE}); una sola fuente de verdad",
        )


if __name__ == "__main__":
    unittest.main()

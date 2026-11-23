#!/usr/bin/env python3
"""Tests del helper de cifras del AGENTS.md (#368).

El helper existe para que un carril no cuente a mano tras mergear la rama del
lote. Si reescribiera una linea de mas, dependiera de este checkout o
escribiera cuando no puede medir, el arreglo del choque de recuentos seria un
punto caliente nuevo. Por eso todos los casos montan un fixture temporal: el
helper se mide contra su propio arbol y el repo real solo aporta los ficheros
bajo prueba.
"""

import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
MODULE = REPO_ROOT / "scripts" / "agents_claims.py"
HELPER = REPO_ROOT / "tools" / "update-agents-claims.sh"


def _claims_module():
    spec = importlib.util.spec_from_file_location("agents_claims_under_test", MODULE)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


CLAIMS = _claims_module()

AGENTS_TEXT = textwrap.dedent(
    """\
    # AGENTS.md

    Texto que el helper no debe tocar: ni esta linea ni las dos siguientes.

    cargo test --locked --no-fail-fast   # 99 objetivos de test: recuento real
    npm --prefix ui run test:run         # 98 ficheros de test UI
    .venv/bin/python -m unittest discover -s scripts/tests   # 97 tests (2 skipped)
    Todo corre sin red salvo los 96 tests `#[ignore]`, que necesitan servicios reales.
    hay ~95 `let _ = sqlx::query(...)` en produccion (18 ya propagan con map_err)

    Cierre que tampoco se toca.
    """
)

MEASURED = {
    "cargo_targets": 362,
    "python_tests": 481,
    "ui_files": 158,
    "ignore_tests": 13,
    "silent_writes": 19,
}

#: El mismo texto con las cifras medias: ni una linea mas cambia.
EXPECTED_TEXT = textwrap.dedent(
    """\
    # AGENTS.md

    Texto que el helper no debe tocar: ni esta linea ni las dos siguientes.

    cargo test --locked --no-fail-fast   # 362 objetivos de test: recuento real
    npm --prefix ui run test:run         # 158 ficheros de test UI
    .venv/bin/python -m unittest discover -s scripts/tests   # 481 tests (2 skipped)
    Todo corre sin red salvo los 13 tests `#[ignore]`, que necesitan servicios reales.
    hay ~19 `let _ = sqlx::query(...)` en produccion (18 ya propagan con map_err)

    Cierre que tampoco se toca.
    """
)

#: Lo que mide el fixture de abajo: 1 objetivo cargo, 2 ficheros UI, 0 casos
#: Python, 2 sitios `#[ignore]` y el suelo 4 del ratchet.
FIXTURE_MEASURED_TEXT = textwrap.dedent(
    """\
    # AGENTS.md

    Texto que el helper no debe tocar: ni esta linea ni las dos siguientes.

    cargo test --locked --no-fail-fast   # 1 objetivos de test: recuento real
    npm --prefix ui run test:run         # 2 ficheros de test UI
    .venv/bin/python -m unittest discover -s scripts/tests   # 0 tests (2 skipped)
    Todo corre sin red salvo los 2 tests `#[ignore]`, que necesitan servicios reales.
    hay ~4 `let _ = sqlx::query(...)` en produccion (18 ya propagan con map_err)

    Cierre que tampoco se toca.
    """
)


def _write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


class RewriteTests(unittest.TestCase):
    """La reescritura es pura: texto y medidas entran, texto sale."""

    def test_rewrite_changes_only_the_five_claim_lines(self):
        updated, _ = CLAIMS.rewrite(AGENTS_TEXT, MEASURED)
        self.assertEqual(updated, EXPECTED_TEXT)
        before = AGENTS_TEXT.splitlines()
        after = updated.splitlines()
        self.assertEqual(len(before), len(after))
        self.assertEqual(sum(1 for old, new in zip(before, after) if old != new), 5)

    def test_rewrite_is_idempotent(self):
        updated, _ = CLAIMS.rewrite(AGENTS_TEXT, MEASURED)
        again, changes = CLAIMS.rewrite(updated, MEASURED)
        self.assertEqual(again, updated)
        self.assertEqual(changes, [])

    def test_rewrite_reports_each_change_with_its_two_values(self):
        _, changes = CLAIMS.rewrite(AGENTS_TEXT, MEASURED)
        self.assertEqual(
            [(change.key, change.before, change.after) for change in changes],
            [
                ("cargo_targets", 99, 362),
                ("python_tests", 97, 481),
                ("ui_files", 98, 158),
                ("ignore_tests", 96, 13),
                ("silent_writes", 95, 19),
            ],
        )

    def test_rewrite_refuses_a_text_without_a_claim(self):
        without_cargo = AGENTS_TEXT.replace(
            "cargo test --locked --no-fail-fast   # 99 objetivos de test: recuento real\n", ""
        )
        with self.assertRaises(CLAIMS.MeasurementError) as raised:
            CLAIMS.rewrite(without_cargo, MEASURED)
        self.assertIn("los objetivos de cargo", str(raised.exception))

    def test_rewrite_refuses_a_duplicated_claim(self):
        duplicated = AGENTS_TEXT + "Otra linea: 3 objetivos de test\n"
        with self.assertRaises(CLAIMS.MeasurementError) as raised:
            CLAIMS.rewrite(duplicated, MEASURED)
        self.assertIn("2 veces", str(raised.exception))

    def test_rewrite_refuses_a_missing_measurement(self):
        incomplete = {key: value for key, value in MEASURED.items() if key != "ui_files"}
        with self.assertRaises(CLAIMS.MeasurementError) as raised:
            CLAIMS.rewrite(AGENTS_TEXT, incomplete)
        self.assertIn("ui_files", str(raised.exception))


class FixtureCase(unittest.TestCase):
    """Un temporizador con su AGENTS.md; el arbol se monta por caso."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = Path(self._tmp.name)
        self.agents = self.root / "AGENTS.md"
        self.agents.write_text(AGENTS_TEXT, encoding="utf-8")

    def build_measurable_tree(self):
        """Arbol minimo medible: paquete cargo, UI, ignores y suelo del ratchet."""
        _write(
            self.root / "Cargo.toml",
            '[package]\nname = "fixture-claims"\nversion = "0.1.0"\nedition = "2021"\n\n'
            '[lib]\npath = "src/lib.rs"\n',
        )
        _write(
            self.root / "Cargo.lock",
            'version = 3\n\n[[package]]\nname = "fixture-claims"\nversion = "0.1.0"\n',
        )
        _write(self.root / "src" / "lib.rs", "#[test]\nfn fixture_unit_test() {}\n")
        _write(self.root / "ui" / "src" / "a.spec.ts", "")
        _write(self.root / "ui" / "src" / "nested" / "b.test.ts", "")
        _write(self.root / "src-tauri" / "tests" / "a.rs", "#[ignore]\nfn one() {}\n#[ignore]\nfn two() {}\n")
        _write(self.root / "scripts" / "tests" / "test_silent_write_ratchet.py", "BASELINE = 4\n")

    def run_cli(self, *args):
        return subprocess.run(
            [sys.executable, str(MODULE), "--repo-root", str(self.root), *args],
            capture_output=True,
            text=True,
            timeout=120,
        )

    def read_agents(self):
        return self.agents.read_text(encoding="utf-8")


class FixtureCliTests(FixtureCase):
    """El CLI mide el fixture, escribe solo lo medido y respeta --check."""

    @unittest.skipUnless(shutil.which("cargo"), "cargo mide los objetivos del fixture")
    def test_cli_rewrites_the_fixture_and_is_idempotent(self):
        self.build_measurable_tree()
        first = self.run_cli()
        self.assertEqual(first.returncode, 0, first.stdout + first.stderr)
        self.assertEqual(self.read_agents(), FIXTURE_MEASURED_TEXT)

        second = self.run_cli()
        self.assertEqual(second.returncode, 0, second.stdout + second.stderr)
        self.assertIn("ya estaba al dia", second.stdout)
        self.assertEqual(self.read_agents(), FIXTURE_MEASURED_TEXT)

    @unittest.skipUnless(shutil.which("cargo"), "cargo mide los objetivos del fixture")
    def test_check_mode_reports_stale_claims_without_writing(self):
        self.build_measurable_tree()
        proc = self.run_cli("--check")
        self.assertEqual(proc.returncode, 1, proc.stdout + proc.stderr)
        self.assertIn("tools/update-agents-claims.sh", proc.stderr)
        self.assertEqual(self.read_agents(), AGENTS_TEXT)

        self.assertEqual(self.run_cli().returncode, 0)
        self.assertEqual(self.run_cli("--check").returncode, 0)

    def test_cli_writes_nothing_when_it_cannot_measure(self):
        # Sin Cargo.toml no hay objetivos: ni una linea del AGENTS.md cambia.
        _write(self.root / "scripts" / "tests" / "test_silent_write_ratchet.py", "BASELINE = 4\n")
        proc = self.run_cli()
        self.assertEqual(proc.returncode, 2, proc.stdout + proc.stderr)
        self.assertIn("update-agents-claims:", proc.stderr)
        self.assertEqual(self.read_agents(), AGENTS_TEXT)

    def test_cli_fails_when_the_agents_md_is_missing(self):
        self.agents.unlink()
        proc = self.run_cli()
        self.assertEqual(proc.returncode, 2, proc.stdout + proc.stderr)
        self.assertIn("no existe", proc.stderr)


@unittest.skipIf(os.name == "nt", "el wrapper es bash; se prueba en POSIX")
class ShellWrapperTests(FixtureCase):
    """El comando que corre un carril: bash -> python, sin logica propia."""

    def run_helper(self, *args):
        return subprocess.run(
            ["bash", str(HELPER), "--repo-root", str(self.root), *args],
            capture_output=True,
            text=True,
            timeout=120,
            env={**os.environ, "PYTHON": sys.executable},
        )

    def test_wrapper_is_a_valid_bash_script(self):
        proc = subprocess.run(
            ["bash", "-n", str(HELPER)], capture_output=True, text=True, timeout=60
        )
        self.assertEqual(proc.returncode, 0, proc.stderr)

    @unittest.skipUnless(shutil.which("cargo"), "cargo mide los objetivos del fixture")
    def test_wrapper_rewrites_a_fixture_tree(self):
        self.build_measurable_tree()
        proc = self.run_helper()
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertEqual(self.read_agents(), FIXTURE_MEASURED_TEXT)


if __name__ == "__main__":
    unittest.main()

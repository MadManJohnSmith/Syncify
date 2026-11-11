#!/usr/bin/env python3
"""
Behavior tests for the CI area resolver (scripts/ci/resolve_areas.py, #248).

The `changed` job of ci.yml delegates to this script both the detection and the
fallback ("if nothing was detected, run them all"), and it emits a single
`areas` output as a JSON list that the three consumer jobs query with exact
membership (`contains(fromJSON(...), 'rust')`).

The fake here is dorny/paths-filter simulated as it behaves in Actions: it
speaks through environment variables and its outputs are the STRINGS
"true"/"false". That string-ness is the trap that cost five repair commits —
in GitHub expressions any non-empty string (including "false") is truthy — so
the trap is asserted as behavior, not as a comment: a simulated dorny returning
"false" enables nothing on its own.

Every test crosses the CLI seam: it runs the script as a subprocess with the
simulated filter outputs in the environment and, when set, a temp file standing
in for $GITHUB_OUTPUT. No network, no GitHub Actions. Assertions are observable
behavior — the emitted `areas=` line, the notice, the output file — never the
script's internal text and never workflow YAML.
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
RESOLVER_SCRIPT = REPO_ROOT / "scripts" / "ci" / "resolve_areas.py"

ALL_AREAS = ["rust", "frontend", "python"]
FILTER_ENV_VARS = {
    "rust": "FILTER_RUST",
    "frontend": "FILTER_FRONTEND",
    "python": "FILTER_PYTHON",
}


class ResolveAreasCLITest(unittest.TestCase):
    """The emitted `areas` output, with dorny simulated via environment."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.output_file = Path(self.tmp.name) / "github_output.txt"

    def tearDown(self):
        self.tmp.cleanup()

    # ── helpers ────────────────────────────────────────────────────────

    def _run_resolver(self, *, rust=None, frontend=None, python=None, github_output=False):
        """Run the script with dorny simulated: outputs arrive as strings.

        An argument of None means dorny said nothing about that area (the
        variable is not set at all), exactly like a filter that did not run.
        """
        env = {
            key: value
            for key, value in os.environ.items()
            if key not in FILTER_ENV_VARS.values() and key != "GITHUB_OUTPUT"
        }
        simulated = {"rust": rust, "frontend": frontend, "python": python}
        for area, value in simulated.items():
            if value is not None:
                env[FILTER_ENV_VARS[area]] = value
        if github_output:
            env["GITHUB_OUTPUT"] = str(self.output_file)
        return subprocess.run(
            [sys.executable, str(RESOLVER_SCRIPT)],
            capture_output=True,
            text=True,
            env=env,
            cwd=self.tmp.name,
            timeout=60,
        )

    def _emitted_areas(self, proc: subprocess.CompletedProcess) -> list:
        """The parsed JSON list of the single `areas=` line on stdout."""
        lines = [line for line in proc.stdout.splitlines() if line.startswith("areas=")]
        self.assertEqual(len(lines), 1, f"exactly one areas= line expected: {proc.stdout!r}")
        return json.loads(lines[0].partition("=")[2])

    def _assert_single_area_line(self, proc: subprocess.CompletedProcess, areas: list) -> None:
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertEqual(self._emitted_areas(proc), areas)

    # ── la trampa: "false" es truthy en GitHub, aqui no ────────────────

    def test_simulated_dorny_false_strings_enable_nothing(self):
        # El caso de la trampa, tal cual vivio en ci.yml: dorny emite el
        # STRING "false" para dos areas y "true" para una. Con semantica
        # truthy, esos "false" habilitaban rust y python; aqui solo cuenta
        # el "true": la unica area en la lista es frontend.
        proc = self._run_resolver(rust="false", frontend="true", python="false")
        self._assert_single_area_line(proc, ["frontend"])
        self.assertNotIn("::notice::", proc.stdout)

    def test_false_strings_alone_never_count_as_detection(self):
        # Ni "0" ni cadenas con espacios ni el vacio: solo "true" detecta.
        proc = self._run_resolver(rust=" true ", frontend="0", python="")
        self._assert_single_area_line(proc, ["rust"])

    # ── el fallback: si nada detectado, probarlas todas ────────────────

    def test_all_false_falls_back_to_all_areas(self):
        proc = self._run_resolver(rust="false", frontend="false", python="false")
        self._assert_single_area_line(proc, ALL_AREAS)
        self.assertIn("::notice::El filtro de rutas no detecto areas", proc.stdout)

    def test_silent_dorny_falls_back_to_all_areas(self):
        # Ni una variable definida: el filtro no dijo nada (p. ej. una PR
        # solo-de-.github/). Correr todo es el fallo seguro; no correr nada,
        # el bug original que dejo PRs en verde sin ejecutar nada.
        proc = self._run_resolver()
        self._assert_single_area_line(proc, ALL_AREAS)
        self.assertIn("::notice::", proc.stdout)

    # ── deteccion parcial y orden ──────────────────────────────────────

    def test_partial_detection_runs_only_the_detected_area(self):
        proc = self._run_resolver(rust="true", frontend="false", python=None)
        self._assert_single_area_line(proc, ["rust"])

    def test_all_detected_lists_the_three_areas_in_fixed_order(self):
        proc = self._run_resolver(python="true", frontend="true", rust="true")
        # Orden fijo (rust, frontend, python), no el orden de las variables.
        self._assert_single_area_line(proc, ALL_AREAS)
        self.assertNotIn("::notice::", proc.stdout)

    def test_detected_subset_keeps_the_fixed_order(self):
        proc = self._run_resolver(rust=None, frontend="true", python="true")
        self._assert_single_area_line(proc, ["frontend", "python"])

    # ── pertenencia exacta: elementos, no substrings ───────────────────

    def test_membership_is_exact_elements_not_substrings(self):
        # El contrato del consumidor: contains sobre la lista parseada
        # compara elementos completos. Si el output fuera una cadena plana
        # ("frontend python"), buscar "front" o "end" colisionaria por
        # substring; sobre la lista JSON, ninguna consulta parcial acierta.
        proc = self._run_resolver(frontend="true")
        areas = self._emitted_areas(proc)
        self.assertEqual(areas, ["frontend"])
        self.assertTrue("frontend" in areas)
        for partial in ("front", "end", "rust", "python"):
            self.assertNotIn(partial, areas)
        proc = self._run_resolver(rust="true", frontend="true", python="true")
        areas = self._emitted_areas(proc)
        for area in ALL_AREAS:
            self.assertIn(area, areas)
        for partial in ("front", "end", "py", "ust"):
            self.assertNotIn(partial, areas)

    def test_emitted_value_is_a_json_array_not_a_plain_string(self):
        # Un solo valor, serializado como lista JSON: fromJSON del workflow
        # obtiene un array y contains pregunta por elementos exactos.
        proc = self._run_resolver(rust="true", python="true")
        raw_line = [line for line in proc.stdout.splitlines() if line.startswith("areas=")][0]
        raw_value = raw_line.partition("=")[2]
        self.assertEqual(raw_value, '["rust", "python"]')
        self.assertIsInstance(json.loads(raw_value), list)

    # ── escritura en $GITHUB_OUTPUT ────────────────────────────────────

    def test_github_output_file_receives_exactly_one_line(self):
        proc = self._run_resolver(frontend="true", github_output=True)
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertEqual(
            self.output_file.read_text(encoding="utf-8"),
            'areas=["frontend"]\n',
        )
        # El log tambien lleva la linea, para lectura humana de la corrida.
        self.assertIn("areas=", proc.stdout)


if __name__ == "__main__":
    unittest.main()

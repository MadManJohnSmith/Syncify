#!/usr/bin/env python3
"""Tests de comportamiento de las patas del job Rust (scripts/ci/rust_matrix.py, #368).

El job `rust` de ci.yml toma su `matrix.os` del output `rust_os` que emite este
script. La proteccion de `main` exige el contexto
`Rust (...) (windows-latest)` con `strict=true`: si la regla se equivoca hacia
"solo Ubuntu" en un PR a main, el PR no puede fusionarse; si se equivoca hacia
"las dos" en un PR a wave, el carril paga una pata que nadie exige.

Cada test cruza el CLI como subproceso con el evento simulado por entorno y,
cuando se pide, un fichero temporal en lugar de $GITHUB_OUTPUT. Nada de red ni
de Actions.
"""

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
MATRIX_SCRIPT = REPO_ROOT / "scripts" / "ci" / "rust_matrix.py"

UBUNTU = "ubuntu-latest"
WINDOWS = "windows-latest"


class RustMatrixCLITest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.output_file = Path(self.tmp.name) / "github_output.txt"

    def _run(self, *, event_name=None, base_ref=None, ref=None, github_output=False):
        env = {
            key: value
            for key, value in os.environ.items()
            if key not in ("GITHUB_EVENT_NAME", "GITHUB_BASE_REF", "GITHUB_REF", "GITHUB_OUTPUT")
        }
        for key, value in (
            ("GITHUB_EVENT_NAME", event_name),
            ("GITHUB_BASE_REF", base_ref),
            ("GITHUB_REF", ref),
        ):
            if value is not None:
                env[key] = value
        if github_output:
            env["GITHUB_OUTPUT"] = str(self.output_file)
        return subprocess.run(
            [sys.executable, str(MATRIX_SCRIPT)],
            capture_output=True,
            text=True,
            env=env,
            cwd=self.tmp.name,
            timeout=60,
        )

    def _emitted(self, proc):
        lines = [line for line in proc.stdout.splitlines() if line.startswith("rust_os=")]
        self.assertEqual(len(lines), 1, f"exactly one rust_os= line expected: {proc.stdout!r}")
        return json.loads(lines[0].partition("=")[2])

    def _assert_legs(self, proc, expected):
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertEqual(self._emitted(proc), expected)

    # ── main: las dos patas, que es lo que exige la proteccion ──────────

    def test_push_to_main_runs_both_legs(self):
        proc = self._run(event_name="push", ref="refs/heads/main")
        self._assert_legs(proc, [UBUNTU, WINDOWS])

    def test_pull_request_targeting_main_runs_both_legs(self):
        proc = self._run(event_name="pull_request", base_ref="main", ref="refs/pull/1/merge")
        self._assert_legs(proc, [UBUNTU, WINDOWS])

    # ── carriles: solo Ubuntu ──────────────────────────────────────────

    def test_pull_request_to_a_wave_branch_runs_ubuntu_only(self):
        proc = self._run(
            event_name="pull_request",
            base_ref="wave/ola-0-batch-4",
            ref="refs/pull/2/merge",
        )
        self._assert_legs(proc, [UBUNTU])

    def test_pull_request_to_dev_runs_ubuntu_only(self):
        proc = self._run(event_name="pull_request", base_ref="dev", ref="refs/pull/3/merge")
        self._assert_legs(proc, [UBUNTU])

    def test_push_to_any_other_branch_runs_ubuntu_only(self):
        proc = self._run(event_name="push", ref="refs/heads/wave/ola-0-batch-4")
        self._assert_legs(proc, [UBUNTU])

    def test_workflow_dispatch_runs_ubuntu_only(self):
        # Sin base_ref ni ref de main: el fallo seguro es la pata barata, que
        # no produce un check requerido de mas ni de menos en main.
        proc = self._run(event_name="workflow_dispatch", ref="refs/heads/main")
        self._assert_legs(proc, [UBUNTU])

    # ── serializacion y $GITHUB_OUTPUT ─────────────────────────────────

    def test_value_is_a_json_array(self):
        proc = self._run(event_name="push", ref="refs/heads/main")
        raw = [line for line in proc.stdout.splitlines() if line.startswith("rust_os=")][0]
        self.assertEqual(raw.partition("=")[2], '["ubuntu-latest", "windows-latest"]')

    def test_github_output_file_receives_the_same_line(self):
        proc = self._run(event_name="push", ref="refs/heads/main", github_output=True)
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertEqual(
            self.output_file.read_text(encoding="utf-8"),
            'rust_os=["ubuntu-latest", "windows-latest"]\n',
        )


if __name__ == "__main__":
    unittest.main()

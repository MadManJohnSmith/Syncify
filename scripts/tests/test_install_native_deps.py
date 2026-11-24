#!/usr/bin/env python3
"""Behavior tests for scripts/install_native_deps.sh (#250).

The native package list used to live twice: ci.yml installed 7 packages and
build-linux.yml installed 15, with a silent overlap. The script is now the
single source and both workflows consume it; both apt caches key on the
script hash, not on the whole workflow.

Every test crosses the script seam: it runs the bash script as a subprocess
with fake `sudo` and `apt-get` first in PATH. The fakes record their argv, so
the assertions read the update+install shape and the installed package list a
runner would execute, never the script's internal text.

No network, no apt, no sudo.
"""

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import yaml

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT = REPO_ROOT / "scripts" / "install_native_deps.sh"
SCRIPT_REF = "scripts/install_native_deps.sh"
CI_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "ci.yml"
BUILD_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "build-linux.yml"

# Unión exacta medida el 2026-11-24: build-linux aporta 15, ci aporta 7 y solo
# `flac` cae fuera de los 15 → 16 únicos. El issue hablaba de «superset de 15»;
# manda la unión medida.
EXPECTED_PACKAGES = (
    "build-essential",
    "curl",
    "ffmpeg",
    "file",
    "flac",
    "libasound2-dev",
    "libayatana-appindicator3-dev",
    "libchromaprint-tools",
    "libgtk-3-dev",
    "librsvg2-dev",
    "libssl-dev",
    "libwebkit2gtk-4.1-dev",
    "python3-pip",
    "python3-venv",
    "wget",
    "xvfb",
)

SCRIPT_HASH = "hashFiles('scripts/install_native_deps.sh')"

# Cada workflow guarda su propia entrada: `runner.os` es `Linux` tanto en
# ubuntu-latest (ci) como en ubuntu-22.04 (build), y una clave de actions/cache
# solo se guarda una vez. El prefijo evita que un job herede los .deb del otro.
EXPECTED_CACHE_KEYS = {
    "ci.yml": f"apt-v1-ci-${{{{ runner.os }}}}-${{{{ {SCRIPT_HASH} }}}}",
    "build-linux.yml": f"apt-v1-build-${{{{ runner.os }}}}-${{{{ {SCRIPT_HASH} }}}}",
}

# actions/cache corre como `runner`; apt deja `partial` y `lock` de root.
CHOWN_APT_ARCHIVES = 'sudo chown -R "$(id -u):$(id -g)" /var/cache/apt/archives'

# sudo de verdad ejecutaria el comando; el fake registra la invocacion y delega
# en el apt-get falso que tambien esta primero en PATH.
FAKE_SUDO = r"""#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "${SYNCIFY_FAKE_SUDO_LOG:?}"
exec "$@"
"""

FAKE_APT_GET = r"""#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "${SYNCIFY_FAKE_APT_LOG:?}"
"""


def _write(path: Path, content: bytes, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    path.chmod(mode)


def _steps(path: Path):
    workflow = yaml.safe_load(path.read_text(encoding="utf-8"))
    for job in (workflow.get("jobs") or {}).values():
        for step in job.get("steps") or []:
            if isinstance(step, dict):
                yield step


# El script es maquinaria apt de Linux; en Windows la pata Python no puede
# ejecutarlo (mismo criterio que el resto de tests de empaquetado POSIX).
@unittest.skipIf(sys.platform.startswith("win"), "Linux apt machinery: POSIX tooling")
class InstallNativeDepsBehaviorTests(unittest.TestCase):
    """«Lista única de paquetes nativos» contra fakes, no contra su texto."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name)

        self.fakes = root / "fakes"
        _write(self.fakes / "sudo", FAKE_SUDO.encode(), 0o755)
        _write(self.fakes / "apt-get", FAKE_APT_GET.encode(), 0o755)

        self.apt_log = root / "apt.log"
        self.sudo_log = root / "sudo.log"
        self.apt_log.write_text("", encoding="utf-8")
        self.sudo_log.write_text("", encoding="utf-8")

        self.env = {
            **os.environ,
            "PATH": f"{self.fakes}:{os.environ['PATH']}",
            "SYNCIFY_FAKE_APT_LOG": str(self.apt_log),
            "SYNCIFY_FAKE_SUDO_LOG": str(self.sudo_log),
        }

    def _run(self):
        return subprocess.run(
            ["bash", str(SCRIPT)],
            capture_output=True,
            text=True,
            env=self.env,
            timeout=60,
        )

    def _apt_calls(self):
        """(retries, command, args) por invocacion registrada de apt-get."""
        calls = []
        for line in self.apt_log.read_text(encoding="utf-8").splitlines():
            tokens = line.split()
            retries = None
            while tokens[:1] == ["-o"]:
                retries = tokens[1]
                tokens = tokens[2:]
            self.assertTrue(tokens, f"apt-get sin comando: {line!r}")
            calls.append((retries, tokens[0], tokens[1:]))
        return calls

    def _sudo_calls(self):
        return self.sudo_log.read_text(encoding="utf-8").splitlines()

    def test_runs_update_then_installs_the_full_superset(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stderr)

        calls = self._apt_calls()
        self.assertEqual(
            [command for _, command, _ in calls],
            ["update", "install"],
            f"forma esperada `update` + `install`, medida: {calls}",
        )
        self.assertEqual(len(self._sudo_calls()), 2, self._sudo_calls())

        _, _, args = calls[1]
        self.assertEqual(
            args[0],
            "-y",
            f"`install` debe llevar -y como primer argumento: {args}",
        )
        packages = args[1:]
        self.assertEqual(
            sorted(packages),
            sorted(EXPECTED_PACKAGES),
            "la lista instalada no es la unión medida de ambos workflows",
        )
        self.assertEqual(
            len(packages),
            len(set(packages)),
            f"la lista tiene paquetes duplicados: {packages}",
        )

    def test_every_apt_get_call_retries_transient_mirror_failures(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stderr)

        for retries, command, _ in self._apt_calls():
            self.assertEqual(
                retries,
                "Acquire::Retries=5",
                f"`apt-get {command}` no lleva Acquire::Retries=5: {retries!r}",
            )

    def test_workflows_delegate_the_package_list_to_the_script(self):
        for label, path in (("ci.yml", CI_WORKFLOW), ("build-linux.yml", BUILD_WORKFLOW)):
            with self.subTest(workflow=label):
                steps = list(_steps(path))
                consumers = [
                    step
                    for step in steps
                    if isinstance(step.get("run"), str) and SCRIPT_REF in step["run"]
                ]
                self.assertEqual(
                    len(consumers),
                    1,
                    f"{label} debe invocar {SCRIPT_REF} exactamente una vez",
                )
                if label == "ci.yml":
                    # El job Rust corre desde src-tauri/ por defecto.
                    self.assertEqual(
                        consumers[0].get("working-directory"),
                        "${{ github.workspace }}",
                        "el paso de ci.yml necesita la raíz del repo para resolver el script",
                    )

                for step in steps:
                    run = step.get("run")
                    if not isinstance(run, str) or "apt-get install" not in run:
                        continue
                    # Los plugins de GStreamer son otro contrato y no forman
                    # parte de #250: su lista se queda donde está.
                    if "gstreamer1.0-" in run:
                        continue
                    self.assertIn(
                        SCRIPT_REF,
                        run,
                        f"{label} conserva una instalación apt paralela: {run!r}",
                    )
                    leaked = [package for package in EXPECTED_PACKAGES if package in run]
                    self.assertEqual(
                        leaked,
                        [],
                        f"{label} duplica la lista de paquetes: {leaked}",
                    )

    def test_both_apt_caches_key_on_the_script_hash_and_fix_permissions(self):
        for label, path in (("ci.yml", CI_WORKFLOW), ("build-linux.yml", BUILD_WORKFLOW)):
            with self.subTest(workflow=label):
                steps = list(_steps(path))
                cache_steps = [
                    step
                    for step in steps
                    if str(step.get("uses", "")).startswith("actions/cache")
                    and (step.get("with") or {}).get("path") == "/var/cache/apt/archives"
                ]
                self.assertEqual(
                    len(cache_steps),
                    1,
                    f"{label} debe cachear /var/cache/apt/archives una sola vez",
                )
                key = cache_steps[0]["with"].get("key", "")
                self.assertEqual(
                    key,
                    EXPECTED_CACHE_KEYS[label],
                    f"{label} no declara la clave esperada con prefijo propio y hash del script: {key!r}",
                )
                self.assertIn(
                    SCRIPT_HASH,
                    key,
                    f"{label} no invalida la caché apt con el hash del script: {key!r}",
                )

                # El post-save de actions/cache corre como `runner` y no puede
                # tar un arbol de root: hacen falta dos cesiones de propiedad,
                # una antes de restaurar y otra despues de todo uso de apt.
                chown_steps = [
                    (index, step)
                    for index, step in enumerate(steps)
                    if CHOWN_APT_ARCHIVES in str(step.get("run") or "")
                ]
                self.assertEqual(
                    len(chown_steps),
                    2,
                    f"{label} necesita chown antes del restore y antes del post-save: "
                    f"{[step.get('name') for _, step in chown_steps]}",
                )
                cache_index = steps.index(cache_steps[0])
                consumer_index = next(
                    index
                    for index, step in enumerate(steps)
                    if isinstance(step.get("run"), str) and SCRIPT_REF in step["run"]
                )
                self.assertLess(
                    chown_steps[0][0],
                    cache_index,
                    f"{label}: el chown de restore debe ir antes de actions/cache",
                )
                apt_indices = [
                    index
                    for index, step in enumerate(steps)
                    if isinstance(step.get("run"), str) and "apt-get" in step["run"]
                ]
                last_apt_index = max(apt_indices + [consumer_index])
                self.assertGreater(
                    chown_steps[1][0],
                    last_apt_index,
                    f"{label}: el chown de post-save debe ir después del último apt",
                )


if __name__ == "__main__":
    unittest.main()

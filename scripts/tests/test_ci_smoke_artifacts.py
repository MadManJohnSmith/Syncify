#!/usr/bin/env python3
"""
Behavior tests for the Linux artifact smokes (scripts/ci/smoke_artifacts.sh, #245).

The script runs the canonical import smoke against the interpreter mounted in
each FINAL artifact (tarball, AppImage, DEB) and then launches the extracted
AppRun under xvfb, failing explicitly on the EGL abort, on WebKit's
"Unable to spawn a new child process", on a missing "Syncify starting" marker
and on any exit other than timeout's 124 (the app must stay alive).

Every test crosses the script seam: the bash script runs as a subprocess
against a disposable dist/linux fixture, with fake AppImage, dpkg-deb and
xvfb-run injected via PATH and fixture interpreters that record how they were
invoked. Assertions read observable behavior — exit codes, ::error::
diagnostics, the exact import statement the artifact interpreter received,
which binary xvfb-run launched — never the script's internal text and never
workflow YAML.

The canonical import list is not copied here either: the test imports the
provider (scripts/dependency_manager.py) and asserts the artifact interpreters
received ITS statement. This is the declared destination of
TestLinuxBundledPythonPackaging.test_final_smoke_test_covers_all_three_artifacts,
which froze the now-deleted inline smoke as workflow strings; it died in #245
and lives here as behavior.

No network, no real AppImage/dpkg/xvfb.
"""

import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT = REPO_ROOT / "scripts" / "ci" / "smoke_artifacts.sh"

sys.path.insert(0, str(REPO_ROOT / "scripts"))
import dependency_manager  # noqa: E402  (repo scripts live beside the suite)

CANONICAL_STATEMENT = dependency_manager.smoke_import_statement()

# Interprete de fixture: registra argv y las variables PYTHON*/LD_LIBRARY_PATH
# que le llegan. El proveedor lo ejecuta con `-c "<imports>; print('OK')"`.
FAKE_INTERPRETER = r"""#!/usr/bin/env python3
import json, os, sys

entry = {
    "exe": sys.argv[0],
    "args": sys.argv[1:],
    "env": {
        key: value
        for key, value in os.environ.items()
        if key.startswith("PYTHON") or key == "LD_LIBRARY_PATH"
    },
}
with open(os.environ["SYNCIFY_SMOKE_FAKE_LOG"], "a", encoding="utf-8") as fh:
    fh.write(json.dumps(entry) + "\n")
print("OK")
"""

# Un AppImage de verdad responde a --appimage-extract creando squashfs-root.
# En bash y no en Python: la extraccion no tiene por que arrastrar el entorno
# de Python del anfitrion (el test de env lo exporta a proposito).
FAKE_APPIMAGE = r"""#!/usr/bin/env bash
set -euo pipefail
if [[ $# -ne 1 || $1 != "--appimage-extract" ]]; then
  echo "fake AppImage: unexpected invocation: $*" >&2
  exit 64
fi
cp -a "$SYNCIFY_SMOKE_APPIMAGE_TEMPLATE" ./squashfs-root
"""

FAKE_DPKG_DEB = r"""#!/usr/bin/env bash
set -euo pipefail
if [[ $# -ne 3 || $1 != "-x" ]]; then
  echo "fake dpkg-deb: expected -x DEB DIR" >&2
  exit 64
fi
mkdir -p "$3"
cp -a "$SYNCIFY_SMOKE_DEB_PAYLOAD/." "$3/"
"""

# xvfb-run -a COMANDO: registra que binario se lanzo (AppRun, no el pelado) y
# lo executa, como la guarda real.
FAKE_XVFB_RUN = r"""#!/usr/bin/env python3
import json, os, sys

args = sys.argv[1:]
with open(os.environ["SYNCIFY_SMOKE_FAKE_LOG"], "a", encoding="utf-8") as fh:
    fh.write(json.dumps({"tool": "xvfb-run", "args": args}) + "\n")
if len(args) != 2 or args[0] != "-a":
    sys.stderr.write("fake xvfb-run: expected -a COMMAND\n")
    sys.exit(64)
os.execv(args[1], [args[1]])
"""

# Cada escenario imita una salida real del AppRun; el guion decide si la
# guarda debe pasar o fallar.
FAKE_APPRUN = r"""#!/usr/bin/env python3
import json, os, sys, time

with open(os.environ["SYNCIFY_SMOKE_FAKE_LOG"], "a", encoding="utf-8") as fh:
    fh.write(json.dumps({"tool": "apprun", "args": sys.argv[1:]}) + "\n")

scenario = os.environ.get("SYNCIFY_SMOKE_APP_SCENARIO", "alive")
if scenario == "alive":
    print("Syncify starting", flush=True)
    try:
        time.sleep(30)
    except KeyboardInterrupt:
        pass
elif scenario == "egl":
    print("Syncify starting", flush=True)
    print("Could not create default EGL display: EGL_BAD_PARAMETER. Aborting...", flush=True)
    sys.exit(1)
elif scenario == "webkit":
    print("Syncify starting", flush=True)
    print("Unable to spawn a new child process", flush=True)
    sys.exit(1)
elif scenario == "silent":
    sys.exit(0)
elif scenario == "early":
    print("Syncify starting", flush=True)
    sys.exit(0)
else:
    sys.stderr.write("fake AppRun: unknown scenario: %s\n" % scenario)
    sys.exit(64)
"""

# El binario pelado: si la guarda lo lanzase a el, quedaria este rastro y el
# test lo veria. Tambien imprime "Syncify starting" para que el unico delator
# sea el log de invocacion, no el marcador.
FAKE_BARE_BINARY = r"""#!/usr/bin/env python3
import json, os, sys, time

with open(os.environ["SYNCIFY_SMOKE_FAKE_LOG"], "a", encoding="utf-8") as fh:
    fh.write(json.dumps({"tool": "bare", "args": sys.argv[1:]}) + "\n")
print("Syncify starting", flush=True)
time.sleep(30)
"""


def _write(path: Path, content, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(content, str):
        content = content.encode("utf-8")
    path.write_bytes(content)
    path.chmod(mode)


# Maquinaria de release Linux: el script usa tar, dpkg-deb, xvfb-run y el
# proveedor del runtime. En Windows el subprocess cae en el stub de WSL; los
# tests corren donde la maquinaria existe (patron de test_ci_package_deb).
@unittest.skipIf(sys.platform.startswith("win"), "Linux release machinery: POSIX tooling")
class SmokeArtifactsBehaviorTests(unittest.TestCase):
    """«Tres artefactos importan + AppRun vivo», contra fixtures."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

        self.fakes = self.root / "fakes"
        self.fakes.mkdir()
        _write(self.fakes / "dpkg-deb", FAKE_DPKG_DEB, 0o755)
        _write(self.fakes / "xvfb-run", FAKE_XVFB_RUN, 0o755)
        self.log = self.root / "invocations.jsonl"
        self.log.write_text("", encoding="utf-8")

        self.dist = self.root / "dist" / "linux"
        self.dist.mkdir(parents=True)

        # AppDir que el fake AppImage materializa al extraer.
        self.template = self.root / "appdir"
        _write(
            self.template / "resources" / "python" / "bin" / "python",
            FAKE_INTERPRETER,
            0o755,
        )
        _write(self.template / "AppRun", FAKE_APPRUN, 0o755)
        _write(self.template / "usr" / "bin" / "syncify-tauri", FAKE_BARE_BINARY, 0o755)
        (self.template / "usr" / "lib").mkdir(parents=True)
        (self.template / "usr" / "lib" / "libwebkit2gtk-4.1.so.0").write_text(
            "webkit", encoding="utf-8"
        )

        # Tarball final: el .tar.gz que se publica, con el python/ hermano de
        # scripts/ (forma install-runtime linux).
        tarball_src = self.root / "tarball-src"
        _write(
            tarball_src / "Syncify-Linux-Raw" / "python" / "bin" / "python3",
            FAKE_INTERPRETER,
            0o755,
        )
        self.tarball = self.dist / "Syncify-Linux-x86_64.tar.gz"
        with tarfile.open(self.tarball, "w:gz") as tf:
            tf.add(tarball_src / "Syncify-Linux-Raw", arcname="Syncify-Linux-Raw")

        self.appimage = self.dist / "syncify_0.1.0_amd64.AppImage"
        _write(self.appimage, FAKE_APPIMAGE, 0o755)

        # DEB inyectado: _up_/python/bin/python3 junto al _up_/scripts del
        # marcador (package_deb.sh).
        self.deb_payload = self.root / "deb-payload"
        _write(
            self.deb_payload / "usr" / "lib" / "syncify" / "_up_" / "python" / "bin" / "python3",
            FAKE_INTERPRETER,
            0o755,
        )
        _write(
            self.deb_payload / "usr" / "lib" / "syncify" / "_up_" / "scripts" / "dependency_manager.py",
            b"marker",
        )
        self.deb = self.dist / "syncify_0.1.0_amd64.deb"
        _write(self.deb, b"not-a-real-deb")

    # ── harness ─────────────────────────────────────────────────────────

    def _base_env(self, path: str) -> dict:
        env = {
            key: value
            for key, value in os.environ.items()
            if not key.startswith("SYNCIFY_")
        }
        env.update(
            {
                "PATH": path,
                "SYNCIFY_SMOKE_FAKE_LOG": str(self.log),
                "SYNCIFY_SMOKE_APPIMAGE_TEMPLATE": str(self.template),
                "SYNCIFY_SMOKE_DEB_PAYLOAD": str(self.deb_payload),
                "SYNCIFY_SMOKE_APP_SCENARIO": "alive",
            }
        )
        return env

    def _run(self, scenario: str = "alive", path: str | None = None,
             env_updates: dict | None = None) -> subprocess.CompletedProcess:
        env = self._base_env(path or f"{self.fakes}:{os.environ['PATH']}")
        env["SYNCIFY_SMOKE_APP_SCENARIO"] = scenario
        if env_updates:
            env.update(env_updates)
        return subprocess.run(
            ["bash", str(SCRIPT), "--dist-dir", str(self.dist), "--apprun-timeout", "1"],
            capture_output=True,
            text=True,
            env=env,
            timeout=120,
        )

    def _entries(self) -> list[dict]:
        return [
            json.loads(line)
            for line in self.log.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]

    def _smokes(self) -> list[dict]:
        return [entry for entry in self._entries() if "-c" in entry.get("args", [])]

    @staticmethod
    def _output(proc: subprocess.CompletedProcess) -> str:
        return proc.stdout + proc.stderr

    # ── imports canonicos en los tres artefactos ────────────────────────

    def test_smokes_every_artifact_with_the_provider_canonical_imports(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, self._output(proc))

        smokes = self._smokes()
        self.assertEqual(len(smokes), 3, smokes)
        for entry in smokes:
            self.assertEqual(entry["args"][0], "-c")
            self.assertIn(
                CANONICAL_STATEMENT,
                entry["args"][1],
                "el artefacto no recibio la lista canonica del proveedor",
            )

        executables = [entry["exe"] for entry in smokes]
        self.assertTrue(
            any(p.endswith("Syncify-Linux-Raw/python/bin/python3") for p in executables),
            executables,
        )
        self.assertTrue(
            any(p.endswith("squashfs-root/resources/python/bin/python") for p in executables),
            executables,
        )
        self.assertTrue(
            any(p.endswith("_up_/python/bin/python3") for p in executables),
            executables,
        )
        self.assertIn("OK tarball", proc.stdout)
        self.assertIn("OK appimage", proc.stdout)
        self.assertIn("OK deb", proc.stdout)
        self.assertIn("Arranque verificado", proc.stdout)

    def test_artifact_smokes_inherit_no_python_env_from_the_host(self):
        proc = self._run(
            env_updates={
                "PYTHONHOME": "/host/python",
                "PYTHONPATH": "/host/site-packages",
                "PYTHONDONTWRITEBYTECODE": "1",
            }
        )
        self.assertEqual(proc.returncode, 0, self._output(proc))

        smokes = self._smokes()
        self.assertEqual(len(smokes), 3, smokes)
        for entry in smokes:
            leaked = sorted(entry["env"])
            self.assertEqual(
                leaked,
                [key for key in leaked if key == "LD_LIBRARY_PATH"],
                f"el entorno del host ensucio el smoke: {entry['env']}",
            )
        # El AppImage conserva el LD_LIBRARY_PATH que deja AppRun; los otros
        # dos no lo necesitan.
        appimage = next(e for e in smokes if "squashfs-root" in e["exe"])
        self.assertTrue(
            appimage["env"]["LD_LIBRARY_PATH"].endswith("squashfs-root/usr/lib"),
            appimage["env"],
        )

    # ── guarda EGL/xvfb ─────────────────────────────────────────────────

    def test_guard_launches_the_extracted_apprun_under_xvfb(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, self._output(proc))

        entries = self._entries()
        xvfb = [entry for entry in entries if entry.get("tool") == "xvfb-run"]
        self.assertEqual(len(xvfb), 1, entries)
        self.assertEqual(xvfb[0]["args"][0], "-a")
        self.assertTrue(
            xvfb[0]["args"][1].endswith("/squashfs-root/AppRun"),
            xvfb[0]["args"],
        )
        self.assertTrue(
            any(entry.get("tool") == "apprun" for entry in entries),
            "el AppRun extraido no se ejecuto",
        )
        # Lanzar usr/bin/syncify-tauri a pelo deja a WebKit sin los hooks de
        # AppRun; la guarda no debe hacerlo jamas.
        self.assertFalse(
            any(entry.get("tool") == "bare" for entry in entries),
            "la guarda lanzo el binario pelado en vez del AppRun",
        )

    def test_egl_abort_is_an_explicit_failure(self):
        proc = self._run(scenario="egl")
        output = self._output(proc)
        self.assertNotEqual(proc.returncode, 0, output)
        self.assertIn("::error::", output)
        self.assertIn("display EGL", output)
        self.assertNotIn("Arranque verificado", output)

    def test_webkit_child_spawn_failure_is_an_explicit_failure(self):
        proc = self._run(scenario="webkit")
        output = self._output(proc)
        self.assertNotEqual(proc.returncode, 0, output)
        self.assertIn("::error::", output)
        self.assertIn("WebKit", output)

    def test_missing_startup_marker_fails(self):
        proc = self._run(scenario="silent")
        output = self._output(proc)
        self.assertNotEqual(proc.returncode, 0, output)
        self.assertIn("::error::", output)
        self.assertIn("Syncify starting", output)

    def test_app_exiting_before_timeout_fails(self):
        proc = self._run(scenario="early")
        output = self._output(proc)
        self.assertNotEqual(proc.returncode, 0, output)
        self.assertIn("::error::", output)
        self.assertIn("codigo 0", output)

    def test_appimage_without_apprun_aborts_before_launching(self):
        (self.template / "AppRun").unlink()
        proc = self._run()
        output = self._output(proc)
        self.assertNotEqual(proc.returncode, 0, output)
        self.assertIn("::error::", output)
        self.assertIn("AppRun", output)
        self.assertFalse(
            any(entry.get("tool") == "xvfb-run" for entry in self._entries()),
            "xvfb-run se ejecuto sin AppRun",
        )

    # ── guardas de entrada ──────────────────────────────────────────────

    def test_missing_artifact_aborts_with_diagnostic(self):
        for label, artifact, fragment in (
            ("tarball", self.tarball, "tarball"),
            ("appimage", self.appimage, "AppImage"),
            ("deb", self.deb, "DEB"),
        ):
            with self.subTest(artifact=label):
                hidden = artifact.with_name(artifact.name + ".hidden")
                artifact.rename(hidden)
                try:
                    proc = self._run()
                finally:
                    hidden.rename(artifact)
                output = self._output(proc)
                self.assertNotEqual(proc.returncode, 0, output)
                self.assertIn("::error::", output)
                self.assertIn(fragment, output)

    def test_missing_xvfb_run_aborts_with_diagnostic(self):
        # PATH minimo sin xvfb-run: el fallo debe ser explicito, no un skip.
        shim = self.root / "noxvfb-bin"
        shim.mkdir()
        for tool in (
            "bash", "basename", "cat", "cp", "dirname", "env", "find", "grep",
            "gzip", "head", "ls", "mkdir", "mktemp", "python3", "rm", "tar",
            "timeout",
        ):
            found = shutil.which(tool)
            self.assertIsNotNone(found, f"falta {tool} para el PATH minimo del test")
            (shim / tool).symlink_to(found)
        shutil.copy2(self.fakes / "dpkg-deb", shim / "dpkg-deb")

        proc = self._run(path=str(shim))
        output = self._output(proc)
        self.assertNotEqual(proc.returncode, 0, output)
        self.assertIn("::error::", output)
        self.assertIn("xvfb-run", output)


if __name__ == "__main__":
    unittest.main()

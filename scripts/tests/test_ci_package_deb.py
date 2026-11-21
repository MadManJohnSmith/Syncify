#!/usr/bin/env python3
"""
Behavior tests for the DEB packaging script (scripts/ci/package_deb.sh, #244).

The script mounts the pinned CPython runtime (`dependency_manager.py
install-runtime` -> bundle/python) inside a Tauri DEB: locates `_up_/scripts`
by the dependency_manager.py marker, copies `python/` beside it, regenerates
DEBIAN/md5sums from the final tree and repacks with --root-owner-group.

Every test crosses the script seam: it runs the bash script as a subprocess
against a disposable fixture DEB, with a fake `dpkg-deb` injected via PATH.
The fake records every invocation and builds the repacked artifact as a real
tar.gz, so assertions read the artifact a user would install — injected tree,
md5sums content, owner-group flags — never the script's internal text. This
is the declared destination of the frozen assertions that
TestLinuxBundledPythonPackaging.test_deb_injection_places_python_beside_up_scripts
held about the now-deleted inline step: they died in #244 and live here as
behavior.

No network, no real dpkg. Mirrors the harness of test_release_gate.py.
"""

import hashlib
import os
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT = REPO_ROOT / "scripts" / "ci" / "package_deb.sh"

# El fake no se limita a responder: exige el contrato de la invocacion. -R
# despliega el payload indicado por el entorno (no sabe leer DEBs reales y no
# hace falta); -b solo acepta el repack con --root-owner-group y produce un
# tar.gz real para que las aserciones lean el artefacto final.
FAKE_DPKG_DEB = r"""#!/usr/bin/env bash
set -euo pipefail
LOG=${SYNCIFY_FAKE_DPKG_LOG:?}
echo "$*" >> "$LOG"
if [[ $1 == "-R" || $1 == "--raw-extract" ]]; then
  mkdir -p "$3"
  cp -a "$SYNCIFY_FAKE_DEB_PAYLOAD/." "$3/"
elif [[ $1 == "--root-owner-group" && $2 == "-b" ]]; then
  ( cd "$3" && tar -czf "$4" . )
else
  echo "fake dpkg-deb: unexpected invocation: $*" >&2
  exit 64
fi
"""


def _write(path: Path, content: bytes, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    path.chmod(mode)


# Maquinaria de release Linux: package_deb.sh es un script POSIX (dpkg-deb,
# fakeroot). En Windows el subprocess cae en el stub de WSL; los tests corren
# donde la maquinaria existe.
@unittest.skipIf(sys.platform.startswith("win"), "Linux release machinery: POSIX tooling")
class PackageDebBehaviorTests(unittest.TestCase):
    """«DEB con runtime inyectado» contra fixtures, no contra su texto."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name)

        # Fake dpkg-deb primero en PATH; log de invocaciones aparte.
        self.fakes = root / "fakes"
        _write(self.fakes / "dpkg-deb", FAKE_DPKG_DEB.encode(), 0o755)
        self.invocations = root / "invocations.log"
        self.invocations.write_text("", encoding="utf-8")

        # DEB de fragmento: DEBIAN/ con md5sums PODRIDO (hash inventado para
        # usr/bin/syncify) y _up_/scripts con el marcador contractual.
        self.payload = root / "payload"
        _write(self.payload / "DEBIAN" / "control", b"Package: syncify\n")
        _write(
            self.payload / "DEBIAN" / "md5sums",
            b"deadbeefdeadbeefdeadbeefdeadbeef  usr/bin/syncify\n",
        )
        _write(self.payload / "usr" / "bin" / "syncify", b"ELF-binary")
        up_scripts = self.payload / "usr" / "lib" / "syncify" / "_up_" / "scripts"
        _write(up_scripts / "dependency_manager.py", b"marker")
        _write(up_scripts / "auth_bridge.py", b"bridge")
        _write(up_scripts / "services" / "qobuz_service.py", b"svc")

        # Runtime montado (forma install-runtime linux): bin/python3 real y
        # bin/python como symlink, como deja el tarball standalone.
        self.runtime = root / "bundle" / "python"
        _write(self.runtime / "bin" / "python3", b"PY3", 0o755)
        _write(self.runtime / "lib" / "python3.11" / "site.py", b"site")
        pylink = self.runtime / "bin" / "python"
        pylink.symlink_to("python3")

        # El "DEB" de entrada es un fichero cualquiera: el fake -R ignora su
        # contenido y despliega el payload.
        self.deb = root / "syncify_0.1.0_amd64.deb"
        _write(self.deb, b"not-a-real-deb")

        self.env = {
            **os.environ,
            "PATH": f"{self.fakes}:{os.environ['PATH']}",
            "SYNCIFY_FAKE_DEB_PAYLOAD": str(self.payload),
            "SYNCIFY_FAKE_DPKG_LOG": str(self.invocations),
        }

    def _run(self, deb=None, runtime=None, payload=None):
        env = dict(self.env)
        if payload is not None:
            # El fake -R despliega el payload que el test pida, no el del
            # fixture por defecto.
            env["SYNCIFY_FAKE_DEB_PAYLOAD"] = str(payload)
        return subprocess.run(
            [
                "bash",
                str(SCRIPT),
                str(deb if deb is not None else self.deb),
                str(runtime if runtime is not None else self.runtime),
            ],
            capture_output=True,
            text=True,
            env=env,
            timeout=120,
        )

    def _read_invocations(self):
        return self.invocations.read_text(encoding="utf-8").splitlines()

    def _extract_repacked(self) -> Path:
        """El artefacto que el fake -b produjo: el DEB que instalaria el usuario."""
        dest = Path(self.tmp.name) / "repacked"
        with tarfile.open(self.deb, "r:gz") as tf:
            tf.extractall(dest, filter="data")
        return dest

    # ═══════════════════════════════════════════════════════
    # Camino feliz
    # ═══════════════════════════════════════════════════════

    def test_injects_python_beside_up_scripts_and_repacks_with_root_owner_group(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stderr)

        tree = self._extract_repacked()
        injected = tree / "usr" / "lib" / "syncify" / "_up_" / "python"
        # python/ HERMANO del _up_/scripts: el candidate que acepta
        # cmd_utils::packaged_python_candidates.
        self.assertTrue((injected.parent / "scripts" / "dependency_manager.py").is_file())
        self.assertTrue((injected / "bin" / "python3").is_file())
        self.assertTrue((injected / "lib" / "python3.11" / "site.py").is_file())
        # cp -a conserva el symlink bin/python -> python3 del standalone.
        self.assertTrue((injected / "bin" / "python").is_symlink())

        # Repack exactamente dos llamadas: -R para desplegar y -b con
        # --root-owner-group (propietario root:root, no el del builder).
        self.assertEqual(len(self._read_invocations()), 2, self._read_invocations())
        self.assertIn("--root-owner-group", self._read_invocations()[1])

    def test_repacked_tree_keeps_original_payload_files(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        tree = self._extract_repacked()
        for kept in (
            tree / "DEBIAN" / "control",
            tree / "usr" / "bin" / "syncify",
            tree / "usr" / "lib" / "syncify" / "_up_" / "scripts" / "auth_bridge.py",
            tree / "usr" / "lib" / "syncify" / "_up_" / "scripts" / "services" / "qobuz_service.py",
        ):
            self.assertTrue(kept.is_file(), f"missing after repack: {kept}")

    # ═══════════════════════════════════════════════════════
    # md5sums regenerado, verificado contra fixture
    # ═══════════════════════════════════════════════════════

    def test_md5sums_regenerated_matches_fixture_exactly(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        tree = self._extract_repacked()

        actual = (tree / "DEBIAN" / "md5sums").read_text(encoding="utf-8")

        # Esperado reconstruido desde el arbol final: cada fichero de datos
        # (DEBIAN/ fuera), ruta relativa sin ./, `<md5><dos espacios><ruta>`,
        # ordenado. Es el formato que dpkg espera verificar.
        files = sorted(
            rel
            for p in tree.rglob("*")
            if p.is_file()
            and not p.is_symlink()
            and (rel := p.relative_to(tree).as_posix()).split("/")[0] != "DEBIAN"
        )
        expected = "".join(
            f"{hashlib.md5((tree / rel).read_bytes()).hexdigest()}  {rel}\n"
            for rel in files
        )
        self.assertEqual(actual, expected)
        # Explicito: la inyeccion tambien queda verificada (los ficheros del
        # runtime aparecen con su hash real, no con el PODRIDO del fixture).
        self.assertIn("usr/lib/syncify/_up_/python/bin/python3\n", actual)
        self.assertNotIn("deadbeef", actual)
        # Los symlinks no entran (dpkg solo lista ficheros regulares) y
        # DEBIAN/ tampoco.
        self.assertNotIn("python/bin/python\n", actual)
        self.assertNotIn("DEBIAN/", actual)

    # ═══════════════════════════════════════════════════════
    # Caminos de fallo con diagnostico
    # ═══════════════════════════════════════════════════════

    def test_deb_without_up_scripts_marker_aborts_without_repack(self):
        # Payload sin _up_/scripts: nada que localizar, y NINGUN repack (no
        # se publica un DEB sin Python creyendo que lo lleva).
        empty = Path(self.tmp.name) / "payload-empty"
        _write(empty / "DEBIAN" / "control", b"Package: syncify\n")
        deb = Path(self.tmp.name) / "no-marker.deb"
        _write(deb, b"not-a-real-deb")

        proc = self._run(deb=deb, payload=empty)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("::error::", proc.stdout)
        self.assertIn("_up_/scripts", proc.stdout)
        # Desempaquetar si ocurrio (-R); REEMPAQUETAR no: el DEB de entrada
        # queda intacto, no se publica uno sin Python creyendo que lo lleva.
        self.assertFalse(
            any(" -b " in line for line in self._read_invocations()),
            "repacked a DEB without _up_/scripts",
        )

    def test_payload_without_debian_dir_aborts(self):
        broken = Path(self.tmp.name) / "payload-broken"
        _write(broken / "usr" / "lib" / "syncify" / "_up_" / "scripts" / "dependency_manager.py", b"m")
        deb = Path(self.tmp.name) / "no-debian.deb"
        _write(deb, b"not-a-real-deb")

        proc = self._run(deb=deb, payload=broken)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("::error::", proc.stdout)
        self.assertIn("DEBIAN", proc.stdout)

    def test_missing_deb_file_aborts(self):
        proc = self._run(deb=Path(self.tmp.name) / "missing.deb")
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("::error::", proc.stdout)

    def test_missing_runtime_dir_aborts(self):
        proc = self._run(runtime=Path(self.tmp.name) / "missing-runtime")
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("::error::", proc.stdout)

    def test_runtime_without_interpreter_aborts(self):
        # Forma que no es la de install-runtime: sin bin/python3 no hay
        # paquete que valga; mejor morir aqui que publicar un DEB roto.
        bare = Path(self.tmp.name) / "bare-runtime"
        _write(bare / "bin" / "python", b"not the canonical name")
        proc = self._run(runtime=bare)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("::error::", proc.stdout)
        self.assertIn("bin/python3", proc.stdout)

    def test_deb_already_injected_aborts(self):
        # Payload que ya trae _up_/python: cp -a anidaria el arbol dentro
        # (python/python) y el paquete saldria roto sin ruido.
        twice = Path(self.tmp.name) / "payload-twice"
        up = twice / "usr" / "lib" / "syncify" / "_up_"
        _write(twice / "DEBIAN" / "control", b"Package: syncify\n")
        _write(up / "scripts" / "dependency_manager.py", b"marker")
        _write(up / "python" / "bin" / "python3", b"PY3-old")
        deb = Path(self.tmp.name) / "already-injected.deb"
        _write(deb, b"not-a-real-deb")

        proc = self._run(deb=deb, payload=twice)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("::error::", proc.stdout)
        self.assertIn("dos veces", proc.stdout)
        self.assertFalse(
            any(" -b " in line for line in self._read_invocations()),
            "repacked an already-injected DEB",
        )

    def test_wrong_arity_shows_usage(self):
        proc = subprocess.run(
            ["bash", str(SCRIPT), "only-one-arg"],
            capture_output=True,
            text=True,
            env=self.env,
            timeout=120,
        )
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("uso:", proc.stderr)


if __name__ == "__main__":
    unittest.main()

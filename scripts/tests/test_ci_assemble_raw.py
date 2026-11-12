#!/usr/bin/env python3
"""
Behavior tests for the raw tarball assembly (scripts/ci/assemble_raw.sh, #244).

The script builds dist/linux/Syncify-Linux-Raw — raw binary + bin/ (ffmpeg,
ffprobe, fpcalc placed by the provider with a triple) + scripts/ + the
autonomous python/ from bundle/python — and compresses it into
Syncify-Linux-x86_64.tar.gz. The security exclusions (TASK-95 / SEC-011) are
NOT inline in the script: it consumes scripts/ci/exclusions.txt via
`rsync --exclude-from`, the single source Windows adopts in #247.

The central case is the spec's own: a scripts/ tree full of garbage (dotfiles,
tokens, credentials, caches, *.pyc, __pycache__) must produce a clean output.
Since `*.pyc` is exactly the glob Linux GAINED when the source became common
(the old inline list lacked it), garbage of that kind being absent proves the
script consumes the shared file rather than its own memory of the list.

Tests run the bash script as a subprocess against a synthetic repo root, with
real rsync/tar (no network, no Actions). This is the declared destination of
the SEC-011 packaging-exclusion behavior that
test_ci_packaging_exclusions_hygiene.py could only freeze as workflow text;
the canonical 11-glob contract is asserted as data, never as YAML.
"""

import fnmatch
import os
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT = REPO_ROOT / "scripts" / "ci" / "assemble_raw.sh"
EXCLUSIONS = REPO_ROOT / "scripts" / "ci" / "exclusions.txt"
TRIPLE = "x86_64-unknown-linux-gnu"

# El contrato congelado de SEC-011: las 11 globs, en el orden del fichero.
# Windows lleva la misma lista inline hasta #247; este es el dato, no texto
# de workflow.
CANONICAL_EXCLUSIONS = [
    ".*",
    "*.json",
    "*.cache",
    "*.pyc",
    "__pycache__",
    "*.log",
    "*.tmp",
    "*.db",
    "*.key",
    "credentials*",
    ".env*",
]

SENSITIVE_SAMPLES = [
    ".gui_credentials_cache.json",
    ".gui_settings.json",
    ".spotify_token_cache.json",
    ".env",
    ".env.local",
    ".env.production",
    "credentials.json",
    "credentials.txt",
    "credentials_backup",
    "syncify.db",
    "test_library.sqlite.db",
    "app.log",
    "error_debug.log",
    "temporary_worker.tmp",
    "private_signing.key",
    "auth_token.cache",
    "bridge.pyc",
    "__pycache__",
    ".browser_profile_default",
    ".gitignore",
    ".git",
]

LEGITIMATE_FILES = [
    "auth_bridge.py",
    "conversion_bridge.py",
    "fingerprint_bridge.py",
    "lyrics_bridge.py",
    "metadata_bridge.py",
    "organizer_bridge.py",
    "playlist_bridge.py",
    "scanner_bridge.py",
    "download_bridge.py",
    "dependency_manager.py",
    "requirements.txt",
    "services/__init__.py",
    "services/qobuz_service.py",
    "services/tidal_service.py",
    "services/deezer_service.py",
    "services/soundcloud_service.py",
    "services/acoustid_matcher.py",
    "services/metadata_enrichment.py",
]


def _write(path: Path, content: bytes, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    path.chmod(mode)


# Maquinaria de release Linux: assemble_raw.sh es un script POSIX y el arbol
# que ensambla es el del AppImage. En Windows el subprocess cae en el stub de
# WSL; los tests corren donde la maquinaria existe (patron del suite: los
# casos POSIX de test_browser_launcher_detection ya se declaran igual).
@unittest.skipIf(sys.platform.startswith("win"), "Linux release machinery: POSIX tooling")
class AssembleRawBehaviorTests(unittest.TestCase):
    """«Arbol de scripts con basura -> salida limpia», mas el tarball."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

        _write(self.root / "target" / "release" / "syncify-tauri", b"ELF-fake", 0o755)
        for tool in ("ffmpeg", "ffprobe", "fpcalc"):
            _write(self.root / "src-tauri" / "binaries" / f"{tool}-{TRIPLE}", b"ELF-tool", 0o755)

        # Runtime montado (forma install-runtime linux).
        _write(self.root / "bundle" / "python" / "bin" / "python3", b"PY3", 0o755)
        _write(self.root / "bundle" / "python" / "lib" / "python3.11" / "site.py", b"site")

        # scripts/ con legitimos y BASURA: secretos, tokens, caches, pycs,
        # logs, temporales, bases, claves, dotfiles. La fuente unica va dentro
        # (es un fichero del repo y, ademas, el fixture necesita que el
        # default del script lo encuentre).
        scripts = self.root / "scripts"
        for legit in LEGITIMATE_FILES:
            _write(scripts / legit, b"legit\n")
        garbage = [
            ".env",
            ".env.local",
            ".gitignore",
            ".git/config",
            ".gui_settings.json",
            "gui_config.json",
            "credentials.json",
            "credentials_backup",
            "syncify.db",
            "app.log",
            "worker.tmp",
            "private_signing.key",
            "auth_token.cache",
            "bridge.pyc",
            "__pycache__/compile1.pyc",
            "services/__pycache__/compile2.pyc",
        ]
        for junk in garbage:
            _write(scripts / junk, b"junk")

        # La fuente unica REAL: mismo fichero que consumira el build.
        ci = scripts / "ci"
        ci.mkdir(parents=True, exist_ok=True)
        ci.joinpath("exclusions.txt").write_bytes(EXCLUSIONS.read_bytes())

    def _run(self):
        return subprocess.run(
            ["bash", str(SCRIPT)],
            capture_output=True,
            text=True,
            cwd=self.root,
            env=os.environ.copy(),
            timeout=120,
        )

    @property
    def raw(self) -> Path:
        return self.root / "dist" / "linux" / "Syncify-Linux-Raw"

    @property
    def tarball(self) -> Path:
        return self.root / "dist" / "linux" / "Syncify-Linux-x86_64.tar.gz"

    def _output_names(self):
        return {p.relative_to(self.raw).as_posix() for p in self.raw.rglob("*")}

    # ═══════════════════════════════════════════════════════
    # Caso central: basura dentro, salida limpia
    # ═══════════════════════════════════════════════════════

    def test_garbage_tree_produces_clean_output(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stderr)

        names = self._output_names()
        # Ninguna basura sobrevive: cada nombre de la salida se enfrenta a
        # las 11 globs canonicas (incluida la de .pyc que Linux GANO con la
        # fuente comun; si el script dejara de consumir exclusions.txt y
        # volviera a una lista sin *.pyc, esta asercion cae).
        for name in names:
            for junk in SENSITIVE_SAMPLES:
                self.assertFalse(
                    fnmatch.fnmatch(Path(name).name, junk) or name == junk,
                    f"garbage survived in output: {name} (matched {junk!r})",
                )
        # Y la basura sembrada no esta ni con su ruta completa.
        for junk in ("bridge.pyc", "__pycache__", "credentials.json", ".env", "syncify.db"):
            self.assertNotIn(junk, names)

    def test_legitimate_scripts_and_services_survive(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        names = self._output_names()
        for legit in LEGITIMATE_FILES:
            self.assertIn(f"scripts/{legit}", names, f"legit file lost: {legit}")

    def test_layout_binary_bin_and_python(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        names = self._output_names()
        self.assertIn("syncify", names)
        for tool in ("ffmpeg", "ffprobe", "fpcalc"):
            self.assertIn(f"bin/{tool}", names)
        self.assertIn("python/bin/python3", names)
        self.assertIn("python/lib/python3.11/site.py", names)

    def test_executables_are_executable(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        for rel in (
            "syncify",
            "bin/ffmpeg",
            "bin/ffprobe",
            "bin/fpcalc",
            "python/bin/python3",
        ):
            mode = (self.raw / rel).stat().st_mode
            self.assertTrue(mode & stat.S_IXUSR, f"not executable: {rel}")

    # ═══════════════════════════════════════════════════════
    # Tarball
    # ═══════════════════════════════════════════════════════

    def test_tarball_contains_clean_tree_with_expected_layout(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertTrue(self.tarball.is_file())

        with tarfile.open(self.tarball, "r:gz") as tf:
            members = tf.getnames()
        self.assertTrue(members)
        for member in members:
            self.assertTrue(
                member == "Syncify-Linux-Raw" or member.startswith("Syncify-Linux-Raw/"),
                f"member outside the raw root: {member}",
            )
        flat = {
            m.removeprefix("Syncify-Linux-Raw").removeprefix("/").rstrip("/")
            for m in members
        }
        for must in (
            "syncify",
            "bin/ffmpeg",
            "python/bin/python3",
            "scripts/auth_bridge.py",
            "scripts/services/qobuz_service.py",
        ):
            self.assertIn(must, flat, f"missing from tarball: {must}")
        for junk in ("scripts/bridge.pyc", "scripts/credentials.json", "scripts/.env"):
            self.assertNotIn(junk, flat, f"garbage in tarball: {junk}")

    # ═══════════════════════════════════════════════════════
    # La fuente unica: dato congelado y comportamiento
    # ═══════════════════════════════════════════════════════

    def test_exclusions_file_holds_exactly_the_canonical_globs(self):
        lines = [
            line.strip()
            for line in EXCLUSIONS.read_text(encoding="utf-8").splitlines()
        ]
        code = [line for line in lines if line and not line.startswith("#")]
        self.assertEqual(code, CANONICAL_EXCLUSIONS)
        # La ganancia de Linux que motivo SEC-011: *.pyc presente.
        self.assertIn("*.pyc", code)

    def test_canonical_globs_catch_every_sensitive_sample(self):
        for sample in SENSITIVE_SAMPLES:
            matched = any(fnmatch.fnmatch(sample, glob) for glob in CANONICAL_EXCLUSIONS)
            self.assertTrue(matched, f"sensitive sample not excluded: {sample}")

    def test_canonical_globs_spare_legitimate_scripts(self):
        # Mismo matiz del congelador historico: para ficheros legitimos NO
        # ocultos, .* no cuenta (en rsync las globs sin barra casan contra el
        # nombre en cualquier nivel, y ningun legitimo es oculto).
        file_globs = [g for g in CANONICAL_EXCLUSIONS if g != ".*"]
        for legit in LEGITIMATE_FILES:
            name = Path(legit).name
            matched = any(fnmatch.fnmatch(name, glob) for glob in file_globs)
            self.assertFalse(matched, f"legit script falsely excluded: {legit}")

    # ═══════════════════════════════════════════════════════
    # Caminos de fallo: validacion ANTES de crear el arbol
    # ═══════════════════════════════════════════════════════

    def test_missing_binary_aborts_without_creating_output(self):
        (self.root / "target" / "release" / "syncify-tauri").unlink()
        proc = self._run()
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("::error::", proc.stdout)
        self.assertIn("syncify-tauri", proc.stdout)
        self.assertFalse((self.root / "dist").exists(), "output tree created despite failed validation")

    def test_missing_exclusions_file_aborts(self):
        (self.root / "scripts" / "ci" / "exclusions.txt").unlink()
        proc = self._run()
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("::error::", proc.stdout)
        self.assertIn("exclusions", proc.stdout)

    def test_missing_runtime_interpreter_aborts(self):
        (self.root / "bundle" / "python" / "bin" / "python3").unlink()
        proc = self._run()
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("::error::", proc.stdout)
        self.assertIn("bin/python3", proc.stdout)

    def test_missing_external_tool_aborts(self):
        (self.root / "src-tauri" / "binaries" / f"fpcalc-{TRIPLE}").unlink()
        proc = self._run()
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("::error::", proc.stdout)
        self.assertIn("fpcalc", proc.stdout)

    def test_missing_scripts_dir_aborts(self):
        proc = subprocess.run(
            ["bash", str(SCRIPT)],
            capture_output=True,
            text=True,
            cwd=self.root / "bundle",  # cwd sin scripts/ ni el resto
            env=os.environ.copy(),
            timeout=120,
        )
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("::error::", proc.stdout)

    def test_rerun_over_stale_output_is_idempotent(self):
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        # Basura del build anterior en el arbol de salida: el rerun la pisa.
        _write(self.raw / "scripts" / "stale_junk.pyc", b"stale")
        proc = self._run()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertNotIn("scripts/stale_junk.pyc", self._output_names())


if __name__ == "__main__":
    unittest.main()

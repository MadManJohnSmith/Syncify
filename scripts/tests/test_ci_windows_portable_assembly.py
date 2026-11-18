#!/usr/bin/env python3
"""
Behavior tests for the Windows portable assembly (#247).

The portable used to be assembled inline in ``.github/workflows/build-windows.yml``
with a copy of the SEC-011 exclusion list. Since #247 the assembly lives in
``scripts/ci/assemble_portable.ps1`` and consumes the same
``scripts/ci/exclusions.txt`` as the Linux raw tarball
(``scripts/ci/assemble_raw.sh``, rsync ``--exclude-from``): one source, two
adapters. The LEEME is a file template (``scripts/ci/portable/LEEME.txt``),
the Tauri overlay and its ``_up_`` mapping are untouched, and WebView2 keeps
the fwlink download + SHA-256 allowlist + release-engineering tolerance.

The central case is the spec's own: a scripts/ tree full of garbage (dotfiles,
tokens, credentials, caches, *.pyc, __pycache__) must produce a clean output.
The cross-adapter case runs BOTH assemblers against the same synthetic repo and
compares their clean ``scripts/`` trees, so a future divergence in exclusion
semantics fails here instead of shipping secrets or losing bridges.

Tests run each assembler as a subprocess against a disposable repo root, with
real rsync/tar and real pwsh (no network, no Actions). The Linux leg skips on
Windows (no rsync); the PowerShell leg runs wherever pwsh exists, which
includes ubuntu-latest where the python job runs.
"""

import fnmatch
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
PORTABLE_SCRIPT = REPO_ROOT / "scripts" / "ci" / "assemble_portable.ps1"
LINUX_SCRIPT = REPO_ROOT / "scripts" / "ci" / "assemble_raw.sh"
EXCLUSIONS = REPO_ROOT / "scripts" / "ci" / "exclusions.txt"
README_TEMPLATE = REPO_ROOT / "scripts" / "ci" / "portable" / "LEEME.txt"

PWSH = shutil.which("pwsh")
PWSH_SKIP = (
    "pwsh no esta instalado: el adaptador Windows se ejecuta en ubuntu-latest "
    "(pwsh preinstalado); instala PowerShell 7 para correrlo en local"
)

LINUX_TOOLS = shutil.which("bash") and shutil.which("rsync") and shutil.which("tar")
LINUX_SKIP = (
    "el adaptador Linux necesita bash, rsync y tar (maquinaria POSIX del "
    "build-linux.yml); esta pata corre en ubuntu-latest"
)

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

GARBAGE = [
    ".env",
    ".env.local",
    ".gitignore",
    ".git/config",
    ".git/objects/aa/bb",
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


def _write(path: Path, content: bytes = b"fake\n", mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    path.chmod(mode)


def _seed_repo(
    root: Path, *, custom_exclusion: str | None = None, readme: bytes | None = None
) -> None:
    """Repo sintetico con lo que necesitan AMBOS adaptadores."""

    # Binario compilado: cada adaptador prefiere el suyo.
    _write(root / "target" / "release" / "Syncify.exe", b"MZ-fake")
    _write(root / "target" / "release" / "syncify-tauri", b"ELF-fake", 0o755)

    # Externos: triple en src-tauri/binaries (Linux) y nombre pelado (Windows).
    for tool in ("ffmpeg", "ffprobe", "fpcalc"):
        _write(
            root / "src-tauri" / "binaries" / f"{tool}-x86_64-unknown-linux-gnu",
            b"ELF-tool",
            0o755,
        )
        _write(root / "bin" / f"{tool}.exe", b"MZ-tool")

    # Runtime autonomo: forma Linux (bin/python3) y forma Windows (python.exe).
    _write(root / "bundle" / "python" / "bin" / "python3", b"PY3", 0o755)
    _write(root / "bundle" / "python" / "lib" / "python3.11" / "site.py", b"site\n")
    _write(root / "python" / "python.exe", b"MZ-python")
    _write(root / "python" / "Lib" / "site.py", b"site\n")

    scripts = root / "scripts"
    for legit in LEGITIMATE_FILES:
        _write(scripts / legit)
    for junk in GARBAGE:
        _write(scripts / junk, b"junk\n")

    # La fuente unica REAL (con una glob extra opcional para probar que se lee
    # el fichero y no una copia memorizada).
    exclusions = EXCLUSIONS.read_text(encoding="utf-8")
    if custom_exclusion:
        exclusions += f"{custom_exclusion}\n"
    _write(scripts / "ci" / "exclusions.txt", exclusions.encode("utf-8"))

    # Plantilla LEEME REAL (o una alterada para probar el origen del texto).
    _write(
        scripts / "ci" / "portable" / "LEEME.txt",
        readme if readme is not None else README_TEMPLATE.read_bytes(),
    )


def _files(root: Path) -> set:
    return {
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if path.is_file()
    }


def _exclusion_patterns() -> list:
    return [
        line.strip()
        for line in EXCLUSIONS.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]


def _run_portable(root: Path, *extra):
    command = [
        PWSH,
        "-NoProfile",
        "-NonInteractive",
        "-File",
        str(PORTABLE_SCRIPT),
        "-RepoRoot",
        str(root),
        "-SkipWebView2",
    ]
    command.extend(str(argument) for argument in extra)
    return subprocess.run(
        command,
        capture_output=True,
        text=True,
        env=os.environ.copy(),
        timeout=180,
    )


def _run_linux(root: Path):
    return subprocess.run(
        ["bash", str(LINUX_SCRIPT)],
        capture_output=True,
        text=True,
        cwd=root,
        env=os.environ.copy(),
        timeout=120,
    )


def _combined(proc) -> str:
    return f"{proc.stdout}\n{proc.stderr}"


class _ScriptsTreeAssertions:
    """Asserts compartidas por la pata Linux y la pata Windows."""

    def assert_clean_scripts_tree(self, scripts_root: Path, extra_patterns=()) -> set:
        self.assertTrue(scripts_root.is_dir(), f"falta el arbol scripts/: {scripts_root}")
        names = _files(scripts_root)
        patterns = _exclusion_patterns() + list(extra_patterns)

        # Cada componente de cada ruta de salida se enfrenta a la fuente unica:
        # asi una glob nueva (o una que el adaptador memorice mal) no deja
        # pasar basura anidada sin que el test lo nombre.
        for relative in names:
            for part in Path(relative).parts:
                for pattern in patterns:
                    self.assertFalse(
                        fnmatch.fnmatch(part, pattern),
                        f"garbage survived in output: {relative} (component {part!r} matched {pattern!r})",
                    )

        for junk in ("bridge.pyc", "__pycache__", "credentials.json", ".env", "syncify.db"):
            self.assertNotIn(junk, names, f"garbage survived in output: {junk}")

        for legit in LEGITIMATE_FILES:
            self.assertIn(legit, names, f"legit file lost: {legit}")
        return names


@unittest.skipUnless(PWSH, PWSH_SKIP)
class PortableAssemblyBehaviorTests(_ScriptsTreeAssertions, unittest.TestCase):
    """«Arbol con basura -> salida limpia», layout y fuente de cada entrada."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        _seed_repo(self.root)

    @property
    def portable(self) -> Path:
        return self.root / "dist" / "portable" / "Syncify-Portable"

    @property
    def zip_path(self) -> Path:
        return self.root / "dist" / "portable" / "Syncify-Windows-Portable.zip"

    def _run(self):
        proc = _run_portable(self.root)
        self.assertEqual(proc.returncode, 0, _combined(proc))
        return proc

    def test_garbage_tree_produces_clean_output(self):
        self._run()
        names = self.assert_clean_scripts_tree(self.portable / "scripts")
        # El filtro no se lleva la fuente unica ni la plantilla: viajan con el
        # resto de scripts/ (no son secretos).
        self.assertIn("ci/exclusions.txt", names)
        self.assertIn("ci/portable/LEEME.txt", names)

    def test_layout_has_binary_bin_python_and_readme(self):
        self._run()
        self.assertTrue((self.portable / "Syncify.exe").is_file())
        for tool in ("ffmpeg.exe", "ffprobe.exe", "fpcalc.exe"):
            self.assertTrue((self.portable / "bin" / tool).is_file(), tool)
        self.assertTrue((self.portable / "python" / "python.exe").is_file())
        self.assertTrue((self.portable / "python" / "Lib" / "site.py").is_file())
        self.assertEqual(
            (self.portable / "LEEME.txt").read_bytes(),
            README_TEMPLATE.read_bytes(),
        )

    def test_readme_comes_from_the_template_file(self):
        # Si el texto volviera a un here-string inline, este cambio no llegaria
        # al portable.
        custom = "LEEME DE PRUEBA: plantilla de fichero, no inline\n"
        _write(self.root / "scripts" / "ci" / "portable" / "LEEME.txt", custom.encode())
        self._run()
        self.assertEqual((self.portable / "LEEME.txt").read_text(encoding="utf-8"), custom)

    def test_exclusions_are_read_from_the_shared_source(self):
        # Una glob que no esta en la lista canonica: si el script recordara la
        # lista vieja en vez de leer el fichero, este fichero sobreviviria.
        glob = "fixture_private_*.dat"
        _write(self.root / "scripts" / "services" / "fixture_private_token.dat", b"secret\n")
        _write(self.root / "scripts" / "fixture_private_root.dat", b"secret\n")
        _write(
            self.root / "scripts" / "ci" / "exclusions.txt",
            (EXCLUSIONS.read_text(encoding="utf-8") + f"{glob}\n").encode(),
        )
        self._run()
        names = _files(self.portable / "scripts")
        self.assertNotIn("services/fixture_private_token.dat", names)
        self.assertNotIn("fixture_private_root.dat", names)

    def test_zip_contains_clean_tree_with_expected_layout(self):
        self._run()
        self.assertTrue(self.zip_path.is_file())
        with zipfile.ZipFile(self.zip_path) as archive:
            members = {
                name.replace("\\", "/").rstrip("/")
                for name in archive.namelist()
                if not name.endswith("/")
            }
        for must in (
            "Syncify.exe",
            "LEEME.txt",
            "bin/ffmpeg.exe",
            "python/python.exe",
            "scripts/auth_bridge.py",
            "scripts/services/qobuz_service.py",
        ):
            self.assertIn(must, members, f"missing from zip: {must}")
        for junk in ("scripts/bridge.pyc", "scripts/credentials.json", "scripts/.env"):
            self.assertNotIn(junk, members, f"garbage in zip: {junk}")

    def test_rerun_over_stale_output_is_idempotent(self):
        self._run()
        _write(self.portable / "scripts" / "stale_junk.pyc", b"stale")
        self._run()
        self.assertNotIn(
            "stale_junk.pyc",
            {Path(name).name for name in _files(self.portable / "scripts")},
        )

    # ═══════════════════════════════════════════════════════
    # Caminos de fallo: validacion ANTES de crear el arbol
    # ═══════════════════════════════════════════════════════

    def test_missing_exclusions_aborts_without_output(self):
        (self.root / "scripts" / "ci" / "exclusions.txt").unlink()
        proc = _run_portable(self.root)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("exclusions.txt", _combined(proc))
        self.assertFalse((self.root / "dist").exists(), "output tree created despite failed validation")

    def test_missing_readme_template_aborts(self):
        (self.root / "scripts" / "ci" / "portable" / "LEEME.txt").unlink()
        proc = _run_portable(self.root)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("LEEME.txt", _combined(proc))
        self.assertFalse((self.root / "dist").exists())

    def test_missing_binary_aborts(self):
        (self.root / "target" / "release" / "Syncify.exe").unlink()
        (self.root / "target" / "release" / "syncify-tauri").unlink()
        proc = _run_portable(self.root)
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("target/release/", _combined(proc))
        self.assertFalse((self.root / "dist").exists())


@unittest.skipUnless(PWSH and LINUX_TOOLS and not sys.platform.startswith("win"), LINUX_SKIP)
class CrossAdapterExclusionTests(_ScriptsTreeAssertions, unittest.TestCase):
    """El mismo arbol con basura -> el mismo scripts/ limpio en los dos adaptadores."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        # Glob extra: si cualquiera de los dos adaptadores memorizara la lista
        # canonica en vez de leer exclusions.txt, su salida la retendria y la
        # comparacion fallaria.
        _seed_repo(self.root, custom_exclusion="fixture_private_*.dat")
        _write(self.root / "scripts" / "services" / "fixture_private_token.dat", b"secret\n")
        _write(self.root / "scripts" / "fixture_private_root.dat", b"secret\n")

    def test_both_adapters_emit_the_same_clean_scripts_tree(self):
        linux_proc = _run_linux(self.root)
        self.assertEqual(linux_proc.returncode, 0, _combined(linux_proc))
        linux_raw = self.root / "dist" / "linux" / "Syncify-Linux-Raw"
        linux_names = self.assert_clean_scripts_tree(
            linux_raw / "scripts", extra_patterns=["fixture_private_*.dat"]
        )

        portable_proc = _run_portable(self.root)
        self.assertEqual(portable_proc.returncode, 0, _combined(portable_proc))
        portable = self.root / "dist" / "portable" / "Syncify-Portable"
        portable_names = self.assert_clean_scripts_tree(
            portable / "scripts", extra_patterns=["fixture_private_*.dat"]
        )

        self.assertEqual(
            linux_names,
            portable_names,
            "los adaptadores Linux y Windows dejaron arboles scripts/ distintos",
        )
        self.assertNotIn("fixture_private_root.dat", linux_names)
        self.assertNotIn("services/fixture_private_token.dat", linux_names)


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""
Behavior tests for the Windows packaging guards (#246).

The two verification guards of the Windows release live in
``scripts/ci/guard_keyring.ps1`` and ``scripts/ci/guard_installer.ps1``. These
tests run them as subprocesses against disposable fixtures with faked tools,
never against their text and never against workflow YAML:

* NSIS guard: an installer whose 7-Zip listing carries
  ``_up_/python/python.exe`` passes; a listing without it fails; a 7-Zip that
  cannot list the installer warns and exits 0 (release-engineering tolerance,
  documented in the script).
* Keyring guard: the comctl32 manifest is embedded from
  ``scripts/ci/comctl32.manifest`` (not a here-string) and then the keyring
  test runs; when that test fails, the loader diagnostic (minimal crate +
  dumpbin dependents/imports) runs on its own failure path and the original
  exit code is preserved.

Fake ``cargo``/``mt.exe``/``dumpbin``/``7z`` are PowerShell scripts passed
through the guards' tool parameters, so the same fixtures work on Linux and
Windows. ``pwsh`` is preinstalled on ubuntu-latest, where the ``python`` job
runs; environments without it skip these cases explicitly.

This is the declared destination of the frozen guard-text assertions that
``test_windows_bundled_python_installer.py`` held before #246: the workflow
half (step names, order, guard text) died there, the repo-config half (the
Tauri overlay declares the embedded runtime) lives here.
"""

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CI_DIR = REPO_ROOT / "scripts" / "ci"
KEYRING_SCRIPT = CI_DIR / "guard_keyring.ps1"
INSTALLER_SCRIPT = CI_DIR / "guard_installer.ps1"
MANIFEST = CI_DIR / "comctl32.manifest"
WINDOWS_TAURI_CONFIG = REPO_ROOT / "src-tauri" / "tauri.windows.conf.json"

PWSH = shutil.which("pwsh")
PWSH_SKIP = (
    "pwsh no esta instalado: las guardias Windows se ejecutan en ubuntu-latest "
    "(pwsh preinstalado); instala PowerShell 7 para correrlas en local"
)

# Los fakes son scripts PowerShell, no ejecutables POSIX: el mismo fixture
# corre en la pata Linux y en la pata Windows del job python. Cada uno registra
# su invocacion para que las aserciones lean lo que el guard pidio de verdad.
FAKE_CARGO = r"""
Add-Content -LiteralPath $env:SYNCIFY_FAKE_LOG -Value ("cargo " + ($args -join " ") + " @ " + (Get-Location).Path)
if ($args -contains "--no-run") {
    if ($env:SYNCIFY_FAKE_CARGO_NO_EXE -ne "1") {
        $deps = Join-Path (Get-Location).Path "target/debug/deps"
        New-Item -ItemType Directory -Force -Path $deps | Out-Null
        Set-Content -LiteralPath (Join-Path $deps "syncify_tauri_lib-fake.exe") -Value "fake-test-exe"
    }
    exit 0
}
if (($args -join " ") -like "*windows_build_uses_a_persistent_credential_store*") {
    exit [int]$env:SYNCIFY_FAKE_KEYRING_EXIT
}
if ($args -contains "syncify-fmp4-flac") {
    exit 0
}
Write-Error ("fake cargo: unexpected invocation: " + ($args -join " "))
exit 64
"""

FAKE_MT = r"""
Add-Content -LiteralPath $env:SYNCIFY_FAKE_LOG -Value ("mt " + ($args -join " "))
$manifestIndex = [array]::IndexOf($args, "-manifest")
if ($manifestIndex -lt 0) {
    Write-Error "fake mt: missing -manifest"
    exit 65
}
$manifestPath = $args[$manifestIndex + 1]
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    Write-Error ("fake mt: manifest does not exist: " + $manifestPath)
    exit 66
}
if (-not (($args -join " ").Contains(";#1"))) {
    Write-Error "fake mt: missing outputresource resource id ;#1"
    exit 67
}
exit [int]$env:SYNCIFY_FAKE_MT_EXIT
"""

FAKE_DUMPBIN = r"""
Add-Content -LiteralPath $env:SYNCIFY_FAKE_LOG -Value ("dumpbin " + ($args -join " "))
Write-Output ("fake-dumpbin " + ($args -join " "))
exit 0
"""

FAKE_7Z = r"""
Add-Content -LiteralPath $env:SYNCIFY_FAKE_LOG -Value ("7z " + ($args -join " "))
if ($args.Count -lt 1 -or $args[0] -ne "l") {
    Write-Error ("fake 7z: unexpected invocation: " + ($args -join " "))
    exit 64
}
Get-Content -LiteralPath $env:SYNCIFY_FAKE_7Z_LISTING
exit [int]$env:SYNCIFY_FAKE_7Z_EXIT
"""


def _write_script(path: Path, body: str) -> None:
    path.write_text(body.lstrip(), encoding="utf-8")


def _resource_relpath(relative: str) -> str:
    """Replica el mapeo de Tauri: cada `..` del glob se vuelve `_up_`."""
    parts = []
    for segment in relative.split("/"):
        if segment in ("", "."):
            continue
        parts.append("_up_" if segment == ".." else segment)
    return "\\".join(parts)


def _run(script: Path, *arguments, env=None):
    command = [PWSH, "-NoProfile", "-NonInteractive", "-File", str(script)]
    command.extend(str(argument) for argument in arguments)
    return subprocess.run(
        command,
        capture_output=True,
        text=True,
        env={**os.environ, **(env or {})},
        timeout=120,
    )


def _combined(proc) -> str:
    return f"{proc.stdout}\n{proc.stderr}"


@unittest.skipUnless(PWSH, PWSH_SKIP)
class GuardInstallerBehaviorTests(unittest.TestCase):
    """La guardia NSIS mira DENTRO del instalador: runtime presente o fallo."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)

        self.repo = self.root / "repo"
        nsis_dir = self.repo / "target" / "release" / "bundle" / "nsis"
        nsis_dir.mkdir(parents=True)
        self.installer = nsis_dir / "Syncify_0.1.0_x64-setup.exe"
        self.installer.write_bytes(b"fake-nsis-installer")
        self.python_exe = self.repo / "python" / "python.exe"
        self.python_exe.parent.mkdir(parents=True)
        self.python_exe.write_bytes(b"fake-python")

        self.log = self.root / "invocations.log"
        self.log.write_text("", encoding="utf-8")
        self.listing = self.root / "nsis-listing.txt"
        self.listing.write_text("", encoding="utf-8")

        self.fake_7z = self.root / "fake-7z.ps1"
        _write_script(self.fake_7z, FAKE_7Z)

        self.env = {
            "SYNCIFY_FAKE_LOG": str(self.log),
            "SYNCIFY_FAKE_7Z_LISTING": str(self.listing),
            "SYNCIFY_FAKE_7Z_EXIT": "0",
        }

    def _set_listing(self, *lines: str) -> None:
        self.listing.write_text("\n".join(lines) + "\n", encoding="utf-8")

    def _invoke(self, *extra, env=None):
        return _run(
            INSTALLER_SCRIPT,
            "-RepoRoot",
            self.repo,
            "-Installer",
            self.installer,
            "-SevenZip",
            self.fake_7z,
            *extra,
            env={**self.env, **(env or {})},
        )

    def _invocations(self):
        return self.log.read_text(encoding="utf-8").splitlines()

    def test_installer_with_runtime_passes(self):
        self._set_listing(
            "Path = Syncify_0.1.0_x64-setup.exe",
            "Type = Nsis",
            r"$PLUGINSDIR\_up_\python\python.exe",
        )
        proc = self._invoke()
        self.assertEqual(proc.returncode, 0, _combined(proc))
        self.assertIn("Runtime Python embebido confirmado", proc.stdout)
        self.assertTrue(
            any("7z l" in line and str(self.installer) in line for line in self._invocations()),
            self._invocations(),
        )

    def test_installer_without_runtime_fails(self):
        self._set_listing(
            "Path = Syncify_0.1.0_x64-setup.exe",
            r"\\$PLUGINSDIR\\Syncify.exe",
            r"\\$PLUGINSDIR\\scripts\\dependency_manager.py",
        )
        proc = self._invoke()
        self.assertNotEqual(proc.returncode, 0)
        output = _combined(proc)
        self.assertIn("no contiene _up_/python/python.exe", output)
        self.assertIn("Primeras entradas del instalador", proc.stdout)

    def test_sevenzip_cannot_list_warns_and_exits_zero(self):
        # La tolerancia de release engineering: una limitacion de 7-Zip no
        # bloquea la release. El warning debe decirlo y el guard sale 0.
        self._set_listing("7z: cannot open the file as archive")
        proc = self._invoke(env={"SYNCIFY_FAKE_7Z_EXIT": "5"})
        self.assertEqual(proc.returncode, 0, _combined(proc))
        output = _combined(proc)
        self.assertIn("7-Zip no pudo listar el instalador", output)
        self.assertIn("verificacion omitida", output)
        self.assertIn("salida 5", output)

    def test_missing_python_tree_aborts_before_listing(self):
        self.python_exe.unlink()
        proc = self._invoke()
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("python/python.exe no existe", _combined(proc))
        self.assertEqual([], self._invocations(), "7z no debe ejecutarse sin runtime")

    def test_missing_installer_aborts(self):
        proc = _run(
            INSTALLER_SCRIPT,
            "-RepoRoot",
            self.repo,
            "-Installer",
            self.root / "missing-installer.exe",
            "-SevenZip",
            self.fake_7z,
            env=self.env,
        )
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("No existe el instalador NSIS", _combined(proc))


@unittest.skipUnless(PWSH, PWSH_SKIP)
class GuardKeyringBehaviorTests(unittest.TestCase):
    """Manifiesto desde fichero, test del keyring y diagnostico en su fallo."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()

        self.log = self.root / "invocations.log"
        self.log.write_text("", encoding="utf-8")

        self.fake_cargo = self.root / "fake-cargo.ps1"
        self.fake_mt = self.root / "fake-mt.ps1"
        self.fake_dumpbin = self.root / "fake-dumpbin.ps1"
        _write_script(self.fake_cargo, FAKE_CARGO)
        _write_script(self.fake_mt, FAKE_MT)
        _write_script(self.fake_dumpbin, FAKE_DUMPBIN)

        self.env = {
            "SYNCIFY_FAKE_LOG": str(self.log),
            "SYNCIFY_FAKE_KEYRING_EXIT": "0",
            "SYNCIFY_FAKE_MT_EXIT": "0",
            "SYNCIFY_FAKE_CARGO_NO_EXE": "0",
        }

    def _invoke(self, *extra, env=None):
        return _run(
            KEYRING_SCRIPT,
            "-RepoRoot",
            self.repo,
            "-Cargo",
            self.fake_cargo,
            "-Mt",
            self.fake_mt,
            "-Dumpbin",
            self.fake_dumpbin,
            *extra,
            env={**self.env, **(env or {})},
        )

    def _invocations(self):
        return self.log.read_text(encoding="utf-8").splitlines()

    def test_embeds_manifest_from_file_then_runs_keyring_test(self):
        proc = self._invoke()
        self.assertEqual(proc.returncode, 0, _combined(proc))
        calls = self._invocations()

        self.assertTrue(calls[0].startswith("cargo test --locked -p syncify-tauri --lib --no-run"))
        self.assertTrue(calls[1].startswith("mt "), calls)
        self.assertIn(
            os.path.normcase(f"-manifest {MANIFEST}"),
            os.path.normcase(calls[1]),
            calls[1],
        )
        self.assertIn(";#1", calls[1])
        self.assertTrue(
            any("windows_build_uses_a_persistent_credential_store" in line for line in calls[2:]),
            calls,
        )
        self.assertFalse(any(line.startswith("dumpbin ") for line in calls), calls)
        # cargo corre desde la raiz del repo, como en el workflow.
        self.assertTrue(
            os.path.normcase(calls[0]).endswith(os.path.normcase(f"@ {self.repo}")),
            calls[0],
        )

    def test_keyring_failure_runs_loader_diagnostics_and_keeps_the_code(self):
        proc = self._invoke(env={"SYNCIFY_FAKE_KEYRING_EXIT": "3"})
        self.assertEqual(proc.returncode, 3, _combined(proc))
        calls = self._invocations()

        keyring_index = next(
            index
            for index, line in enumerate(calls)
            if "windows_build_uses_a_persistent_credential_store" in line
        )
        diagnostics = calls[keyring_index + 1 :]
        self.assertTrue(
            any("syncify-fmp4-flac" in line for line in diagnostics),
            diagnostics,
        )
        self.assertTrue(
            any(line.startswith("dumpbin /DEPENDENTS ") for line in diagnostics),
            diagnostics,
        )
        self.assertTrue(
            any(line.startswith("dumpbin /IMPORTS:comctl32.dll ") for line in diagnostics),
            diagnostics,
        )
        output = _combined(proc)
        self.assertIn(">>> 1. Carga un binario de test minimo", output)
        self.assertIn(">>> 3. Imports de comctl32", output)

    def test_missing_test_executable_aborts_without_embedding(self):
        proc = self._invoke(env={"SYNCIFY_FAKE_CARGO_NO_EXE": "1"})
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("No se encontro el exe de test", _combined(proc))
        self.assertFalse(
            any(line.startswith("mt ") for line in self._invocations()),
            self._invocations(),
        )

    def test_missing_mt_aborts_before_running_the_keyring_test(self):
        proc = _run(
            KEYRING_SCRIPT,
            "-RepoRoot",
            self.repo,
            "-Cargo",
            self.fake_cargo,
            "-Mt",
            self.root / "missing-mt.exe",
            "-Dumpbin",
            self.fake_dumpbin,
            env=self.env,
        )
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("mt.exe no existe", _combined(proc))
        self.assertFalse(
            any(
                "windows_build_uses_a_persistent_credential_store" in line
                for line in self._invocations()
            ),
            self._invocations(),
        )

    def test_missing_manifest_aborts(self):
        proc = self._invoke("-Manifest", self.root / "missing.manifest")
        self.assertNotEqual(proc.returncode, 0)
        self.assertIn("No existe el manifiesto comctl32", _combined(proc))
        self.assertEqual([], self._invocations())


class WindowsPackagingConfigTests(unittest.TestCase):
    """Config del repo que la guardia NSIS asume (no texto de workflow)."""

    def test_windows_resources_include_the_embedded_runtime(self):
        self.assertTrue(
            WINDOWS_TAURI_CONFIG.is_file(),
            "Falta src-tauri/tauri.windows.conf.json: sin el, el NSIS instala sin Python",
        )
        config = json.loads(WINDOWS_TAURI_CONFIG.read_text(encoding="utf-8"))
        resources = config["bundle"]["resources"]
        self.assertTrue(
            any(entry.startswith("../python/") for entry in resources),
            f"bundle.resources debe incluir el runtime embebido: {resources}",
        )

    def test_shared_resources_are_kept_alongside_the_runtime(self):
        # La CLI fusiona la config de plataforma con json_patch::merge, que
        # sustituye arrays: repetir solo el glob de Python dejaria fuera los
        # scripts y la app se quedaria sin puentes.
        shared = json.loads(
            (REPO_ROOT / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8")
        )["bundle"]["resources"]
        windows = json.loads(WINDOWS_TAURI_CONFIG.read_text(encoding="utf-8"))["bundle"]["resources"]
        for entry in shared:
            self.assertIn(entry, windows, f"tauri.windows.conf.json pierde el recurso {entry}")

    def test_declared_glob_lands_under_up_python(self):
        resources = json.loads(WINDOWS_TAURI_CONFIG.read_text(encoding="utf-8"))["bundle"][
            "resources"
        ]
        python_globs = [entry for entry in resources if entry.startswith("../python/")]
        self.assertTrue(python_globs, "No hay glob de Python declarado")
        for glob in python_globs:
            # `../python/**/*` -> `_up_\python\...`: la guardia busca ahi dentro.
            self.assertTrue(
                _resource_relpath(glob).startswith("_up_\\python"),
                f"El glob {glob} no instala bajo _up_\\python: la guardia miraria otra ruta",
            )

    def test_comctl32_manifest_is_a_file_with_the_v6_dependency(self):
        self.assertTrue(MANIFEST.is_file(), f"Falta el manifiesto: {MANIFEST}")
        manifest = MANIFEST.read_text(encoding="utf-8")
        self.assertIn('version="6.0.0.0"', manifest)
        self.assertIn('name="Microsoft.Windows.Common-Controls"', manifest)
        self.assertIn("6595b64144ccf1df", manifest)


if __name__ == "__main__":
    unittest.main(verbosity=2)

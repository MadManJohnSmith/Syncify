#!/usr/bin/env python3
"""install-runtime flow and canonical import smoke for dependency_manager.

install-runtime (#242) mounts the pinned CPython 3.11.9 the app ships:
Linux gets python-build-standalone 20240415 in bundle/python, Windows gets
the embed distribution in python/ with `import site` enabled and pip
bootstrapped from the pinned get-pip.py. Both platforms end with the
canonical 10-import smoke run by the mounted interpreter.

These tests prove the FLOW with fakes/fixtures (no network): a fake
`download_file` serves synthetic archives against patched table hashes and a
recording fake stands in for subprocess (pip bootstrap, requirements, smoke)
— same pattern as the security suite, which stays untouched. Nothing here
freezes workflow YAML: the builds consume the module through the CLI, whose
JSON envelope is asserted at the end of this file.
"""

import io
import json
import os
import subprocess
import sys
import tarfile
import tempfile
import types
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import dependency_manager

FAKE_CPYTHON_PAYLOAD = b"#!/bin/sh\n# fake cpython 3.11.9\n"
FAKE_GET_PIP_PAYLOAD = b"# fake get-pip bootstrap"

# The embed distribution's ._pth ships with `import site` commented out.
EMBED_PTH_NAME = "python311._pth"
EMBED_PTH_COMMENTED = "python311.zip\n.\n\n#import site\n"
EMBED_PTH_ENABLED = "python311.zip\n.\n\nimport site\n"


def sha256_of(data: bytes) -> str:
    import hashlib

    return hashlib.sha256(data).hexdigest()


def build_standalone_tarball() -> bytes:
    """Synthetic install_only tarball: top-level python/ with bin/python3."""
    fixture = io.BytesIO()
    with tarfile.open(fileobj=fixture, mode="w:gz") as tf:
        payload = FAKE_CPYTHON_PAYLOAD
        info = tarfile.TarInfo(name="python/bin/python3")
        info.size = len(payload)
        info.mode = 0o755
        tf.addfile(info, io.BytesIO(payload))
        lib = b"# fake stdlib marker\n"
        info = tarfile.TarInfo(name="python/lib/marker.txt")
        info.size = len(lib)
        tf.addfile(info, io.BytesIO(lib))
    return fixture.getvalue()


def build_embed_zip(pth_content: str = EMBED_PTH_COMMENTED) -> bytes:
    """Synthetic python.org embed zip: flat layout with the ._pth file."""
    fixture = io.BytesIO()
    with zipfile.ZipFile(fixture, "w") as zf:
        zf.writestr("python.exe", FAKE_CPYTHON_PAYLOAD)
        zf.writestr(EMBED_PTH_NAME, pth_content)
        zf.writestr("python311.zip", b"pk-zip")
    return fixture.getvalue()


def make_fake_run(fail_when=None):
    """A subprocess.run stand-in that records calls and can fail selectively.

    fail_when(cmd) -> (returncode, stderr) or None to succeed.
    """

    calls = []

    def fake_run(cmd, capture_output=False, text=False, timeout=None, env=None):
        calls.append({"cmd": list(cmd), "timeout": timeout, "env": env})
        failure = fail_when(list(cmd)) if fail_when else None
        if failure:
            returncode, stderr = failure
            return types.SimpleNamespace(returncode=returncode, stdout="", stderr=stderr)
        return types.SimpleNamespace(returncode=0, stdout="ok", stderr="")

    return fake_run, calls


def classify_windows_calls(calls):
    """Split recorded subprocess calls into (get_pip, pip, smoke)."""
    get_pip = [c for c in calls if c["cmd"][1].endswith("get-pip.py")]
    pip = [c for c in calls if "-m" in c["cmd"] and "pip" in c["cmd"]]
    smoke = [c for c in calls if "-c" in c["cmd"]]
    return get_pip, pip, smoke


class RuntimeLayoutContractTest(unittest.TestCase):
    """The trees the Tauri overlay and the packaging steps consume."""

    def test_default_destinations_match_the_packaging_contract(self):
        self.assertEqual(dependency_manager.RUNTIME_DEFAULT_DEST["linux"], "bundle")
        self.assertEqual(dependency_manager.RUNTIME_DEFAULT_DEST["windows"], "python")

    def test_interpreter_paths_inside_the_mounted_tree(self):
        self.assertEqual(
            dependency_manager.RUNTIME_INTERPRETER["linux"],
            Path("python") / "bin" / "python3",
        )
        self.assertEqual(
            dependency_manager.RUNTIME_INTERPRETER["windows"], Path("python.exe")
        )


class InstallRuntimeLinuxTest(unittest.TestCase):
    """python-build-standalone: download, verify, extract, requirements, smoke."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.base_dir = Path(self.temp_dir.name).resolve()
        self.dest = self.base_dir / "bundle"
        self.fixture = build_standalone_tarball()
        self.fixture_hash = sha256_of(self.fixture)

    def tearDown(self):
        self.temp_dir.cleanup()

    def _fake_download(self, downloaded_urls):
        def fake(url, dest_path, progress_callback=None):
            downloaded_urls.append(url)
            dest_path.write_bytes(self.fixture)
            return True

        return fake

    def test_mounts_pinned_standalone_and_runs_requirements_then_smoke(self):
        downloaded = []
        fake_run, calls = make_fake_run()

        with patch.object(dependency_manager, "download_file",
                          side_effect=self._fake_download(downloaded)), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["cpython"],
                           {"linux": self.fixture_hash}), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.install_runtime(dest=self.dest, plat="linux")

        self.assertTrue(result["success"], result)
        self.assertEqual(result["platform"], "linux")
        self.assertEqual(result["runtime_version"], "3.11.9")
        self.assertEqual(
            downloaded,
            [dependency_manager.TOOL_URLS["cpython"]["linux"]],
            "exactly one download: the pinned standalone tarball",
        )

        interpreter = self.dest / "python" / "bin" / "python3"
        self.assertTrue(interpreter.is_file())
        self.assertEqual(interpreter.read_bytes(), FAKE_CPYTHON_PAYLOAD)
        self.assertEqual(result["python"], str(interpreter))

        # requirements first, canonical smoke second, nothing else.
        self.assertEqual(len(calls), 2, [c["cmd"] for c in calls])
        pip_cmd, smoke_cmd = calls[0]["cmd"], calls[1]["cmd"]
        self.assertEqual(pip_cmd[1:3], ["-m", "pip"])
        self.assertIn("-r", pip_cmd)
        self.assertTrue(pip_cmd[-1].endswith("scripts/requirements.txt"))
        self.assertEqual(
            smoke_cmd[2], dependency_manager.smoke_import_statement() + "; print('OK')"
        )
        self.assertEqual(calls[1]["timeout"], 120)

        # The archive is consumed, not shipped: nothing left next to the tree.
        leftovers = [p.name for p in self.base_dir.iterdir() if p.is_file()]
        self.assertEqual(leftovers, [])

    def test_hash_mismatch_blocks_extraction_and_cleans_up(self):
        downloaded = []
        fake_run, calls = make_fake_run()

        with patch.object(dependency_manager, "download_file",
                          side_effect=self._fake_download(downloaded)), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["cpython"],
                           {"linux": "0" * 64}), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.install_runtime(dest=self.dest, plat="linux")

        self.assertFalse(result["success"])
        self.assertIn("SHA-256 mismatch", result["error"])
        self.assertEqual(downloaded, [dependency_manager.TOOL_URLS["cpython"]["linux"]])
        self.assertFalse((self.dest / "python").exists(), "nothing extracted")
        self.assertEqual(calls, [], "no subprocess runs without a verified archive")
        self.assertEqual([p.name for p in self.base_dir.iterdir() if p.is_file()], [])

    def test_missing_hash_row_prevents_the_download(self):
        downloaded = []
        fake_run, _ = make_fake_run()
        empty_hashes = dict(dependency_manager.KNOWN_SHA256_HASHES)
        empty_hashes["cpython"] = {}

        with patch.object(dependency_manager, "download_file",
                          side_effect=self._fake_download(downloaded)), \
                patch.object(dependency_manager, "KNOWN_SHA256_HASHES", empty_hashes), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.install_runtime(dest=self.dest, plat="linux")

        self.assertFalse(result["success"])
        self.assertIn("No pinned CPython runtime", result["error"])
        self.assertEqual(downloaded, [], "no hash, no download")
        self.assertFalse(self.dest.exists())

    def test_unexpected_layout_fails_loudly(self):
        truncated = io.BytesIO()
        with tarfile.open(fileobj=truncated, mode="w:gz") as tf:
            payload = FAKE_CPYTHON_PAYLOAD
            info = tarfile.TarInfo(name="not-python/bin/python3")
            info.size = len(payload)
            info.mode = 0o755
            tf.addfile(info, io.BytesIO(payload))
        self.fixture = truncated.getvalue()
        self.fixture_hash = sha256_of(self.fixture)
        fake_run, _ = make_fake_run()

        with patch.object(dependency_manager, "download_file",
                          side_effect=self._fake_download([])), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["cpython"],
                           {"linux": self.fixture_hash}), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.install_runtime(dest=self.dest, plat="linux")

        self.assertFalse(result["success"])
        self.assertIn("unexpected runtime layout", result["error"])

    def test_pip_failure_surfaces_and_skips_the_smoke(self):
        def fail_pip(cmd):
            if "-m" in cmd and "pip" in cmd:
                return (1, "pip: resolution impossible")
            return None

        fake_run, calls = make_fake_run(fail_when=fail_pip)

        with patch.object(dependency_manager, "download_file",
                          side_effect=self._fake_download([])), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["cpython"],
                           {"linux": self.fixture_hash}), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.install_runtime(dest=self.dest, plat="linux")

        self.assertFalse(result["success"])
        self.assertIn("requirements install failed", result["error"])
        self.assertIn("resolution impossible", result["error"])
        self.assertEqual(len(calls), 1, "smoke must not run after a failed install")

    def test_darwin_is_rejected_without_downloading(self):
        downloaded = []
        fake_run, calls = make_fake_run()

        with patch.object(dependency_manager, "download_file",
                          side_effect=self._fake_download(downloaded)), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.install_runtime(dest=self.dest, plat="darwin")

        self.assertFalse(result["success"])
        self.assertIn("not available", result["error"])
        self.assertEqual(downloaded, [])
        self.assertEqual(calls, [])


class InstallRuntimeWindowsTest(unittest.TestCase):
    """embed + ._pth patch + pinned get-pip + requirements + 10-import smoke."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.base_dir = Path(self.temp_dir.name).resolve()
        self.dest = self.base_dir / "python"
        self.fixture = build_embed_zip()
        self.fixture_hash = sha256_of(self.fixture)
        self.get_pip_hash = sha256_of(FAKE_GET_PIP_PAYLOAD)

    def tearDown(self):
        self.temp_dir.cleanup()

    def _fake_download(self, downloaded_urls):
        def fake(url, dest_path, progress_callback=None):
            downloaded_urls.append(url)
            if url == dependency_manager.TOOL_URLS["get-pip"]["windows"]:
                dest_path.write_bytes(FAKE_GET_PIP_PAYLOAD)
            else:
                dest_path.write_bytes(self.fixture)
            return True

        return fake

    def test_patches_pth_bootstraps_pip_and_smokes_ten_imports(self):
        downloaded = []
        fake_run, calls = make_fake_run()

        with patch.object(dependency_manager, "download_file",
                          side_effect=self._fake_download(downloaded)), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["cpython"],
                           {"windows": self.fixture_hash}), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["get-pip"],
                           {"windows": self.get_pip_hash}), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.install_runtime(dest=self.dest, plat="windows")

        self.assertTrue(result["success"], result)
        self.assertEqual(result["platform"], "windows")

        # `import site` enabled, no commented line left behind.
        pth = self.dest / EMBED_PTH_NAME
        self.assertEqual(pth.read_text(encoding="utf-8"), EMBED_PTH_ENABLED)

        # Both pins downloaded, nothing else.
        self.assertEqual(
            sorted(downloaded),
            sorted([
                dependency_manager.TOOL_URLS["cpython"]["windows"],
                dependency_manager.TOOL_URLS["get-pip"]["windows"],
            ]),
        )

        # Order: get-pip bootstrap -> requirements -> canonical smoke.
        get_pip, pip, smoke = classify_windows_calls(calls)
        self.assertEqual(len(get_pip), 1)
        self.assertEqual(len(pip), 1)
        self.assertEqual(len(smoke), 1)
        self.assertLess(
            calls.index(get_pip[0]), calls.index(pip[0]), "get-pip before pip"
        )
        self.assertLess(calls.index(pip[0]), calls.index(smoke[0]), "pip before smoke")

        # get-pip runs with the mounted interpreter and is removed afterwards.
        self.assertEqual(get_pip[0]["cmd"][0], str(self.dest / "python.exe"))
        self.assertFalse((self.dest / "get-pip.py").exists())

        # The smoke carries the full canonical list — the four modules the old
        # six-import check never verified on Windows.
        smoke_cmd = smoke[0]["cmd"]
        for module in ("spotipy", "acoustid", "dotenv", "requests", "mutagen",
                       "yarl", "aiohttp", "yaml", "musicbrainzngs", "m3u8"):
            self.assertIn(module, smoke_cmd[2])
        self.assertTrue(pth.exists())

    def test_missing_pth_fails_the_mount(self):
        no_pth = io.BytesIO()
        with zipfile.ZipFile(no_pth, "w") as zf:
            zf.writestr("python.exe", FAKE_CPYTHON_PAYLOAD)
        self.fixture = no_pth.getvalue()
        self.fixture_hash = sha256_of(self.fixture)
        fake_run, calls = make_fake_run()

        with patch.object(dependency_manager, "download_file",
                          side_effect=self._fake_download([])), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["cpython"],
                           {"windows": self.fixture_hash}), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["get-pip"],
                           {"windows": self.get_pip_hash}), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.install_runtime(dest=self.dest, plat="windows")

        self.assertFalse(result["success"])
        self.assertIn("no *._pth file", result["error"])
        self.assertEqual(
            calls, [], "without site there is no working pip: nothing must run"
        )

    def test_pth_already_enabled_is_idempotent(self):
        self.fixture = build_embed_zip(pth_content=EMBED_PTH_ENABLED)
        self.fixture_hash = sha256_of(self.fixture)
        fake_run, calls = make_fake_run()

        with patch.object(dependency_manager, "download_file",
                          side_effect=self._fake_download([])), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["cpython"],
                           {"windows": self.fixture_hash}), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["get-pip"],
                           {"windows": self.get_pip_hash}), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.install_runtime(dest=self.dest, plat="windows")

        self.assertTrue(result["success"], result)
        # Nothing got double-patched and the flow still completed.
        self.assertEqual(
            (self.dest / EMBED_PTH_NAME).read_text(encoding="utf-8"),
            EMBED_PTH_ENABLED,
        )
        _, _, smoke = classify_windows_calls(calls)
        self.assertEqual(len(smoke), 1)

    def test_get_pip_hash_mismatch_aborts_before_bootstrap(self):
        downloaded = []
        fake_run, calls = make_fake_run()

        with patch.object(dependency_manager, "download_file",
                          side_effect=self._fake_download(downloaded)), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["cpython"],
                           {"windows": self.fixture_hash}), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["get-pip"],
                           {"windows": "f" * 64}), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.install_runtime(dest=self.dest, plat="windows")

        self.assertFalse(result["success"])
        self.assertIn("get-pip.py SHA-256 mismatch", result["error"])
        self.assertEqual(
            calls, [], "an unverified bootstrap must never be executed"
        )
        self.assertFalse((self.dest / "get-pip.py").exists(), "purged, not kept")

    def test_get_pip_run_failure_aborts(self):
        def fail_get_pip(cmd):
            if cmd[1].endswith("get-pip.py"):
                return (3, "ensurepip is unavailable in this build")
            return None

        fake_run, calls = make_fake_run(fail_when=fail_get_pip)

        with patch.object(dependency_manager, "download_file",
                          side_effect=self._fake_download([])), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["cpython"],
                           {"windows": self.fixture_hash}), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["get-pip"],
                           {"windows": self.get_pip_hash}), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.install_runtime(dest=self.dest, plat="windows")

        self.assertFalse(result["success"])
        self.assertIn("get-pip bootstrap failed", result["error"])
        _, pip, smoke = classify_windows_calls(calls)
        self.assertEqual(pip, [], "no requirements without pip")
        self.assertEqual(smoke, [])

    def test_smoke_failure_fails_the_install(self):
        canonical = dependency_manager.smoke_import_statement()

        def fail_smoke(cmd):
            if "-c" in cmd and canonical in cmd[2]:
                return (1, "ModuleNotFoundError: No module named 'm3u8'")
            return None

        fake_run, _ = make_fake_run(fail_when=fail_smoke)

        with patch.object(dependency_manager, "download_file",
                          side_effect=self._fake_download([])), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["cpython"],
                           {"windows": self.fixture_hash}), \
                patch.dict(dependency_manager.KNOWN_SHA256_HASHES["get-pip"],
                           {"windows": self.get_pip_hash}), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.install_runtime(dest=self.dest, plat="windows")

        self.assertFalse(result["success"])
        self.assertIn("import smoke failed", result["error"])
        self.assertIn("No module named 'm3u8'", result["error"])


class CanonicalSmokeTest(unittest.TestCase):
    """The smoke runner: clean env, honest failures, bounded time."""

    def test_clears_python_env_and_keeps_the_rest(self):
        fake_run, calls = make_fake_run()
        host_env = {
            "PATH": "/usr/bin",
            "PYTHONHOME": "/host/python",
            "PYTHONPATH": "/host/site-packages",
            "PYTHONDONTWRITEBYTECODE": "1",
            "LD_LIBRARY_PATH": "/appimage/usr/lib",
        }
        with patch.dict(os.environ, host_env, clear=True), \
                patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.run_canonical_smoke("/artifact/python")

        self.assertTrue(result["success"], result)
        env = calls[0]["env"]
        for cleared in dependency_manager.RUNTIME_ENV_VARS_TO_CLEAR:
            self.assertNotIn(cleared, env)
        self.assertEqual(env.get("LD_LIBRARY_PATH"), "/appimage/usr/lib")
        self.assertEqual(env.get("PATH"), "/usr/bin")

    def test_failure_carries_the_stderr(self):
        fake_run, _ = make_fake_run(fail_when=lambda cmd: (1, "No module named 'yarl'"))
        with patch.object(dependency_manager.subprocess, "run", fake_run):
            result = dependency_manager.run_canonical_smoke("/artifact/python")
        self.assertFalse(result["success"])
        self.assertIn("No module named 'yarl'", result["error"])

    def test_timeout_is_reported(self):
        def timed_out(cmd, capture_output=False, text=False, timeout=None, env=None):
            raise subprocess.TimeoutExpired(cmd=cmd, timeout=timeout)

        with patch.object(dependency_manager.subprocess, "run", timed_out):
            result = dependency_manager.run_canonical_smoke("/artifact/python")
        self.assertFalse(result["success"])
        self.assertIn("timed out", result["error"])


class RuntimeCliEnvelopeTest(unittest.TestCase):
    """install-runtime / smoke-imports speak the same JSON envelope contract."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.base_dir = Path(self.temp_dir.name).resolve()
        self.dest = self.base_dir / "bundle"
        self.fixture = build_standalone_tarball()
        self.fixture_hash = sha256_of(self.fixture)
        self.get_pip_hash = sha256_of(FAKE_GET_PIP_PAYLOAD)
        self.stdout = io.StringIO()

    def tearDown(self):
        self.temp_dir.cleanup()

    def _run_cli(self, *argv, extra_patches=()):
        patches = [
            patch.object(sys, "argv", ["dependency_manager.py", *argv]),
            patch("sys.stdout", self.stdout),
            *extra_patches,
        ]
        for patcher in patches:
            patcher.start()
        try:
            rc = dependency_manager.main()
        finally:
            for patcher in reversed(patches):
                patcher.stop()
        return rc, self.stdout.getvalue()

    def _linux_fakes(self):
        def fake_download(url, dest_path, progress_callback=None):
            dest_path.write_bytes(self.fixture)
            return True

        fake_run, calls = make_fake_run()
        return [
            patch.object(dependency_manager, "download_file", side_effect=fake_download),
            patch.dict(dependency_manager.KNOWN_SHA256_HASHES["cpython"],
                       {"linux": self.fixture_hash}),
            patch.object(dependency_manager.subprocess, "run", fake_run),
        ]

    def test_install_runtime_success_envelope(self):
        rc, out = self._run_cli(
            "install-runtime", "--dest", str(self.dest),
            extra_patches=self._linux_fakes(),
        )
        self.assertEqual(rc, 0, out)
        envelope = json.loads(out)
        self.assertTrue(envelope["success"])
        self.assertEqual(envelope["data"]["platform"], "linux")
        self.assertEqual(envelope["data"]["runtime_version"], "3.11.9")
        self.assertTrue(
            envelope["data"]["python"].endswith("python/bin/python3"),
            envelope["data"]["python"],
        )
        self.assertEqual(len(envelope["data"]["imports"]), 10)

    def test_install_runtime_failure_envelope(self):
        bad_linux_fakes = self._linux_fakes()
        # Replace the good hash patch with a wrong one.
        bad_linux_fakes[1] = patch.dict(
            dependency_manager.KNOWN_SHA256_HASHES["cpython"], {"linux": "0" * 64}
        )
        rc, out = self._run_cli(
            "install-runtime", "--dest", str(self.dest),
            extra_patches=bad_linux_fakes,
        )
        self.assertEqual(rc, 1, out)
        envelope = json.loads(out)
        self.assertFalse(envelope["success"])
        self.assertIn("SHA-256 mismatch", envelope["error"])

    def test_smoke_imports_success_envelope(self):
        fake_run, calls = make_fake_run()
        rc, out = self._run_cli(
            "smoke-imports", "--python", "/artifact/python/bin/python3",
            extra_patches=[patch.object(dependency_manager.subprocess, "run", fake_run)],
        )
        self.assertEqual(rc, 0, out)
        envelope = json.loads(out)
        self.assertTrue(envelope["success"])
        self.assertEqual(envelope["data"]["python"], "/artifact/python/bin/python3")
        self.assertEqual(calls[0]["cmd"][0], "/artifact/python/bin/python3")

    def test_smoke_imports_failure_envelope(self):
        fake_run, _ = make_fake_run(
            fail_when=lambda cmd: (1, "ModuleNotFoundError: No module named 'yaml'")
        )
        rc, out = self._run_cli(
            "smoke-imports", "--python", "/artifact/python/bin/python3",
            extra_patches=[patch.object(dependency_manager.subprocess, "run", fake_run)],
        )
        self.assertEqual(rc, 1, out)
        envelope = json.loads(out)
        self.assertFalse(envelope["success"])
        self.assertIn("No module named 'yaml'", envelope["error"])


if __name__ == "__main__":
    unittest.main(verbosity=2)

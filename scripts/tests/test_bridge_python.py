#!/usr/bin/env python3
"""Gate for the interpreter resolution the CLI contract tests depend on.

`sys.executable` is not always a Python: in a session hosted by an AppImage the
environment exports `APPIMAGE` and CPython reports the host application as its
own executable, so spawning it launches a GUI process instead of the bridge.
`bridge_python.python_executable()` is the single place that decides, and this
module is what keeps that decision honest.
"""

import contextlib
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import bridge_python


@contextlib.contextmanager
def hosted_by_an_appimage():
    """Simulate the host that started this bug: `sys.executable` is the app.

    The stand-in is a real executable file of its own, not a path aliased onto
    the interpreter: a symlink would make `sys.executable` and the genuine
    interpreter share a realpath, which is not what the AppImage layout does
    and would hide the fallback this test exists to exercise.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        host_app = Path(tmpdir) / "Host.AppImage"
        host_app.write_bytes(b"not a python interpreter")
        host_app.chmod(0o755)
        with patch.dict(os.environ, {"APPIMAGE": str(host_app)}), \
             patch.object(bridge_python.sys, "executable", str(host_app)):
            yield host_app


class PythonExecutableTests(unittest.TestCase):
    """The resolver returns a runnable interpreter in every host layout."""

    def test_resolution_is_an_executable_file(self):
        resolved = bridge_python.python_executable()

        path = Path(resolved)
        self.assertTrue(path.is_file(), f"resolved to something that is not a file: {resolved}")
        self.assertTrue(os.access(path, os.X_OK), f"resolved to a non-executable file: {resolved}")
        self.assertTrue(
            bridge_python.is_interpreter(resolved), "the resolver returned its own rejection"
        )

    def test_a_clean_host_keeps_sys_executable(self):
        """Without `APPIMAGE` there is nothing to distrust, so nothing changes."""
        with patch.dict(os.environ, {}, clear=False):
            os.environ.pop("APPIMAGE", None)
            with patch.object(bridge_python.sys, "executable", sys.executable):
                self.assertEqual(bridge_python.python_executable(), sys.executable)

    def test_the_hosting_appimage_is_never_returned(self):
        """The regression: `sys.executable` pointing at the AppImage must lose.

        This is the state that made the CLI tests read Electron logs: the host
        application occupies the slot where the interpreter should be.
        """
        with hosted_by_an_appimage() as host_app:
            self.assertFalse(
                bridge_python.is_interpreter(str(host_app)),
                "the hosting AppImage passed as an interpreter",
            )
            resolved = bridge_python.python_executable()

        self.assertNotEqual(
            os.path.realpath(resolved), os.path.realpath(host_app),
            "the resolver handed back the hosting application",
        )
        self.assertTrue(Path(resolved).is_file(), f"fallback is not a file: {resolved}")

    def test_the_fallback_is_the_running_interpreter_prefix(self):
        """With `sys.executable` rejected, the prefix interpreter comes next."""
        prefix_interpreter = Path(sys.base_prefix) / "bin" / "python3"
        if not bridge_python.is_interpreter(prefix_interpreter):
            self.skipTest("this interpreter has no <base_prefix>/bin/python3 to fall back to")

        with hosted_by_an_appimage():
            self.assertEqual(bridge_python.python_executable(), str(prefix_interpreter))

    def test_a_non_executable_or_missing_candidate_is_rejected(self):
        """The check is structural, so a plain path never passes as an interpreter."""
        for candidate in (None, "", "/nonexistent/python3", sys.executable + "-not-a-file"):
            with self.subTest(candidate=candidate):
                self.assertFalse(bridge_python.is_interpreter(candidate))

    def test_no_interpreter_anywhere_is_an_explicit_error(self):
        """Failing loudly beats launching the wrong binary."""
        with patch.object(bridge_python.sys, "executable", "/nonexistent/python3"), \
             patch.object(bridge_python.shutil, "which", return_value=None), \
             patch.object(bridge_python.Path, "is_file", return_value=False):
            with self.assertRaises(RuntimeError) as ctx:
                bridge_python.python_executable()

        self.assertIn("no Python interpreter available", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()

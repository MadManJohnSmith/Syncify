#!/usr/bin/env python3
"""
Contract tests for `auth_bridge.py` actions (PY-9).

`refresh` was accepted for every service, but only Spotify and Tidal implement
it: for Qobuz, Deezer, SoundCloud and Apple Music the handler returned None and
`main()` printed nothing while exiting 0 — indistinguishable from success for
every caller of the bridge.

Validates:
1. Only services whose provider issues renewable tokens are refresh-capable.
2. `refresh` on any other service returns {"success": false, "error": ...} and
   a non-zero exit code, from the CLI and from `main()`.
3. A handler that produces no payload can never exit 0 in silence.
"""

import io
import json
import subprocess
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import auth_bridge
from bridge_python import python_executable

NON_REFRESHABLE_SERVICES = ("qobuz", "deezer", "soundcloud", "apple_music")


class AuthActionContractTests(unittest.TestCase):
    """No action may answer with silence and exit code 0."""

    def test_refresh_capable_services(self):
        self.assertEqual(set(auth_bridge.REFRESH_CAPABLE_SERVICES), {"spotify", "tidal"})

    def test_main_rejects_refresh_for_services_without_token_refresh(self):
        for service in NON_REFRESHABLE_SERVICES:
            with self.subTest(service=service):
                argv = ["auth_bridge.py", service, "refresh"]
                buffer = io.StringIO()
                with patch.object(sys, "argv", argv), redirect_stdout(buffer):
                    with self.assertRaises(SystemExit) as ctx:
                        auth_bridge.main()

                self.assertEqual(ctx.exception.code, 1)
                payload = json.loads(buffer.getvalue())
                self.assertFalse(payload["success"])
                self.assertIn("refresh is not supported", payload["error"])
                self.assertIn(service, payload["error"])
                self.assertIn("spotify, tidal", payload["error"])

    def test_main_reports_handlers_that_return_nothing(self):
        buffer = io.StringIO()
        argv = ["auth_bridge.py", "qobuz", "status"]
        with patch.object(sys, "argv", argv), \
             patch.dict(auth_bridge.HANDLERS, {"qobuz": lambda action: None}), \
             redirect_stdout(buffer):
            with self.assertRaises(SystemExit) as ctx:
                auth_bridge.main()

        self.assertEqual(ctx.exception.code, 1)
        payload = json.loads(buffer.getvalue())
        self.assertFalse(payload["success"])
        self.assertIn("No result produced", payload["error"])

    def test_cli_refresh_for_non_refreshable_service_emits_json(self):
        for service in NON_REFRESHABLE_SERVICES:
            with self.subTest(service=service):
                proc = subprocess.run(
                    [python_executable(), str(SCRIPTS_DIR / "auth_bridge.py"), service, "refresh"],
                    capture_output=True,
                    text=True,
                    timeout=120,
                )

                self.assertEqual(proc.returncode, 1, proc.stderr)
                payload = json.loads(proc.stdout)
                self.assertFalse(payload["success"])
                self.assertIn("refresh", payload["error"].lower())

    def test_module_docstring_states_the_refresh_limitation(self):
        source = (SCRIPTS_DIR / "auth_bridge.py").read_text(encoding="utf-8")

        self.assertIn("refresh only for spotify and tidal", source)


if __name__ == "__main__":
    unittest.main()
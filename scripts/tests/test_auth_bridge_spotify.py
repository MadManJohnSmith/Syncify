#!/usr/bin/env python3
"""
TASK-50: Regression tests for Spotify auth handling in auth_bridge.py.

Verifies that:
1. handle_spotify does not attempt to import non-existent services.spotify_auth.
2. handle_spotify returns a structured response indicating Spotify auth is handled
   natively in Rust via OAuth PKCE in Tauri.
3. Invoking auth_bridge.py with `--service spotify` or `spotify <action>` outputs
   clean JSON without ModuleNotFoundError and exits with controlled status.
"""

import json
import subprocess
import sys
import unittest
from pathlib import Path

# Add scripts directory to path
SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from auth_bridge import handle_spotify  # noqa: E402


class TestAuthBridgeSpotify(unittest.TestCase):
    """Regression test suite for Spotify auth bridge behavior."""

    def test_handle_spotify_function_direct(self):
        """Verify calling handle_spotify directly returns valid structured dict."""
        for action in ["status", "login", "logout", None]:
            result = handle_spotify(action)
            self.assertIsInstance(result, dict)
            self.assertFalse(result.get("success"), "Expected success to be False")
            self.assertEqual(result.get("service"), "spotify")
            self.assertTrue(result.get("native"), "Expected native flag to be True")
            self.assertIn("PKCE", result.get("message", ""))

    def test_handle_spotify_with_dict_arg(self):
        """Verify calling handle_spotify with dict or namespace arguments works."""
        result = handle_spotify({"service": "spotify", "action": "status"})
        self.assertIsInstance(result, dict)
        self.assertFalse(result.get("success"))
        self.assertEqual(result.get("service"), "spotify")
        self.assertTrue(result.get("native"))

    def test_cli_positional_spotify_status(self):
        """Verify CLI execution: python3 scripts/auth_bridge.py spotify status."""
        script_path = SCRIPTS_DIR / "auth_bridge.py"
        res = subprocess.run(
            [sys.executable, str(script_path), "spotify", "status"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(res.returncode, 0, f"Process failed with stderr: {res.stderr}")
        self.assertNotIn("ModuleNotFoundError", res.stderr)
        self.assertNotIn("Traceback", res.stderr)

        data = json.loads(res.stdout.strip())
        self.assertFalse(data["success"])
        self.assertEqual(data["service"], "spotify")
        self.assertTrue(data["native"])
        self.assertIn("Rust", data["message"])
        self.assertIn("PKCE", data["message"])

    def test_cli_positional_spotify_login(self):
        """Verify CLI execution: python3 scripts/auth_bridge.py spotify login."""
        script_path = SCRIPTS_DIR / "auth_bridge.py"
        res = subprocess.run(
            [sys.executable, str(script_path), "spotify", "login"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(res.returncode, 0, f"Process failed with stderr: {res.stderr}")
        self.assertNotIn("ModuleNotFoundError", res.stderr)
        self.assertNotIn("Traceback", res.stderr)

        data = json.loads(res.stdout.strip())
        self.assertFalse(data["success"])
        self.assertEqual(data["service"], "spotify")
        self.assertTrue(data["native"])

    def test_cli_flag_service_spotify(self):
        """Verify CLI execution: python3 scripts/auth_bridge.py --service spotify."""
        script_path = SCRIPTS_DIR / "auth_bridge.py"
        res = subprocess.run(
            [sys.executable, str(script_path), "--service", "spotify"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(res.returncode, 0, f"Process failed with stderr: {res.stderr}")
        self.assertNotIn("ModuleNotFoundError", res.stderr)
        self.assertNotIn("Traceback", res.stderr)

        data = json.loads(res.stdout.strip())
        self.assertFalse(data["success"])
        self.assertEqual(data["service"], "spotify")
        self.assertTrue(data["native"])

    def test_cli_flag_service_and_action(self):
        """Verify CLI execution: python3 scripts/auth_bridge.py --service spotify --action login."""
        script_path = SCRIPTS_DIR / "auth_bridge.py"
        res = subprocess.run(
            [sys.executable, str(script_path), "--service", "spotify", "--action", "login"],
            capture_output=True,
            text=True,
        )
        self.assertEqual(res.returncode, 0, f"Process failed with stderr: {res.stderr}")
        self.assertNotIn("ModuleNotFoundError", res.stderr)
        self.assertNotIn("Traceback", res.stderr)

        data = json.loads(res.stdout.strip())
        self.assertFalse(data["success"])
        self.assertEqual(data["service"], "spotify")
        self.assertTrue(data["native"])


if __name__ == "__main__":
    unittest.main()

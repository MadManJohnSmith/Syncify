#!/usr/bin/env python3
"""
TASK-149 / SEC-022: Security Test Suite for Auth Bridge.

Verifies:
1. Executing `python3 scripts/auth_bridge.py spotify refresh` without stdin or env fails cleanly and controlledly.
2. Attempting to pass `sp_dc` as a CLI argument (positional or flag) is strictly prohibited and rejected.
3. Passing `sp_dc` via stdin (JSON `{"sp_dc": "..."}` or stripped string) is processed securely without leaking the cookie in stdout/stderr.
4. Passing `sp_dc` via secure environment variable `SYNCIFY_SP_DC` works securely.
5. In-flight inspection confirms the sensitive token is never present in process argv / `/proc/<pid>/cmdline`.
"""

import json
import os
import subprocess
import sys
import unittest
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
SCRIPT_PATH = SCRIPTS_DIR / "auth_bridge.py"


class TestAuthBridgeSecurity(unittest.TestCase):
    def setUp(self):
        # Base environment without pre-existing sp_dc cookies
        self.clean_env = os.environ.copy()
        self.clean_env.pop("SYNCIFY_SP_DC", None)
        self.clean_env.pop("SPOTIFY_SP_DC", None)
        self.clean_env["SYNCIFY_AUTH_MOCK"] = "1"

    def test_refresh_without_stdin_fails_controlled(self):
        """Executing spotify refresh without stdin or env var fails controlledly."""
        proc = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "spotify", "refresh"],
            input="",
            capture_output=True,
            text=True,
            env=self.clean_env,
        )

        self.assertNotEqual(proc.returncode, 0, "Expected non-zero exit code when stdin is missing")
        self.assertNotIn("Traceback", proc.stderr, f"Unexpected traceback: {proc.stderr}")

        data = json.loads(proc.stdout.strip())
        self.assertFalse(data.get("success"), "Expected success=False")
        self.assertIn("error", data)
        self.assertIn("sp_dc", data["error"])

    def test_refresh_with_cli_positional_sp_dc_prohibited(self):
        """Attempting to pass sp_dc as sys.argv[3] must be blocked and rejected."""
        leaked_secret = "AQB_SUPER_SECRET_COOKIE_LEAKED_IN_ARGV_9999"
        proc = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "spotify", "refresh", leaked_secret],
            capture_output=True,
            text=True,
            env=self.clean_env,
        )

        self.assertNotEqual(proc.returncode, 0, "CLI arguments with sp_dc must be rejected")
        self.assertNotIn("Traceback", proc.stderr)

        data = json.loads(proc.stdout.strip())
        self.assertFalse(data.get("success"))
        self.assertIn("CLI arguments is strictly prohibited", data.get("error", ""))

        # Verify sensitive cookie value is NOT leaked in stdout or stderr
        self.assertNotIn(leaked_secret, proc.stdout)
        self.assertNotIn(leaked_secret, proc.stderr)

    def test_refresh_with_cli_flag_sp_dc_prohibited(self):
        """Attempting to pass sp_dc via flags like --sp-dc must be blocked."""
        leaked_secret = "AQB_FLAG_SECRET_COOKIE_4321"
        proc = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "--service", "spotify", "--action", "refresh", f"--sp-dc={leaked_secret}"],
            capture_output=True,
            text=True,
            env=self.clean_env,
        )

        self.assertNotEqual(proc.returncode, 0)
        data = json.loads(proc.stdout.strip())
        self.assertFalse(data.get("success"))
        self.assertIn("CLI arguments is strictly prohibited", data.get("error", ""))
        self.assertNotIn(leaked_secret, proc.stdout)
        self.assertNotIn(leaked_secret, proc.stderr)

    def test_refresh_with_stdin_json_success(self):
        """Passing {'sp_dc': '...'} through stdin securely refreshes token."""
        secret_cookie = "mock_secret_sp_dc_cookie_payload_123"
        payload = json.dumps({"sp_dc": secret_cookie})

        proc = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "spotify", "refresh"],
            input=payload,
            capture_output=True,
            text=True,
            env=self.clean_env,
        )

        self.assertEqual(proc.returncode, 0, f"Failed with stderr: {proc.stderr}")
        data = json.loads(proc.stdout.strip())
        self.assertTrue(data.get("success"))
        self.assertIn("data", data)
        self.assertIn("accessToken", data["data"])
        self.assertFalse(data["data"].get("isAnonymous", True))

        # The input cookie must never be echoed in stdout or stderr
        self.assertNotIn(secret_cookie, proc.stdout)
        self.assertNotIn(secret_cookie, proc.stderr)

    def test_refresh_with_stdin_raw_text_success(self):
        """Passing raw sp_dc string through stdin securely refreshes token."""
        secret_cookie = "mock_secret_sp_dc_raw_cookie_456"

        proc = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "spotify", "refresh"],
            input=f"  {secret_cookie}  \n",
            capture_output=True,
            text=True,
            env=self.clean_env,
        )

        self.assertEqual(proc.returncode, 0, f"Failed with stderr: {proc.stderr}")
        data = json.loads(proc.stdout.strip())
        self.assertTrue(data.get("success"))
        self.assertIn("accessToken", data["data"])
        self.assertNotIn(secret_cookie, proc.stdout)
        self.assertNotIn(secret_cookie, proc.stderr)

    def test_refresh_with_secure_env_var_success(self):
        """Passing sp_dc through SYNCIFY_SP_DC environment variable works securely."""
        env_with_secret = self.clean_env.copy()
        secret_cookie = "mock_secret_sp_dc_from_env_789"
        env_with_secret["SYNCIFY_SP_DC"] = secret_cookie

        proc = subprocess.run(
            [sys.executable, str(SCRIPT_PATH), "spotify", "refresh"],
            input="",
            capture_output=True,
            text=True,
            env=env_with_secret,
        )

        self.assertEqual(proc.returncode, 0, f"Failed with stderr: {proc.stderr}")
        data = json.loads(proc.stdout.strip())
        self.assertTrue(data.get("success"))
        self.assertIn("accessToken", data["data"])
        self.assertNotIn(secret_cookie, proc.stdout)
        self.assertNotIn(secret_cookie, proc.stderr)

    def test_argv_inspection_confirms_no_sensitive_token(self):
        """Verify inspecting process cmdline shows no cookie token in argv."""
        secret_cookie = "mock_confidential_session_token_1111"
        payload = json.dumps({"sp_dc": secret_cookie})

        # Start child process with piped stdin
        p = subprocess.Popen(
            [sys.executable, str(SCRIPT_PATH), "spotify", "refresh"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env=self.clean_env,
        )

        # Inspect /proc/<pid>/cmdline if available (Linux)
        proc_cmdline_path = Path(f"/proc/{p.pid}/cmdline")
        if proc_cmdline_path.exists():
            cmdline = proc_cmdline_path.read_text(errors="replace")
            self.assertNotIn(secret_cookie, cmdline, "CRITICAL: Sensitive token found in /proc/<pid>/cmdline!")

        stdout, stderr = p.communicate(input=payload)
        self.assertEqual(p.returncode, 0)
        data = json.loads(stdout.strip())
        self.assertTrue(data.get("success"))


if __name__ == "__main__":
    unittest.main()

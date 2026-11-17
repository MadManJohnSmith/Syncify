#!/usr/bin/env python3
"""In-app webview login (issue #84): the Tidal device code flow is split.

The app's own webview shows the verification URL, so the bridge MUST NOT open
any browser on the `login_start` / `login_poll` path (issue #62: the consent of
the device code must not jump to an external browser). The split exists so the
app can open the window BEFORE the polling starts: `login_start` returns the
device code and the full verification URL without any browser, and
`login_poll` polls the token endpoint with the device code (delivered via
stdin, SEC-022 mold) also without any browser.

Validates:
1. `tidal_login_start` / `tidal_login_poll` never call `open_in_system_browser`
   (or `webbrowser`) and return the contract payload.
2. `auth_bridge._parse` accepts `login_start`/`login_poll` only for `tidal`.
3. `handle_tidal("login_start")` and `handle_tidal("login_poll")` return the
   payloads the Rust side consumes, with the tokens on success.
4. The legacy `login` action is untouched (it still opens the browser path).
"""

import io
import json
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import MagicMock, patch

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

try:
    import aiohttp  # noqa: F401
except ImportError:
    # Only `close()` and session creation are needed; a stub suffices.
    sys.modules["aiohttp"] = MagicMock()

import auth_bridge  # noqa: E402
from bridge_python import python_executable  # noqa: E402
from services import browser_launcher  # noqa: E402
from services import tidal_auth  # noqa: E402

VERIFICATION_URL = "https://link.tidal.com/XYZ789"
DEVICE_CODE = "device-code-xyz789"

TOKENS = {
    "access_token": "fake-access-token",
    "refresh_token": "fake-refresh-token",
    "token_expiry": 1750000000.0,
    "user_id": 424242,
    "user": {"userId": 424242, "countryCode": "US", "username": "ana"},
}


class _FakeSession:
    def __init__(self):
        self.closed = False

    async def close(self):
        self.closed = True


def _forbid_browser_launches(testcase):
    """Patch every browser launcher so a launch FAILS the test loudly."""
    for target in (
        "services.browser_launcher.open_in_system_browser",
        "webbrowser.open",
    ):
        p = patch(target, side_effect=AssertionError(f"browser launched on in-app path: {target}"))
        p.start()
        testcase.addCleanup(p.stop)


class TidalInAppLoginSplitTests(unittest.TestCase):
    """login_start/login_poll: device code without launching a browser."""

    def test_login_start_returns_device_code_without_browser(self):
        _forbid_browser_launches(self)
        auth = tidal_auth.TidalAuth(verbose=False)
        auth.session = _FakeSession()

        async def fake_device_code():
            return DEVICE_CODE, VERIFICATION_URL

        auth._get_device_code = fake_device_code

        result = tidal_auth.tidal_login_start(auth)

        self.assertEqual(result["status"], "success")
        self.assertEqual(result["device_code"], DEVICE_CODE)
        self.assertEqual(result["verification_url"], VERIFICATION_URL)
        self.assertTrue(auth.session is None or auth.session.closed,
                        "login_start no deja la sesión abierta")

    def test_login_start_reports_errors_without_browser(self):
        _forbid_browser_launches(self)
        auth = tidal_auth.TidalAuth(verbose=False)

        async def broken_device_code():
            raise RuntimeError("provider down")

        auth._get_device_code = broken_device_code
        auth.session = _FakeSession()

        result = tidal_auth.tidal_login_start(auth)

        self.assertEqual(result["status"], "error")
        self.assertIn("provider down", result["message"])

    def test_login_poll_never_opens_a_browser_and_returns_tokens(self):
        _forbid_browser_launches(self)
        auth = tidal_auth.TidalAuth(verbose=False)
        auth._stored_tokens = None
        poll_calls = {"n": 0}

        async def fake_auth_status(device_code):
            poll_calls["n"] += 1
            if poll_calls["n"] < 3:
                return 2, {}  # pending
            return 0, dict(TOKENS)

        auth._get_auth_status = fake_auth_status

        with patch.object(tidal_auth.asyncio, "sleep", __import__("asyncio").sleep):
            result = tidal_auth.tidal_login_poll(DEVICE_CODE, auth, timeout_seconds=30)

        self.assertEqual(result["status"], "success")
        self.assertEqual(result["message"], "Successfully connected to Tidal")
        self.assertGreaterEqual(poll_calls["n"], 3, "el sondeo itera hasta el éxito")

    def test_login_poll_rejects_an_empty_device_code(self):
        _forbid_browser_launches(self)
        result = tidal_auth.tidal_login_poll("")
        self.assertEqual(result["status"], "error")
        self.assertIn("device code", result["message"])

    def test_login_poll_times_out_without_browser(self):
        _forbid_browser_launches(self)
        auth = tidal_auth.TidalAuth(verbose=False)

        async def always_pending(device_code):
            return 2, {}

        auth._get_auth_status = always_pending
        result = tidal_auth.tidal_login_poll(DEVICE_CODE, auth, timeout_seconds=0.05)
        self.assertEqual(result["status"], "error")
        self.assertEqual(result["message"], "Authorization timed out")


class TidalInAppBridgeContractTests(unittest.TestCase):
    """auth_bridge: login_start/login_poll only for tidal, with tokens out."""

    def test_parse_accepts_the_split_actions_for_tidal(self):
        self.assertEqual(auth_bridge._parse(["tidal", "login_start"]), ("tidal", "login_start"))
        self.assertEqual(auth_bridge._parse(["tidal", "login_poll"]), ("tidal", "login_poll"))

    def test_parse_rejects_the_split_actions_for_other_services(self):
        for service in ("spotify", "qobuz", "deezer", "soundcloud", "apple_music"):
            with self.subTest(service=service):
                with self.assertRaises(auth_bridge.BridgeFailure) as ctx:
                    auth_bridge._parse([service, "login_start"])
                self.assertIn("only supported for 'tidal'", str(ctx.exception))

    def test_parse_still_rejects_unknown_actions(self):
        with self.assertRaises(auth_bridge.BridgeFailure):
            auth_bridge._parse(["tidal", "login_start_extra"])

    def test_handle_tidal_login_start_payload(self):
        with patch("services.tidal_auth.tidal_login_start") as fake_start:
            fake_start.return_value = {
                "status": "success",
                "message": "Device code created",
                "device_code": DEVICE_CODE,
                "verification_url": VERIFICATION_URL,
            }
            result = auth_bridge.handle_tidal("login_start")

        self.assertEqual(result["device_code"], DEVICE_CODE)
        self.assertEqual(result["verification_url"], VERIFICATION_URL)
        self.assertNotIn("success", result, "el dict viaja sin envelope propio")

    def test_handle_tidal_login_start_failure_raises_bridge_failure(self):
        with patch("services.tidal_auth.tidal_login_start") as fake_start:
            fake_start.return_value = {"status": "error", "message": "provider down"}
            with self.assertRaises(auth_bridge.BridgeFailure) as ctx:
                auth_bridge.handle_tidal("login_start")
        self.assertIn("provider down", str(ctx.exception))

    def test_handle_tidal_login_poll_reads_device_code_from_stdin(self):
        # SEC-022 mold: el device code entra por stdin (JSON), nunca por argv.
        stdin_payload = json.dumps({"device_code": DEVICE_CODE})
        with patch("services.tidal_auth.tidal_login_poll") as fake_poll, \
             patch("sys.stdin", new=io.StringIO(stdin_payload)), \
             patch("services.tidal_auth.TidalAuth") as fake_auth_cls:
            fake_poll.return_value = {"status": "success", "message": "Connected to Tidal"}
            fake_auth_cls.return_value.get_stored_tokens.return_value = dict(TOKENS)

            result = auth_bridge.handle_tidal("login_poll")

        fake_poll.assert_called_once_with(DEVICE_CODE)
        self.assertEqual(result["access_token"], "fake-access-token")
        self.assertEqual(result["refresh_token"], "fake-refresh-token")
        self.assertEqual(result["user_id"], "424242")
        self.assertEqual(result["display_name"], "ana")

    def test_handle_tidal_login_poll_failure_raises_bridge_failure(self):
        with patch("services.tidal_auth.tidal_login_poll") as fake_poll, \
             patch("sys.stdin", new=io.StringIO(json.dumps({"device_code": DEVICE_CODE}))):
            fake_poll.return_value = {"status": "error", "message": "Authorization failed"}
            with self.assertRaises(auth_bridge.BridgeFailure) as ctx:
                auth_bridge.handle_tidal("login_poll")
        self.assertIn("Authorization failed", str(ctx.exception))

    def test_main_login_start_via_cli_emits_envelope_without_browser(self):
        _forbid_browser_launches(self)
        with patch("services.tidal_auth.tidal_login_start") as fake_start:
            fake_start.return_value = {
                "status": "success",
                "message": "Device code created",
                "device_code": DEVICE_CODE,
                "verification_url": VERIFICATION_URL,
            }
            buffer = io.StringIO()
            with patch.object(sys, "argv", ["auth_bridge.py", "tidal", "login_start"]), \
                 redirect_stdout(buffer):
                code = auth_bridge.main()

        self.assertEqual(code, 0)
        payload = json.loads(buffer.getvalue())
        self.assertTrue(payload["success"])
        self.assertEqual(payload["data"]["device_code"], DEVICE_CODE)
        self.assertEqual(payload["data"]["verification_url"], VERIFICATION_URL)

    def test_legacy_login_action_still_exists(self):
        # El camino histórico no cambia en este paso (#62: conviven hasta que
        # los seis flujos migren).
        from services.tidal_auth import tidal_login  # noqa: F401
        self.assertTrue(callable(tidal_login))


if __name__ == "__main__":
    unittest.main()

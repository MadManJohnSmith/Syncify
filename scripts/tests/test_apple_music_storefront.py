#!/usr/bin/env python3
"""
Apple Music storefront plumbing (DO-1).

The Rust client drives every `/catalog/{storefront}/` request from the
storefront stored with the account credentials, and the only producer of that
credential is the login response of `auth_bridge.handle_apple_music`.

Verifies that:
1. `validate_token` remembers the storefront reported by `me/storefront`.
2. The login payload carries it, so it is persisted with the credentials.
"""

import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from services.apple_music_auth import AppleMusicAuth  # noqa: E402


class _FakeResponse:
    def __init__(self, status_code, payload):
        self.status_code = status_code
        self._payload = payload

    def json(self):
        return self._payload


class AppleMusicStorefrontTests(unittest.TestCase):
    """The storefront must reach the credentials saved by the Rust side."""

    def test_validate_token_keeps_the_storefront_reported_by_apple(self):
        auth = AppleMusicAuth(verbose=False)
        auth._access_token = "dev-token"

        with patch(
            "services.apple_music_auth.requests.get",
            return_value=_FakeResponse(200, {"data": [{"id": "mx"}]}),
        ):
            is_valid, storefront = auth.validate_token("user-token")

        self.assertTrue(is_valid)
        self.assertEqual(storefront, "mx")
        self.assertEqual(
            auth.storefront,
            "mx",
            "the storefront must survive after validation for the login payload",
        )

    def test_storefront_stays_unset_when_apple_does_not_report_one(self):
        auth = AppleMusicAuth(verbose=False)
        auth._access_token = "dev-token"

        with patch(
            "services.apple_music_auth.requests.get",
            return_value=_FakeResponse(200, {"data": [{}]}),
        ):
            is_valid, storefront = auth.validate_token("user-token")

        self.assertTrue(is_valid)
        self.assertEqual(storefront, "unknown")
        self.assertIsNone(auth.storefront)

    def test_login_payload_includes_the_storefront(self):
        import auth_bridge

        class _StubAuth:
            """Stands in for a completed browser login."""

            _access_token = "dev-token"

            def __init__(self, settings_file=None, verbose=False):
                pass

            async def login_with_browser(self, timeout_seconds: int = 300):
                return True, "user-token"

            def _fetch_access_token(self):
                return "dev-token"

            @property
            def storefront(self):
                return "de"

        captured = {}

        def fake_json_response(success, payload=None, error=None):
            captured["success"] = success
            captured["payload"] = payload
            raise SystemExit(0 if success else 1)

        with patch.object(
            auth_bridge, "json_response", fake_json_response
        ), patch(
            "services.apple_music_auth.AppleMusicAuth", _StubAuth
        ):
            try:
                auth_bridge.handle_apple_music("login")
            except SystemExit:
                pass

        self.assertTrue(captured.get("success"), captured)
        payload = captured.get("payload") or {}
        self.assertEqual(payload.get("music_user_token"), "user-token")
        self.assertEqual(payload.get("developer_token"), "dev-token")
        self.assertEqual(
            payload.get("storefront"),
            "de",
            "the storefront must be stored with the credentials",
        )


if __name__ == "__main__":
    unittest.main()
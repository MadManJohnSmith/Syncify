#!/usr/bin/env python3
"""Offline proof that Qobuz requests are signed with the operator's secret.

SYNC-AUD-062: `QobuzService` signed every `track/getFileUrl` request with the
class constant `APP_SECRET = ""`, and the app secret supplied by the operator
(`ServiceCredentials.app_secret` / `client_secret` / `extra["app_secret"]`, fed
by `QOBUZ_APP_SECRET` in `download_bridge.get_qobuz_service`) was accepted and
then discarded. The audit could not confirm the failure against the live Qobuz
API, so this module proves the contract hermetically: the secret reaches the
signature, an empty secret is refused before any request, and the digest is
byte-identical to the Rust core algorithm. No network access is required.
"""

import hashlib
import os
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
for site_packages in REPO_ROOT.glob(".venv/lib/python*/site-packages"):
    if site_packages.is_dir():
        sys.path.insert(0, str(site_packages))
sys.path.insert(0, str(REPO_ROOT / "scripts"))

# Keep the module importable in dependency-minimal checkouts: the assertions
# below are about signing, not about aiohttp.
try:
    import aiohttp  # noqa: F401
except ModuleNotFoundError:
    from unittest.mock import MagicMock

    sys.modules["aiohttp"] = MagicMock()

from services.qobuz_service import QobuzService, QobuzSigningSecretMissing
from services.service_base import ServiceCredentials

# (format_id, track_id, float timestamp, secret) pinned as a golden vector.
VECTOR_FORMAT_ID = "6"
VECTOR_TRACK_ID = "12345"
VECTOR_TIMESTAMP = 1700000000.0
VECTOR_SECRET = "offline-vector-secret"
VECTOR_SIGNATURE = "a7e181ae0107455acbba8117a071bda9"


class _FakeResponse:
    def __init__(self, payload, status=200):
        self._payload = payload
        self.status = status

    async def json(self):
        return self._payload


class _AsyncContext:
    def __init__(self, response):
        self._response = response

    async def __aenter__(self):
        return self._response

    async def __aexit__(self, *_exc):
        return False


class RecordingSession:
    """Minimal aiohttp.ClientSession stand-in that records the request params."""

    def __init__(self, payload=None, status=200):
        self.payload = payload if payload is not None else {}
        self.status = status
        self.calls = []

    def get(self, url, params=None, headers=None, timeout=None):
        self.calls.append({"url": url, "params": dict(params or {})})
        return _AsyncContext(_FakeResponse(self.payload, self.status))

    @property
    def sent_params(self):
        return self.calls[0]["params"] if self.calls else None


def _service(**credential_kwargs) -> QobuzService:
    service = QobuzService(ServiceCredentials(username="user", password="pass", **credential_kwargs))
    service._authenticated = True
    service.user_auth_token = "offline-test-token"
    return service


class QobuzSigningSecretTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        for name in ("QOBUZ_APP_ID", "QOBUZ_APP_SECRET"):
            os.environ.pop(name, None)

    async def test_operator_secret_from_credentials_reaches_the_signature(self):
        """The secret the operator supplies is used to sign, not discarded."""
        service = _service(app_secret="operator-secret")
        session = RecordingSession({"url": "https://example.invalid/track.flac", "format_id": 6})
        service.session = session

        with patch("services.qobuz_service.time.time", return_value=VECTOR_TIMESTAMP):
            url = await service._get_download_url(VECTOR_TRACK_ID, 6)

        self.assertEqual(url, "https://example.invalid/track.flac")
        params = session.sent_params
        self.assertIsNotNone(params, "the signed request must actually be sent")
        expected = hashlib.md5(
            (
                f"trackgetFileUrlformat_id{VECTOR_FORMAT_ID}intentstream"
                f"track_id{VECTOR_TRACK_ID}{VECTOR_TIMESTAMP}operator-secret"
            ).encode()
        ).hexdigest()
        self.assertEqual(params["request_sig"], expected)
        empty_secret_signature = hashlib.md5(
            (
                f"trackgetFileUrlformat_id{VECTOR_FORMAT_ID}intentstream"
                f"track_id{VECTOR_TRACK_ID}{VECTOR_TIMESTAMP}"
            ).encode()
        ).hexdigest()
        self.assertNotEqual(
            params["request_sig"],
            empty_secret_signature,
            "the signature must not be the one produced with an empty secret",
        )

    async def test_secret_aliases_and_environment_are_honoured(self):
        """client_secret and extra['app_secret'] are equivalent entry points."""
        self.assertEqual(
            _service(client_secret="from-client-secret").APP_SECRET, "from-client-secret"
        )
        self.assertEqual(
            QobuzService(
                ServiceCredentials(
                    username="u", password="p", extra={"app_secret": "from-extra"}
                )
            ).APP_SECRET,
            "from-extra",
        )
        with patch.dict(os.environ, {"QOBUZ_APP_SECRET": "from-env"}):
            self.assertEqual(_service().APP_SECRET, "from-env")
        # Credentials win over the environment.
        with patch.dict(os.environ, {"QOBUZ_APP_SECRET": "from-env"}):
            self.assertEqual(_service(app_secret="from-credentials").APP_SECRET, "from-credentials")

    async def test_app_id_is_overridable_without_touching_the_committed_value(self):
        self.assertEqual(
            _service(app_id="operator-app-id").APP_ID, "operator-app-id"
        )
        with patch.dict(os.environ, {"QOBUZ_APP_ID": "env-app-id"}):
            self.assertEqual(_service().APP_ID, "env-app-id")
        self.assertEqual(_service().APP_ID, QobuzService.APP_ID)

    async def test_no_secret_refuses_to_sign_and_sends_nothing(self):
        """An unconfigured secret is a hard stop, not a silently invalid signature."""
        service = _service()
        session = RecordingSession({"url": "https://example.invalid/track.flac"})
        service.session = session

        self.assertFalse(service.has_signing_secret())
        with self.assertRaises(QobuzSigningSecretMissing):
            service._sign_file_url_request(VECTOR_FORMAT_ID, VECTOR_TRACK_ID, VECTOR_TIMESTAMP)

        url = await service._get_download_url(VECTOR_TRACK_ID, 6)
        self.assertIsNone(url)
        self.assertEqual(session.calls, [], "no request may be sent without a secret")

    async def test_signature_matches_the_pinned_rust_algorithm_vector(self):
        """Same concatenation and digest as download/qobuz.rs::build_request_signature.

        Pinned with the same (format_id, track_id, timestamp, secret) inputs; the
        timestamp is passed here as the string the algorithm consumes.
        """
        service = _service(app_secret=VECTOR_SECRET)
        self.assertTrue(service.has_signing_secret())
        self.assertEqual(
            service._sign_file_url_request(VECTOR_FORMAT_ID, VECTOR_TRACK_ID, VECTOR_TIMESTAMP),
            VECTOR_SIGNATURE,
        )

    async def test_placeholder_secret_stays_empty_in_source(self):
        """No signing secret is committed: the class default is an empty placeholder."""
        self.assertEqual(QobuzService.APP_SECRET.strip(), "")


if __name__ == "__main__":
    unittest.main()

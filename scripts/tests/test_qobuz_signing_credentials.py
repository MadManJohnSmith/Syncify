#!/usr/bin/env python3
"""Offline proof of the Qobuz request-signing contract.

Revisado 2026-10-04: el secreto por defecto es el bundle PÚBLICO de la API de
Qobuz (el mismo par app_id/secret de los clientes open-source; no es un secreto
personal). Las credenciales del operador (`ServiceCredentials.app_secret` /
`client_secret` / `extra["app_secret"]`, alimentadas por `QOBUZ_APP_SECRET` en
`download_bridge.get_qobuz_service`) tienen prioridad sobre él. El contrato que
este módulo prueba herméticamente: el secreto elegido llega a la firma, un
secreto VACÍO se rechaza antes de enviar nada (rama de guardia), y el digest es
byte-idéntico al algoritmo del núcleo Rust. No se requiere red.

El literal del bundle no se repite aquí: `qobuz_credentials_leak_test` escanea
scripts/ y este fichero no es un hogar canónico; se lee en runtime.
"""

import hashlib
import os
import sys
import unittest
from pathlib import Path
from unittest.mock import AsyncMock, patch

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

    async def test_default_secret_is_the_public_bundle_and_it_signs(self):
        """Sin credenciales del operador se firma con el bundle público."""
        service = _service()
        session = RecordingSession({"url": "https://example.invalid/track.flac", "format_id": 6})
        service.session = session

        self.assertTrue(service.has_signing_secret())
        with patch("services.qobuz_service.time.time", return_value=VECTOR_TIMESTAMP):
            url = await service._get_download_url(VECTOR_TRACK_ID, 6)

        self.assertEqual(url, "https://example.invalid/track.flac")
        expected = hashlib.md5(
            (
                f"trackgetFileUrlformat_id{VECTOR_FORMAT_ID}intentstream"
                f"track_id{VECTOR_TRACK_ID}{VECTOR_TIMESTAMP}{QobuzService.APP_SECRET}"
            ).encode()
        ).hexdigest()
        self.assertEqual(session.sent_params["request_sig"], expected)

    async def test_empty_secret_refuses_to_sign_and_sends_nothing(self):
        """Un secreto VACÍO es un tope duro, no una firma silenciosamente inválida.

        El estado ya no es alcanzable por defecto (el bundle público existe),
        así que la rama de guardia se ejerce forzando el atributo de clase.
        """
        with patch.object(QobuzService, "APP_SECRET", ""):
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

    async def test_committed_default_is_a_real_public_secret(self):
        """El valor por defecto es el bundle público: 32 hex, no un placeholder vacío.

        El literal exacto lo congela el suite Rust (`secrets_isolation_security_test.rs`);
        aquí se comprueba la forma y que la resolución sin credenciales ni env lo usa.
        """
        committed = QobuzService.APP_SECRET.strip()
        self.assertEqual(len(committed), 32)
        self.assertTrue(all(c in "0123456789abcdef" for c in committed))
        self.assertEqual(_service().APP_SECRET, committed)


class QobuzDownloadFailureDiagnosisTests(unittest.IsolatedAsyncioTestCase):
    """SYNC-AUD-076: the closed failure branch must report its own cause.

    `_get_download_url` already refused to sign without an app secret, but its
    only return was None and the caller turned every None into "check
    subscription tier", so an operator configuring a new deployment was pointed
    at the plan instead of at the missing `QOBUZ_APP_SECRET`. The security
    behaviour is unchanged (nothing is signed or sent); what was missing was the
    diagnosis.
    """

    def setUp(self):
        for name in ("QOBUZ_APP_ID", "QOBUZ_APP_SECRET"):
            os.environ.pop(name, None)

    async def _download(self, service):
        """Run `download_track` with authentication and metadata stubbed out."""
        with patch.object(service, "is_authenticated", AsyncMock(return_value=True)):
            with patch.object(
                service, "get_track_metadata", AsyncMock(return_value={"id": VECTOR_TRACK_ID})
            ):
                return await service.download_track(
                    VECTOR_TRACK_ID, "/tmp/syncify-offline-diagnosis.flac"
                )

    async def test_missing_secret_is_named_and_the_tier_is_not_blamed(self):
        with patch.object(QobuzService, "APP_SECRET", ""):
            service = _service()
            session = RecordingSession({"url": "https://example.invalid/track.flac"})
            service.session = session

            result = await self._download(service)

            self.assertFalse(result.success)
            self.assertIn("QOBUZ_APP_SECRET", result.error_message or "")
            self.assertIn("app secret", (result.error_message or "").lower())
            self.assertNotIn("check subscription tier", result.error_message or "")
            self.assertEqual(session.calls, [], "no request may be sent without a secret")

    async def test_200_without_url_keeps_the_tier_as_the_stated_cause(self):
        service = _service(app_secret="operator-secret")
        service.session = RecordingSession({})

        result = await self._download(service)

        self.assertFalse(result.success)
        self.assertIn("tier", (result.error_message or "").lower())
        self.assertNotIn("QOBUZ_APP_SECRET", result.error_message or "")

    async def test_http_failure_names_the_status_and_the_upstream_message(self):
        service = _service(app_secret="operator-secret")
        service.session = RecordingSession({"message": "forbidden"}, status=403)

        result = await self._download(service)

        self.assertFalse(result.success)
        self.assertIn("HTTP 403", result.error_message or "")
        self.assertIn("forbidden", result.error_message or "")

    async def test_a_later_success_clears_the_previous_diagnosis(self):
        refused = _service()
        refused.session = RecordingSession({})
        await refused._get_download_url(VECTOR_TRACK_ID, 6)
        self.assertIsNotNone(refused._last_download_url_error)

        service = _service(app_secret="operator-secret")
        service.session = RecordingSession({"url": "https://example.invalid/track.flac"})
        url = await service._get_download_url(VECTOR_TRACK_ID, 6)

        self.assertEqual(url, "https://example.invalid/track.flac")
        self.assertIsNone(service._last_download_url_error)


if __name__ == "__main__":
    unittest.main()

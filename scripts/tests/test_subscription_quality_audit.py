#!/usr/bin/env python3
"""Q-17: evidencia offline de suscripción y calidad en los puentes Python.

Estas pruebas caracterizan la señal de entitlement y la lista de calidades que
sí produce cada servicio; no convierten calidad de catálogo en derecho de
cuenta. Los caminos de descarga Python se retiraron en D14 (#126): la descarga
es nativa (ADR-0009).
"""

import sys
import unittest
from pathlib import Path
from unittest.mock import MagicMock

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

try:
    import aiohttp  # noqa: F401
except ModuleNotFoundError:
    sys.modules["aiohttp"] = MagicMock()

from services.deezer_service import DeezerService  # noqa: E402
from services.qobuz_service import QobuzService  # noqa: E402
from services.service_base import DownloadQuality, ServiceCredentials  # noqa: E402
from services.soundcloud_service import SoundCloudService  # noqa: E402
from services.tidal_service import TidalService  # noqa: E402


class _AsyncContext:
    def __init__(self, value):
        self.value = value

    async def __aenter__(self):
        return self.value

    async def __aexit__(self, *_exc):
        return False


class _JsonResponse:
    def __init__(self, payload, *, status=200, ok=True):
        self.payload = payload
        self.status = status
        self.ok = ok

    async def json(self):
        return self.payload

    def raise_for_status(self):
        return None


class QobuzSubscriptionAuditTests(unittest.IsolatedAsyncioTestCase):
    async def test_direct_login_captures_the_provider_credential(self):
        credential = {
            "label": "Studio",
            "lossless_streaming": True,
            "hires_streaming": False,
        }
        service = QobuzService(
            ServiceCredentials(username="owner@example.test", password="secret"),
            enable_metadata_enrichment=False,
        )
        service.session = MagicMock()
        service.session.post.return_value = _AsyncContext(_JsonResponse({
            "user_auth_token": "offline-token",
            "user": {"id": 42, "display_name": "Owner", "credential": credential},
        }))

        self.assertTrue(await service.authenticate())
        self.assertEqual(service.subscription_credential, credential)

    async def test_unknown_credential_does_not_advertise_lossless(self):
        service = QobuzService(
            ServiceCredentials(username="owner@example.test", password="secret"),
            enable_metadata_enrichment=False,
        )
        service._authenticated = True
        service.user_auth_token = "offline-token"
        service.subscription_credential = {}

        qualities = await service.get_available_qualities("track-1")

        self.assertEqual(qualities, [DownloadQuality.LOSSY_STANDARD])

    async def test_legacy_hires_purchases_flag_unlocks_both_hires_tiers(self):
        service = QobuzService(
            ServiceCredentials(username="owner@example.test", password="secret"),
            enable_metadata_enrichment=False,
        )
        service._authenticated = True
        service.user_auth_token = "offline-token"
        service.subscription_credential = {
            "lossless_streaming": False,
            "hires_streaming": False,
            "hires_purchases_streaming": True,
        }

        qualities = await service.get_available_qualities("track-1")

        self.assertEqual(qualities, [
            DownloadQuality.LOSSY_STANDARD,
            DownloadQuality.LOSSLESS_HIRES_96,
            DownloadQuality.LOSSLESS_HIRES,
        ])


class TidalSubscriptionAuditTests(unittest.IsolatedAsyncioTestCase):
    async def test_session_identity_does_not_gate_advertised_qualities(self):
        service = TidalService(ServiceCredentials(token="offline-token"))
        service.access_token = "offline-token"
        service.token_type = "Bearer"
        service.session = MagicMock()
        service.session.get.return_value = _AsyncContext(_JsonResponse({
            "sessionId": "session-1",
            "userId": 42,
            "countryCode": "US",
        }))

        self.assertTrue(await service._validate_session())
        self.assertEqual((service.user_id, service.country_code), (42, "US"))
        self.assertEqual(await service.get_available_qualities("track-1"), [
            DownloadQuality.LOSSY_LOW,
            DownloadQuality.LOSSY_STANDARD,
            DownloadQuality.LOSSLESS_CD,
            DownloadQuality.LOSSLESS_HIRES,
        ])


class DeezerSubscriptionAuditTests(unittest.IsolatedAsyncioTestCase):
    async def test_user_options_are_the_only_plan_signal(self):
        cases = (
            ({}, ["MP3_128"]),
            ({"web_hq": True}, ["MP3_128", "MP3_320"]),
            ({"web_hq": True, "web_lossless": True}, ["MP3_128", "MP3_320", "FLAC"]),
        )
        for options, expected in cases:
            with self.subTest(options=options):
                service = DeezerService(ServiceCredentials(token="offline-arl"))
                service.session = MagicMock()
                service.session.post.return_value = _AsyncContext(_JsonResponse({
                    "results": {
                        "checkForm": "api-token",
                        "COUNTRY": "US",
                        "USER": {
                            "USER_ID": "42",
                            "OPTIONS": {"license_token": "license-token", **options},
                        },
                    },
                }))

                await service._api_call("deezer.getUserData")

                self.assertEqual(service.available_formats, expected)

    async def test_legacy_quality_list_ignores_account_formats(self):
        service = DeezerService(ServiceCredentials(token="offline-arl"))
        service.available_formats = ["MP3_128"]

        qualities = await service.get_available_qualities("track-1")

        self.assertEqual(qualities, [
            DownloadQuality.LOSSY_LOW,
            DownloadQuality.LOSSY_STANDARD,
            DownloadQuality.LOSSLESS_CD,
        ])


class SoundCloudSubscriptionAuditTests(unittest.IsolatedAsyncioTestCase):
    async def test_legacy_quality_list_advertises_standard_without_track_evidence(self):
        service = SoundCloudService()

        qualities = await service.get_available_qualities("track-1")

        self.assertEqual(qualities, [
            DownloadQuality.LOSSY_LOW,
            DownloadQuality.LOSSY_STANDARD,
        ])


if __name__ == "__main__":
    unittest.main()

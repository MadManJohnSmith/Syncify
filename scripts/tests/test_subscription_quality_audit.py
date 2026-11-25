#!/usr/bin/env python3
"""Q-17: evidencia offline de suscripción y calidad en los puentes Python.

El camino de descarga Python es legado (ADR-0009). Estas pruebas caracterizan la
señal de entitlement que sí produce cada servicio y los huecos que ya tienen
ticket; no cambian la política de descarga ni convierten calidad de catálogo en
derecho de cuenta.
"""

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

try:
    import aiohttp  # noqa: F401
except ModuleNotFoundError:
    sys.modules["aiohttp"] = MagicMock()

from services.deezer_service import DeezerService  # noqa: E402
from services.qobuz_service import QobuzService  # noqa: E402
from services.service_base import (  # noqa: E402
    DownloadQuality,
    DownloadResult,
    ServiceCredentials,
    TrackMetadata,
)
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


class _ChunkedContent:
    async def iter_chunked(self, _size):
        yield b"ID3 offline fixture"


class _DownloadResponse(_JsonResponse):
    def __init__(self, payload):
        super().__init__(payload)
        self.headers = {"Content-Length": "19"}
        self.content = _ChunkedContent()


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

    async def test_legacy_file_url_accepts_a_lower_format_than_requested(self):
        service = QobuzService(
            ServiceCredentials(
                username="owner@example.test",
                password="secret",
                app_secret="offline-signing-secret",
            ),
            enable_metadata_enrichment=False,
        )
        service._authenticated = True
        service.user_auth_token = "offline-token"
        service.session = MagicMock()
        service.session.get.return_value = _AsyncContext(_JsonResponse({
            "url": "https://cdn.example.test/track.flac",
            "format_id": 6,
            "bit_depth": 16,
            "sampling_rate": 44100,
        }))

        url = await service._get_download_url("track-1", 27)

        self.assertEqual(url, "https://cdn.example.test/track.flac")
        self.assertEqual(service.session.get.call_args.kwargs["params"]["format_id"], "27")


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

    async def test_legacy_stream_returns_aac_even_when_lossless_was_requested(self):
        service = TidalService(ServiceCredentials(token="offline-token"))
        service.access_token = "offline-token"
        service.session_id = "session-1"
        service.country_code = "US"
        service.token_type = "Bearer"
        service.session = MagicMock()
        service.session.get.return_value = _AsyncContext(_JsonResponse({
            "url": "https://cdn.example.test/track.m4a",
            "audioQuality": "AAC",
        }))

        url = await service._get_stream_url("track-1", DownloadQuality.LOSSLESS_CD)

        self.assertEqual(url, "https://cdn.example.test/track.m4a")


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

    async def test_download_falls_back_to_the_best_account_format(self):
        service = DeezerService(ServiceCredentials(token="offline-arl"))
        service.arl = "offline-arl"
        service.api_token = "api-token"
        service.user_id = "42"
        service.available_formats = ["MP3_128"]
        service.get_track_metadata = AsyncMock(return_value=TrackMetadata(
            service_id="track-1",
            service_type=service.service_type,
            title="Track",
            artists=["Artist"],
            album="Album",
            custom_tags={"track_token": "track-token"},
        ))
        service._get_download_url = AsyncMock(return_value=None)

        result = await service.download_track(
            "track-1", Path("offline.flac"), DownloadQuality.LOSSLESS_CD
        )

        self.assertFalse(result.success)
        service._get_download_url.assert_awaited_once_with(
            "track-1", "track-token", "MP3_128"
        )


class SoundCloudSubscriptionAuditTests(unittest.IsolatedAsyncioTestCase):
    def _service_with_track(self, track):
        service = SoundCloudService()
        service.session = MagicMock()
        service.session.get.return_value = _AsyncContext(_JsonResponse(track))
        service._download_hls_stream = AsyncMock(return_value=DownloadResult(
            success=True, filepath="offline.mp3"
        ))
        service._download_original_file = AsyncMock(return_value=DownloadResult(
            success=True, filepath="offline-original.flac"
        ))
        return service

    async def test_track_rights_not_a_subscription_gate_streaming(self):
        for track in (
            {"streamable": False, "policy": "ALLOW"},
            {"streamable": True, "policy": "BLOCK"},
        ):
            with self.subTest(track=track):
                service = self._service_with_track(track)

                with self.assertRaisesRegex(Exception, "not streamable"):
                    await service.download_track("track-1", "offline.mp3")

                service._download_hls_stream.assert_not_awaited()
                service._download_original_file.assert_not_awaited()

    async def test_streamable_track_without_original_uses_hls(self):
        track = {
            "streamable": True,
            "policy": "ALLOW",
            "downloadable": False,
            "has_downloads_left": False,
        }
        service = self._service_with_track(track)

        result = await service.download_track(
            "track-1", "offline.mp3", DownloadQuality.LOSSY_LOW
        )

        self.assertTrue(result.success)
        service._download_hls_stream.assert_awaited_once_with(
            track, "offline.mp3", None
        )
        service._download_original_file.assert_not_awaited()

    async def test_original_requires_uploader_and_download_rights(self):
        track = {
            "streamable": True,
            "policy": "ALLOW",
            "downloadable": True,
            "has_downloads_left": True,
        }
        service = self._service_with_track(track)

        result = await service.download_track(
            "track-1", "offline.flac", DownloadQuality.LOSSLESS_CD
        )

        self.assertTrue(result.success)
        service._download_original_file.assert_awaited_once_with(
            "track-1", "offline.flac", None
        )
        service._download_hls_stream.assert_not_awaited()

    async def test_legacy_original_mp3_is_still_labelled_lossless(self):
        service = SoundCloudService()
        service.session = MagicMock()
        service.session.get.side_effect = [
            _AsyncContext(_JsonResponse({
                "redirectUri": "https://cdn.example.test/original.mp3",
            })),
            _AsyncContext(_DownloadResponse({})),
        ]
        with tempfile.TemporaryDirectory() as tmpdir:
            output = Path(tmpdir) / "original.mp3"

            result = await service._download_original_file("track-1", str(output), None)

            self.assertEqual(output.read_bytes(), b"ID3 offline fixture")
        self.assertEqual(result.format, "MP3")
        self.assertEqual(result.quality, DownloadQuality.LOSSLESS_CD)

    async def test_legacy_quality_list_advertises_standard_without_track_evidence(self):
        service = SoundCloudService()

        qualities = await service.get_available_qualities("track-1")

        self.assertEqual(qualities, [
            DownloadQuality.LOSSY_LOW,
            DownloadQuality.LOSSY_STANDARD,
        ])


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Offline contract tests for Qobuz album normalization and entitlements."""

import sys
import unittest
from pathlib import Path
from unittest.mock import AsyncMock

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
for site_packages in REPO_ROOT.glob(".venv/lib/python*/site-packages"):
    if site_packages.is_dir():
        sys.path.insert(0, str(site_packages))
sys.path.insert(0, str(REPO_ROOT / "scripts"))

# The contract exercised here is pure normalization. Keep it runnable in
# dependency-minimal checkouts without performing package installation or I/O.
try:
    import aiohttp  # noqa: F401
except ModuleNotFoundError:
    from unittest.mock import MagicMock
    sys.modules["aiohttp"] = MagicMock()

from services.qobuz_service import QobuzService
from services.service_base import DownloadQuality, ServiceCredentials, ServiceType, TrackMetadata


class QobuzAlbumContractTests(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.service = QobuzService(ServiceCredentials(username="user", password="secret"))
        self.service._authenticated = True
        self.service.user_auth_token = "offline-test-token"

    async def test_album_metadata_normalizes_qobuz_payload(self):
        self.service._get_album_payload = AsyncMock(return_value={
            "id": "album-1", "title": "Album", "artist": {"name": "Artist"},
            "release_date_original": "2024-03-02", "label": {"name": "Label"},
            "genre": {"name": "Jazz"}, "tracks_count": 2,
            "image": {"large": "https://example.invalid/cover.jpg"}, "upc": "123",
        })
        album = await self.service.get_album_metadata("album-1")
        self.assertEqual(album.service_type, ServiceType.QOBUZ)
        self.assertEqual((album.title, album.artist, album.year), ("Album", "Artist", 2024))
        self.assertEqual(album.genres, ["Jazz"])

    async def test_album_tracks_use_track_normalizer(self):
        self.service._get_album_payload = AsyncMock(return_value={
            "tracks": {"items": [{"id": 10}, {"id": 11}, {}]}
        })
        track = TrackMetadata("10", ServiceType.QOBUZ, "Track", ["Artist"], "Album")
        self.service.get_track_metadata = AsyncMock(side_effect=[track, None])
        tracks = await self.service.get_album_tracks("album-1")
        self.assertEqual(tracks, [track])
        self.assertEqual(self.service.get_track_metadata.await_count, 2)

    async def test_available_qualities_follow_subscription_entitlements(self):
        self.service.subscription_credential = {"lossless_streaming": True, "hires_streaming": False}
        qualities = await self.service.get_available_qualities("track-1")
        self.assertEqual(qualities, [DownloadQuality.LOSSY_STANDARD, DownloadQuality.LOSSLESS_CD])

        self.service.subscription_credential["hires_streaming"] = True
        qualities = await self.service.get_available_qualities("track-1")
        self.assertIn(DownloadQuality.LOSSLESS_HIRES_96, qualities)
        self.assertIn(DownloadQuality.LOSSLESS_HIRES, qualities)


if __name__ == "__main__":
    unittest.main()

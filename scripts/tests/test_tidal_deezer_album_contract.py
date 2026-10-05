#!/usr/bin/env python3
"""Offline contracts for Tidal/Deezer albums and Tidal playlist bridge."""
import os
import sys
import unittest
from pathlib import Path
from types import ModuleType
from unittest.mock import AsyncMock, MagicMock, patch

try:
    import aiohttp  # noqa: F401
except ModuleNotFoundError:
    sys.modules["aiohttp"] = MagicMock()

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS_DIR))

from services.service_base import ServiceCredentials, ServiceType, TrackMetadata
from services.tidal_service import TidalService
from services.deezer_service import DeezerService
import playlist_bridge


class AlbumContractTests(unittest.IsolatedAsyncioTestCase):
    async def test_tidal_album_contract_normalizes_metadata_and_tracks(self):
        service = TidalService(ServiceCredentials(service_type=ServiceType.TIDAL, token="token"))
        service.access_token = "token"
        service._get_api_json = AsyncMock(side_effect=[
            {"id": 7, "title": "Tidal Album", "artist": {"name": "Artist"},
             "artists": [{"name": "Artist"}], "releaseDate": "2023-05-04",
             "numberOfTracks": 2, "cover": "abc-def", "upc": "123"},
            {"items": [{"id": 1}, {"id": 2}]},
        ])
        service.get_track_metadata = AsyncMock(side_effect=[
            TrackMetadata("1", ServiceType.TIDAL, "One", ["Artist"], "Tidal Album"),
            TrackMetadata("2", ServiceType.TIDAL, "Two", ["Artist"], "Tidal Album"),
        ])
        album = await service.get_album_metadata("7")
        tracks = await service.get_album_tracks("7")
        self.assertEqual((album.title, album.artist, album.year, album.track_count),
                         ("Tidal Album", "Artist", 2023, 2))
        self.assertEqual([track.service_id for track in tracks], ["1", "2"])

    async def test_deezer_album_contract_normalizes_metadata_and_tracks(self):
        service = DeezerService(ServiceCredentials(service_type=ServiceType.DEEZER, token="arl"))
        service.arl = "arl"
        service.api_token = "token"
        service.user_id = "42"
        service._api_call = AsyncMock(side_effect=[
            {"DATA": {"ALB_ID": "8", "ALB_TITLE": "Deezer Album", "ART_NAME": "Artist",
                      "PHYSICAL_RELEASE_DATE": "2022-01-02", "NB_SONG": "2",
                      "ALB_PICTURE": "cover", "UPC": "456"}},
            {"SONGS": {"data": [{"SNG_ID": "3"}, {"SNG_ID": "4"}]}},
        ])
        service.get_track_metadata = AsyncMock(side_effect=[
            TrackMetadata("3", ServiceType.DEEZER, "Three", ["Artist"], "Deezer Album"),
            TrackMetadata("4", ServiceType.DEEZER, "Four", ["Artist"], "Deezer Album"),
        ])
        album = await service.get_album_metadata("8")
        tracks = await service.get_album_tracks("8")
        self.assertEqual((album.title, album.artist, album.year, album.track_count),
                         ("Deezer Album", "Artist", 2022, 2))
        self.assertEqual([track.service_id for track in tracks], ["3", "4"])


class TidalPlaylistBridgeContractTests(unittest.TestCase):
    def _make_fake_tidal_service(self):
        fake_service = MagicMock()
        fake_service.get_user_playlists = AsyncMock(return_value=[{"id": "p1"}])
        fake_service._validate_session = AsyncMock(return_value=True)
        fake_service.close = AsyncMock()
        return fake_service

    def test_bridge_constructs_tidal_with_credentials(self):
        fake_service = self._make_fake_tidal_service()
        service_cls = MagicMock(return_value=fake_service)
        module = ModuleType("services.tidal_service")
        module.TidalService = service_cls
        with patch.dict(sys.modules, {"services.tidal_service": module}), patch.dict(
            os.environ,
            {"TIDAL_ACCESS_TOKEN": "token", "TIDAL_USER_ID": "42", "TIDAL_COUNTRY_CODE": "US"},
            clear=False,
        ):
            self.assertEqual(playlist_bridge.get_tidal_playlists(), [{"id": "p1"}])
        credentials = service_cls.call_args.args[0]
        self.assertEqual(credentials.service_type, ServiceType.TIDAL)
        self.assertEqual(credentials.token, "token")
        # The token must be validated against /sessions: without it the service
        # reports is_authenticated() == False and every call silently returns [].
        fake_service._validate_session.assert_awaited_once_with()
        self.assertEqual(fake_service.user_id, 42)
        self.assertEqual(fake_service.country_code, "US")
        fake_service.get_user_playlists.assert_awaited_once_with()
        fake_service.close.assert_awaited_once_with()

    def test_bridge_raises_when_tidal_token_is_rejected(self):
        """A rejected TIDAL_ACCESS_TOKEN must fail loudly, never return [] as success."""
        fake_service = self._make_fake_tidal_service()
        fake_service._validate_session = AsyncMock(return_value=False)
        service_cls = MagicMock(return_value=fake_service)
        module = ModuleType("services.tidal_service")
        module.TidalService = service_cls
        with patch.dict(sys.modules, {"services.tidal_service": module}), patch.dict(
            os.environ, {"TIDAL_ACCESS_TOKEN": "expired", "TIDAL_USER_ID": "42"}, clear=False
        ):
            with self.assertRaises(Exception) as ctx:
                playlist_bridge.get_tidal_playlists()

        self.assertIn("Tidal authentication failed", str(ctx.exception))
        fake_service.get_user_playlists.assert_not_awaited()
        fake_service.close.assert_awaited_once_with()

    def test_bridge_requires_tidal_access_token(self):
        """Without TIDAL_ACCESS_TOKEN the bridge must fail clearly before touching the service."""
        service_cls = MagicMock()
        module = ModuleType("services.tidal_service")
        module.TidalService = service_cls
        with patch.dict(sys.modules, {"services.tidal_service": module}), patch.dict(
            os.environ, {}, clear=True
        ):
            with self.assertRaises(Exception) as ctx:
                playlist_bridge.get_tidal_playlists()

        self.assertIn("TIDAL_ACCESS_TOKEN", str(ctx.exception))
        service_cls.assert_not_called()


if __name__ == "__main__":
    unittest.main()

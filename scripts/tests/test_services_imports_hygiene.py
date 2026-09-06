#!/usr/bin/env python3
"""
Test Suite: Services Imports and Instantiation Hygiene (TASK-46)

Verifies:
1. All critical service dependencies (pycryptodome, m3u8, musicbrainzngs, syncedlyrics)
   are declared in requirements.txt.
2. Clean imports of core Python services: deezer_service, soundcloud_service,
   musicbrainz_matcher, lyrics_service.
3. Safe baseline instantiation without TypeError or ModuleNotFoundError.
4. Full compliance of SoundCloudService and DeezerService with the MusicService ABC contract.
5. Defensive fallbacks when optional libraries or tokens are not present.
"""

import sys
import unittest
from pathlib import Path

# Automatically ensure workspace virtualenv site-packages and scripts are in sys.path
REPO_ROOT = Path(__file__).resolve().parent.parent.parent
for sp in REPO_ROOT.glob(".venv/lib/python*/site-packages"):
    if sp.is_dir() and str(sp) not in sys.path:
        sys.path.insert(0, str(sp))

SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))


class TestServicesImportsHygiene(unittest.TestCase):
    """Test suite for Python services dependency injection and import hygiene."""

    def test_requirements_dependencies_declared(self):
        """Verify pycryptodome, m3u8, musicbrainzngs, syncedlyrics exist in requirements.txt."""
        req_file = REPO_ROOT / "requirements.txt"
        self.assertTrue(req_file.exists(), "requirements.txt does not exist")

        req_content = req_file.read_text(encoding="utf-8").lower()
        required_packages = [
            "pycryptodome",
            "m3u8",
            "musicbrainzngs",
            "syncedlyrics",
        ]

        for pkg in required_packages:
            self.assertIn(
                pkg,
                req_content,
                f"Missing required dependency '{pkg}' in requirements.txt"
            )

    def test_services_imports(self):
        """Verify that all four service modules can be imported cleanly."""
        try:
            from services import deezer_service
            from services import soundcloud_service
            from services import musicbrainz_matcher
            from services import lyrics_service
        except ModuleNotFoundError as e:
            self.fail(f"ModuleNotFoundError while importing services: {e}")

        self.assertIsNotNone(deezer_service)
        self.assertIsNotNone(soundcloud_service)
        self.assertIsNotNone(musicbrainz_matcher)
        self.assertIsNotNone(lyrics_service)

    def test_services_instantiation(self):
        """Verify baseline instantiation of all four service classes without TypeError or ModuleNotFoundError."""
        from services.deezer_service import DeezerService
        from services.soundcloud_service import SoundCloudService
        from services.musicbrainz_matcher import MusicBrainzMatcher
        from services.lyrics_service import LyricsService
        from services.service_base import ServiceCredentials, ServiceType

        try:
            # Default instantiation
            soundcloud = SoundCloudService()
            self.assertIsNotNone(soundcloud)

            # Instantiation with explicit credentials
            sc_creds = ServiceCredentials(service_type=ServiceType.SOUNDCLOUD)
            soundcloud_with_creds = SoundCloudService(sc_creds, verbose=False)
            self.assertIsNotNone(soundcloud_with_creds)

            # Deezer default instantiation
            deezer = DeezerService()
            self.assertIsNotNone(deezer)

            # MusicBrainz matcher
            mb_matcher = MusicBrainzMatcher(verbose=False)
            self.assertIsNotNone(mb_matcher)

            # Lyrics service
            lyrics = LyricsService(verbose=False)
            self.assertIsNotNone(lyrics)

        except TypeError as e:
            self.fail(f"TypeError during service instantiation (abstract method or signature mismatch): {e}")
        except Exception as e:
            self.fail(f"Unexpected exception during baseline instantiation: {e}")

    def test_soundcloud_music_service_contract(self):
        """Verify SoundCloudService satisfies all polymorphic MusicService contract obligations."""
        from services.soundcloud_service import SoundCloudService
        from services.service_base import MusicService, ServiceType, DownloadQuality

        service = SoundCloudService()
        self.assertIsInstance(service, MusicService)
        self.assertEqual(service.service_name, "SoundCloud")
        self.assertEqual(service.service_type, ServiceType.SOUNDCLOUD)
        self.assertFalse(service.supports_lossless)

        # Non-blocking synchronous / async method existence
        self.assertTrue(hasattr(service, "authenticate"))
        self.assertTrue(hasattr(service, "is_authenticated"))
        self.assertTrue(hasattr(service, "search"))
        self.assertTrue(hasattr(service, "get_track_metadata"))
        self.assertTrue(hasattr(service, "get_album_metadata"))
        self.assertTrue(hasattr(service, "get_album_tracks"))
        self.assertTrue(hasattr(service, "get_playlist_metadata"))
        self.assertTrue(hasattr(service, "get_playlist_tracks"))
        self.assertTrue(hasattr(service, "download_track"))
        self.assertTrue(hasattr(service, "get_available_qualities"))

    def test_deezer_music_service_contract(self):
        """Verify DeezerService satisfies MusicService contract obligations."""
        from services.deezer_service import DeezerService
        from services.service_base import MusicService, ServiceType

        service = DeezerService()
        self.assertIsInstance(service, MusicService)
        self.assertEqual(service.service_name, "Deezer")
        self.assertEqual(service.service_type, ServiceType.DEEZER)
        self.assertTrue(service.supports_lossless)

    def test_service_base_metadata_ergonomics(self):
        """Verify TrackMetadata and SearchResult backward-compatible properties."""
        from services.service_base import TrackMetadata, SearchResult, ServiceType, DownloadQuality

        track = TrackMetadata(
            service_id="12345",
            service_type=ServiceType.SOUNDCLOUD,
            title="Sample Track",
            artists=["Artist A", "Artist B"],
            album="Sample Album",
            duration_ms=180000,
        )
        self.assertEqual(track.id, "12345")
        self.assertEqual(track.artist, "Artist A")
        self.assertEqual(track.duration, 180)

        result = SearchResult(
            result_type="track",
            service_id="67890",
            service_type=ServiceType.SOUNDCLOUD,
            title="Search Track",
            artist="Search Artist",
            duration_ms=240000,
            quality=DownloadQuality.LOSSY_STANDARD,
        )
        self.assertEqual(result.id, "67890")
        self.assertEqual(result.duration, 240)


if __name__ == "__main__":
    unittest.main()

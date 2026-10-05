#!/usr/bin/env python3
"""
Test Suite: Metadata Bridge & AcoustID Matcher Robustness (TASK-47)

Verifies:
1. Safe attribute extraction in metadata_bridge without AttributeError on missing attributes.
2. Proper fallbacks for artist_mbids -> musicbrainz_artist_id, genre_tags -> genres, lastfm_tags -> tags.
3. Clean JSON serialization without AttributeError or serialization crashes.
4. AcoustIDMatcher operates without core_logic dependency and gracefully handles missing API keys.
5. End-to-end enrich_track pipeline behavior with mocked enrichment service.
6. The album context reaches the enrichment service (PY-5) and selects the
   matching MusicBrainz release instead of an arbitrary edition.
"""

import asyncio
import io
import json
import os
import sys
import unittest
from contextlib import contextmanager
from pathlib import Path
from unittest.mock import MagicMock, patch, AsyncMock

# Add repo root and scripts to sys.path, and discover venv site-packages
REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

for sp in REPO_ROOT.glob(".venv/lib/python*/site-packages"):
    if sp.is_dir() and str(sp) not in sys.path:
        sys.path.insert(0, str(sp))

# Same convention as tests/test_tidal_deezer_album_contract.py: the enrichment
# service only needs aiohttp for its network calls, so an offline run of this
# module must not depend on aiohttp being installed.
try:
    import aiohttp  # noqa: F401
except ModuleNotFoundError:
    sys.modules["aiohttp"] = MagicMock()

from metadata_bridge import extract_enriched_metadata, enrich_track, json_response
from services.metadata_enrichment import (
    EnrichedMetadata,
    enrich_metadata,
    normalize_album_title,
    select_release_for_album,
)
from services.acoustid_matcher import (
    AcoustIDMatcher,
    AcoustIDNotConfiguredError,
    AcoustIDResult,
)


class TestMetadataBridgeRobustness(unittest.TestCase):
    """Validation for metadata_bridge attribute extraction and serialization."""

    def test_extract_enriched_metadata_real_dataclass(self):
        """EnrichedMetadata does not define musicbrainz_artist_id or genres directly."""
        meta = EnrichedMetadata(
            language="eng",
            country="US",
            recording_location="Studio A",
            musicbrainz_recording_id="rec-uuid-1",
            musicbrainz_release_id="rel-uuid-2",
            mood_tags=["upbeat"],
            occasion_tags=["party"],
            style_tags=["synthpop"],
            lastfm_tags=["electronic", "pop"],
            bpm=128.0,
            key="C#",
            energy=0.85,
            danceability=0.72,
            valence=0.65,
            acousticness=0.10,
            instrumentalness=0.05,
            speechiness=0.04,
            liveness=0.12,
            loudness=-5.2,
            spotify_popularity=85,
        )

        extracted = extract_enriched_metadata(meta)

        self.assertIsInstance(extracted, dict)
        self.assertEqual(extracted.get("language"), "eng")
        self.assertEqual(extracted.get("country"), "US")
        self.assertEqual(extracted.get("musicbrainz_recording_id"), "rec-uuid-1")
        self.assertEqual(extracted.get("musicbrainz_release_id"), "rel-uuid-2")
        self.assertEqual(extracted.get("bpm"), 128.0)
        # Verify fallback for genres from style_tags when genres is None
        self.assertEqual(extracted.get("genres"), ["synthpop"])
        # Verify fallback for tags from lastfm_tags when tags is None
        self.assertEqual(extracted.get("tags"), ["electronic", "pop"])
        # musicbrainz_artist_id should be omitted or None (filtered out) without AttributeError
        self.assertNotIn("musicbrainz_artist_id", extracted)

    def test_extract_enriched_metadata_with_fallbacks(self):
        """Objects with artist_mbids and genre_tags fall back cleanly to canonical keys."""
        class MockEnriched:
            artist_mbids = ["artist-mbid-999"]
            genre_tags = ["alternative", "indie"]
            lastfm_tags = ["rock", "90s"]
            recording_id = "rec-fallback-456"

        mock_obj = MockEnriched()
        extracted = extract_enriched_metadata(mock_obj)

        self.assertEqual(extracted.get("musicbrainz_artist_id"), "artist-mbid-999")
        self.assertEqual(extracted.get("artist_mbids"), ["artist-mbid-999"])
        self.assertEqual(extracted.get("genres"), ["alternative", "indie"])
        self.assertEqual(extracted.get("genre_tags"), ["alternative", "indie"])
        self.assertEqual(extracted.get("tags"), ["rock", "90s"])
        self.assertEqual(extracted.get("musicbrainz_recording_id"), "rec-fallback-456")

    def test_extract_enriched_metadata_artist_mbids_string(self):
        """artist_mbids provided as a single string instead of list."""
        class MockEnriched:
            artist_mbids = "single-artist-mbid"

        extracted = extract_enriched_metadata(MockEnriched())
        self.assertEqual(extracted.get("musicbrainz_artist_id"), "single-artist-mbid")

    def test_extract_enriched_metadata_from_dict(self):
        """Input provided as dictionary instead of object."""
        payload = {
            "language": "fra",
            "country": "FR",
            "artist_mbids": ["artist-fr-1"],
            "genre_tags": ["chanson"],
            "bpm": 95.0,
        }
        extracted = extract_enriched_metadata(payload)
        self.assertEqual(extracted.get("language"), "fra")
        self.assertEqual(extracted.get("musicbrainz_artist_id"), "artist-fr-1")
        self.assertEqual(extracted.get("genres"), ["chanson"])

    def test_extract_enriched_metadata_none_and_empty(self):
        """Gracefully handle None or empty objects."""
        self.assertEqual(extract_enriched_metadata(None), {})

        class Empty:
            pass

        self.assertEqual(extract_enriched_metadata(Empty()), {})

    def test_json_serialization_safety(self):
        """Verify serialization to JSON never fails with AttributeError or TypeError."""
        class ComplexPayload:
            artist_mbids = ["mbid-1"]
            path = Path("/tmp/song.flac")

        data = extract_enriched_metadata(ComplexPayload())
        data["path_object"] = ComplexPayload.path

        # Simulate json_response serialization logic
        serialized = json.dumps({"success": True, "data": data}, ensure_ascii=False, default=str)
        deserialized = json.loads(serialized)

        self.assertTrue(deserialized["success"])
        self.assertEqual(deserialized["data"]["musicbrainz_artist_id"], "mbid-1")
        self.assertEqual(deserialized["data"]["path_object"], "/tmp/song.flac")

    def test_enrich_track_pipeline_mocked(self):
        """Verify end-to-end enrich_track call completes and formats response without crash."""
        meta = EnrichedMetadata(language="jpn", country="JP", bpm=135.0)

        with patch("services.metadata_enrichment.enrich_metadata", new=AsyncMock(return_value=meta)):
            with patch("sys.stdout", new_callable=io.StringIO) as mock_stdout:
                with self.assertRaises(SystemExit) as cm:
                    enrich_track("Test Song", "Test Artist", isrc="JP1234567890")

                self.assertEqual(cm.exception.code, 0)
                output = mock_stdout.getvalue()
                parsed = json.loads(output)
                self.assertTrue(parsed["success"])
                self.assertEqual(parsed["data"]["language"], "jpn")
                self.assertEqual(parsed["data"]["country"], "JP")
                self.assertEqual(parsed["data"]["bpm"], 135.0)


class TestAlbumContextEnrichment(unittest.TestCase):
    """PY-5: the album argument is forwarded and used to pick the MusicBrainz release."""

    def test_enrich_track_forwards_album_to_enrichment_service(self):
        """The --album value must reach enrich_metadata (it used to be dropped)."""
        meta = EnrichedMetadata(language="eng", country="GB")

        with patch("services.metadata_enrichment.enrich_metadata", new=AsyncMock(return_value=meta)) as mock_enrich:
            with patch("sys.stdout", new_callable=io.StringIO):
                with self.assertRaises(SystemExit) as cm:
                    enrich_track("Song", "Artist", isrc="GBAYE0000123", album="Parachutes")

        self.assertEqual(cm.exception.code, 0)
        self.assertEqual(mock_enrich.await_args.kwargs["album"], "Parachutes")
        self.assertEqual(mock_enrich.await_args.kwargs["isrc"], "GBAYE0000123")

    def test_enrich_metadata_selects_release_matching_the_album(self):
        """Without album context an arbitrary edition was taken; now the album decides."""
        releases = [
            {"id": "release-other", "title": "Live at Wembley", "country": "GB", "language": "eng"},
            {"id": "release-album", "title": "Parachutes", "country": "GB", "language": "eng"},
        ]

        mb_data = {"recording_id": "rec-1", "releases": releases}

        class FakeEnricher:
            def __init__(self, *args, **kwargs):
                pass

            async def __aenter__(self):
                return self

            async def __aexit__(self, *args):
                return False

            async def query_by_isrc(self, isrc):
                return mb_data

        with patch("services.metadata_enrichment.MusicBrainzEnricher", FakeEnricher):
            enriched = asyncio.run(enrich_metadata("GBAYE0000123", "Coldplay", "Yellow", album="Parachutes"))

        self.assertEqual(enriched.musicbrainz_release_id, "release-album")

    def test_enrich_metadata_without_album_keeps_first_release(self):
        """No album context: behavior is unchanged (first release)."""
        releases = [
            {"id": "release-first", "title": "Any Album", "country": "FR", "language": "fra"},
            {"id": "release-second", "title": "Other", "country": "GB", "language": "eng"},
        ]

        class FakeEnricher:
            def __init__(self, *args, **kwargs):
                pass

            async def __aenter__(self):
                return self

            async def __aexit__(self, *args):
                return False

            async def query_by_isrc(self, isrc):
                return {"recording_id": "rec-1", "releases": releases}

        with patch("services.metadata_enrichment.MusicBrainzEnricher", FakeEnricher):
            enriched = asyncio.run(enrich_metadata("FRABC0000123", "Artist", "Titre"))

        self.assertEqual(enriched.musicbrainz_release_id, "release-first")
        self.assertEqual(enriched.country, "FR")

    def test_select_release_for_album_variants(self):
        """Case, accents, whitespace and edition suffixes resolve to the same release."""
        releases = [
            {"id": "rel-1", "title": "Abbey Road (Remastered)"},
            {"id": "rel-2", "title": "Rumours"},
        ]

        self.assertEqual(select_release_for_album(releases, "rumours")["id"], "rel-2")
        self.assertEqual(select_release_for_album(releases, "  RUMOURS ")["id"], "rel-2")
        self.assertEqual(select_release_for_album(releases, "Abbey Road (Remastered)")["id"], "rel-1")
        self.assertEqual(select_release_for_album(releases, "Abbey Road")["id"], "rel-1")
        # Unknown album falls back to the first release instead of dropping the data.
        self.assertEqual(select_release_for_album(releases, "Nonexistent")["id"], "rel-1")
        self.assertIsNone(select_release_for_album([], "Rumours"))

    def test_normalize_album_title(self):
        self.assertEqual(normalize_album_title("  Álbum  remix "), "album remix")
        self.assertEqual(normalize_album_title(None), "")
        self.assertEqual(normalize_album_title(""), "")


@contextmanager
def _acoustid_installed():
    """Fuerza ACOUSTID_AVAILABLE=True.

    Sin pyacoustid el matcher corta antes de mirar la API key y devuelve [],
    así que el contrato que se prueba (credencial ausente -> error explícito)
    sólo se observa fijando la bandera.
    """
    with patch("services.acoustid_matcher.ACOUSTID_AVAILABLE", True):
        yield


class TestAcoustIDMatcherRobustness(unittest.TestCase):
    """Validation for AcoustIDMatcher decoupling and missing key handling."""

    def test_acoustid_matcher_has_no_core_logic_reference(self):
        """Verify core_logic module is never referenced in acoustid_matcher.py."""
        matcher_file = SCRIPTS_DIR / "services" / "acoustid_matcher.py"
        content = matcher_file.read_text(encoding="utf-8")
        self.assertNotIn("core_logic", content)
        self.assertNotIn("87qWJy7qMk", content)

    def test_acoustid_matcher_init_no_api_key(self):
        """Without API key in args or env, matcher initializes cleanly with api_key=None."""
        with patch.dict(os.environ, {}, clear=True):
            matcher = AcoustIDMatcher(api_key=None)
            self.assertIsNone(matcher.api_key)

    def test_acoustid_matcher_init_from_env(self):
        """Matcher reads ACOUSTID_API_KEY from environment."""
        with patch.dict(os.environ, {"ACOUSTID_API_KEY": "env-acoustid-key-xyz"}):
            matcher = AcoustIDMatcher()
            self.assertEqual(matcher.api_key, "env-acoustid-key-xyz")

    def test_acoustid_matcher_init_explicit_override(self):
        """Explicit api_key parameter overrides environment."""
        with patch.dict(os.environ, {"ACOUSTID_API_KEY": "env-key"}):
            matcher = AcoustIDMatcher(api_key="explicit-key")
            self.assertEqual(matcher.api_key, "explicit-key")

    def test_identify_without_api_key_raises_not_configured(self):
        """Sin API key el matcher dice que falta la credencial, no que no hay coincidencias.

        Antes devolvía una lista vacía y el puente la pintaba como
        `No matches found`, que empuja a reintentar un comando que no puede
        funcionar nunca (auditoría 67).
        """
        with patch.dict(os.environ, {}, clear=True), _acoustid_installed():
            matcher = AcoustIDMatcher(api_key=None, verbose=True)
            with self.assertRaises(AcoustIDNotConfiguredError) as ctx:
                matcher.identify(Path("/nonexistent/audio.mp3"))
            self.assertIn("ACOUSTID_API_KEY", str(ctx.exception))

    def test_identify_with_fingerprint_without_api_key_raises_not_configured(self):
        """Idem por el camino de fingerprint, que es el que usa el puente."""
        with patch.dict(os.environ, {}, clear=True), _acoustid_installed():
            matcher = AcoustIDMatcher(api_key=None, verbose=True)
            with self.assertRaises(AcoustIDNotConfiguredError) as ctx:
                matcher.identify_with_fingerprint(180, "AQADtEmSREmURIn...")
            self.assertIn("ACOUSTID_API_KEY", str(ctx.exception))


if __name__ == "__main__":
    unittest.main()

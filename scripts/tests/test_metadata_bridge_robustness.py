#!/usr/bin/env python3
"""
Test Suite: Metadata Bridge & AcoustID Matcher Robustness (TASK-47)

Verifies:
1. Safe attribute extraction in metadata_bridge without AttributeError on missing attributes.
2. Proper fallbacks for artist_mbids -> musicbrainz_artist_id, genre_tags -> genres, lastfm_tags -> tags.
3. Clean JSON serialization without AttributeError or serialization crashes.
4. AcoustIDMatcher operates without core_logic dependency and gracefully handles missing API keys.
5. End-to-end enrich_track pipeline behavior with mocked enrichment service.
"""

import io
import json
import os
import sys
import unittest
from pathlib import Path
from unittest.mock import patch, AsyncMock

# Add repo root and scripts to sys.path, and discover venv site-packages
REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

for sp in REPO_ROOT.glob(".venv/lib/python*/site-packages"):
    if sp.is_dir() and str(sp) not in sys.path:
        sys.path.insert(0, str(sp))

from metadata_bridge import extract_enriched_metadata, enrich_track, json_response
from services.metadata_enrichment import EnrichedMetadata
from services.acoustid_matcher import AcoustIDMatcher, AcoustIDResult


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

    def test_identify_without_api_key_returns_empty_gracefully(self):
        """When API key is not configured, identify returns empty list without raising."""
        with patch.dict(os.environ, {}, clear=True):
            matcher = AcoustIDMatcher(api_key=None, verbose=True)
            results = matcher.identify(Path("/nonexistent/audio.mp3"))
            self.assertEqual(results, [])

    def test_identify_with_fingerprint_without_api_key_returns_empty_gracefully(self):
        """When API key is not configured, identify_with_fingerprint returns empty list."""
        with patch.dict(os.environ, {}, clear=True):
            matcher = AcoustIDMatcher(api_key=None, verbose=True)
            results = matcher.identify_with_fingerprint(180, "AQADtEmSREmURIn...")
            self.assertEqual(results, [])


if __name__ == "__main__":
    unittest.main()

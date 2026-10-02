#!/usr/bin/env python3
"""
Test Suite: AcoustID fingerprint contract, bridge <-> UI.

The `identify_audio` command used to be a no-op end to end: the bridge emitted
`data.matches[]` while the UI read `data.recordings[0]`, a key the bridge never
produces, and the matcher built matches with an empty `acoustid` and no artist
MBID. These tests pin both halves of the contract:

1. `scripts/fingerprint_bridge.py identify` emits `data.matches[]` carrying
   `recording_id`, `acoustid_id` and `musicbrainz_artistid`.
2. `AcoustIDMatcher.identify()` / `identify_with_fingerprint()` resolve those ids
   from the AcoustID web service (and percent-encode the base64 fingerprint).
3. The UI reads exactly those keys.
"""
import contextlib
import io
import json
import sys
import unittest
import urllib.parse
from pathlib import Path
from unittest.mock import patch

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPTS_DIR = REPO_ROOT / "scripts"
UI_DIR = REPO_ROOT / "ui"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))
for sp in REPO_ROOT.glob(".venv/lib/python*/site-packages"):
    if sp.is_dir() and str(sp) not in sys.path:
        sys.path.insert(0, str(sp))

import fingerprint_bridge
from services.acoustid_matcher import AcoustIDMatcher, AcoustIDResult

# Shape returned by https://api.acoustid.org/v2/lookup?meta=recordings
ACOUSTID_LOOKUP_PAYLOAD = {
    "status": "ok",
    "results": [
        {
            "id": "acoustid-id-1",
            "score": 0.98,
            "recordings": [
                {
                    "id": "recording-mbid-1",
                    "title": "Test Track",
                    "duration": 180,
                    "artists": [{"id": "artist-mbid-1", "name": "Test Artist"}],
                }
            ],
        },
        {
            "id": "acoustid-id-2",
            "score": 0.71,
            "recordings": [
                {
                    "id": "recording-mbid-2",
                    "title": "Test Track (Live)",
                    "duration": 191,
                    "artists": [{"id": "artist-mbid-2", "name": "Other Artist"}],
                }
            ],
        },
    ],
}

BASE64_FINGERPRINT = "AQADtEmSREmURIn+aE/z8+MyM="


class _FakeUrlopenResponse:
    """Minimal stand-in for the object returned by urllib.request.urlopen."""

    def __init__(self, payload):
        self._body = json.dumps(payload).encode("utf-8")

    def read(self):
        return self._body

    def __enter__(self):
        return self

    def __exit__(self, *exc_info):
        return False


class TestAcoustIDMatcherIdentify(unittest.TestCase):
    """identify() must surface real AcoustID ids and artist MBIDs."""

    def setUp(self):
        self.requested_urls = []

        def fake_urlopen(url, timeout=None):
            self.requested_urls.append(url)
            return _FakeUrlopenResponse(ACOUSTID_LOOKUP_PAYLOAD)

        patcher_urlopen = patch("urllib.request.urlopen", fake_urlopen)
        patcher_available = patch("services.acoustid_matcher.ACOUSTID_AVAILABLE", True)
        patcher_urlopen.start()
        patcher_available.start()
        self.addCleanup(patcher_urlopen.stop)
        self.addCleanup(patcher_available.stop)

    def test_identify_with_fingerprint_resolves_acoustid_id_and_artist_mbid(self):
        matcher = AcoustIDMatcher(api_key="test-key")
        results = matcher.identify_with_fingerprint(180, BASE64_FINGERPRINT)

        self.assertEqual(len(results), 2)
        top = results[0]
        self.assertEqual(top.acoustid, "acoustid-id-1")
        self.assertEqual(top.recording_id, "recording-mbid-1")
        self.assertEqual(top.title, "Test Track")
        self.assertEqual(top.artist, "Test Artist")
        self.assertEqual(top.artist_mbid, "artist-mbid-1")
        self.assertEqual(top.score, 0.98)
        # Ranked by score, highest first.
        self.assertEqual([r.recording_id for r in results],
                         ["recording-mbid-1", "recording-mbid-2"])

    def test_lookup_url_percent_encodes_base64_fingerprint(self):
        matcher = AcoustIDMatcher(api_key="test-key")
        matcher.identify_with_fingerprint(180, BASE64_FINGERPRINT)

        self.assertEqual(len(self.requested_urls), 1)
        query = urllib.parse.urlparse(self.requested_urls[0]).query
        params = urllib.parse.parse_qs(query)
        # '+' must survive the round trip; raw interpolation turns it into a space.
        self.assertEqual(params["fingerprint"], [BASE64_FINGERPRINT])
        self.assertEqual(params["duration"], ["180"])
        self.assertEqual(params["meta"], ["recordings"])

    def test_identify_fingerprints_then_uses_the_same_lookup(self):
        """identify() must not build empty `acoustid` results any more."""
        audio = Path(__file__).resolve()  # any existing path: fpcalc is stubbed out
        matcher = AcoustIDMatcher(api_key="test-key")

        with patch.object(
            AcoustIDMatcher, "get_fingerprint",
            return_value=(180, BASE64_FINGERPRINT),
        ) as mock_get_fingerprint:
            results = matcher.identify(audio)

        mock_get_fingerprint.assert_called_once()
        self.assertEqual(len(results), 2)
        self.assertTrue(all(r.acoustid for r in results))
        self.assertTrue(all(r.artist_mbid for r in results))
        self.assertEqual(results[0].recording_id, "recording-mbid-1")

    def test_identify_returns_empty_when_fingerprinting_fails(self):
        matcher = AcoustIDMatcher(api_key="test-key")
        with patch.object(AcoustIDMatcher, "get_fingerprint", return_value=None):
            self.assertEqual(matcher.identify(Path(__file__).resolve()), [])


class _FakeMatcher:
    """AcoustIDMatcher stand-in that records how the bridge drove it."""

    def __init__(self, fingerprint=(180, BASE64_FINGERPRINT), results=None):
        self._fingerprint = fingerprint
        self._results = results if results is not None else [
            AcoustIDResult(
                acoustid="acoustid-id-1",
                score=0.98,
                recording_id="recording-mbid-1",
                title="Test Track",
                artist="Test Artist",
                artist_mbid="artist-mbid-1",
                duration=180,
            ),
            AcoustIDResult(
                acoustid="acoustid-id-2",
                score=0.71,
                recording_id="recording-mbid-2",
                title="Test Track (Live)",
                artist="Other Artist",
                artist_mbid="artist-mbid-2",
                duration=191,
            ),
        ]
        self.lookup_calls = []

    def is_available(self):
        return True

    def get_fingerprint(self, path):
        return self._fingerprint

    def identify_with_fingerprint(self, duration, fingerprint):
        self.lookup_calls.append((duration, fingerprint))
        return list(self._results)


class TestFingerprintBridgeIdentifyContract(unittest.TestCase):
    """`identify` must emit data.matches[] with the keys the UI consumes."""

    def setUp(self):
        self.audio = Path(__file__).resolve()
        self.fake = _FakeMatcher()
        patcher = patch("services.acoustid_matcher.AcoustIDMatcher", return_value=self.fake)
        patcher.start()
        self.addCleanup(patcher.stop)

    def _run_identify(self, path=None):
        buffer = io.StringIO()
        with contextlib.redirect_stdout(buffer):
            with self.assertRaises(SystemExit) as exit_ctx:
                fingerprint_bridge.identify_track(str(path or self.audio))
        payload = json.loads(buffer.getvalue().strip())
        return payload, exit_ctx.exception.code

    def test_identify_emits_matches_with_contract_keys(self):
        payload, code = self._run_identify()

        self.assertEqual(code, 0)
        self.assertTrue(payload["success"])
        data = payload["data"]
        self.assertIn("matches", data)
        # The key the UI used to read does not exist in the bridge payload.
        self.assertNotIn("recordings", data)
        self.assertEqual(len(data["matches"]), 2)

        top = data["matches"][0]
        self.assertEqual(top["recording_id"], "recording-mbid-1")
        self.assertEqual(top["acoustid"], "acoustid-id-1")
        self.assertEqual(top["acoustid_id"], "acoustid-id-1")
        self.assertEqual(top["musicbrainz_artistid"], "artist-mbid-1")
        self.assertEqual(top["title"], "Test Track")
        self.assertEqual(top["artist"], "Test Artist")
        self.assertEqual(top["score"], 0.98)
        self.assertEqual(top["duration"], 180)
        self.assertEqual(data["file"], str(self.audio.absolute()))

    def test_identify_uses_identify_with_fingerprint(self):
        self._run_identify()

        self.assertEqual(self.fake.lookup_calls, [(180, BASE64_FINGERPRINT)])

    def test_identify_caps_matches_at_five(self):
        many = [
            AcoustIDResult(
                acoustid=f"acoustid-{i}",
                score=1.0 - i / 100,
                recording_id=f"recording-{i}",
                title=f"Track {i}",
                artist="Artist",
                artist_mbid="artist-mbid-1",
            )
            for i in range(8)
        ]
        with patch("services.acoustid_matcher.AcoustIDMatcher",
                   return_value=_FakeMatcher(results=many)):
            payload, code = self._run_identify()

        self.assertEqual(code, 0)
        self.assertEqual(len(payload["data"]["matches"]), 5)

    def test_identify_reports_fingerprint_failure(self):
        with patch("services.acoustid_matcher.AcoustIDMatcher",
                   return_value=_FakeMatcher(fingerprint=None)):
            payload, code = self._run_identify()

        self.assertEqual(code, 1)
        self.assertFalse(payload["success"])
        self.assertEqual(payload["error"], "Failed to generate fingerprint")
        self.assertNotIn("data", payload)

    def test_identify_reports_no_matches(self):
        with patch("services.acoustid_matcher.AcoustIDMatcher",
                   return_value=_FakeMatcher(results=[])):
            payload, code = self._run_identify()

        self.assertEqual(code, 1)
        self.assertFalse(payload["success"])
        self.assertEqual(payload["error"], "No matches found")

    def test_identify_reports_missing_file(self):
        missing = SCRIPTS_DIR / "no-such-track.flac"
        payload, code = self._run_identify(path=missing)

        self.assertEqual(code, 1)
        self.assertFalse(payload["success"])
        self.assertIn("File not found", payload["error"])


class TestUiConsumesAcoustIDContract(unittest.TestCase):
    """The UI must read the keys the bridge emits."""

    def setUp(self):
        self.view = (UI_DIR / "src" / "views" / "MetadataView.vue").read_text(encoding="utf-8")
        self.api = (UI_DIR / "src" / "api" / "metadata.ts").read_text(encoding="utf-8")

    def test_metadata_view_does_not_read_the_absent_recordings_key(self):
        for forbidden in ("data?.recordings", "data.recordings", "recordings?.[0]"):
            self.assertNotIn(forbidden, self.view)

    def test_metadata_view_identifies_through_the_api_wrapper(self):
        self.assertIn("metadataApi.identifyAudio(", self.view)
        self.assertIn("match.recording_id", self.view)
        # Single-track and batch identification must both go through the wrapper.
        self.assertEqual(self.view.count("metadataApi.identifyAudio("), 2)
        self.assertNotIn("'identify_audio'", self.view)

    def test_api_wrapper_maps_matches_from_the_bridge_envelope(self):
        self.assertIn("export async function identifyAudio(filePath: string): Promise<AcoustIDMatch[]>", self.api)
        self.assertIn("pickArray<unknown>(pick(raw, ['data']), ['matches'])", self.api)
        for key in ("acoustid_id", "recording_id", "musicbrainz_artistid"):
            self.assertIn(f"'{key}'", self.api)


if __name__ == "__main__":
    unittest.main()
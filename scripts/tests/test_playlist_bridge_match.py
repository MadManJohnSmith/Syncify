#!/usr/bin/env python3
"""
Contract tests for `playlist_bridge.py match` (PY-7).

The command used to be advertised as "Match playlist tracks to another service
using ISRC" while it never contacted any service: it only split the entries by
the presence of an ISRC and echoed `target_service` back. The contract is now
explicit - it classifies entries by ISRC availability and says so in the payload,
the CLI help, the Rust doc comment and the TypeScript API wrapper.

Validates:
1. Entries are split by ISRC availability and the ISRC is normalized.
2. JSON and M3U/M3U8 playlist files are both accepted.
3. The payload declares that the target service catalog is NOT queried.
4. Targets without ISRC support (e.g. SoundCloud) are rejected loudly.
5. The CLI emits exactly that payload.
"""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import playlist_bridge
from bridge_python import python_executable


class PlaylistMatchContractTests(unittest.TestCase):
    """The `match` command must not promise a catalog lookup it does not perform."""

    def _write_json_playlist(self, tracks):
        handle = tempfile.NamedTemporaryFile("w", suffix=".json", delete=False, encoding="utf-8")
        json.dump({"tracks": tracks}, handle)
        handle.close()
        self.addCleanup(Path(handle.name).unlink)
        return handle.name

    def test_classify_splits_entries_by_isrc_availability(self):
        playlist = self._write_json_playlist([
            {"title": "Yellow", "artist": "Coldplay", "isrc": "GBAYE0000123"},
            {"title": "No Identifier", "artist": "Someone"},
        ])

        result = playlist_bridge.classify_playlist_by_isrc(playlist, "qobuz")

        self.assertEqual(result["total_tracks"], 2)
        self.assertEqual(result["matchable"], 1)
        self.assertEqual(result["unmatchable"], 1)
        self.assertEqual(result["matchable_tracks"][0]["title"], "Yellow")
        self.assertEqual(result["matchable_tracks"][0]["isrc"], "GBAYE0000123")
        self.assertEqual(result["unmatchable_tracks"][0]["title"], "No Identifier")

    def test_classify_normalizes_isrc(self):
        playlist = self._write_json_playlist([
            {"title": "Yellow", "artist": "Coldplay", "isrc": " gb-aye-00-00123 "},
        ])

        result = playlist_bridge.classify_playlist_by_isrc(playlist, "spotify")

        self.assertEqual(result["matchable"], 1)
        self.assertEqual(result["matchable_tracks"][0]["isrc"], "GBAYE0000123")

    def test_classify_reads_m3u_isrc_comments(self):
        with tempfile.TemporaryDirectory() as tmp:
            playlist_file = Path(tmp) / "list.m3u"
            playlist_file.write_text(
                "#EXTM3U\n"
                "#EXTINF:239,Blue Monday - New Order\n"
                "# ISRC:GBARC7800001\n"
                "/tmp/blue-monday.flac\n",
                encoding="utf-8",
            )

            result = playlist_bridge.classify_playlist_by_isrc(str(playlist_file), "deezer")

        self.assertEqual(result["matchable"], 1)
        self.assertEqual(result["matchable_tracks"][0]["artist"], "Blue Monday")
        self.assertEqual(result["matchable_tracks"][0]["title"], "New Order")

    def test_payload_declares_the_service_is_not_queried(self):
        playlist = self._write_json_playlist([{"title": "T", "artist": "A", "isrc": "GBAYE0000123"}])

        result = playlist_bridge.classify_playlist_by_isrc(playlist, "tidal")

        self.assertIs(result["service_queried"], False)
        self.assertEqual(result["matching"], "isrc-availability")
        self.assertEqual(result["target_service"], "tidal")
        self.assertIn("NOT queried", result["note"])

    def test_target_services_without_isrc_support_are_rejected(self):
        playlist = self._write_json_playlist([{"title": "T", "artist": "A", "isrc": "GBAYE0000123"}])

        # SoundCloud has no ISRCs at all: an ISRC transfer can never succeed there.
        with self.assertRaises(ValueError) as ctx:
            playlist_bridge.classify_playlist_by_isrc(playlist, "soundcloud")

        self.assertIn("soundcloud", str(ctx.exception))
        self.assertIn("spotify", str(ctx.exception))

    def test_missing_playlist_file_raises(self):
        with self.assertRaises(FileNotFoundError):
            playlist_bridge.classify_playlist_by_isrc("/nonexistent/playlist.json", "spotify")

    def test_cli_match_emits_the_honest_payload(self):
        playlist = self._write_json_playlist([
            {"title": "Yellow", "artist": "Coldplay", "isrc": "GBAYE0000123"},
            {"title": "No Identifier", "artist": "Someone"},
        ])

        proc = subprocess.run(
            [python_executable(), str(SCRIPTS_DIR / "playlist_bridge.py"), "match", playlist, "qobuz"],
            capture_output=True,
            text=True,
            timeout=120,
        )

        self.assertEqual(proc.returncode, 0, proc.stderr)
        payload = json.loads(proc.stdout)
        self.assertTrue(payload["success"])
        self.assertIs(payload["data"]["service_queried"], False)
        self.assertEqual(payload["data"]["matchable"], 1)
        self.assertEqual(payload["data"]["unmatchable"], 1)

    def test_cli_match_rejects_unsupported_service(self):
        playlist = self._write_json_playlist([{"title": "T", "artist": "A"}])

        proc = subprocess.run(
            [python_executable(), str(SCRIPTS_DIR / "playlist_bridge.py"), "match", playlist, "soundcloud"],
            capture_output=True,
            text=True,
            timeout=120,
        )

        self.assertEqual(proc.returncode, 1)
        payload = json.loads(proc.stdout)
        self.assertFalse(payload["success"])
        self.assertIn("Unsupported target service", payload["error"])

    def test_module_docstring_does_not_promise_a_service_lookup(self):
        source = (SCRIPTS_DIR / "playlist_bridge.py").read_text(encoding="utf-8")

        self.assertNotIn("Match tracks", source.split('"""')[1])


if __name__ == "__main__":
    unittest.main()
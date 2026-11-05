#!/usr/bin/env python3
"""ffprobe resolution for scanner_bridge's metadata fallback.

The packaged app injects FFPROBE_PATH / FFMPEG_PATH for its bundled tools; the
fallback must honour them — on a Windows install the binaries live next to the
executable, not on PATH.
"""

import os
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import scanner_bridge


class ScannerBridgeFfprobeResolutionTest(unittest.TestCase):
    def setUp(self):
        self._saved = {
            key: os.environ.get(key) for key in ("FFPROBE_PATH", "FFMPEG_PATH")
        }
        for key in self._saved:
            os.environ.pop(key, None)

    def tearDown(self):
        for key, value in self._saved.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value

    def test_ffprobe_path_env_wins(self):
        with tempfile.TemporaryDirectory() as tmp:
            probe = Path(tmp) / "ffprobe"
            probe.write_bytes(b"stub")
            os.environ["FFPROBE_PATH"] = str(probe)
            self.assertEqual(scanner_bridge.resolve_ffprobe_path(), str(probe))

    def test_ffmpeg_sibling_is_used_when_ffprobe_env_is_absent(self):
        with tempfile.TemporaryDirectory() as tmp:
            ffmpeg = Path(tmp) / ("ffmpeg.exe" if os.name == "nt" else "ffmpeg")
            ffmpeg.write_bytes(b"stub")
            probe = Path(tmp) / ("ffprobe.exe" if os.name == "nt" else "ffprobe")
            probe.write_bytes(b"stub")

            os.environ["FFMPEG_PATH"] = str(ffmpeg)
            self.assertEqual(scanner_bridge.resolve_ffprobe_path(), str(probe))

    def test_missing_env_falls_back_to_path_lookup(self):
        resolved = scanner_bridge.resolve_ffprobe_path()
        self.assertTrue(resolved, "resolution must never return an empty value")
        self.assertIn("ffprobe", resolved.lower())


if __name__ == "__main__":
    unittest.main(verbosity=2)

#!/usr/bin/env python3
"""Pinned download integrity for dependency_manager.

A rolling release URL can never be combined with a fixed SHA-256: the next
upstream rebuild makes the integrity check fail forever (this happened with
johnvansickle's `ffmpeg-release-amd64-static` tarball). Supported platforms
must point at dated releases with pinned digests.
"""

import re
import sys
import unittest
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import dependency_manager

SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


class DependencyManagerPinTest(unittest.TestCase):
    def test_supported_platforms_have_pinned_hashes(self):
        for tool, per_platform in dependency_manager.KNOWN_SHA256_HASHES.items():
            for platform in ("windows", "linux"):
                digest = per_platform.get(platform, "")
                self.assertRegex(
                    digest,
                    SHA256_RE,
                    f"{tool}/{platform} must carry a pinned sha256 (got {digest!r})",
                )

    def test_ffmpeg_urls_are_dated_releases(self):
        ffmpeg = dependency_manager.TOOL_URLS["ffmpeg"]

        self.assertIn("autobuild-20", ffmpeg["windows"])
        self.assertIn("autobuild-20", ffmpeg["linux"])
        self.assertNotIn(
            "release-amd64-static",
            ffmpeg["linux"],
            "johnvansickle's rolling tarball can never match a fixed hash",
        )
        self.assertNotIn("/latest/", ffmpeg["windows"])
        self.assertNotIn("/latest/", ffmpeg["linux"])

    def test_fpcalc_is_version_pinned(self):
        fpcalc = dependency_manager.TOOL_URLS["fpcalc"]
        for platform in ("windows", "linux"):
            self.assertIn("v1.5.1", fpcalc[platform])


if __name__ == "__main__":
    unittest.main(verbosity=2)

#!/usr/bin/env python3
"""Pin-table integrity and bundled-binary placement for dependency_manager.

The table (TOOL_URLS + KNOWN_SHA256_HASHES) is the single pin store of the
repo (#241/#242): the release builds consume it through `place-external` and
`install-runtime`, so a repin must be one commit here. These tests PROVE the
table instead of freezing workflow YAML: every URL carries a hash, rolling
URLs are rejected, the runtime rows pin the exact CPython builds both
platforms ship, and placement produces exactly the `<tool>-<triple>[.exe]`
names that `bundle.externalBin` demands at build time.

Behavior tests use fakes/fixtures (no network): a fake `download_file` and a
patched table entry simulate the pinned download, same pattern as the
security suite.
"""

import io
import json
import os
import re
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import dependency_manager

SHA256_RE = re.compile(r"^[0-9a-f]{64}$")

# The exact file names bundle.externalBin (tauri.conf.json) resolves at build
# time: three tools x two published triples. Tauri appends the triple (and
# .exe on Windows) to each `binaries/<tool>` entry, so these literals are the
# bundler contract, not a frozen copy of the table.
EXTERNAL_BIN_NAMES = {
    "x86_64-unknown-linux-gnu": (
        "ffmpeg-x86_64-unknown-linux-gnu",
        "ffprobe-x86_64-unknown-linux-gnu",
        "fpcalc-x86_64-unknown-linux-gnu",
    ),
    "x86_64-pc-windows-msvc": (
        "ffmpeg-x86_64-pc-windows-msvc.exe",
        "ffprobe-x86_64-pc-windows-msvc.exe",
        "fpcalc-x86_64-pc-windows-msvc.exe",
    ),
}


class PinTableIntegrityTest(unittest.TestCase):
    """Every URL has a hash, every hash has a URL, nothing rolling."""

    def test_urls_and_hashes_cover_the_same_keys(self):
        self.assertEqual(
            set(dependency_manager.TOOL_URLS), set(dependency_manager.KNOWN_SHA256_HASHES)
        )
        for tool in dependency_manager.TOOL_URLS:
            self.assertEqual(
                set(dependency_manager.TOOL_URLS[tool]),
                set(dependency_manager.KNOWN_SHA256_HASHES[tool]),
                f"{tool}: URL and hash tables must cover the same platforms",
            )

    def test_supported_platforms_have_pinned_hashes(self):
        # The two published platforms must carry a digest for every row that
        # declares them; get-pip only declares windows (the standalone tarball
        # ships pip already), so the rule is per declared row.
        for tool, per_platform in dependency_manager.KNOWN_SHA256_HASHES.items():
            for platform, digest in per_platform.items():
                self.assertRegex(
                    digest,
                    SHA256_RE,
                    f"{tool}/{platform} must carry a pinned sha256 (got {digest!r})",
                )
        for platform in ("windows", "linux"):
            self.assertIn(
                platform,
                dependency_manager.KNOWN_SHA256_HASHES["cpython"],
                f"the shipped runtime must be pinned on {platform}",
            )

    def test_darwin_rows_stay_declared(self):
        # macOS is not a released platform, but the table keeps its rows so
        # the local darwin flow keeps working; the security suite holds the
        # same contract.
        for tool in ("ffmpeg", "fpcalc"):
            self.assertIn("darwin", dependency_manager.TOOL_URLS[tool])
            self.assertRegex(dependency_manager.KNOWN_SHA256_HASHES[tool]["darwin"], SHA256_RE)

    def test_ffmpeg_urls_are_dated_releases(self):
        ffmpeg = dependency_manager.TOOL_URLS["ffmpeg"]

        for platform in ("windows", "linux"):
            url = ffmpeg[platform]
            self.assertIn("autobuild-20", url, f"ffmpeg/{platform} must pin a dated BtbN release")
            self.assertIn("ffmpeg-N-", url, f"ffmpeg/{platform} must pin an explicit build id")
            self.assertNotIn("/latest/", url)
        self.assertNotIn(
            "release-amd64-static",
            ffmpeg["linux"],
            "johnvansickle's rolling tarball can never match a fixed hash",
        )

    def test_fpcalc_is_version_pinned(self):
        fpcalc = dependency_manager.TOOL_URLS["fpcalc"]
        for platform in ("windows", "linux"):
            self.assertIn("v1.5.1", fpcalc[platform])

    def test_ffprobe_is_a_byproduct_with_no_url_row(self):
        # ffprobe travels inside the pinned ffmpeg archive; a URL row of its
        # own would promise a source that does not exist.
        self.assertNotIn("ffprobe", dependency_manager.TOOL_URLS)
        self.assertIn("ffprobe", dependency_manager.BUNDLED_TOOLS)
        # check_all surfaces it alongside the rest, as before.
        self.assertIn("ffprobe", dependency_manager.check_all()["tools"])


class RuntimePinRowsTest(unittest.TestCase):
    """The cpython/get-pip rows: same 3.11.x runtime on both platforms."""

    def test_linux_pins_python_build_standalone_20240415(self):
        url = dependency_manager.TOOL_URLS["cpython"]["linux"]
        self.assertIn("python-build-standalone", url)
        self.assertIn("/20240415/", url, "the standalone release tag must be explicit")
        self.assertIn("cpython-3.11.9", url, "the runtime patch must match the embed")
        self.assertIn("install_only", url, "the relocatable install_only build")
        self.assertNotIn("/latest/", url)

    def test_windows_pins_the_python_org_embed(self):
        url = dependency_manager.TOOL_URLS["cpython"]["windows"]
        self.assertIn("python.org/ftp/python/", url)
        self.assertIn("/3.11.9/", url, "same 3.11.x patch as the Linux runtime")
        self.assertIn("embed-amd64.zip", url)
        self.assertNotIn("/latest/", url)

    def test_runtime_version_constant_matches_the_table(self):
        for platform, url in dependency_manager.TOOL_URLS["cpython"].items():
            self.assertIn(
                dependency_manager.RUNTIME_PYTHON_VERSION,
                url,
                f"{platform}: RUNTIME_PYTHON_VERSION drifted from the pinned URL",
            )

    def test_runtime_rows_have_no_darwin_entry(self):
        # macOS is not a published platform; install-runtime rejects it.
        self.assertNotIn("darwin", dependency_manager.TOOL_URLS["cpython"])
        self.assertNotIn("darwin", dependency_manager.TOOL_URLS["get-pip"])

    def test_get_pip_is_pinned_with_a_digest(self):
        url = dependency_manager.TOOL_URLS["get-pip"]["windows"]
        self.assertIn("bootstrap.pypa.io/get-pip.py", url)
        self.assertRegex(
            dependency_manager.KNOWN_SHA256_HASHES["get-pip"]["windows"], SHA256_RE,
            "the mutable bootstrap must carry its hash in the table",
        )

    def test_runtime_rows_are_not_installable_tools(self):
        # `install cpython` would download+extract into bin/ and then fail
        # tool-path verification: the runtime has its own subcommand.
        for tool in sorted(dependency_manager.RUNTIME_TOOLS):
            result = dependency_manager.install_tool(tool)
            self.assertFalse(result["success"])
            self.assertIn("install-runtime", result["error"])

    def test_canonical_smoke_list_is_the_frozen_ten(self):
        # Ten modules on BOTH platforms; Windows used to verify only six and
        # could ship without aiohttp/yaml/musicbrainzngs/m3u8. The spec
        # freezes this list, so the equality is exact.
        self.assertEqual(
            dependency_manager.CANONICAL_SMOKE_IMPORTS,
            (
                "spotipy",
                "acoustid",
                "dotenv",
                "requests",
                "mutagen",
                "yarl",
                "aiohttp",
                "yaml",
                "musicbrainzngs",
                "m3u8",
            ),
        )
        statement = dependency_manager.smoke_import_statement()
        self.assertEqual(statement, "import " + ", ".join(dependency_manager.CANONICAL_SMOKE_IMPORTS))


class ExternalBinPlacementTest(unittest.TestCase):
    """Placement yields the exact externalBin names, from the pinned tree only."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.base_dir = Path(self.temp_dir.name).resolve()
        self.bin_dir = self.base_dir / "bin"
        self.bin_dir.mkdir()
        self.binaries_dir = self.base_dir / "src-tauri" / "binaries"

    def tearDown(self):
        self.temp_dir.cleanup()

    def _dir_patches(self):
        return [
            patch.object(dependency_manager, "BIN_DIR", self.bin_dir),
            patch.object(dependency_manager, "BINARIES_DIR", self.binaries_dir),
        ]

    def _seed_bin_tree(self):
        for tool in dependency_manager.BUNDLED_TOOLS:
            (self.bin_dir / tool).write_bytes(f"#!/bin/sh\n# pinned {tool}\n".encode())

    def test_bundled_binary_names_are_the_external_bin_contract(self):
        for triple, names in EXTERNAL_BIN_NAMES.items():
            for name in names:
                tool = name.split("-")[0]
                self.assertEqual(dependency_manager.bundled_binary_name(tool, triple), name)
        self.assertEqual(
            set(dependency_manager.SUPPORTED_TRIPLES.values()), set(EXTERNAL_BIN_NAMES)
        )

    def test_place_copies_and_marks_executable_with_triple_names(self):
        self._seed_bin_tree()

        patches = self._dir_patches()
        for patcher in patches:
            patcher.start()
        try:
            result = dependency_manager.place_bundled_binaries(
                triple="x86_64-unknown-linux-gnu"
            )
        finally:
            for patcher in patches:
                patcher.stop()

        self.assertTrue(result["success"], result)
        self.assertEqual(result["triple"], "x86_64-unknown-linux-gnu")
        self.assertEqual(
            sorted(entry["tool"] for entry in result["placed"]),
            ["ffmpeg", "ffprobe", "fpcalc"],
        )
        for name in EXTERNAL_BIN_NAMES["x86_64-unknown-linux-gnu"]:
            dest = self.binaries_dir / name
            self.assertTrue(dest.is_file(), f"missing placed binary {name}")
            self.assertTrue(
                dest.stat().st_mode & 0o111,
                f"{name} must be executable after placement",
            )

    def test_place_overwrites_stale_placeholders(self):
        self._seed_bin_tree()
        self.binaries_dir.mkdir(parents=True)
        (self.binaries_dir / "fpcalc-x86_64-unknown-linux-gnu").write_text("# PLACEHOLDER")

        patches = self._dir_patches()
        for patcher in patches:
            patcher.start()
        try:
            result = dependency_manager.place_bundled_binaries(
                triple="x86_64-unknown-linux-gnu"
            )
        finally:
            for patcher in patches:
                patcher.stop()

        self.assertTrue(result["success"], result)
        self.assertEqual(
            (self.binaries_dir / "fpcalc-x86_64-unknown-linux-gnu").read_bytes(),
            b"#!/bin/sh\n# pinned fpcalc\n",
        )

    def test_find_bundled_tool_never_falls_back_to_system_path(self):
        # Placement must ship the pinned builds: a system ffmpeg on the
        # runner (the Ubuntu image installs one via apt) must not satisfy it.
        (self.bin_dir / "fpcalc").write_bytes(b"pinned fpcalc")
        with patch.object(dependency_manager, "BIN_DIR", self.bin_dir), \
                patch("dependency_manager.shutil.which", return_value="/usr/bin/ffmpeg") as which:
            found = dependency_manager.find_bundled_tool("fpcalc")
            missing = dependency_manager.find_bundled_tool("ffmpeg")
        self.assertEqual(found, self.bin_dir / "fpcalc")
        self.assertIsNone(missing)
        which.assert_not_called()

    def test_missing_fpcalc_is_downloaded_from_the_pin_table(self):
        (self.bin_dir / "ffmpeg").write_bytes(b"pinned ffmpeg")
        (self.bin_dir / "ffprobe").write_bytes(b"pinned ffprobe")

        # Fake archive that stands in for the pinned chromaprint tarball.
        fixture = io.BytesIO()
        with tarfile.open(fileobj=fixture, mode="w:gz") as tf:
            payload = b"#!/bin/sh\n# fake pinned fpcalc\n"
            info = tarfile.TarInfo(name="fpcalc")
            info.size = len(payload)
            info.mode = 0o755
            tf.addfile(info, io.BytesIO(payload))
        fixture_path = self.base_dir / "fixture-fpcalc.tar.gz"
        fixture_path.write_bytes(fixture.getvalue())
        archive_hash = dependency_manager.compute_file_sha256(fixture_path)

        def fake_download(url, dest, progress_callback=None):
            dest.write_bytes(fixture.getvalue())
            return True

        with patch.object(dependency_manager, "BIN_DIR", self.bin_dir), \
                patch.object(dependency_manager, "BINARIES_DIR", self.binaries_dir), \
                patch.dict(
                    dependency_manager.KNOWN_SHA256_HASHES["fpcalc"],
                    {"linux": archive_hash},
                ), \
                patch.object(dependency_manager, "download_file", side_effect=fake_download):
            result = dependency_manager.place_bundled_binaries(
                triple="x86_64-unknown-linux-gnu"
            )

        self.assertTrue(result["success"], result)
        dest = self.binaries_dir / "fpcalc-x86_64-unknown-linux-gnu"
        self.assertTrue(dest.is_file())
        self.assertIn(b"fake pinned fpcalc", dest.read_bytes())
        # The extracted tree also lands in bin/, so the portable packaging
        # step keeps finding it there.
        self.assertTrue((self.bin_dir / "fpcalc").is_file())

    def test_missing_ffprobe_fails_as_a_byproduct_of_ffmpeg(self):
        (self.bin_dir / "ffmpeg").write_bytes(b"pinned ffmpeg")
        (self.bin_dir / "fpcalc").write_bytes(b"pinned fpcalc")

        patches = self._dir_patches()
        for patcher in patches:
            patcher.start()
        try:
            result = dependency_manager.place_bundled_binaries(
                triple="x86_64-unknown-linux-gnu"
            )
        finally:
            for patcher in patches:
                patcher.stop()

        self.assertFalse(result["success"])
        self.assertIn("byproduct", result["error"])


class TripleResolutionTest(unittest.TestCase):
    def test_explicit_triple_wins(self):
        with patch.dict(os.environ, {"TARGET_TRIPLE": "aarch64-unknown-linux-gnu"}):
            self.assertEqual(
                dependency_manager.resolve_triple("x86_64-unknown-linux-gnu"),
                "x86_64-unknown-linux-gnu",
            )

    def test_env_triple_is_honored(self):
        with patch.dict(os.environ, {"TARGET_TRIPLE": "aarch64-pc-windows-msvc"}):
            self.assertEqual(
                dependency_manager.resolve_triple(), "aarch64-pc-windows-msvc"
            )

    def test_falls_back_to_published_triple_of_the_platform(self):
        with patch.dict(os.environ, {}, clear=True), \
                patch("dependency_manager.shutil.which", return_value=None):
            for plat, triple in dependency_manager.SUPPORTED_TRIPLES.items():
                with patch.object(dependency_manager, "get_platform", return_value=plat):
                    self.assertEqual(dependency_manager.resolve_triple(), triple)


class PlaceExternalCliEnvelopeTest(unittest.TestCase):
    """The place-external subcommand speaks the same JSON envelope contract."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.base_dir = Path(self.temp_dir.name).resolve()
        self.bin_dir = self.base_dir / "bin"
        self.bin_dir.mkdir()
        self.binaries_dir = self.base_dir / "src-tauri" / "binaries"

    def tearDown(self):
        self.temp_dir.cleanup()

    def _run_cli(self, *argv, extra_patches=()):
        stdout = io.StringIO()
        patches = [
            patch.object(dependency_manager, "BIN_DIR", self.bin_dir),
            patch.object(dependency_manager, "BINARIES_DIR", self.binaries_dir),
            patch.object(sys, "argv", ["dependency_manager.py", *argv]),
            patch("sys.stdout", stdout),
            *extra_patches,
        ]
        for patcher in patches:
            patcher.start()
        try:
            rc = dependency_manager.main()
        finally:
            for patcher in reversed(patches):
                patcher.stop()
        return rc, stdout.getvalue()

    def test_success_envelope_reports_placed_binaries(self):
        for tool in dependency_manager.BUNDLED_TOOLS:
            (self.bin_dir / tool).write_bytes(f"#!/bin/sh\n# pinned {tool}\n".encode())

        rc, out = self._run_cli("place-external", "--triple", "x86_64-unknown-linux-gnu")
        self.assertEqual(rc, 0, out)
        envelope = json.loads(out)
        self.assertTrue(envelope["success"])
        self.assertEqual(envelope["data"]["triple"], "x86_64-unknown-linux-gnu")
        self.assertEqual(len(envelope["data"]["placed"]), 3)
        for name in EXTERNAL_BIN_NAMES["x86_64-unknown-linux-gnu"]:
            self.assertTrue((self.binaries_dir / name).is_file(), name)

    def test_failure_envelope_on_download_failure(self):
        rc, out = self._run_cli(
            "place-external",
            extra_patches=[patch.object(dependency_manager, "download_file", return_value=False)],
        )
        self.assertEqual(rc, 1, out)
        envelope = json.loads(out)
        self.assertFalse(envelope["success"])
        self.assertIn("ffmpeg", envelope["error"])


if __name__ == "__main__":
    unittest.main(verbosity=2)

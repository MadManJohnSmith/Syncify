#!/usr/bin/env python3
"""System-browser detection for the auth bridges.

The detection must accept the thin launcher scripts distros ship (snap,
Flatpak exports, Debian's chromium wrapper), while still rejecting uninstall
stubs, and the SYNCIFY_BROWSER_PATH override must win over everything else.
"""

import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from services.browser_launcher import _is_valid_browser_exe, chrome_launch_kwargs

IS_WINDOWS = sys.platform.startswith("win")


def _make_executable(path: Path, content: str) -> None:
    path.write_text(content, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


class BrowserDetectionTest(unittest.TestCase):
    def test_missing_path_is_rejected(self):
        self.assertFalse(_is_valid_browser_exe(""))
        self.assertFalse(_is_valid_browser_exe("/definitely/not/here/chrome"))

    def test_empty_stub_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            stub = Path(tmp) / "chrome"
            stub.write_bytes(b"")
            stub.chmod(0o755)
            self.assertFalse(_is_valid_browser_exe(str(stub)))

    def test_large_binary_is_trusted_by_size(self):
        with tempfile.TemporaryDirectory() as tmp:
            big = Path(tmp) / "big-chrome"
            big.write_bytes(b"\0" * 200_000)
            big.chmod(0o755)
            self.assertTrue(_is_valid_browser_exe(str(big)))

    @unittest.skipIf(IS_WINDOWS, "shell wrapper probe only runs on POSIX")
    def test_small_wrapper_announcing_chromium_is_accepted(self):
        # snap/flatpak exports and Debian's chromium launcher are small
        # scripts: the version banner is what proves they are real browsers.
        with tempfile.TemporaryDirectory() as tmp:
            wrapper = Path(tmp) / "chromium"
            _make_executable(wrapper, '#!/bin/sh\necho "Chromium 120.0.6099.109"\n')
            self.assertTrue(_is_valid_browser_exe(str(wrapper)))

    @unittest.skipIf(IS_WINDOWS, "shell wrapper probe only runs on POSIX")
    def test_small_stub_without_browser_banner_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            stub = Path(tmp) / "chrome"
            _make_executable(stub, "#!/bin/sh\nexit 1\n")
            self.assertFalse(_is_valid_browser_exe(str(stub)))

    def test_small_non_executable_file_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            plain = Path(tmp) / "notes.txt"
            plain.write_text("not a browser", encoding="utf-8")
            if not IS_WINDOWS:
                plain.chmod(0o644)
            self.assertFalse(_is_valid_browser_exe(str(plain)))

    def test_env_override_wins_in_chrome_launch_kwargs(self):
        with tempfile.TemporaryDirectory() as tmp:
            browser = Path(tmp) / "my-chrome"
            browser.write_bytes(b"\0" * 200_000)
            browser.chmod(0o755)

            previous = os.environ.get("SYNCIFY_BROWSER_PATH")
            os.environ["SYNCIFY_BROWSER_PATH"] = str(browser)
            try:
                kwargs = chrome_launch_kwargs()
            finally:
                if previous is None:
                    os.environ.pop("SYNCIFY_BROWSER_PATH", None)
                else:
                    os.environ["SYNCIFY_BROWSER_PATH"] = previous

            self.assertEqual(kwargs, {"executable_path": str(browser)})


if __name__ == "__main__":
    unittest.main(verbosity=2)

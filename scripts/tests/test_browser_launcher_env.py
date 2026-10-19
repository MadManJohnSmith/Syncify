#!/usr/bin/env python3
"""
Test Suite: Browser Launcher Environment Scrubbing.

Validates that `services.browser_launcher.scrub_loader_env` removes the
AppImage's bundled library paths from LD_LIBRARY_PATH / LD_PRELOAD before any
browser or URL opener is spawned. Inside the AppImage, the runtime exports
LD_LIBRARY_PATH pointing at the bundled Ubuntu 22.04 copies (libssl, glib,
...), which shadow the host's newer libraries and crash the desktop URL
openers (kde-open5, gio) that the auth flows (Tidal/Qobuz/Deezer device-code
login) depend on.
"""

import os
import sys
import unittest
from unittest import mock

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from services.browser_launcher import _is_bundled_path, scrub_loader_env  # noqa: E402


class IsBundledPathTest(unittest.TestCase):
    def test_mount_path_is_bundled(self):
        self.assertTrue(_is_bundled_path("/tmp/.mount_SyncifABCD/usr/lib"))
        self.assertTrue(_is_bundled_path("/tmp/.mount_SyncifABCD/resources/python"))

    def test_squashfs_root_is_bundled(self):
        self.assertTrue(_is_bundled_path("/home/u/Downloads/squashfs-root/usr/lib"))

    def test_appdir_root_is_bundled(self):
        with mock.patch.dict(os.environ, {"APPDIR": "/tmp/.mount_SyncifABCD"}):
            self.assertTrue(_is_bundled_path("/tmp/.mount_SyncifABCD/usr/lib/libssl.so.3"))

    def test_system_paths_are_not_bundled(self):
        self.assertFalse(_is_bundled_path("/usr/lib"))
        self.assertFalse(_is_bundled_path("/usr/lib/x86_64-linux-gnu/libssl.so.3"))
        self.assertFalse(_is_bundled_path(""))
        # Un anfitrión que simplemente contenga "/.mount_" en otro contexto no
        # existe en la práctica, pero /usr/lib nunca debe clasificarse bundled.
        self.assertFalse(_is_bundled_path("/opt/mytools/lib"))


class ScrubLoaderEnvTest(unittest.TestCase):
    def setUp(self):
        self._saved = {k: os.environ.get(k) for k in ("APPDIR", "LD_LIBRARY_PATH", "LD_PRELOAD")}

    def tearDown(self):
        for key, value in self._saved.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value

    def test_removes_bundled_entries_keeps_system_ones(self):
        env = {
            "APPDIR": "/tmp/.mount_SyncifABCD",
            "LD_LIBRARY_PATH": "/tmp/.mount_SyncifABCD/usr/lib:/usr/lib:/opt/otro",
            "LD_PRELOAD": "",
        }
        with mock.patch.dict(os.environ, env, clear=False):
            scrub_loader_env()
            self.assertEqual(os.environ["LD_LIBRARY_PATH"], "/usr/lib:/opt/otro")

    def test_removes_variable_when_all_entries_are_bundled(self):
        env = {
            "APPDIR": "/tmp/.mount_SyncifABCD",
            "LD_LIBRARY_PATH": "/tmp/.mount_SyncifABCD/usr/lib:/tmp/.mount_SyncifABCD/usr/lib/x86_64-linux-gnu",
            "LD_PRELOAD": "",
        }
        with mock.patch.dict(os.environ, env, clear=False):
            scrub_loader_env()
            self.assertNotIn("LD_LIBRARY_PATH", os.environ)

    def test_ld_preload_is_space_separated(self):
        env = {
            "APPDIR": "/tmp/.mount_SyncifABCD",
            "LD_LIBRARY_PATH": "/usr/lib",
            "LD_PRELOAD": "/tmp/.mount_SyncifABCD/usr/lib/poison.so /usr/lib/fine.so",
        }
        with mock.patch.dict(os.environ, env, clear=False):
            scrub_loader_env()
            self.assertEqual(os.environ["LD_PRELOAD"], "/usr/lib/fine.so")

    def test_squashfs_root_entries_removed_without_appdir(self):
        env = {"LD_LIBRARY_PATH": "/home/u/app/squashfs-root/usr/lib:/usr/lib"}
        with mock.patch.dict(os.environ, env, clear=False):
            os.environ.pop("APPDIR", None)
            os.environ.pop("LD_PRELOAD", None)
            scrub_loader_env()
            self.assertEqual(os.environ["LD_LIBRARY_PATH"], "/usr/lib")

    def test_noop_when_environment_is_clean(self):
        env = {
            "APPDIR": "/tmp/.mount_SyncifABCD",
            "LD_LIBRARY_PATH": "/usr/lib:/opt/otro",
            "LD_PRELOAD": "/usr/lib/fine.so",
        }
        with mock.patch.dict(os.environ, env, clear=False):
            scrub_loader_env()
            self.assertEqual(os.environ["LD_LIBRARY_PATH"], "/usr/lib:/opt/otro")
            self.assertEqual(os.environ["LD_PRELOAD"], "/usr/lib/fine.so")


if __name__ == "__main__":
    unittest.main()

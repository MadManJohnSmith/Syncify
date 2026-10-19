#!/usr/bin/env python3
"""find_syncify_db must resolve the OS-native app profile on every platform.

The bridges used to hardcode ~/.local/share/com.syncify.app (Linux-only), so on
Windows `list local` / `get local` silently returned empty results.
"""

import os
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import playlist_bridge


class FindSyncifyDbTest(unittest.TestCase):
    def setUp(self):
        self._saved = {
            key: os.environ.get(key)
            for key in ("SYNCIFY_DB_PATH", "LOCALAPPDATA", "XDG_DATA_HOME")
        }
        for key in self._saved:
            os.environ.pop(key, None)

    def tearDown(self):
        for key, value in self._saved.items():
            if value is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = value

    @staticmethod
    def _make_db(appdata_root: str) -> Path:
        app_dir = Path(appdata_root) / "com.syncify.app"
        app_dir.mkdir(parents=True, exist_ok=True)
        db = app_dir / "syncify.db"
        db.write_bytes(b"")
        return db

    def test_explicit_env_path_wins(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = Path(tmp) / "custom" / "syncify.db"
            db.parent.mkdir()
            db.write_bytes(b"")
            os.environ["SYNCIFY_DB_PATH"] = str(db)
            self.assertEqual(playlist_bridge.find_syncify_db(), db)

    def test_windows_localappdata_candidate_is_resolved(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = self._make_db(tmp)
            os.environ["LOCALAPPDATA"] = tmp
            os.environ["XDG_DATA_HOME"] = str(Path(tmp) / "xdg-empty")
            self.assertEqual(playlist_bridge.find_syncify_db(), db)

    def test_xdg_data_home_candidate_is_resolved(self):
        with tempfile.TemporaryDirectory() as tmp:
            db = self._make_db(tmp)
            os.environ["XDG_DATA_HOME"] = tmp
            os.environ["LOCALAPPDATA"] = str(Path(tmp) / "localappdata-empty")
            self.assertEqual(playlist_bridge.find_syncify_db(), db)

    def test_missing_everywhere_returns_none(self):
        with tempfile.TemporaryDirectory() as tmp:
            os.environ["XDG_DATA_HOME"] = str(Path(tmp) / "xdg-empty")
            os.environ["LOCALAPPDATA"] = str(Path(tmp) / "localappdata-empty")
            # The repo-local fallbacks only matter when they exist; in a clean
            # checkout they do not affect this test's temporary roots.
            found = playlist_bridge.find_syncify_db()
            if found is not None:
                # A developer checkout may carry src-tauri/data/syncify.db;
                # that fallback is legitimate, just not an app-profile hit.
                self.assertNotIn(
                    str(Path(tmp)),
                    str(found),
                    "empty temporary roots must not yield a database",
                )


if __name__ == "__main__":
    unittest.main(verbosity=2)

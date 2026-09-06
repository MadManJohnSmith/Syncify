#!/usr/bin/env python3
"""
Test Suite: Playlist Bridge Hygiene & M3U Export Validation (TASK-42).

Validates:
1. Complete removal of references to nonexistent 'spotify_sync_lib' across scripts/.
2. Absence of zombie modules (spotify_api.py, local_file_scanner.py) from production paths.
3. Clean importation of playlist_bridge without ModuleNotFoundError.
4. M3U / M3U8 generation and export for local and remote playlists.
5. CLI invocations for playlist export without crashing.
"""

import json
import os
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

# Add scripts directory to sys.path
SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import playlist_bridge


class TestPlaylistBridgeHygiene(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.scripts_dir = SCRIPTS_DIR
        cls.services_dir = cls.scripts_dir / "services"

    def test_clean_import_without_modulenotfounderror(self):
        """Verify playlist_bridge can be imported without any ModuleNotFoundError."""
        self.assertTrue(hasattr(playlist_bridge, "export_playlist_data"))
        self.assertTrue(hasattr(playlist_bridge, "build_m3u_content"))
        self.assertTrue(hasattr(playlist_bridge, "get_tracks_for_service"))

    def test_no_spotify_sync_lib_references_in_scripts(self):
        """Verify no files in scripts/ reference the phantom 'spotify_sync_lib' package."""
        forbidden = "spotify_sync_lib"
        offenders = []
        this_file = Path(__file__).resolve()
        for path in self.scripts_dir.rglob("*.py"):
            if path.resolve() == this_file:
                continue
            try:
                content = path.read_text(encoding="utf-8", errors="ignore")
                if forbidden in content:
                    offenders.append(str(path))
            except Exception as e:
                self.fail(f"Failed to read file {path}: {e}")

        self.assertEqual(
            offenders,
            [],
            f"Found forbidden references to '{forbidden}' in: {offenders}",
        )

    def test_zombie_modules_absent_from_production(self):
        """Ensure zombie modules (spotify_api.py and local_file_scanner.py) do not exist in production."""
        zombies = [
            self.services_dir / "spotify_api.py",
            self.services_dir / "local_file_scanner.py",
        ]
        for zombie in zombies:
            self.assertFalse(
                zombie.exists(),
                f"Zombie module must not exist in production services: {zombie}",
            )

    def test_build_m3u_content_formatting(self):
        """Ensure build_m3u_content formats valid M3U entries with #EXTM3U and file paths."""
        tracks = [
            {
                "id": "trk-1",
                "title": "Bohemian Rhapsody",
                "artist": "Queen",
                "album": "A Night at the Opera",
                "duration_ms": 354000,
                "file_path": "/music/Queen/Bohemian Rhapsody.flac",
            },
            {
                "id": "trk-2",
                "title": "Stairway to Heaven",
                "artist": "Led Zeppelin",
                "album": "Led Zeppelin IV",
                "duration_ms": 482000,
                "file_path": "/music/Led Zeppelin/Stairway to Heaven.flac",
            },
        ]

        m3u_text = playlist_bridge.build_m3u_content(tracks)
        lines = [line for line in m3u_text.splitlines() if line.strip()]

        self.assertEqual(lines[0], "#EXTM3U")
        self.assertEqual(lines[1], "#EXTINF:354,Queen - Bohemian Rhapsody")
        self.assertEqual(lines[2], "/music/Queen/Bohemian Rhapsody.flac")
        self.assertEqual(lines[3], "#EXTINF:482,Led Zeppelin - Stairway to Heaven")
        self.assertEqual(lines[4], "/music/Led Zeppelin/Stairway to Heaven.flac")

    def test_export_local_json_playlist_to_m3u_and_m3u8(self):
        """Ensure export_playlist_data exports local JSON playlists to M3U and M3U8."""
        sample_tracks = [
            {
                "id": "1",
                "title": "Song One",
                "artist": "Artist A",
                "duration_ms": 120000,
                "file_path": "/path/to/song_one.flac",
            },
            {
                "id": "2",
                "title": "Song Two",
                "artist": "Artist B",
                "duration_ms": 200000,
                "file_path": "/path/to/song_two.flac",
            },
        ]

        with tempfile.TemporaryDirectory() as tmpdir:
            json_file = Path(tmpdir) / "playlist.json"
            json_file.write_text(json.dumps({"tracks": sample_tracks}), encoding="utf-8")

            # Test M3U export
            out_m3u = Path(tmpdir) / "exported.m3u"
            res_m3u = playlist_bridge.export_playlist_data(
                service="local",
                playlist_id=str(json_file),
                format_type="m3u",
                output_path=str(out_m3u),
            )

            self.assertEqual(res_m3u["format"], "m3u")
            self.assertEqual(res_m3u["track_count"], 2)
            self.assertTrue(out_m3u.is_file())
            content_m3u = out_m3u.read_text(encoding="utf-8")
            self.assertIn("#EXTM3U", content_m3u)
            self.assertIn("#EXTINF:120,Artist A - Song One", content_m3u)
            self.assertIn("/path/to/song_one.flac", content_m3u)

            # Test M3U8 export
            out_m3u8 = Path(tmpdir) / "exported.m3u8"
            res_m3u8 = playlist_bridge.export_playlist_data(
                service="local",
                playlist_id=str(json_file),
                format_type="m3u8",
                output_path=str(out_m3u8),
            )

            self.assertEqual(res_m3u8["format"], "m3u8")
            self.assertEqual(res_m3u8["track_count"], 2)
            self.assertTrue(out_m3u8.is_file())
            content_m3u8 = out_m3u8.read_text(encoding="utf-8")
            self.assertIn("#EXTM3U", content_m3u8)
            self.assertIn("#EXTINF:200,Artist B - Song Two", content_m3u8)
            self.assertIn("/path/to/song_two.flac", content_m3u8)

    def test_export_local_sqlite_db_playlist(self):
        """Ensure local playlists stored in SQLite database export cleanly to M3U."""
        with tempfile.TemporaryDirectory() as tmpdir:
            db_path = Path(tmpdir) / "syncify.db"
            conn = sqlite3.connect(str(db_path))
            cur = conn.cursor()

            cur.executescript("""
                CREATE TABLE playlists (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    service_playlist_id TEXT,
                    name TEXT NOT NULL,
                    description TEXT,
                    track_count INTEGER DEFAULT 0
                );
                CREATE TABLE tracks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    title TEXT NOT NULL,
                    artist_name TEXT,
                    album_title TEXT,
                    duration_ms INTEGER,
                    isrc TEXT
                );
                CREATE TABLE downloads (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    track_id INTEGER UNIQUE,
                    file_path TEXT
                );
                CREATE TABLE playlist_tracks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    playlist_id INTEGER,
                    track_id INTEGER,
                    position INTEGER DEFAULT 0
                );

                INSERT INTO playlists (id, service_playlist_id, name, description, track_count)
                VALUES (1, 'pl-local-01', 'Favorites', 'My favorites', 2);

                INSERT INTO tracks (id, title, artist_name, album_title, duration_ms, isrc)
                VALUES (101, 'DB Song 1', 'DB Artist 1', 'Album 1', 180000, 'US12345'),
                       (102, 'DB Song 2', 'DB Artist 2', 'Album 2', 240000, 'US67890');

                INSERT INTO downloads (track_id, file_path)
                VALUES (101, '/storage/music/db_song_1.flac'),
                       (102, '/storage/music/db_song_2.flac');

                INSERT INTO playlist_tracks (playlist_id, track_id, position)
                VALUES (1, 101, 1),
                       (1, 102, 2);
            """)
            conn.commit()
            conn.close()

            old_env = os.environ.get("SYNCIFY_DB_PATH")
            try:
                os.environ["SYNCIFY_DB_PATH"] = str(db_path)

                export_res = playlist_bridge.export_playlist_data(
                    service="local",
                    playlist_id="1",
                    format_type="m3u",
                )

                self.assertEqual(export_res["track_count"], 2)
                content = export_res["content"]
                self.assertIn("#EXTM3U", content)
                self.assertIn("#EXTINF:180,DB Artist 1 - DB Song 1", content)
                self.assertIn("/storage/music/db_song_1.flac", content)
                self.assertIn("#EXTINF:240,DB Artist 2 - DB Song 2", content)
                self.assertIn("/storage/music/db_song_2.flac", content)

                # Test listing local playlists
                local_pls = playlist_bridge.get_local_playlists()
                self.assertEqual(len(local_pls), 1)
                self.assertEqual(local_pls[0]["name"], "Favorites")

            finally:
                if old_env is not None:
                    os.environ["SYNCIFY_DB_PATH"] = old_env
                else:
                    os.environ.pop("SYNCIFY_DB_PATH", None)

    def test_cli_export_command_subprocesses(self):
        """Verify invoking playlist_bridge CLI export command returns valid JSON without crashing."""
        sample_tracks = [
            {
                "id": "10",
                "title": "Subprocess Track",
                "artist": "Test Artist",
                "duration_ms": 300000,
                "file_path": "/music/test_artist/test_track.flac",
            }
        ]

        with tempfile.TemporaryDirectory() as tmpdir:
            json_file = Path(tmpdir) / "cli_playlist.json"
            json_file.write_text(json.dumps({"tracks": sample_tracks}), encoding="utf-8")

            # Execute CLI command
            cmd = [
                sys.executable,
                str(self.scripts_dir / "playlist_bridge.py"),
                "export",
                "local",
                str(json_file),
                "--format",
                "m3u",
            ]
            proc = subprocess.run(cmd, capture_output=True, text=True)

            self.assertEqual(
                proc.returncode,
                0,
                f"CLI command failed with code {proc.returncode}. Stderr: {proc.stderr}",
            )

            data = json.loads(proc.stdout)
            self.assertTrue(data.get("success"))
            payload = data.get("data", {})
            self.assertEqual(payload.get("format"), "m3u")
            self.assertIn("#EXTM3U", payload.get("content", ""))
            self.assertIn("/music/test_artist/test_track.flac", payload.get("content", ""))


if __name__ == "__main__":
    unittest.main()

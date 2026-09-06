#!/usr/bin/env python3
"""
Contract and Regression Test Suite for download_bridge.py (TASK-41).

Validates the five failure levels of the original diagnosis plus the Rust CLI
contract (src-tauri/src/commands/tools.rs :: download_track / run_bridge_command):
1. Canonical quality mapping to DownloadQuality enum (including compatibility aliases).
2. ServiceCredentials contract instantiation and backward compatibility parameters.
3. DownloadResult attribute contracts (filepath vs file_path, file_size_bytes vs
   size_bytes) AND legacy constructor kwargs (file_path/file_size/format/quality).
4. Asynchronous execution loop and HANDLERS registry integrity (everything runs
   via asyncio.run / awaited coroutines).
5. Structured JSON response contract for success, failure, and exception states.
6. CLI argument parsing, including optional 'download' subcommand handling.
7. Missing credentials validation per streaming service (clean JSON, no traceback).
8. stdout hygiene: stdout carries exactly one JSON document; incidental service
   output is diverted to stderr (Rust parses the whole stdout as JSON).
9. Exit code coherence: 0 success, 1 failure, 2 usage.
"""

import asyncio
import inspect
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import AsyncMock, patch

# Add scripts directory to sys.path
SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from services.service_base import (
    DownloadQuality,
    DownloadResult,
    DownloadStatus,
    ServiceCredentials,
    ServiceType,
)
import download_bridge
from download_bridge import (
    HANDLERS,
    async_execute_download,
    async_main,
    build_response,
    map_quality,
    resolve_output_path,
)


class TestDownloadBridgeContract(unittest.TestCase):
    """Test contract compliance between download_bridge.py and service_base.py."""

    def test_quality_mapping(self):
        """Verify mapping of string quality specifiers to canonical DownloadQuality enums."""
        # Lossless / CD mappings
        self.assertEqual(map_quality("lossless"), DownloadQuality.LOSSLESS_CD)
        self.assertEqual(map_quality("cd"), DownloadQuality.LOSSLESS_CD)
        self.assertEqual(map_quality("flac"), DownloadQuality.LOSSLESS_CD)
        self.assertEqual(map_quality("lossless_cd"), DownloadQuality.LOSSLESS_CD)

        # Hi-Res mappings
        self.assertEqual(map_quality("hires"), DownloadQuality.LOSSLESS_HIRES)
        self.assertEqual(map_quality("hi_res"), DownloadQuality.LOSSLESS_HIRES)
        self.assertEqual(map_quality("hi-res"), DownloadQuality.LOSSLESS_HIRES)
        self.assertEqual(map_quality("hifi"), DownloadQuality.LOSSLESS_HIRES)
        self.assertEqual(map_quality("lossless_hires"), DownloadQuality.LOSSLESS_HIRES)

        # 96kHz Hi-Res
        self.assertEqual(map_quality("hires_96"), DownloadQuality.LOSSLESS_HIRES_96)
        self.assertEqual(map_quality("hi_res_96"), DownloadQuality.LOSSLESS_HIRES_96)
        self.assertEqual(map_quality("lossless_hires_96"), DownloadQuality.LOSSLESS_HIRES_96)

        # Standard lossy
        self.assertEqual(map_quality("high"), DownloadQuality.LOSSY_STANDARD)
        self.assertEqual(map_quality("standard"), DownloadQuality.LOSSY_STANDARD)
        self.assertEqual(map_quality("320"), DownloadQuality.LOSSY_STANDARD)
        self.assertEqual(map_quality("lossy_standard"), DownloadQuality.LOSSY_STANDARD)

        # Low lossy
        self.assertEqual(map_quality("low"), DownloadQuality.LOSSY_LOW)
        self.assertEqual(map_quality("128"), DownloadQuality.LOSSY_LOW)
        self.assertEqual(map_quality("lossy_low"), DownloadQuality.LOSSY_LOW)

        # Fallback default
        self.assertEqual(map_quality("nonexistent_quality"), DownloadQuality.LOSSLESS_CD)
        self.assertEqual(map_quality(""), DownloadQuality.LOSSLESS_CD)

        # Pass-through if already enum
        self.assertEqual(map_quality(DownloadQuality.LOSSY_STANDARD), DownloadQuality.LOSSY_STANDARD)

    def test_download_quality_enum_aliases(self):
        """Verify backward compatibility aliases on DownloadQuality enum in service_base.py."""
        self.assertEqual(DownloadQuality.LOSSLESS, DownloadQuality.LOSSLESS_CD)
        self.assertEqual(DownloadQuality.HI_RES, DownloadQuality.LOSSLESS_HIRES)
        self.assertEqual(DownloadQuality.HI_RES_24_96, DownloadQuality.LOSSLESS_HIRES_96)
        self.assertEqual(DownloadQuality.HIGH, DownloadQuality.LOSSY_STANDARD)
        self.assertEqual(DownloadQuality.STANDARD, DownloadQuality.LOSSY_STANDARD)

    def test_service_credentials_contract(self):
        """Verify ServiceCredentials accepts required and backward-compatibility arguments."""
        # Standard typed construction
        creds = ServiceCredentials(
            service_type=ServiceType.QOBUZ,
            username="user@example.com",
            password="secret_password",
            token="token_abc",
            refresh_token="ref_123",
            client_id="cid_99",
            client_secret="csec_88",
            extra={"custom_key": "val"},
        )
        self.assertEqual(creds.service_type, ServiceType.QOBUZ)
        self.assertEqual(creds.username, "user@example.com")
        self.assertEqual(creds.password, "secret_password")
        self.assertEqual(creds.token, "token_abc")
        self.assertEqual(creds.refresh_token, "ref_123")
        self.assertEqual(creds.client_id, "cid_99")
        self.assertEqual(creds.client_secret, "csec_88")
        self.assertEqual(creds.extra.get("custom_key"), "val")

        # Compatibility keyword arguments: access_token, arl, app_id, app_secret
        compat_creds = ServiceCredentials(
            service_type=ServiceType.TIDAL,
            access_token="legacy_token",
            arl="legacy_arl",
            app_id="legacy_app_id",
            app_secret="legacy_app_sec",
        )
        self.assertEqual(compat_creds.token, "legacy_token")
        self.assertEqual(compat_creds.client_id, "legacy_app_id")
        self.assertEqual(compat_creds.client_secret, "legacy_app_sec")
        self.assertEqual(compat_creds.extra.get("arl"), "legacy_arl")
        self.assertEqual(compat_creds.extra.get("app_id"), "legacy_app_id")
        self.assertEqual(compat_creds.extra.get("app_secret"), "legacy_app_sec")

    def test_download_result_contract(self):
        """Verify DownloadResult dual access: canonical (filepath/file_size_bytes) and legacy (file_path/size_bytes)."""
        result = DownloadResult(
            success=True,
            filepath="/tmp/test_song.flac",
            file_size_bytes=10485760,
            download_duration_seconds=3.14,
            status=DownloadStatus.COMPLETED,
        )
        self.assertTrue(result.success)
        self.assertEqual(result.filepath, "/tmp/test_song.flac")
        self.assertEqual(result.file_path, "/tmp/test_song.flac")
        self.assertEqual(result.file_size_bytes, 10485760)
        self.assertEqual(result.size_bytes, 10485760)
        self.assertEqual(result.download_duration_seconds, 3.14)

        # Verify property setters
        result.file_path = "/tmp/updated.flac"
        result.size_bytes = 20971520
        self.assertEqual(result.filepath, "/tmp/updated.flac")
        self.assertEqual(result.file_size_bytes, 20971520)

    def test_handlers_registry_integrity(self):
        """Verify HANDLERS maps all expected streaming services to async coroutine functions."""
        expected_services = {"qobuz", "tidal", "deezer", "soundcloud"}
        self.assertEqual(set(HANDLERS.keys()), expected_services)
        for srv, fn in HANDLERS.items():
            self.assertTrue(
                inspect.iscoroutinefunction(fn),
                f"Handler for {srv} must be an async coroutine function",
            )

    def test_resolve_output_path_handling(self):
        """Verify resolve_output_path converts directories to explicit filenames and preserves files."""
        # When passed a directory or path without suffix
        resolved = resolve_output_path("/tmp/syncify_test_dir", "qobuz", "9988", DownloadQuality.LOSSLESS_CD)
        self.assertTrue(resolved.endswith("qobuz_9988.flac"))

        # When passed lossy quality
        resolved_mp3 = resolve_output_path("/tmp/syncify_test_dir", "deezer", "5544", DownloadQuality.LOSSY_STANDARD)
        self.assertTrue(resolved_mp3.endswith("deezer_5544.mp3"))

        # When passed an explicit file path with extension
        resolved_explicit = resolve_output_path("/tmp/syncify_test_dir/my_track.flac", "tidal", "123", DownloadQuality.LOSSLESS_CD)
        self.assertEqual(resolved_explicit, "/tmp/syncify_test_dir/my_track.flac")

    def test_async_execute_download_success(self):
        """Verify successful download returns standardized JSON payload."""
        mock_result = DownloadResult(
            success=True,
            filepath="/tmp/syncify_downloads/track_123.flac",
            file_size_bytes=15728640,
            download_duration_seconds=2.45,
            status=DownloadStatus.COMPLETED,
        )

        mock_handler = AsyncMock(return_value=mock_result)
        with patch.dict(HANDLERS, {"qobuz": mock_handler}):
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:
                response = loop.run_until_complete(
                    async_execute_download("qobuz", "123", "/tmp/syncify_downloads", "lossless")
                )
            finally:
                loop.close()

            self.assertTrue(response["success"])
            self.assertIn("data", response)
            data = response["data"]
            self.assertEqual(data["file_path"], "/tmp/syncify_downloads/track_123.flac")
            self.assertEqual(data["filepath"], "/tmp/syncify_downloads/track_123.flac")
            self.assertEqual(data["size_bytes"], 15728640)
            self.assertEqual(data["file_size_bytes"], 15728640)
            self.assertEqual(data["format"], "flac")
            self.assertEqual(data["download_duration_seconds"], 2.45)
            mock_handler.assert_awaited_once_with("123", "/tmp/syncify_downloads", "lossless")

    def test_async_execute_download_failure(self):
        """Verify failed download returns success: false with error description."""
        mock_result = DownloadResult(
            success=False,
            error_message="Track unavailable in user's territory",
            status=DownloadStatus.FAILED,
        )

        mock_handler = AsyncMock(return_value=mock_result)
        with patch.dict(HANDLERS, {"tidal": mock_handler}):
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:
                response = loop.run_until_complete(
                    async_execute_download("tidal", "456", "/tmp/syncify_downloads", "hires")
                )
            finally:
                loop.close()

            self.assertFalse(response["success"])
            self.assertNotIn("data", response)
            self.assertEqual(response["error"], "Track unavailable in user's territory")

    def test_async_execute_download_exception_caught(self):
        """Verify exceptions during download execution are caught and wrapped in failure payload."""
        mock_handler = AsyncMock(side_effect=ConnectionResetError("Remote server closed connection"))
        with patch.dict(HANDLERS, {"deezer": mock_handler}):
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:
                response = loop.run_until_complete(
                    async_execute_download("deezer", "789", "/tmp/syncify_downloads", "lossless")
                )
            finally:
                loop.close()

            self.assertFalse(response["success"])
            self.assertIn("Remote server closed connection", response["error"])

    def test_async_execute_download_unsupported_service(self):
        """Verify unsupported service name returns structured error payload."""
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            response = loop.run_until_complete(
                async_execute_download("invalid_service", "000", "/tmp", "lossless")
            )
        finally:
            loop.close()

        self.assertFalse(response["success"])
        self.assertIn("Unsupported service", response["error"])

    def test_cli_download_subcommand_compatibility(self):
        """Verify CLI correctly strips optional 'download' subcommand invoked by Rust Tauri backend."""
        mock_handler = AsyncMock(
            return_value=DownloadResult(
                success=True,
                filepath="/tmp/song.flac",
                file_size_bytes=1000,
                status=DownloadStatus.COMPLETED,
            )
        )

        with patch.dict(HANDLERS, {"qobuz": mock_handler}):
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:
                # Emulate CLI call: python download_bridge.py download qobuz 9999 --quality hires
                payload, exit_code = loop.run_until_complete(
                    async_main(["download", "qobuz", "9999", "--quality", "hires", "--output", "/tmp/out.flac"])
                )
            finally:
                loop.close()

            self.assertEqual(exit_code, 0)
            mock_handler.assert_awaited_once_with("9999", "/tmp/out.flac", "hires")
            self.assertTrue(payload["success"])
            self.assertEqual(payload["data"]["file_path"], "/tmp/song.flac")
            # Serialized form matches the Rust DownloadBridgeResult contract keys
            encoded = json.dumps(payload)
            self.assertIn('"success": true', encoded)
            self.assertIn('"file_path"', encoded)

    def test_download_result_legacy_init_kwargs(self):
        """Level-4 regression: soundcloud_service.py builds DownloadResult with legacy kwargs
        (file_path, file_size, format, quality) and omits success."""
        result = DownloadResult(
            file_path="/tmp/sc_track.mp3",
            file_size=2048,
            format="MP3",
            quality=DownloadQuality.LOSSLESS,
        )
        # success defaults to True for legacy success-path constructions
        self.assertTrue(result.success)
        self.assertEqual(result.filepath, "/tmp/sc_track.mp3")
        self.assertEqual(result.file_path, "/tmp/sc_track.mp3")
        self.assertEqual(result.file_size_bytes, 2048)
        self.assertEqual(result.size_bytes, 2048)
        self.assertEqual(result.format, "MP3")
        self.assertEqual(result.quality, DownloadQuality.LOSSLESS)

        # Canonical construction still behaves, including explicit failure
        canonical = DownloadResult(success=False, error_message="boom")
        self.assertFalse(canonical.success)
        self.assertEqual(canonical.error_message, "boom")

    def test_async_execute_download_soundcloud_legacy_result(self):
        """Legacy SoundCloud DownloadResult flows through the pipeline into the Rust JSON contract."""
        mock_handler = AsyncMock(
            return_value=DownloadResult(
                file_path="/tmp/syncify_downloads/sc_777.mp3",
                file_size=4096,
                format="MP3",
                quality=DownloadQuality.LOSSLESS,
            )
        )
        with patch.dict(HANDLERS, {"soundcloud": mock_handler}):
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:
                response = loop.run_until_complete(
                    async_execute_download("soundcloud", "777", "/tmp/syncify_downloads", "low")
                )
            finally:
                loop.close()

        self.assertTrue(response["success"])
        data = response["data"]
        self.assertEqual(data["file_path"], "/tmp/syncify_downloads/sc_777.mp3")
        self.assertEqual(data["filepath"], "/tmp/syncify_downloads/sc_777.mp3")
        self.assertEqual(data["size_bytes"], 4096)
        self.assertEqual(data["file_size_bytes"], 4096)
        self.assertEqual(data["format"], "mp3")

    def test_cli_service_flag_style(self):
        """Acceptance criterion form: 'python download_bridge.py --service qobuz ...' works too."""
        mock_handler = AsyncMock(
            return_value=DownloadResult(success=True, filepath="/tmp/flag.flac", file_size_bytes=5)
        )
        with patch.dict(HANDLERS, {"qobuz": mock_handler}):
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:
                payload, exit_code = loop.run_until_complete(
                    async_main(["--service", "qobuz", "555", "--output", "/tmp/flag.flac"])
                )
            finally:
                loop.close()

        self.assertEqual(exit_code, 0)
        mock_handler.assert_awaited_once_with("555", "/tmp/flag.flac", "lossless")
        self.assertTrue(payload["success"])

    def test_cli_missing_track_id_structured_error(self):
        """Missing track_id yields a structured error payload with exit code 2."""
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            payload, exit_code = loop.run_until_complete(async_main(["qobuz"]))
        finally:
            loop.close()
        self.assertEqual(exit_code, 2)
        self.assertFalse(payload["success"])
        self.assertIn("track_id", payload["error"])

    def test_main_stdout_hygiene_and_exit_codes(self):
        """Rust parses the WHOLE stdout as one JSON value: service noise must go to stderr."""
        async def noisy_handler(track_id, output_path, quality):
            print("SERVICE NOISE on stdout")
            sys.stderr.write("SERVICE WARN on stderr\n")
            return DownloadResult(
                file_path="/tmp/hygiene.flac",
                file_size=256,
                format="FLAC",
                quality=DownloadQuality.LOSSLESS,
            )

        fake_out, fake_err = io.StringIO(), io.StringIO()
        argv = ["download_bridge.py", "download", "qobuz", "42", "--output", "/tmp/hygiene.flac"]
        with patch.dict(HANDLERS, {"qobuz": noisy_handler}), \
                patch("sys.stdout", fake_out), \
                patch("sys.stderr", fake_err), \
                patch("sys.argv", argv):
            with self.assertRaises(SystemExit) as ctx:
                download_bridge.main()

        self.assertEqual(ctx.exception.code, 0)
        stdout_text = fake_out.getvalue()
        # Whole stdout must be exactly one JSON document
        payload = json.loads(stdout_text)
        self.assertTrue(payload["success"])
        self.assertEqual(payload["data"]["file_path"], "/tmp/hygiene.flac")
        self.assertEqual(payload["data"]["format"], "flac")
        self.assertNotIn("SERVICE NOISE", stdout_text)
        self.assertIn("SERVICE NOISE on stdout", fake_err.getvalue())
        self.assertIn("SERVICE WARN on stderr", fake_err.getvalue())

    def test_main_failure_exit_code_and_structured_error(self):
        """Failures emit structured JSON on stdout with exit code 1 (never a traceback)."""
        mock_handler = AsyncMock(side_effect=RuntimeError("auth exploded"))
        fake_out, fake_err = io.StringIO(), io.StringIO()
        argv = ["download_bridge.py", "tidal", "7", "--output", "/tmp/x.flac"]
        with patch.dict(HANDLERS, {"tidal": mock_handler}), \
                patch("sys.stdout", fake_out), \
                patch("sys.stderr", fake_err), \
                patch("sys.argv", argv):
            with self.assertRaises(SystemExit) as ctx:
                download_bridge.main()

        self.assertEqual(ctx.exception.code, 1)
        payload = json.loads(fake_out.getvalue())
        self.assertFalse(payload["success"])
        self.assertIn("auth exploded", payload["error"])

    def test_async_main_invalid_arguments_exit_code(self):
        """Unknown service returns exit code 2 plus a structured (non-crashing) payload."""
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            payload, exit_code = loop.run_until_complete(
                async_main(["download", "not_a_service", "1"])
            )
        finally:
            loop.close()
        self.assertEqual(exit_code, 2)
        self.assertFalse(payload["success"])
        self.assertIn("Unsupported service", payload["error"])
        self.assertIn("not_a_service", payload["error"])

    def test_cli_subprocess_smoke_without_credentials(self):
        """End-to-end smoke test exactly as Rust invokes it: 'download' subcommand, no
        credentials available -> structured JSON error, exit code 1, no traceback."""
        env = os.environ.copy()
        for key in (
            "QOBUZ_USERNAME", "QOBUZ_PASSWORD", "QOBUZ_APP_ID", "QOBUZ_APP_SECRET",
            "TIDAL_ACCESS_TOKEN", "TIDAL_REFRESH_TOKEN", "DEEZER_ARL",
            "SOUNDCLOUD_AUTH_TOKEN", "SOUNDCLOUD_CLIENT_ID",
        ):
            env[key] = ""  # blank out; dotenv (override=False) will not restore them

        # Resolve the interpreter exactly like Rust's get_python_executable():
        # via PATH. (sys.executable can point to an AppImage wrapper that would
        # launch a GUI process instead of running the script.)
        python_exe = shutil.which("python3") or shutil.which("python") or sys.executable
        proc = subprocess.run(
            [
                python_exe,
                str(SCRIPTS_DIR / "download_bridge.py"),
                "download", "qobuz", "12345",
                "--output", tempfile.gettempdir(),
                "--quality", "lossless",
            ],
            capture_output=True,
            text=True,
            env=env,
            cwd=str(SCRIPTS_DIR.parent),
            timeout=120,
        )

        self.assertEqual(proc.returncode, 1)
        # Whole stdout parses as a single JSON document
        payload = json.loads(proc.stdout)
        self.assertFalse(payload["success"])
        self.assertIn("QOBUZ_USERNAME", payload["error"])
        self.assertNotIn("Traceback", proc.stderr)

    def test_missing_credentials_raise_clean_error(self):
        """Verify missing required credentials raise descriptive ValueErrors."""
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            # Qobuz without credentials
            with patch.dict("os.environ", {}, clear=True):
                with self.assertRaises(ValueError) as ctx:
                    loop.run_until_complete(download_bridge.get_qobuz_service())
                self.assertIn("QOBUZ_USERNAME", str(ctx.exception))

            # Tidal without credentials
            with patch.dict("os.environ", {}, clear=True):
                with self.assertRaises(ValueError) as ctx:
                    loop.run_until_complete(download_bridge.get_tidal_service())
                self.assertIn("TIDAL_ACCESS_TOKEN", str(ctx.exception))

            # Deezer without credentials
            with patch.dict("os.environ", {}, clear=True):
                with self.assertRaises(ValueError) as ctx:
                    loop.run_until_complete(download_bridge.get_deezer_service())
                self.assertIn("DEEZER_ARL", str(ctx.exception))
        finally:
            loop.close()


if __name__ == "__main__":
    unittest.main()

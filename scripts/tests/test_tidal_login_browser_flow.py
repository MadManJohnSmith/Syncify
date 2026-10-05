#!/usr/bin/env python3
"""R2: el login de Tidal tiene que abrir el navegador, y si no, decirlo.

`login_with_device_code` ignoraba el valor de retorno de `webbrowser.open()`.
Esa función devuelve False —sin excepción— cuando no encuentra `BROWSER` ni
`xdg-open`, así que el fallo se traducía en 120 s de silencio y un timeout
genérico de Rust (R2, auditoría 37). Aquí se fija el contrato roto:

1. Se comprueba si el navegador se abrió de verdad; si no, el flujo lo dice y
   devuelve la URL de verificación en vez de quedarse sondeando en silencio.
2. La URL de verificación viaja en la respuesta del puente (éxito y fallo), que
   es el único canal por el que el usuario puede terminar la conexión a mano.
3. `open_in_system_browser` degrada por plataforma cuando `webbrowser` falla.
"""

import asyncio
import io
import json
import subprocess
import sys
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import MagicMock, patch

try:
    import aiohttp  # noqa: F401
except ImportError:
    # El device code flow sólo necesita aiohttp para hablar con auth.tidal.com;
    # estas pruebas sustituyen la sesión, así que un stub basta.
    sys.modules["aiohttp"] = MagicMock()

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import auth_bridge  # noqa: E402
from services import browser_launcher  # noqa: E402
from services.tidal_auth import TidalAuth  # noqa: E402

VERIFICATION_URL = "https://link.tidal.com/ABC123"
DEVICE_CODE = "device-code-abc123"


class _FakeSession:
    """Sesión aiohttp mínima: `close()` es lo único que el flujo necesita."""

    def __init__(self):
        self.closed = False

    async def close(self):
        self.closed = True


class TidalLoginBrowserTests(unittest.TestCase):
    """R2: el device code flow tiene que abrir el navegador o declararlo."""

    def _auth_with_device_code(self):
        auth = TidalAuth(verbose=False)

        async def fake_device_code():
            return DEVICE_CODE, VERIFICATION_URL

        auth._get_device_code = fake_device_code
        return auth

    def _run_login(self, auth, open_result, opener_side_effect=None):
        # `login_with_device_code` abre su propia ClientSession, así que el
        # doble de aiohttp debe devolver la falsa para que `close()` sea
        # esperable.
        with patch(
            "services.tidal_auth.aiohttp.ClientSession",
            return_value=_FakeSession(),
        ), patch.object(
            browser_launcher,
            "open_in_system_browser",
            side_effect=opener_side_effect,
            return_value=open_result,
        ):
            stderr = io.StringIO()
            with redirect_stderr(stderr):
                success, message, device_info = asyncio.run(
                    auth.login_with_device_code(timeout_seconds=4)
                )
        return success, message, device_info, stderr.getvalue()

    def test_browser_opened_keeps_polling(self):
        """Si el navegador se abre, el flujo sigue sondeando la autorización."""
        auth = self._auth_with_device_code()

        async def pending_status(device_code):
            # 2 = pending: el usuario todavía no confirmó en el navegador.
            await asyncio.sleep(0)
            return 2, {}

        auth._get_auth_status = pending_status

        success, _message, device_info, stderr = self._run_login(auth, True)

        self.assertFalse(success, "sin confirmación del usuario el flujo no puede tener éxito")
        self.assertEqual(device_info["verification_url"], VERIFICATION_URL)
        self.assertIn(VERIFICATION_URL, stderr, "la URL debe quedar en stderr aunque el flujo no termine aquí")

    def test_browser_not_opened_reports_url_instead_of_waiting_silently(self):
        """Si el escritorio no abre nada, se devuelve la URL y no se sigue esperando."""
        auth = self._auth_with_device_code()
        polled = []

        async def never_called(device_code):
            polled.append(device_code)
            return 2, {}

        auth._get_auth_status = never_called

        success, message, device_info, _stderr = self._run_login(auth, False)

        self.assertFalse(success)
        self.assertIn(VERIFICATION_URL, message, "el error debe decir qué URL abrir")
        self.assertEqual(device_info["verification_url"], VERIFICATION_URL)
        self.assertEqual(polled, [], "sin pestaña abierta no hay nada que sondear")

    def test_browser_exception_is_surfaced_not_swallowed(self):
        """Una excepción al abrir el navegador tampoco puede volverse silencio."""
        auth = self._auth_with_device_code()

        success, message, device_info, _stderr = self._run_login(
            auth, False, opener_side_effect=RuntimeError("no hay navegador")
        )

        self.assertFalse(success)
        self.assertIn(VERIFICATION_URL, message)
        self.assertEqual(device_info["verification_url"], VERIFICATION_URL)


class OpenInSystemBrowserTests(unittest.TestCase):
    """El lanzador comprueba el resultado en vez de asumirlo."""

    def test_returns_true_when_webbrowser_succeeds(self):
        with patch("webbrowser.open", return_value=True) as opener:
            self.assertTrue(browser_launcher.open_in_system_browser("https://example.test"))
        opener.assert_called_once_with("https://example.test", new=2)

    def test_falls_back_to_platform_opener_when_webbrowser_returns_false(self):
        completed = type("Completed", (), {"returncode": 0})()

        def fake_run(argv, **kwargs):
            self.assertEqual(argv[-1], "https://example.test")
            return completed

        with patch("webbrowser.open", return_value=False), patch.object(
            browser_launcher.shutil, "which", side_effect=lambda name: f"/usr/bin/{name}"
        ), patch.object(browser_launcher.subprocess, "run", fake_run):
            self.assertTrue(browser_launcher.open_in_system_browser("https://example.test"))

    def test_returns_false_when_nothing_can_open_the_url(self):
        with patch("webbrowser.open", return_value=False), patch.object(
            browser_launcher.shutil, "which", return_value=None
        ):
            self.assertFalse(browser_launcher.open_in_system_browser("https://example.test"))


class TidalBridgeReportsVerificationUrlTests(unittest.TestCase):
    """auth_bridge.py debe devolver la URL aunque el login no se complete."""

    def _capture_response(self, tidal_login_result):
        stdout = io.StringIO()
        with patch(
            "services.tidal_auth.tidal_login", return_value=tidal_login_result
        ), patch("services.tidal_auth.TidalAuth.get_stored_tokens", return_value=None):
            with redirect_stdout(stdout):
                with self.assertRaises(SystemExit) as ctx:
                    auth_bridge.handle_tidal("login")
        return json.loads(stdout.getvalue()), ctx.exception.code

    def test_failed_login_still_returns_verification_url(self):
        result = {
            "status": "error",
            "message": f"No se pudo abrir el navegador automáticamente. Abre esta URL: {VERIFICATION_URL}",
            "device_info": {"verification_url": VERIFICATION_URL},
            "tokens": None,
        }

        payload, code = self._capture_response(result)

        self.assertEqual(code, 1)
        self.assertFalse(payload["success"])
        self.assertEqual(payload["data"]["verification_url"], VERIFICATION_URL)
        self.assertIn(VERIFICATION_URL, payload["error"])

    def test_successful_login_returns_verification_url(self):
        result = {
            "status": "success",
            "message": "Successfully connected to Tidal",
            "device_info": {"verification_url": VERIFICATION_URL},
            "tokens": {"access_token": "at", "refresh_token": "rt", "user_id": 7},
        }

        payload, code = self._capture_response(result)

        self.assertEqual(code, 0)
        self.assertTrue(payload["success"])
        self.assertEqual(payload["data"]["verification_url"], VERIFICATION_URL)


if __name__ == "__main__":
    unittest.main()

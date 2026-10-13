#!/usr/bin/env python3
"""Contrato de identidad del login Qobuz en `auth_bridge.py` (ruta de identidad).

Hallazgo auditado: `handle_qobuz` respondia `"display_name": result`, donde
`result` es lo que devuelve `login_with_browser` — el id numerico capturado, o
el marcador literal `"browser_session"` cuando no descubrio ninguno. Ese valor
acaba en `accounts.display_name` y la UI lo muestra ("Conectado como
123456789"), dejando dos cuentas de la misma persona indistinguibles y exponiendo
un identificador opaco que nadie escribio.

Estos tests fijan el contrato (hermeticos, sin red ni navegador):

1. `display_name` NUNCA es el id numerico ni `browser_session`.
2. El email capturado si puede ser nombre visible cuando es un email real.
3. Sin email utilizable, cae en el generico del servicio.
4. El id real sigue saliendo en `user_id` (identidad deduplicada de Rust) y el
   `username` sigue disponible para que Rust lo propague a email si es plausible.
"""

import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import auth_bridge  # noqa: E402

NUMERIC_ID = "123456789"
SESSION_MARKER = "browser_session"


class QobuzVisibleDisplayNameTest(unittest.TestCase):
    """El nombre visible jamas es un identificador opaco."""

    def test_numeric_provider_id_is_never_the_visible_name(self):
        name = auth_bridge.qobuz_visible_display_name(NUMERIC_ID, None)
        self.assertNotEqual(name, NUMERIC_ID)
        self.assertNotIn(NUMERIC_ID, name)

    def test_session_marker_is_never_the_visible_name(self):
        name = auth_bridge.qobuz_visible_display_name(SESSION_MARKER, None)
        self.assertNotEqual(name, SESSION_MARKER)

    def test_captured_email_is_a_readable_visible_name(self):
        name = auth_bridge.qobuz_visible_display_name(NUMERIC_ID, "  owner@example.com  ")
        self.assertEqual(name, "owner@example.com")

    def test_falls_back_to_the_service_label_without_an_email(self):
        self.assertEqual(
            auth_bridge.qobuz_visible_display_name(NUMERIC_ID, None), "Qobuz User"
        )
        self.assertEqual(
            auth_bridge.qobuz_visible_display_name(SESSION_MARKER, ""), "Qobuz User"
        )

    def test_a_username_without_domain_is_not_used_as_a_name(self):
        """Un nombre de usuario sin email no identifica a nadie de forma estable."""
        name = auth_bridge.qobuz_visible_display_name(NUMERIC_ID, "plainuser")
        self.assertEqual(name, "Qobuz User")
        self.assertNotEqual(name, "plainuser")

    def test_display_name_is_never_a_bare_digit_string(self):
        for candidate in (None, "", "not-an-email", "user", "12345"):
            name = auth_bridge.qobuz_visible_display_name(candidate, candidate)
            self.assertFalse(name.isdigit(), f"{candidate!r} produced a numeric name")
            self.assertTrue(name.strip())


class _FakeQobuzAuth:
    """Sustituye a QobuzAuth sin Playwright."""

    def __init__(self, session, captured_credentials=None):
        self._session = session
        self._captured_credentials = captured_credentials or {}

    def get_stored_session(self):
        return dict(self._session)

    async def login_with_browser(self, timeout_seconds: int = 300):
        return _FakeLoop.result


class _FakeLoop:
    """Sustituye al event loop del navegador."""

    result = (True, NUMERIC_ID)

    @classmethod
    def run_until_complete(cls, _coro):
        return cls.result

    @staticmethod
    def close():
        return None


class QobuzLoginPayloadTest(unittest.TestCase):
    """El payload de login conserva identidad y visibilidad."""

    def _run_login(self, result, session):
        """Invoca `handle_qobuz('login')` con el navegador y json_response sustituidos."""
        captured = {}

        def fake_json_response(success, data=None, error=None):
            captured["success"] = success
            captured["data"] = data
            captured["error"] = error
            raise SystemExit(0)

        with patch("services.qobuz_auth.QobuzAuth",
                   return_value=_FakeQobuzAuth(session)), \
             patch("asyncio.new_event_loop", return_value=_FakeLoop), \
             patch.object(auth_bridge, "json_response", fake_json_response):
            _FakeLoop.result = (True, result)
            try:
                auth_bridge.handle_qobuz("login")
            except SystemExit:
                pass
        return captured

    def test_display_name_is_not_the_numeric_id_even_without_email(self):
        captured = self._run_login(NUMERIC_ID, self._token_only_session())
        self.assertTrue(captured["success"], captured)
        self.assertNotEqual(captured["data"]["display_name"], NUMERIC_ID)

    def test_user_id_is_preserved_as_the_deduplicated_identity(self):
        captured = self._run_login(NUMERIC_ID, self._token_only_session())
        self.assertEqual(captured["data"]["user_id"], NUMERIC_ID)

    def test_session_marker_is_not_the_visible_name(self):
        captured = self._run_login(SESSION_MARKER, self._token_only_session())
        self.assertNotEqual(captured["data"]["display_name"], SESSION_MARKER)

    def test_username_is_still_forwarded_for_email_propagation(self):
        captured = self._run_login(
            NUMERIC_ID,
            {"username": "owner@example.com", "password": "placeholder-not-real"},
        )
        self.assertEqual(captured["data"]["username"], "owner@example.com")
        self.assertEqual(captured["data"]["display_name"], "owner@example.com")

    def test_payload_keeps_token_keys_expected_by_rust(self):
        captured = self._run_login(
            NUMERIC_ID, {"auth_token": "a" * 32, "username": None, "password": None}
        )
        data = captured["data"]
        self.assertEqual(data["user_auth_token"], "a" * 32)
        self.assertEqual(data["auth_token"], "a" * 32)

    def test_success_payload_is_valid_json_without_tracebacks(self):
        captured = self._run_login(NUMERIC_ID, self._token_only_session())
        self.assertIsNone(captured["error"], captured)
        json.dumps(captured["data"])

    @staticmethod
    def _token_only_session():
        """Token valido y sin username: el caso que producia el id como nombre."""
        return {"auth_token": "a" * 32, "username": None, "password": None}


if __name__ == "__main__":
    unittest.main()

class QobuzBrowserEntitlementLimitTest(unittest.TestCase):
    """Limite OBSERVADO del flujo de navegador (WF-QOB-01).

    `qobuz_service.py:215` lee `user.credential` de la respuesta de
    `user/login`, que es la UNICA evidencia real de suscripcion. En el flujo
    de navegador (`qobuz_auth.py`) la sesion se captura de localStorage /
    cabeceras XHR: no hay respuesta `user/login` a la vista, asi que no existe
    bloque `credential` que propagar. Inventarlo convertiria una cuenta de
    solo-token (entitlement desconocido) en "sin suscripcion".

    Estos tests fijan que el puente NUNCA afirma entitlement por su cuenta.
    """

    def _run_login(self, session):
        captured = {}

        def fake_json_response(success, data=None, error=None):
            captured["success"] = success
            captured["data"] = data
            captured["error"] = error
            raise SystemExit(0)

        with patch("services.qobuz_auth.QobuzAuth",
                   return_value=_FakeQobuzAuth(session)), \
             patch("asyncio.new_event_loop", return_value=_FakeLoop), \
             patch.object(auth_bridge, "json_response", fake_json_response):
            _FakeLoop.result = (True, NUMERIC_ID)
            try:
                auth_bridge.handle_qobuz("login")
            except SystemExit:
                pass
        return captured

    def test_browser_payload_never_asserts_streaming_entitlement(self):
        session = {
            "auth_token": "viable_browser_token_123456789012",
            "user_id": NUMERIC_ID,
            "username": "owner@example.com",
            "password": "correct-horse-battery",
        }
        captured = self._run_login(session)
        self.assertTrue(captured["success"], captured)
        data = captured["data"]
        # Unicamente token + username/password capturados: ninguna clave de
        # suscripcion, porque el navegador no la produce.
        for forbidden in ("credential", "streaming_entitled", "entitlement",
                          "lossless_streaming", "hires_streaming"):
            self.assertNotIn(
                forbidden,
                data,
                "el flujo de navegador no observa user.credential; afirmarlo seria inventar la suscripcion",
            )
        self.assertIn("user_auth_token", data)

    def test_token_only_session_still_reaches_rust_without_credential_block(self):
        session = {
            "auth_token": "viable_browser_token_123456789012",
            "user_id": NUMERIC_ID,
        }
        captured = self._run_login(session)
        data = captured["data"]
        self.assertTrue(captured["success"])
        self.assertEqual(data["user_auth_token"], "viable_browser_token_123456789012")
        self.assertIsNone(data.get("username"))
        self.assertIsNone(data.get("password"))
        self.assertNotIn("credential", data)

#!/usr/bin/env python3
"""
A-2: `validate_token` debe distinguir tres estados, no dos.

ANTES DEL FIX, todo lo que no fuera un 200 exacto (429, 500, 502, 503, 504...)
y toda excepción (timeout, DNS, JSON mal formado) caían en el mismo `else` /
`except` y devolvían `False`. El consumidor (`get_status`) traducía ese `False`
a la cadena "Token expired: ...". Es decir: **un 503 de Apple hacía que la app
dijera al usuario que su sesión había caducado**, cuando el token seguía
perfectamente vivo. Estos tests fallan contra esa versión.

El tercer valor (`None` = "no se pudo comprobar") es lo que arregla el
comportamiento observable de la UI.
"""

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from services.apple_music_auth import AppleMusicAuth  # noqa: E402


class _FakeResponse:
    def __init__(self, status_code, payload=None):
        self.status_code = status_code
        self._payload = payload if payload is not None else {}

    def json(self):
        if isinstance(self._payload, Exception):
            raise self._payload
        return self._payload


def _auth_with_dev_token():
    auth = AppleMusicAuth(verbose=False)
    auth._access_token = "dev-token"
    return auth


class AppleMusicVerdictTests(unittest.TestCase):
    """Tri-estado de `validate_token` y su reflejo en `get_status`."""

    # --- Los tres valores ---

    def test_http_200_is_a_real_valid_token(self):
        auth = _auth_with_dev_token()
        with patch(
            "services.apple_music_auth.requests.get",
            return_value=_FakeResponse(200, {"data": [{"id": "mx"}]}),
        ):
            is_valid, storefront = auth.validate_token("user-token")
        self.assertIs(is_valid, True)
        self.assertEqual(storefront, "mx")

    def test_401_and_403_are_real_rejections(self):
        for status in (401, 403):
            with self.subTest(status=status):
                auth = _auth_with_dev_token()
                with patch(
                    "services.apple_music_auth.requests.get",
                    return_value=_FakeResponse(status),
                ):
                    is_valid, _ = auth.validate_token("user-token")
                # Un rechazo real SIGUE siendo False: este es el comportamiento
                # que no se toca.
                self.assertIs(is_valid, False)

    def test_5xx_and_429_are_unverifiable_not_expired(self):
        # ESTE es el test que falla antes del fix: allí estos status devolvían
        # False igual que un 401 y la UI los pintaba como "Token expired".
        for status in (429, 500, 502, 503, 504):
            with self.subTest(status=status):
                auth = _auth_with_dev_token()
                with patch(
                    "services.apple_music_auth.requests.get",
                    return_value=_FakeResponse(status),
                ):
                    is_valid, message = auth.validate_token("user-token")
                self.assertIsNone(
                    is_valid,
                    f"HTTP {status} no puede probar que el token esté caducado",
                )
                self.assertIn("Could not verify", message)

    def test_a_network_exception_is_unverifiable_not_expired(self):
        auth = _auth_with_dev_token()
        with patch(
            "services.apple_music_auth.requests.get",
            side_effect=TimeoutError("timed out"),
        ):
            is_valid, message = auth.validate_token("user-token")
        self.assertIsNone(is_valid)
        self.assertIn("Could not verify", message)

    def test_a_missing_dev_token_is_unverifiable_not_expired(self):
        # Sin developer token no se pudo NI INTENTAR la comprobación.
        auth = AppleMusicAuth(verbose=False)
        auth._access_token = None
        with patch.object(auth, "_fetch_access_token", return_value=None):
            is_valid, message = auth.validate_token("user-token")
        self.assertIsNone(is_valid)
        self.assertIn("could not obtain", message)

    def test_a_malformed_200_body_is_unverifiable(self):
        auth = _auth_with_dev_token()
        with patch(
            "services.apple_music_auth.requests.get",
            return_value=_FakeResponse(200, ValueError("no json")),
        ):
            is_valid, _ = auth.validate_token("user-token")
        self.assertIsNone(is_valid)

    # --- El reflejo en la UI (get_status) ---

    def _get_status_with(self, response):
        auth = _auth_with_dev_token()
        with patch.object(auth, "get_stored_token", return_value="user-token"):
            with patch(
                "services.apple_music_auth.requests.get", return_value=response
            ):
                return auth.get_status()

    def test_get_status_never_says_expired_when_it_only_could_not_verify(self):
        # La aserción clave de A-2: un 503 NO puede producir "Token expired".
        status = self._get_status_with(_FakeResponse(503))
        self.assertFalse(status["connected"])
        self.assertIn("Could not verify", status["status"])
        self.assertNotIn("Token expired", status["status"])

    def test_get_status_still_reports_a_real_rejection_as_expired(self):
        # El comportamiento que NO cambia: un 401 sí sigue siendo caducado.
        status = self._get_status_with(_FakeResponse(401))
        self.assertFalse(status["connected"])
        self.assertIn("Token expired", status["status"])

    def test_get_status_reports_a_verified_session_as_connected(self):
        status = self._get_status_with(
            _FakeResponse(200, {"data": [{"id": "es"}]})
        )
        self.assertTrue(status["connected"])
        self.assertEqual(status["status"], "Connected")
        self.assertEqual(status["storefront"], "es")


if __name__ == "__main__":
    unittest.main(verbosity=2)

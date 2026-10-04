#!/usr/bin/env python3
"""
Contrato de credenciales de Tidal (revisado 2026-10-04).

Las credenciales de cliente de Tidal son PÚBLICAS (el par del cliente de
escritorio oficial y el client id PKCE, los mismos que usan los clientes
open-source). La cadena de resolución es: credenciales guardadas en la cuenta →
variables de entorno → bundle público incrustado. Los tokens PERSONALES del
usuario nunca se incrustan: viven en el keychain o en la configuración de
cuentas.

Valida:
1. Los resolutores caen al bundle público cuando no hay credenciales ni env.
2. La env sobrescribe el bundle público.
3. Las credenciales guardadas tienen prioridad sobre la env.
4. .env.example documenta las variables de override.
"""

import os
import sys
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

try:
    import aiohttp  # noqa: F401
except ModuleNotFoundError:
    sys.modules["aiohttp"] = MagicMock()

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from services.service_base import ServiceCredentials, ServiceType
from services.tidal_auth import (
    PUBLIC_TIDAL_CLIENT_CREDENTIALS,
    PUBLIC_TIDAL_CLIENT_ID,
    PUBLIC_TIDAL_CLIENT_ID_PKCE,
    PUBLIC_TIDAL_CLIENT_SECRET,
    TidalAuth,
    resolve_tidal_client_id,
    resolve_tidal_client_secret,
    resolve_tidal_pkce_client_id,
)
from services.tidal_service import TidalService

TIDAL_CREDENTIAL_VARIABLES = ("TIDAL_CLIENT_ID", "TIDAL_CLIENT_SECRET", "TIDAL_CLIENT_ID_PKCE")


def _without_env():
    env = {k: v for k, v in os.environ.items() if k not in TIDAL_CREDENTIAL_VARIABLES}
    return patch.dict(os.environ, env, clear=True)


class TidalCredentialResolutionTests(unittest.TestCase):
    def test_public_bundle_is_frozen(self):
        """Congela los valores públicos: perderlos rompió el servicio dos veces."""
        self.assertEqual(PUBLIC_TIDAL_CLIENT_ID, "fX2JxdmntZWK0ixT")
        self.assertEqual(PUBLIC_TIDAL_CLIENT_SECRET, "xeuPmY7nbpZ9IIbLAcQ93shka1VNheUAqN6IcszjTG8=")
        self.assertEqual(PUBLIC_TIDAL_CLIENT_ID_PKCE, "6BDSRdpK9hqEBTgU")
        for variable in TIDAL_CREDENTIAL_VARIABLES:
            self.assertIn(variable, PUBLIC_TIDAL_CLIENT_CREDENTIALS)

    def test_resolvers_fall_back_to_the_public_bundle_without_configuration(self):
        with _without_env():
            self.assertEqual(resolve_tidal_client_id(), PUBLIC_TIDAL_CLIENT_ID)
            self.assertEqual(resolve_tidal_client_secret(), PUBLIC_TIDAL_CLIENT_SECRET)
            self.assertEqual(resolve_tidal_pkce_client_id(), PUBLIC_TIDAL_CLIENT_ID_PKCE)

    def test_resolvers_read_the_environment(self):
        with patch.dict(os.environ, {
            "TIDAL_CLIENT_ID": "env-client-id",
            "TIDAL_CLIENT_SECRET": "env-client-secret",
            "TIDAL_CLIENT_ID_PKCE": "env-pkce-client-id",
        }):
            self.assertEqual(resolve_tidal_client_id(), "env-client-id")
            self.assertEqual(resolve_tidal_client_secret(), "env-client-secret")
            self.assertEqual(resolve_tidal_pkce_client_id(), "env-pkce-client-id")

    def test_tidal_auth_exposes_resolved_credentials(self):
        with patch.dict(os.environ, {
            "TIDAL_CLIENT_ID": "env-client-id",
            "TIDAL_CLIENT_SECRET": "env-client-secret",
        }):
            auth = TidalAuth()
            self.assertEqual(auth.client_id, "env-client-id")
            self.assertEqual(auth.client_secret, "env-client-secret")

        with _without_env():
            auth = TidalAuth()
            self.assertEqual(auth.client_id, PUBLIC_TIDAL_CLIENT_ID)
            self.assertEqual(auth.client_secret, PUBLIC_TIDAL_CLIENT_SECRET)


class TidalStoredCredentialsTests(unittest.TestCase):
    def test_stored_credentials_take_precedence_over_the_environment(self):
        credentials = ServiceCredentials(
            service_type=ServiceType.TIDAL,
            client_id="stored-client-id",
            client_secret="stored-client-secret",
        )
        service = TidalService(credentials)

        with patch.dict(os.environ, {
            "TIDAL_CLIENT_ID": "env-client-id",
            "TIDAL_CLIENT_SECRET": "env-client-secret",
        }):
            self.assertEqual(service.client_id, "stored-client-id")
            self.assertEqual(service.client_secret, "stored-client-secret")

    def test_tidal_service_falls_back_to_the_environment(self):
        service = TidalService(ServiceCredentials(service_type=ServiceType.TIDAL))

        with patch.dict(os.environ, {
            "TIDAL_CLIENT_ID": "env-client-id",
            "TIDAL_CLIENT_SECRET": "env-client-secret",
        }):
            self.assertEqual(service.client_id, "env-client-id")
            self.assertEqual(service.client_secret, "env-client-secret")

        with _without_env():
            self.assertEqual(service.client_id, PUBLIC_TIDAL_CLIENT_ID)
            self.assertEqual(service.client_secret, PUBLIC_TIDAL_CLIENT_SECRET)


class TidalEnvExampleTests(unittest.TestCase):
    def test_env_example_documents_every_tidal_variable(self):
        example = (REPO_ROOT / ".env.example").read_text(encoding="utf-8")

        for variable in TIDAL_CREDENTIAL_VARIABLES:
            self.assertIn(variable, example, f".env.example must document {variable}")

        self.assertIn("tidal_auth.py", example, ".env.example must name the scripts that read these variables")


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Offline proof of the Qobuz credential-resolution contract.

Revisado 2026-11-26 (D14, #126): el camino de descarga Python y la firma de
`track/getFileUrl` se retiraron; la firma es responsabilidad del núcleo Rust
(`src-tauri/src/download/qobuz.rs::build_request_signature`). Lo que queda aquí
es el contrato de credenciales que sí conserva el servicio: el bundle público
de la API (el mismo par app_id/secret de los clientes open-source) y la vía de
override del operador (`ServiceCredentials.app_secret` / `client_secret` /
`extra["app_secret"]` y `QOBUZ_APP_SECRET` en el entorno).

El literal del bundle no se repite aquí: `qobuz_credentials_leak_test` escanea
scripts/ y este fichero no es un hogar canónico; se lee en runtime.
"""

import os
import sys
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
for site_packages in REPO_ROOT.glob(".venv/lib/python*/site-packages"):
    if site_packages.is_dir():
        sys.path.insert(0, str(site_packages))
sys.path.insert(0, str(REPO_ROOT / "scripts"))

# Keep the module importable in dependency-minimal checkouts: the assertions
# below are about credentials, not about aiohttp.
try:
    import aiohttp  # noqa: F401
except ModuleNotFoundError:
    sys.modules["aiohttp"] = MagicMock()

from services.qobuz_service import QobuzService
from services.service_base import ServiceCredentials


def _service(**credential_kwargs) -> QobuzService:
    return QobuzService(ServiceCredentials(username="user", password="pass", **credential_kwargs))


class QobuzCredentialResolutionTests(unittest.TestCase):
    def setUp(self):
        for name in ("QOBUZ_APP_ID", "QOBUZ_APP_SECRET"):
            os.environ.pop(name, None)

    def test_secret_aliases_and_environment_are_honoured(self):
        """client_secret and extra['app_secret'] are equivalent entry points."""
        self.assertEqual(
            _service(client_secret="from-client-secret").APP_SECRET, "from-client-secret"
        )
        self.assertEqual(
            QobuzService(
                ServiceCredentials(
                    username="u", password="p", extra={"app_secret": "from-extra"}
                )
            ).APP_SECRET,
            "from-extra",
        )
        with patch.dict(os.environ, {"QOBUZ_APP_SECRET": "from-env"}):
            self.assertEqual(_service().APP_SECRET, "from-env")
        # Credentials win over the environment.
        with patch.dict(os.environ, {"QOBUZ_APP_SECRET": "from-env"}):
            self.assertEqual(_service(app_secret="from-credentials").APP_SECRET, "from-credentials")

    def test_app_id_is_overridable_without_touching_the_committed_value(self):
        self.assertEqual(
            _service(app_id="operator-app-id").APP_ID, "operator-app-id"
        )
        with patch.dict(os.environ, {"QOBUZ_APP_ID": "env-app-id"}):
            self.assertEqual(_service().APP_ID, "env-app-id")
        self.assertEqual(_service().APP_ID, QobuzService.APP_ID)

    def test_committed_default_is_a_real_public_secret(self):
        """El valor por defecto es el bundle público: 32 hex, no un placeholder vacío.

        El literal exacto lo congela el suite Rust (`secrets_isolation_security_test.rs`);
        aquí se comprueba la forma y que la resolución sin credenciales ni env lo usa.
        """
        committed = QobuzService.APP_SECRET.strip()
        self.assertEqual(len(committed), 32)
        self.assertTrue(all(c in "0123456789abcdef" for c in committed))
        self.assertEqual(_service().APP_SECRET, committed)


if __name__ == "__main__":
    unittest.main()

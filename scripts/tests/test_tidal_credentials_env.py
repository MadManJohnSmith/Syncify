#!/usr/bin/env python3
"""
Tidal OAuth credentials come from the environment, never from the source tree (PY-6).

`scripts/services/tidal_auth.py` and `scripts/services/tidal_service.py` embedded
the same Tidal client id and secret as class attributes. The crates already read
TIDAL_CLIENT_ID / TIDAL_CLIENT_SECRET (crates/syncify-tidal-downloader/src/lib.rs);
the Python bridges now follow the same contract and fail with an explicit error
when the variables are absent.

Validates:
1. No Tidal client credential literal survives anywhere under scripts/.
2. The resolvers raise a clear, actionable error without the env variables.
3. Credentials stored on the account take precedence over the environment.
4. .env.example documents every variable the Python Tidal flows read.
"""

import ast
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
    MissingTidalCredentialsError,
    TidalAuth,
    resolve_tidal_client_id,
    resolve_tidal_client_secret,
    resolve_tidal_pkce_client_id,
)
from services.tidal_service import TidalService

TIDAL_CREDENTIAL_VARIABLES = ("TIDAL_CLIENT_ID", "TIDAL_CLIENT_SECRET", "TIDAL_CLIENT_ID_PKCE")


class TidalCredentialsSourceTests(unittest.TestCase):
    TIDAL_MODULES = ("services/tidal_auth.py", "services/tidal_service.py")
    CREDENTIAL_CONSTANTS = ("CLIENT_ID", "CLIENT_SECRET", "CLIENT_ID_PKCE", "CLIENT_SECRET_PKCE")

    def test_tidal_modules_do_not_assign_client_credential_constants(self):
        """The literal values are the ones already listed in the Rust secrets test; here we
        assert structurally that no module assigns them any more."""
        offenders = []
        for module in self.TIDAL_MODULES:
            path = SCRIPTS_DIR / module
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            for node in ast.walk(tree):
                if not isinstance(node, (ast.Assign, ast.AnnAssign)):
                    continue
                targets = node.targets if isinstance(node, ast.Assign) else [node.target]
                for target in targets:
                    if isinstance(target, ast.Name) and target.id in self.CREDENTIAL_CONSTANTS:
                        offenders.append(f"{module}:{target.lineno} {target.id}")

        self.assertEqual(
            offenders, [],
            f"Tidal OAuth credentials must be resolved from the environment, not assigned as "
            f"module/class constants: {offenders}",
        )

    def test_tidal_auth_resolves_every_credential_from_the_environment(self):
        content = (SCRIPTS_DIR / "services/tidal_auth.py").read_text(encoding="utf-8")

        for variable in TIDAL_CREDENTIAL_VARIABLES:
            self.assertIn(
                variable, content,
                f"scripts/services/tidal_auth.py must resolve {variable} from the environment",
            )


class TidalCredentialResolutionTests(unittest.TestCase):
    def _without_env(self):
        env = {k: v for k, v in os.environ.items() if k not in TIDAL_CREDENTIAL_VARIABLES}
        return patch.dict(os.environ, env, clear=True)

    def test_resolvers_fail_with_a_clear_error_when_unconfigured(self):
        resolvers = (
            (resolve_tidal_client_id, "TIDAL_CLIENT_ID"),
            (resolve_tidal_client_secret, "TIDAL_CLIENT_SECRET"),
            (resolve_tidal_pkce_client_id, "TIDAL_CLIENT_ID_PKCE"),
        )

        with self._without_env():
            for resolver, variable in resolvers:
                with self.subTest(variable=variable):
                    with self.assertRaises(MissingTidalCredentialsError) as ctx:
                        resolver()
                    message = str(ctx.exception)
                    self.assertIn(variable, message)
                    self.assertIn(".env.example", message)

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

        with self._without_env():
            with self.assertRaises(MissingTidalCredentialsError):
                _ = TidalAuth().client_id

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

        with self._without_env():
            with self.assertRaises(MissingTidalCredentialsError):
                _ = service.client_id


class TidalEnvExampleTests(unittest.TestCase):
    def test_env_example_documents_every_tidal_variable(self):
        example = (REPO_ROOT / ".env.example").read_text(encoding="utf-8")

        for variable in TIDAL_CREDENTIAL_VARIABLES:
            self.assertIn(variable, example, f".env.example must document {variable}")

        self.assertIn("tidal_auth.py", example, ".env.example must name the scripts that read these variables")


if __name__ == "__main__":
    unittest.main()
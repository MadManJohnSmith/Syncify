#!/usr/bin/env python3
"""
Ítem 68 de auditoría: autenticación real del camino de letras de Apple Music.

`test_provider('apple_music')` de `scripts/lyrics_bridge.py` daba «available»
con solo el HTTP 200/301/302 de la web pública `music.apple.com`, sin token de
usuario — y `LyricsService.get_lyrics` pintaba éxito con solo existir el
proveedor (`scripts/services/lyrics_service.py`, prioridad 1). Aquí se fija el
contrato real del servicio de letras:

1. Sin `media_user_token`, `_ensure_auth()` debe fallar: la web pública no
   autentica a nadie y el indicador verde no puede basarse en ella.
2. Sin token, `LyricsService` no debe anunciar el proveedor Apple Music.
3. Con el token real (env `APPLE_MUSIC_MEDIA_USER_TOKEN`, la misma variable que
   alimenta `lyrics_bridge.py`), la autenticación contra
   `amp-api.music.apple.com/v1/me/storefront` debe responder 200 y la sesión
   autenticada debe poder buscar una pista conocida y bajar sus letras.

Las pruebas 3 y 4 hablan con el servicio real de Apple Music: se saltan
(sin fallo) cuando la variable de entorno no está exportada y nunca usan
mocks — un mock que pasa siempre es exactamente lo que denuncia el ítem 68.
"""

import os
import sys
import unittest
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from services.lyrics_service import AppleMusicLyricsProvider, LyricsService  # noqa: E402

MEDIA_USER_TOKEN_ENV = "APPLE_MUSIC_MEDIA_USER_TOKEN"

# Pista con letras disponibles en prácticamente todos los storefronts; solo
# sirve para comprobar que la sesión autenticada baja datos reales.
KNOWN_TRACK = ("Bohemian Rhapsody", "Queen")


class AppleMusicLyricsAuthTests(unittest.TestCase):
    """Ítem 68: el verde de letras Apple Music exige autenticación real."""

    def test_without_media_user_token_auth_fails(self):
        """Sin token de usuario no hay sesión: la web pública no autentica."""
        provider = AppleMusicLyricsProvider(media_user_token=None, verbose=False)
        self.assertFalse(
            provider._ensure_auth(),
            "sin media_user_token la autenticación debe fallar",
        )
        self.assertNotIn(
            "media-user-token",
            provider._session.headers,
            "nunca debe enviarse la cabecera de usuario sin token",
        )

    def test_service_without_token_does_not_advertise_apple_music(self):
        """Sin token el servicio no crea el proveedor ni lo ofrece como fuente."""
        service = LyricsService(apple_music_token=None, verbose=False)
        self.assertIsNone(
            service.apple_music,
            "sin token el servicio no debe anunciar el proveedor Apple Music",
        )

    @unittest.skipUnless(
        os.getenv(MEDIA_USER_TOKEN_ENV),
        f"requiere credenciales reales: exporta {MEDIA_USER_TOKEN_ENV} "
        "(se salta en CI y en máquinas sin la sesión de Apple Music)",
    )
    def test_real_media_user_token_authenticates_against_amp_api(self):
        """Con el token real, `me/storefront` debe responder 200."""
        provider = AppleMusicLyricsProvider(
            media_user_token=os.environ[MEDIA_USER_TOKEN_ENV], verbose=False
        )
        self.assertTrue(
            provider._ensure_auth(),
            "me/storefront rechazó el token: la obtención de letras de Apple "
            "Music NO está habilitada de verdad",
        )
        self.assertIn(
            "media-user-token",
            provider._session.headers,
            "la sesión autenticada envía el token de usuario en cada petición",
        )

    @unittest.skipUnless(
        os.getenv(MEDIA_USER_TOKEN_ENV),
        f"requiere credenciales reales: exporta {MEDIA_USER_TOKEN_ENV} "
        "(se salta en CI y en máquinas sin la sesión de Apple Music)",
    )
    def test_real_authenticated_session_fetches_lyrics(self):
        """Camino completo real con credenciales: búsqueda + letras sincronizadas."""
        provider = AppleMusicLyricsProvider(
            media_user_token=os.environ[MEDIA_USER_TOKEN_ENV], verbose=False
        )
        song_id = provider.search_track(*KNOWN_TRACK)
        self.assertIsNotNone(
            song_id, "la búsqueda autenticada debe encontrar la pista conocida"
        )
        result = provider.get_lyrics(song_id)
        self.assertIsNotNone(result, "la pista conocida debe tener letras")
        self.assertEqual(result.source, "apple_music")
        self.assertTrue(
            (result.synced_lyrics or "").strip(),
            "las letras deben llegar sincronizadas (LRC no vacío)",
        )


if __name__ == "__main__":
    unittest.main()

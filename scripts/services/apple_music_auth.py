"""
Apple Music Authentication Service - Browser-based login to capture media-user-token.

This service opens a browser window for the user to log in with their Apple ID,
then captures the media-user-token cookie for use with the Apple Music API.
"""

import asyncio
import json
import re
import time
from pathlib import Path
from typing import Optional, Tuple
import requests


class AppleMusicAuth:
    """
    Apple Music authentication via browser automation.
    
    Captures the media-user-token cookie after user logs in.
    """
    
    APPLE_MUSIC_URL = "https://music.apple.com"
    TOKEN_COOKIE_NAME = "media-user-token"
    
    def __init__(self, settings_file: Optional[Path] = None, verbose: bool = False):
        self.settings_file = settings_file or Path(__file__).parent.parent / ".gui_settings.json"
        self.verbose = verbose
        self._cached_token: Optional[str] = None
        self._access_token: Optional[str] = None
    
    def _log(self, message: str):
        if self.verbose:
            print(f"[Apple Music Auth] {message}")
    
    def get_stored_token(self) -> Optional[str]:
        """Get the stored media-user-token from settings file. Also loads cached dev token."""
        try:
            if self.settings_file.exists():
                with open(self.settings_file, 'r') as f:
                    settings = json.load(f)
                    token = settings.get("apple_music_token", "")
                    self._access_token = settings.get("apple_music_dev_token", None)
                    if token:
                        return token
        except Exception as e:
            self._log(f"Error reading settings: {e}")
        return None
    
    def save_token(self, token: str, dev_token: Optional[str] = None) -> bool:
        """Save the media-user-token (and optional dev token) to settings file."""
        try:
            settings = {}
            if self.settings_file.exists():
                with open(self.settings_file, 'r') as f:
                    settings = json.load(f)
            
            settings["apple_music_token"] = token
            if dev_token:
                settings["apple_music_dev_token"] = dev_token
                self._access_token = dev_token
            
            with open(self.settings_file, 'w') as f:
                json.dump(settings, f, indent=2)
            
            self._cached_token = token
            self._log("Token saved successfully")
            return True
        except Exception as e:
            self._log(f"Error saving token: {e}")
            return False
    
    def clear_token(self) -> bool:
        """Clear the stored token (logout)."""
        return self.save_token("")
    
    def _fetch_access_token(self) -> Optional[str]:
        """Respaldo: extraer el developer JWT de la web de Apple con requests.

        FIX 2026-08-25: la red del propietario está marcada por el anti-bot de
        Apple (respuestas vacías), así que esto puede fallar aunque el navegador
        real funcione — por eso primero se intenta SIEMPRE la interceptación en
        el navegador. Aquí: UA real, timeouts, escaneo del HTML y hasta 6
        bundles JS, con diagnóstico por etapa."""
        import requests as _rq
        H = {"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
             "Accept-Language": "en-US,en;q=0.9"}
        jwt_re = re.compile(r'(eyJhbGciOi[^"\' ]{60,700})')
        try:
            r = _rq.get(f'{self.APPLE_MUSIC_URL}/us/browse', headers=H, timeout=15)
            if r.status_code != 200 or not r.text:
                self._log(f"Página Apple no accesible (HTTP {r.status_code}, {len(r.text)} bytes) — ¿red bloqueada?")
                return None
            m = jwt_re.search(r.text)
            if m:
                self._access_token = m.group(1)
                self._log("Dev token extraído del HTML")
                return self._access_token
            bundles = list(dict.fromkeys(re.findall(r"/assets/[^\"]+[.]js", r.text)))[:6]
            for b in bundles:
                try:
                    rj = _rq.get(f'https://music.apple.com{b}', headers=H, timeout=12)
                    if rj.status_code == 200:
                        mj = jwt_re.search(rj.text)
                        if mj:
                            self._access_token = mj.group(1)
                            self._log(f"Dev token extraído de {b[:40]}…")
                            return self._access_token
                except Exception as ej:
                    self._log(f"Bundle {b[:40]}… falló: {ej}")
            self._log("Sin JWT en HTML ni bundles")
            return None
        except Exception as e:
            self._log(f"Error fetching access token: {e}")
            return None

    def validate_token(self, token: Optional[str] = None) -> Tuple[bool, str]:
        """
        Validate the media-user-token against Apple Music API.
        
        Returns:
            (is_valid, storefront_or_error_message)
        """
        token = token or self.get_stored_token()
        if not token:
            return False, "No token provided"
        
        # Get access token first
        if not self._access_token:
            self._fetch_access_token()
        
        if not self._access_token:
            return False, ("no se pudo obtener el developer token de Apple "
                           "(su red parece bloqueada para peticiones directas); "
                           "reintenta la conexión — normalmente al segundo intento "
                           "la recarga del reproductor lo entrega")
        
        try:
            response = requests.get(
                "https://amp-api.music.apple.com/v1/me/storefront",
                headers={
                    "Authorization": f"Bearer {self._access_token}",
                    "media-user-token": token,
                    "Origin": "https://music.apple.com",
                    "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
                },
                timeout=10
            )
            
            if response.status_code == 200:
                data = response.json()
                storefront = data.get("data", [{}])[0].get("id", "unknown")
                self._log(f"Token valid (storefront: {storefront})")
                return True, storefront
            else:
                self._log(f"Token invalid: HTTP {response.status_code}")
                return False, f"Invalid token (HTTP {response.status_code})"
                
        except Exception as e:
            self._log(f"Validation error: {e}")
            return False, str(e)
    
    async def login_with_browser(self, timeout_seconds: int = 300) -> Tuple[bool, str]:
        """
        Open browser for user to log in and capture the media-user-token.
        
        Args:
            timeout_seconds: How long to wait for login (default 5 minutes)
            
        Returns:
            (success, token_or_error_message)
        """
        try:
            from playwright.async_api import async_playwright
        except ImportError:
            return False, "Playwright not installed. Run: pip install playwright && python -m playwright install chromium"

        # Navegador del sistema (decisión propietario 2026-08-25): nunca el
        # Chromium descargable de ms-playwright.
        from services.browser_launcher import chrome_launch_kwargs
        
        self._log("Starting browser login flow...")
        
        async with async_playwright() as p:
            # Use a dedicated profile with system Chrome browser
            import os
            user_data_dir = str(Path(__file__).parent.parent / ".browser_profile_apple")
            Path(user_data_dir).mkdir(exist_ok=True)
            
            self._log("Launching Chrome...")
            
            # Try system Chrome first, fall back to Chromium
            try:
                context = await p.chromium.launch_persistent_context(
                    user_data_dir=user_data_dir,
                    channel="chrome",
                    headless=False,
                    args=[
                        "--disable-blink-features=AutomationControlled",
                        "--disable-infobars",
                    ],
                    viewport={"width": 1280, "height": 800},
                    ignore_default_args=["--enable-automation"],
                )
            except Exception as e:
                # FIX 2026-08-25: el fallback ya NO intenta el Chromium gestionado
                # por Playwright (binario no descargado → "Executable doesn't
                # exist"); resolvemos SIEMPRE un navegador del sistema.
                self._log(f"Chrome vía canal falló ({e}); usando navegador del sistema")
                context = await p.chromium.launch_persistent_context(
                    user_data_dir=user_data_dir,
                    **chrome_launch_kwargs(),
                    headless=False,
                    args=[
                        "--disable-blink-features=AutomationControlled",
                        "--disable-infobars",
                    ],
                    viewport={"width": 1280, "height": 800},
                    ignore_default_args=["--enable-automation"],
                )
            
            # Always create a new page to avoid issues with existing blank tabs
            page = await context.new_page()
            
            # Setup network interception to capture developer token
            dev_token_found = None
            async def on_request(request):
                nonlocal dev_token_found
                if dev_token_found: return
                
                headers = request.headers
                auth = headers.get("authorization", "") or headers.get("Authorization", "")
                
                if auth and auth.startswith("Bearer eyJ"):
                    self._log(f"Captured developer token from request")
                    dev_token_found = auth.replace("Bearer ", "")

            page.on("request", on_request)

            # Navigate to Apple Music
            self._log("Opening Apple Music...")
            try:
                await page.goto(self.APPLE_MUSIC_URL, wait_until="domcontentloaded", timeout=30000)
            except:
                pass
            
            # Wait for user to log in and token cookie to appear
            self._log(f"Waiting for login (timeout: {timeout_seconds}s)...")
            
            start_time = time.time()
            token = None
            
            # FIX: detección de ventana cerrada compatible con cualquier
            # versión de Playwright (eventos en vez de is_closed()).
            _window_closed = {"v": False}
            context.on("close", lambda _: _window_closed.update(v=True))
            
            while time.time() - start_time < timeout_seconds:
                if _window_closed["v"]:
                    return False, "Cerraste la ventana del navegador sin completar el inicio de sesión — vuelve a intentar la conexión."

                # Check for the media-user-token cookie
                cookies = await context.cookies()
                for cookie in cookies:
                    if cookie["name"] == self.TOKEN_COOKIE_NAME:
                        token = cookie["value"]
                        break
                
                if token:
                    self._log("Token captured!")
                    break
                
                # Wait a bit before checking again
                await asyncio.sleep(1)
            
            # FIX 2026-08-25 (orden): el close() estaba ANTES de la recarga — con
            # sesión recordada el token aparecía en ~1s, el navegador se cerraba,
            # y la recarga corría sobre un contexto muerto. Ahora la recarga
            # ocurre PRIMERO (con sesión recordada esto le da al reproductor los
            # segundos que necesita para emitir su developer token) y el cierre
            # va al final de cada rama.
            if token and not dev_token_found:
                self._log("Developer token ausente; recargando para forzar llamadas a la API…")
                try:
                    await page.reload(wait_until="domcontentloaded", timeout=20000)
                    deadline = time.time() + 15
                    while time.time() < deadline and not dev_token_found:
                        await asyncio.sleep(1)
                except Exception as e2:
                    self._log(f"Recarga falló: {e2}")

            if dev_token_found:
                self._access_token = dev_token_found

            await context.close()

            if token:
                
                # Validate the token
                is_valid, result = self.validate_token(token)
                if is_valid:
                    self.save_token(token, self._access_token)
                    return True, token
                else:
                    return False, f"Token captured but invalid: {result}"
            else:
                return False, "Login timed out or cancelled"
                
        return False, "Unknown error"
    
    def get_status(self) -> dict:
        """Get current Apple Music connection status."""
        token = self.get_stored_token()
        
        if not token:
            return {
                "connected": False,
                "status": "Not connected",
                "storefront": None
            }
        
        is_valid, result = self.validate_token(token)
        
        if is_valid:
            return {
                "connected": True,
                "status": "Connected",
                "storefront": result
            }
        else:
            return {
                "connected": False,
                "status": f"Token expired: {result}",
                "storefront": None
            }


# Convenience functions for GUI bridge
def apple_music_login() -> dict:
    """Start Apple Music browser login flow."""
    auth = AppleMusicAuth(verbose=True)
    
    # Run async login
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    try:
        success, result = loop.run_until_complete(auth.login_with_browser())
        return {
            "success": success,
            "message": "Successfully connected to Apple Music" if success else result
        }
    finally:
        loop.close()


def apple_music_status() -> dict:
    """Get Apple Music connection status."""
    auth = AppleMusicAuth(verbose=False)
    return auth.get_status()


def apple_music_logout() -> dict:
    """Log out from Apple Music (clear token)."""
    auth = AppleMusicAuth(verbose=True)
    success = auth.clear_token()
    return {
        "success": success,
        "message": "Logged out from Apple Music" if success else "Failed to log out"
    }


if __name__ == "__main__":
    # Test the authentication
    import sys
    
    if len(sys.argv) > 1 and sys.argv[1] == "login":
        print("Starting Apple Music login...")
        result = apple_music_login()
        print(f"Result: {result}")
    else:
        print("Checking Apple Music status...")
        result = apple_music_status()
        print(f"Status: {result}")

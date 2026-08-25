"""
Deezer Authentication Service - Browser-based ARL cookie extraction.

Opens a browser for user to log in to Deezer, then extracts the ARL cookie.
"""

import asyncio
import json
import time
from pathlib import Path
from typing import Optional, Tuple, Dict, Any


class DeezerAuth:
    """
    Deezer authentication via browser automation.
    
    Launches browser to deezer.com login, waits for user to authenticate,
    then extracts the ARL cookie automatically.
    """
    
    DEEZER_URL = "https://www.deezer.com/login"
    
    def __init__(self, credentials_file: Optional[Path] = None, verbose: bool = False):
        self.credentials_file = credentials_file or Path(__file__).parent.parent / ".gui_credentials_cache.json"
        self.verbose = verbose
    
    def _log(self, message: str):
        if self.verbose:
            print(f"[Deezer Auth] {message}", flush=True)
    
    def get_stored_arl(self) -> Optional[str]:
        """Get stored ARL from credentials cache."""
        try:
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
                    deezer_data = cache.get("deezer", {})
                    return deezer_data.get("arl")
        except Exception as e:
            self._log(f"Error reading credentials: {e}")
        return None
    
    def save_arl(self, arl: str) -> bool:
        """Save ARL to credentials cache."""
        try:
            cache = {}
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
            
            cache["deezer"] = {"arl": arl, "remember": "true"}
            
            with open(self.credentials_file, 'w') as f:
                json.dump(cache, f, indent=2)
            
            self._log("ARL saved successfully")
            return True
        except Exception as e:
            self._log(f"Error saving ARL: {e}")
            return False
    
    def clear_arl(self) -> bool:
        """Clear stored ARL (logout)."""
        try:
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
                
                if "deezer" in cache:
                    del cache["deezer"]
                    
                    with open(self.credentials_file, 'w') as f:
                        json.dump(cache, f, indent=2)
            
            self._log("ARL cleared")
            return True
        except Exception as e:
            self._log(f"Error clearing ARL: {e}")
            return False
    
    async def login_with_browser(self, timeout_seconds: int = 300) -> Tuple[bool, str]:
        """
        Open browser for user to log in and capture the ARL cookie.
        
        Returns:
            (success, arl_or_error_message)
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
            user_data_dir = str(Path(__file__).parent.parent / ".browser_profile_deezer")
            Path(user_data_dir).mkdir(exist_ok=True)
            
            self._log("Launching Chrome...")
            
            # Try system Chrome first, fall back to Chromium
            try:
                context = await p.chromium.launch_persistent_context(
                    user_data_dir=user_data_dir,
                    channel="chrome",  # Use installed Chrome
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
            
            # Anti-detection script
            await page.add_init_script("""
                Object.defineProperty(navigator, 'webdriver', {get: () => undefined});
            """)
            
            self._log("Navigating to Deezer login...")
            await page.goto(self.DEEZER_URL, wait_until="domcontentloaded", timeout=30000)
            
            # Wait for user to log in and ARL cookie to appear
            self._log(f"Waiting for login (timeout: {timeout_seconds}s)...")
            
            start_time = time.time()
            arl = None
            
            while time.time() - start_time < timeout_seconds:
                try:
                    _closed = context.is_closed()
                except Exception:
                    _closed = True
                if _closed:
                    return False, "Cerraste la ventana del navegador sin completar el inicio de sesión — vuelve a intentar la conexión."

                cookies = await context.cookies()
                
                for cookie in cookies:
                    if cookie["name"] == "arl" and cookie.get("value"):
                        arl = cookie["value"]
                        # ARL should be a long string (typically 192 chars)
                        if len(arl) > 100:
                            self._log("ARL cookie captured!")
                            break
                
                if arl and len(arl) > 100:
                    break
                
                await asyncio.sleep(2)
            
            await context.close()
            
            if arl and len(arl) > 100:
                self.save_arl(arl)
                return True, arl
            else:
                return False, "Login timed out or cancelled"
        
        return False, "Unknown error"
    
    def get_status(self) -> Dict[str, Any]:
        """Get current Deezer connection status."""
        arl = self.get_stored_arl()
        
        if not arl:
            return {
                "status": "success",
                "connected": False,
                "message": "Not connected"
            }
        
        return {
            "status": "success",
            "connected": True,
            "message": "Connected"
        }


# Convenience functions for GUI bridge
def deezer_login() -> dict:
    """Start Deezer browser login flow."""
    auth = DeezerAuth(verbose=True)
    
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    try:
        success, result = loop.run_until_complete(auth.login_with_browser())
        return {
            "status": "success" if success else "error",
            "message": "Successfully connected to Deezer" if success else result
        }
    finally:
        loop.close()


def deezer_status() -> dict:
    """Get Deezer connection status."""
    auth = DeezerAuth(verbose=False)
    return auth.get_status()


def deezer_logout() -> dict:
    """Log out from Deezer (clear ARL)."""
    auth = DeezerAuth(verbose=True)
    success = auth.clear_arl()
    return {
        "status": "success" if success else "error",
        "message": "Logged out from Deezer" if success else "Failed to log out"
    }


if __name__ == "__main__":
    import sys
    
    if len(sys.argv) > 1 and sys.argv[1] == "login":
        print("Starting Deezer login...")
        result = deezer_login()
        print(f"Result: {result}")
    else:
        print("Checking Deezer status...")
        result = deezer_status()
        print(f"Status: {result}")

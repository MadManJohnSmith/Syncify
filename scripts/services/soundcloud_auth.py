"""
SoundCloud Authentication Service - Browser-based OAuth token extraction.

Opens a browser for user to log in to SoundCloud, then extracts the OAuth token.
"""

import asyncio
import json
import time
from pathlib import Path
from typing import Optional, Tuple, Dict, Any


class SoundCloudAuth:
    """
    SoundCloud authentication via browser automation.
    
    Launches browser to soundcloud.com login, waits for user to authenticate,
    then extracts the OAuth token from local storage.
    """
    
    SOUNDCLOUD_URL = "https://soundcloud.com/signin"
    
    def __init__(self, credentials_file: Optional[Path] = None, verbose: bool = False):
        self.credentials_file = credentials_file or Path(__file__).parent.parent / ".gui_credentials_cache.json"
        self.verbose = verbose
    
    def _log(self, message: str):
        if self.verbose:
            print(f"[SoundCloud Auth] {message}", flush=True)
    
    def get_stored_token(self) -> Optional[str]:
        """Get stored OAuth token from credentials cache."""
        try:
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
                    sc_data = cache.get("soundcloud", {})
                    return sc_data.get("token") or sc_data.get("oauth_token")
        except Exception as e:
            self._log(f"Error reading credentials: {e}")
        return None
    
    def save_token(self, token: str) -> bool:
        """Save OAuth token to credentials cache."""
        try:
            cache = {}
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
            
            cache["soundcloud"] = {"token": token, "remember": "true"}
            
            with open(self.credentials_file, 'w') as f:
                json.dump(cache, f, indent=2)
            
            self._log("OAuth token saved successfully")
            return True
        except Exception as e:
            self._log(f"Error saving token: {e}")
            return False
    
    def clear_token(self) -> bool:
        """Clear stored OAuth token (logout)."""
        try:
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
                
                if "soundcloud" in cache:
                    del cache["soundcloud"]
                    
                    with open(self.credentials_file, 'w') as f:
                        json.dump(cache, f, indent=2)
            
            self._log("Token cleared")
            return True
        except Exception as e:
            self._log(f"Error clearing token: {e}")
            return False
    
    async def login_with_browser(self, timeout_seconds: int = 300) -> Tuple[bool, str]:
        """
        Open browser for user to log in and capture the OAuth token via network interception.
        """
        try:
            from playwright.async_api import async_playwright
        except ImportError:
            return False, "Playwright not installed. Run: pip install playwright && python -m playwright install chromium"
        
        self._log("Starting browser login flow (network interception method)...")
        
        captured_token: Optional[str] = None
        
        async with async_playwright() as p:
            self._log("Launching browser...")
            browser = await p.chromium.launch(
                headless=False,
                args=["--disable-blink-features=AutomationControlled", "--disable-infobars"],
            )
            context = await browser.new_context(
                viewport={"width": 1280, "height": 800},
                user_agent="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
            )
            page = await context.new_page()
            
            # Anti-detection
            await page.add_init_script("Object.defineProperty(navigator, 'webdriver', {get: () => undefined});")
            
            # Setup network interception
            async def on_request(request):
                nonlocal captured_token
                if captured_token: return
                
                headers = request.headers
                auth = headers.get("authorization", "") or headers.get("Authorization", "")
                
                # SoundCloud uses OAuth usually
                if auth and "OAuth" in auth and "api-v2.soundcloud.com" in request.url:
                    self._log(f"Captured token from request to {request.url}")
                    captured_token = auth

            page.on("request", on_request)
            
            self._log("Navigating to SoundCloud...")
            try:
                await page.goto("https://soundcloud.com/signin", timeout=30000)
            except:
                pass

            self._log(f"Waiting for login (timeout: {timeout_seconds}s)...")
            start_time = time.time()
            
            while time.time() - start_time < timeout_seconds:
                if captured_token:
                    self._log("OAuth token captured successfully!")
                    break
                    
                # Also check cookies
                try:
                    cookies = await context.cookies()
                    for cookie in cookies:
                        if cookie['name'] == 'oauth_token':
                            captured_token = f"OAuth {cookie['value']}"
                            self._log("Captured token from cookies")
                            break
                except:
                    pass

                if captured_token:
                    break

                # Also check localStorage as backup
                try:
                    token_data = await page.evaluate("""() => {
                        const oauth = localStorage.getItem('V2::oauth::token');
                        if (oauth) return oauth;
                        return null;
                    }""")
                    if token_data:
                        try:
                            # It's usually JSON string: "{\"access_token\":\"...\"}"
                            # But sometimes it's double encoded or just the object
                            import json
                            parsed = json.loads(token_data)
                            if isinstance(parsed, dict) and 'access_token' in parsed:
                                captured_token = f"OAuth {parsed['access_token']}"
                                self._log("Captured token from localStorage")
                                break
                        except:
                            pass
                except:
                    pass
                
                await asyncio.sleep(1)
            
            await browser.close()
            
            if captured_token:
                self.save_token(captured_token)
                return True, captured_token
            else:
                return False, "Login timed out or cancelled"
        
        return False, "Unknown error"
    
    def get_status(self) -> Dict[str, Any]:
        """Get current SoundCloud connection status."""
        token = self.get_stored_token()
        
        if not token:
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


if __name__ == "__main__":
    import sys
    
    if len(sys.argv) > 1 and sys.argv[1] == "login":
        print("Starting SoundCloud login...")
        auth = SoundCloudAuth(verbose=True)
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        success, result = loop.run_until_complete(auth.login_with_browser())
        print(f"Result: {success}, {result}")
    else:
        print("Checking SoundCloud status...")
        auth = SoundCloudAuth(verbose=False)
        print(f"Status: {auth.get_status()}")

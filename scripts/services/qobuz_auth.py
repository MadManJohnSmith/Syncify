"""
Qobuz Authentication Service - Browser-based session token extraction.

Opens a browser for user to log in to Qobuz, then extracts session cookies.
"""

import asyncio
import json
import time
from pathlib import Path
from typing import Optional, Tuple, Dict, Any


class QobuzAuth:
    """
    Qobuz authentication via browser automation.
    
    Launches browser to qobuz.com login, waits for user to authenticate,
    then extracts the session token from cookies.
    """
    
    QOBUZ_URL = "https://www.qobuz.com/login"
    
    def __init__(self, credentials_file: Optional[Path] = None, verbose: bool = False):
        self.credentials_file = credentials_file or Path(__file__).parent.parent / ".gui_credentials_cache.json"
        self.verbose = verbose
    
    def _log(self, message: str):
        if self.verbose:
            print(f"[Qobuz Auth] {message}", flush=True)
    
    def get_stored_session(self) -> Optional[Dict[str, str]]:
        """Get stored session data from credentials cache."""
        try:
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
                    return cache.get("qobuz_session")
        except Exception as e:
            self._log(f"Error reading credentials: {e}")
        return None
    
    def save_session(self, session_data: Dict[str, str]) -> bool:
        """Save session data to credentials cache."""
        try:
            cache = {}
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
            
            cache["qobuz_session"] = session_data
            
            with open(self.credentials_file, 'w') as f:
                json.dump(cache, f, indent=2)
            
            self._log("Session saved successfully")
            return True
        except Exception as e:
            self._log(f"Error saving session: {e}")
            return False
    
    def clear_session(self) -> bool:
        """Clear stored session (logout)."""
        try:
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
                
                if "qobuz_session" in cache:
                    del cache["qobuz_session"]
                    
                    with open(self.credentials_file, 'w') as f:
                        json.dump(cache, f, indent=2)
            
            self._log("Session cleared")
            return True
        except Exception as e:
            self._log(f"Error clearing session: {e}")
            return False
    
    async def login_with_browser(self, timeout_seconds: int = 300) -> Tuple[bool, str]:
        """
        Open browser for user to log in and capture session cookies.
        
        Returns:
            (success, user_id_or_error_message)
        """
        try:
            from playwright.async_api import async_playwright
        except ImportError:
            return False, "Playwright not installed. Run: pip install playwright && python -m playwright install chromium"
        
        self._log("Starting browser login flow...")
        
        async with async_playwright() as p:
            self._log("Launching browser...")
            
            # Use non-persistent context to avoid conflicts with existing sessions
            try:
                browser = await p.chromium.launch(
                    channel="chrome",  # Try system Chrome first
                    headless=False,
                    args=[
                        "--disable-blink-features=AutomationControlled",
                        "--disable-infobars",
                        "--no-sandbox",
                    ],
                )
            except Exception as e:
                self._log(f"Chrome not available, using Chromium: {e}")
                browser = await p.chromium.launch(
                    headless=False,
                    args=[
                        "--disable-blink-features=AutomationControlled",
                        "--disable-infobars",
                        "--no-sandbox",
                    ],
                )
            
            # Create context with anti-detection settings
            context = await browser.new_context(
                viewport={"width": 1280, "height": 800},
                user_agent="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
            )
            
            page = await context.new_page()
            
            # Anti-detection script
            await page.add_init_script("""
                Object.defineProperty(navigator, 'webdriver', {get: () => undefined});
            """)
            
            self._log("Navigating to Qobuz login...")
            await page.goto(self.QOBUZ_URL, wait_until="domcontentloaded", timeout=30000)
            
            # Wait for user to log in
            self._log(f"Waiting for login (timeout: {timeout_seconds}s)...")
            
            start_time = time.time()
            session_data = None
            
            while time.time() - start_time < timeout_seconds:
                try:
                    cookies = await context.cookies()
                except Exception as e:
                    # Browser was closed by user
                    self._log(f"Browser closed: {e}")
                    return False, "Browser was closed before login completed"
                
                # Look for Qobuz session cookies - check various possible names
                user_id = None
                auth_token = None
                all_cookies = {}
                
                for cookie in cookies:
                    name = cookie["name"]
                    value = cookie["value"]
                    all_cookies[name] = value[:50] + "..." if len(value) > 50 else value
                    
                    # Check for user ID cookies
                    if name in ("qobuz_user_id", "user_id", "uid"):
                        user_id = value
                    # Check for auth token cookies
                    elif name in ("qobuz_user_token", "auth_token", "user_auth_token", "token", "session", "sid"):
                        auth_token = value
                
                # Check if user is on a logged-in page (profile, favorites, etc.)
                try:
                    current_url = page.url
                    is_logged_in_page = any(x in current_url for x in ["/profile", "/my-", "/favorit", "/playlist", "/discover"])
                    
                    if is_logged_in_page and not session_data:
                        self._log(f"Detected logged-in page: {current_url}")
                        self._log(f"All cookies: {all_cookies}")
                        
                        # User is logged in - try to get user info from page
                        try:
                            user_info = await page.evaluate("""
                                () => {
                                    // Try to find user info in various places
                                    const scripts = document.querySelectorAll('script');
                                    for (let script of scripts) {
                                        const text = script.textContent;
                                        if (text.includes('user') && text.includes('id')) {
                                            const match = text.match(/"user".*?"id"\s*:\s*(\d+)/);
                                            if (match) return { user_id: match[1] };
                                        }
                                    }
                                    // Check for user element
                                    const userEl = document.querySelector('[data-user-id]');
                                    if (userEl) return { user_id: userEl.dataset.userId };
                                    return null;
                                }
                            """)
                            if user_info:
                                user_id = user_id or str(user_info.get('user_id', ''))
                        except:
                            pass
                        
                        # Even without auth token, if we detect user is logged in, save the cookies
                        if user_id or auth_token or is_logged_in_page:
                            session_data = {
                                "user_id": user_id or "browser_session",
                                "auth_token": auth_token or "browser_cookies",
                                "cookies": {k: v for k, v in all_cookies.items() if any(x in k.lower() for x in ['auth', 'token', 'session', 'user', 'sid'])},
                                "browser_login": True
                            }
                            self._log("Session captured from logged-in page!")
                            break
                            
                except Exception as e:
                    self._log(f"Page check error: {e}")
                
                # Also check localStorage for user data
                try:
                    local_data = await page.evaluate("""
                        () => {
                            const userData = localStorage.getItem('user');
                            if (userData) return userData;
                            
                            // Check for qobuz-specific storage
                            for (let key of Object.keys(localStorage)) {
                                if (key.includes('user') || key.includes('auth')) {
                                    return localStorage.getItem(key);
                                }
                            }
                            return null;
                        }
                    """)
                    
                    if local_data:
                        try:
                            data = json.loads(local_data)
                            if isinstance(data, dict):
                                user_id = user_id or str(data.get('id', data.get('user_id', '')))
                                auth_token = auth_token or data.get('auth_token', data.get('token', ''))
                        except:
                            pass
                except Exception as e:
                    # Page may be closed or navigating
                    self._log(f"localStorage check failed: {e}")
                    pass
                
                # Check if we have enough to authenticate
                if user_id and auth_token:
                    session_data = {
                        "user_id": user_id,
                        "auth_token": auth_token,
                        "browser_login": True
                    }
                    self._log("Session captured!")
                    break
                
                # Check if logged in by URL change
                try:
                    current_url = page.url
                    if "qobuz.com" in current_url and "/login" not in current_url and "/signin" not in current_url:
                        # User has navigated away from login - might be logged in
                        self._log(f"URL changed to: {current_url}")
                        await asyncio.sleep(2)
                except:
                    pass
                
                await asyncio.sleep(2)
            
            try:
                await context.close()
                await browser.close()
            except:
                pass  # Already closed
            
            if session_data:
                self.save_session(session_data)
                return True, session_data.get("user_id", "unknown")
            else:
                return False, "Login timed out or cancelled"
        
        return False, "Unknown error"
    
    def get_status(self) -> Dict[str, Any]:
        """Get current Qobuz connection status."""
        session = self.get_stored_session()
        
        # Also check for username/password credentials
        try:
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
                    if "qobuz" in cache and cache["qobuz"].get("username"):
                        return {
                            "status": "success",
                            "connected": True,
                            "message": "Connected (credentials)",
                            "auth_type": "credentials"
                        }
        except:
            pass
        
        if not session:
            return {
                "status": "success",
                "connected": False,
                "message": "Not connected"
            }
        
        return {
            "status": "success",
            "connected": True,
            "message": "Connected (browser)",
            "user_id": session.get("user_id"),
            "auth_type": "browser"
        }


if __name__ == "__main__":
    import sys
    
    if len(sys.argv) > 1 and sys.argv[1] == "login":
        print("Starting Qobuz login...")
        auth = QobuzAuth(verbose=True)
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        success, result = loop.run_until_complete(auth.login_with_browser())
        print(f"Result: {success}, {result}")
    else:
        print("Checking Qobuz status...")
        auth = QobuzAuth(verbose=False)
        print(f"Status: {auth.get_status()}")

"""
Qobuz Authentication Service - Browser-based session token extraction.

Opens a browser for user to log in to Qobuz, then extracts session cookies.
"""

import asyncio
import json
import re
import time
from pathlib import Path
from typing import Optional, Tuple, Dict, Any
from urllib.parse import parse_qs, urlparse


def should_attempt_token_capture_navigation(
    *,
    is_logged_in_page: bool,
    has_viable_token: bool,
    logged_in_streak: int,
    cooldown_polls_left: int = 0,
    min_streak: int = 2,
) -> bool:
    """S195(b): decide when the helper may NAVIGATE the browser to capture an API token.

    Owner report (connect-from-scratch): credentials are entered, the post-login redirect
    starts, the load is CANCELLED, and the page falls back to the login form — looping.
    Forensics: during the polling loop the previous code fired
    ``page.goto("https://play.qobuz.com/favorites/albums")`` on the FIRST poll that
    looked logged-in, which aborts Qobuz's still-in-flight post-login redirect
    ("net::ERR_ABORTED" => the cancelled load). If the session bootstrap had not fully
    settled yet, play.qobuz.com bounces straight back to /login and the user must submit
    again — each new submit racing the same auxiliary navigation. Reautenticar only
    *appeared* healthier because its backend path can succeed via the persisted
    username/password fallback (cache["qobuz"], which logout does not clear) even when
    the browser dance misfires.

    Policy: only navigate once the logged-in page has been observed STABLE across
    ``min_streak`` consecutive polls (the redirect chain finished), never while a viable
    token already arrived, and never during a cooldown window declared after a bounce
    back to /login. Pure function so the capture policy is unit-testable offline.
    """
    if not is_logged_in_page or has_viable_token:
        return False
    if cooldown_polls_left > 0:
        return False
    return logged_in_streak >= min_streak


class QobuzAuth:
    """
    Qobuz authentication via browser automation.
    
    Launches browser to qobuz.com login, waits for user to authenticate,
    then extracts the session token from cookies.
    """
    
    QOBUZ_URL = "https://play.qobuz.com/login"
    
    def __init__(self, credentials_file: Optional[Path] = None, verbose: bool = False):
        self.credentials_file = credentials_file or Path(__file__).parent.parent / ".gui_credentials_cache.json"
        self.verbose = verbose
    
    def _log(self, message: str):
        if self.verbose:
            print(f"[Qobuz Auth] {message}", flush=True)

    def _is_viable_auth_token(self, token: Optional[str]) -> bool:
        """Return True when token looks like a real Qobuz API auth token."""
        if token is None:
            return False

        value = str(token).strip()
        if not value or value in ("null", "undefined", "browser_cookies"):
            return False

        # Reject serialized JSON blobs or base64 tracking cookies like 'eyJ...'
        if value.startswith("{") or value.startswith("[") or value.startswith("eyJ"):
            return False

        # Real Qobuz tokens are not tiny literals.
        if len(value) < 16:
            return False

        if any(ch.isspace() for ch in value):
            return False

        return True

    def _is_error_artifact(value) -> bool:
        """S186: detect console-error strings captured as credential values.

        Forensics: the string '[Error] [Tauri] Command "sync_service" failed: ...'
        was entered into the web login form's password field and got persisted as
        the account password, which can never auto-login. Real credentials never
        look like console output, so these patterns are safe to reject.
        """
        if value is None:
            return False
        v = str(value).strip()
        if not v:
            return False
        return (
            v.startswith("[Error]")
            or v.startswith("[Err")
            or "invokeCommand" in v
            or 'Command "' in v
            or " failed:" in v
        )

    def _has_viable_credentials(self, username: Optional[str], password: Optional[str]) -> bool:
        if not bool((username or "").strip()) or not bool((password or "").strip()):
            return False
        if self._is_error_artifact(username) or self._is_error_artifact(password):
            return False
        return True
    
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

        from services.browser_launcher import chrome_launch_kwargs
        
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
                self._log(f"Canal Chrome falló ({e}); usando navegador del sistema")
                browser = await p.chromium.launch(
                    **chrome_launch_kwargs(),
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

            captured_auth_token = None
            captured_user_id = None
            captured_username = None
            captured_password = None

            def on_request(request):
                nonlocal captured_auth_token, captured_user_id, captured_username, captured_password
                try:
                    headers = request.headers
                    header_token = headers.get("x-user-auth-token") or headers.get("X-User-Auth-Token")
                    if self._is_viable_auth_token(header_token):
                        captured_auth_token = str(header_token).strip()

                    parsed = urlparse(request.url)
                    query = parse_qs(parsed.query)
                    query_token = query.get("user_auth_token", [None])[0]
                    query_user_id = query.get("user_id", [None])[0]

                    if self._is_viable_auth_token(query_token):
                        captured_auth_token = str(query_token).strip()
                    if query_user_id:
                        captured_user_id = query_user_id

                    url = request.url
                    if "qobuz.com" in url and "/login" in url:
                        post_data = request.post_data or ""
                        if post_data:
                            payload = parse_qs(post_data)
                            req_user = payload.get("email", [None])[0] or payload.get("username", [None])[0]
                            req_pass = payload.get("password", [None])[0]

                            if req_user and not captured_username and not self._is_error_artifact(req_user):
                                captured_username = req_user.strip()
                            if req_pass and not captured_password and not self._is_error_artifact(req_pass):
                                captured_password = req_pass.strip()
                except Exception:
                    pass

            page.on("request", on_request)
            
            # Anti-detection script
            await page.add_init_script("""
                Object.defineProperty(navigator, 'webdriver', {get: () => undefined});
            """)

            # Force a clean auth challenge. Qobuz can auto-redirect to discover with an existing
            # web session, which prevents us from capturing credentials needed for API fallback.
            self._log("Resetting Qobuz browser session before login...")
            try:
                await page.goto("https://www.qobuz.com/logout", wait_until="domcontentloaded", timeout=20000)
                await asyncio.sleep(1)
            except Exception as e:
                self._log(f"Logout pre-step skipped: {e}")
            
            self._log("Navigating to Qobuz login...")
            await page.goto(self.QOBUZ_URL, wait_until="domcontentloaded", timeout=30000)
            
            # Hotfix S84.1: Auto-dismiss regional popup if it appears
            try:
                self._log("Checking for regional redirection popup...")
                # Search for "Ir a Qobuz México" or similar regional switch buttons
                popup_btn = await page.wait_for_selector(
                    "text=Ir a Qobuz México, [href*='/mx-es'], button[class*='country'], .country-switch-btn",
                    timeout=5000
                )
                if popup_btn:
                    self._log("Regional popup detected, clicking to proceed to MX domain...")
                    await popup_btn.click()
                    # Wait for redirection to stabilize
                    await page.wait_for_load_state("networkidle", timeout=10000)
            except Exception:
                self._log("No regional popup detected or already on correct domain.")
            
            # Wait for user to log in
            self._log(f"Waiting for login (timeout: {timeout_seconds}s)...")
            
            start_time = time.time()
            session_data = None

            # S195(b): stability tracking for the token-capture navigation (see
            # should_attempt_token_capture_navigation). Polls run every ~2s, so a streak
            # of 2 means the post-login redirect chain had ~4s to settle before we may
            # navigate; the cooldown suppresses our own navigations for ~10s after we
            # detect a bounce back to /login so the user's redirect can finish.
            TOKEN_NAV_MIN_STREAK = 2
            TOKEN_NAV_BOUNCE_COOLDOWN_POLLS = 5
            logged_in_streak = 0
            token_nav_cooldown_polls = 0
            awaiting_nav_settle = False
            
            # FIX: detección de ventana cerrada compatible con cualquier
            # versión de Playwright (eventos en vez de is_closed()).
            _window_closed = {"v": False}
            browser.on("disconnected", lambda _: _window_closed.update(v=True))
            
            while time.time() - start_time < timeout_seconds:
                if _window_closed["v"]:
                    return False, "Cerraste la ventana del navegador sin completar el inicio de sesión — vuelve a intentar la conexión."

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
                full_cookies = {}
                
                for cookie in cookies:
                    name = cookie["name"]
                    value = cookie["value"]
                    full_cookies[name] = value
                    all_cookies[name] = value[:50] + "..." if len(value) > 50 else value
                    
                    # Check for user ID cookies
                    if name in ("qobuz_user_id", "user_id", "uid"):
                        user_id = value
                    # Check for auth token cookies
                    elif name in ("qobuz_user_token", "auth_token", "user_auth_token", "token", "session", "sid"):
                        auth_token = value

                auth_token = auth_token or captured_auth_token
                if not self._is_viable_auth_token(auth_token):
                    auth_token = None
                user_id = user_id or captured_user_id

                try:
                    form_values = await page.evaluate(
                        """
                        () => {
                            const usernameInput =
                                document.querySelector('input[type="email"]') ||
                                document.querySelector('input[name*="email" i]') ||
                                document.querySelector('input[name*="user" i]') ||
                                document.querySelector('input[id*="email" i]') ||
                                document.querySelector('input[id*="user" i]');
                            const passwordInput = document.querySelector('input[type="password"]');

                            return {
                                username: usernameInput ? usernameInput.value : null,
                                password: passwordInput ? passwordInput.value : null,
                            };
                        }
                        """
                    )

                    if isinstance(form_values, dict):
                        form_user = (form_values.get("username") or "").strip()
                        form_pass = (form_values.get("password") or "").strip()

                        if form_user and not self._is_error_artifact(form_user):
                            captured_username = form_user
                        if form_pass and not self._is_error_artifact(form_pass):
                            captured_password = form_pass
                except Exception as e:
                    self._log(f"Form capture failed: {e}")
                
                # Check if user is on a logged-in page (profile, favorites, etc.)
                try:
                    current_url = page.url
                    current_url_lc = current_url.lower()
                    is_login_page = "/login" in current_url_lc or "/signin" in current_url_lc
                    is_logged_in_url = any(
                        x in current_url_lc
                        for x in ["/profile", "/my-", "/favorit", "/playlist", "/account"]
                    )
                    has_session_cookie = bool(full_cookies.get("qobuz-session"))
                    has_uid_cookie = bool(full_cookies.get("uid") or user_id)
                    is_logged_in_page = (not is_login_page) and (is_logged_in_url or (has_session_cookie and has_uid_cookie))

                    # S195(b): maintain stability streak / bounce cooldown for the
                    # token-capture navigation policy.
                    if is_logged_in_page:
                        logged_in_streak += 1
                    else:
                        if awaiting_nav_settle and is_login_page:
                            # Our auxiliary navigation interrupted the post-login
                            # redirect and Qobuz bounced back to the login form; stop
                            # navigating for a while so the next user submit completes.
                            self._log(
                                "Bounced back to /login after token-capture navigation; "
                                f"cooling down auxiliary navigations for {TOKEN_NAV_BOUNCE_COOLDOWN_POLLS} polls"
                            )
                            token_nav_cooldown_polls = TOKEN_NAV_BOUNCE_COOLDOWN_POLLS
                        logged_in_streak = 0
                    awaiting_nav_settle = False
                    
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
                                            const match = text.match(/"user".*?"id"\\s*:\\s*(\\d+)/);
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

                        if should_attempt_token_capture_navigation(
                            is_logged_in_page=is_logged_in_page,
                            has_viable_token=self._is_viable_auth_token(auth_token),
                            logged_in_streak=logged_in_streak,
                            cooldown_polls_left=token_nav_cooldown_polls,
                            min_streak=TOKEN_NAV_MIN_STREAK,
                        ):
                            # The page has been stably logged-in across consecutive
                            # polls; navigating now can no longer cancel a post-login
                            # redirect in flight (S195(b)).
                            awaiting_nav_settle = True
                            self._log("No auth token from storefront, trying play.qobuz.com for token capture...")
                            
                            try:
                                # Navigate to web player — it will auto-login via shared cookies
                                # and issue API calls with X-User-Auth-Token
                                await page.goto("https://play.qobuz.com/favorites/albums", wait_until="domcontentloaded", timeout=15000)
                                # Wait for API calls to fire
                                await asyncio.sleep(5)
                                auth_token = captured_auth_token
                                if not self._is_viable_auth_token(auth_token):
                                    auth_token = None
                                    self._log("play.qobuz.com navigation didn't yield token from XHR headers")
                                else:
                                    self._log(f"Captured auth token from play.qobuz.com XHR! (len={len(auth_token)})")
                            except Exception as e:
                                self._log(f"play.qobuz.com navigation failed: {e}")

                            # Strategy 2: Use JS fetch inside browser context (inherits session cookies)
                            if not self._is_viable_auth_token(auth_token):
                                try:
                                    self._log("Trying JS fetch to Qobuz API from browser context...")
                                    js_result = await page.evaluate("""
                                        async () => {
                                            try {
                                                const resp = await fetch(
                                                    'https://www.qobuz.com/api.json/0.2/user/get?app_id=798273057',
                                                    { credentials: 'include' }
                                                );
                                                if (resp.ok) {
                                                    const data = await resp.json();
                                                    return {
                                                        user_auth_token: (data.user && data.user.auth_token) || data.user_auth_token || null,
                                                        user_id: (data.user && data.user.id && data.user.id.toString()) || (data.id && data.id.toString()) || null,
                                                    };
                                                }
                                                return { error: resp.status + ' ' + resp.statusText };
                                            } catch (e) {
                                                return { error: e.message };
                                            }
                                        }
                                    """)
                                    if isinstance(js_result, dict):
                                        js_token = js_result.get("user_auth_token")
                                        js_user = js_result.get("user_id")
                                        if self._is_viable_auth_token(js_token):
                                            auth_token = str(js_token).strip()
                                            self._log(f"Got auth token via JS fetch! (len={len(auth_token)})")
                                        if js_user:
                                            user_id = user_id or str(js_user)
                                        if js_result.get("error"):
                                            self._log(f"JS fetch error: {js_result['error']}")
                                except Exception as e:
                                    self._log(f"JS fetch failed: {e}")
                        
                        has_viable_token = self._is_viable_auth_token(auth_token)
                        has_fallback_creds = self._has_viable_credentials(captured_username, captured_password)

                        # Only accept success when we can continue in backend:
                        # - direct API token, or
                        # - fallback username/password captured from form/login request.
                        if has_viable_token or has_fallback_creds:
                            session_data = {
                                "user_id": user_id or "browser_session",
                                "auth_token": auth_token,
                                "cookies": full_cookies,
                                "browser_login": True,
                                "username": captured_username,
                                "password": captured_password,
                            }
                            self._log("Session captured from logged-in page!")
                            break
                        else:
                            self._log(
                                "Logged-in session detected but token/credentials are missing; waiting for explicit login signals..."
                            )
                            
                except Exception as e:
                    self._log(f"Page check error: {e}")
                
                # Also check localStorage for user data
                try:
                    storage_dump = await page.evaluate("""
                        () => {
                            const dumpStorage = (storage) => {
                                const data = {};
                                for (let i = 0; i < storage.length; i++) {
                                    const key = storage.key(i);
                                    if (!key) continue;
                                    try {
                                        data[key] = storage.getItem(key);
                                    } catch {
                                        // ignore unreadable keys
                                    }
                                }
                                return data;
                            };

                            return {
                                local: dumpStorage(localStorage),
                                session: dumpStorage(sessionStorage),
                            };
                        }
                    """)

                    if isinstance(storage_dump, dict):
                        def _apply_storage_map(items: Dict[str, Any]):
                            nonlocal user_id, auth_token
                            for raw_key, raw_val in items.items():
                                key = str(raw_key).lower()
                                if raw_val is None:
                                    continue

                                value = str(raw_val).strip()
                                if not value:
                                    continue

                                token_like_key = (
                                    key in ("user_auth_token", "auth_token", "access_token", "token")
                                    or key.endswith("_token")
                                    or ("qobuz" in key and "token" in key)
                                )
                                if token_like_key and self._is_viable_auth_token(value):
                                    auth_token = auth_token or value

                                if (key.endswith("user_id") or key == "uid" or "userid" in key) and value:
                                    user_id = user_id or value

                                parsed_obj = None
                                if value.startswith("{") and value.endswith("}"):
                                    try:
                                        parsed_obj = json.loads(value)
                                    except Exception:
                                        parsed_obj = None

                                if isinstance(parsed_obj, dict):
                                    parsed_token = (
                                        parsed_obj.get("user_auth_token")
                                        or parsed_obj.get("auth_token")
                                        or parsed_obj.get("access_token")
                                        or parsed_obj.get("token")
                                    )
                                    parsed_user = parsed_obj.get("user_id") or parsed_obj.get("id")

                                    if self._is_viable_auth_token(str(parsed_token) if parsed_token is not None else None):
                                        auth_token = auth_token or str(parsed_token).strip()
                                    if parsed_user:
                                        user_id = user_id or str(parsed_user).strip()

                                if auth_token is None:
                                    m = re.search(r'"(?:user_auth_token|auth_token|access_token|token)"\s*:\s*"([^"]+)"', value)
                                    if m:
                                        candidate = m.group(1).strip()
                                        if self._is_viable_auth_token(candidate):
                                            auth_token = candidate

                                if user_id is None:
                                    m_uid = re.search(r'"(?:user_id|id)"\s*:\s*"?([A-Za-z0-9_\-+=/]+)"?', value)
                                    if m_uid:
                                        candidate_uid = m_uid.group(1).strip()
                                        if candidate_uid:
                                            user_id = candidate_uid

                        _apply_storage_map(storage_dump.get("local") or {})
                        _apply_storage_map(storage_dump.get("session") or {})
                        if not self._is_viable_auth_token(auth_token):
                            auth_token = None
                except Exception as e:
                    # Page may be closed or navigating
                    self._log(f"localStorage check failed: {e}")
                    pass
                
                # Check if we have enough to authenticate
                if user_id and self._is_viable_auth_token(auth_token):
                    session_data = {
                        "user_id": user_id,
                        "auth_token": auth_token,
                        "cookies": full_cookies,
                        "username": captured_username,
                        "password": captured_password,
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

                if token_nav_cooldown_polls > 0:
                    token_nav_cooldown_polls -= 1

                await asyncio.sleep(2)
            
            try:
                await context.close()
                await browser.close()
            except:
                pass  # Already closed
            
            if session_data:
                if self._has_viable_credentials(captured_username, captured_password):
                    try:
                        cache = {}
                        if self.credentials_file.exists():
                            with open(self.credentials_file, 'r') as f:
                                cache = json.load(f)

                        cache["qobuz"] = {
                            "username": captured_username,
                            "password": captured_password,
                        }

                        with open(self.credentials_file, 'w') as f:
                            json.dump(cache, f, indent=2)

                        self._log("Captured Qobuz username/password for API fallback")
                    except Exception as e:
                        self._log(f"Failed to persist Qobuz credentials: {e}")

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
                    creds = cache.get("qobuz") or {}
                    if self._has_viable_credentials(creds.get("username"), creds.get("password")):
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

        if not self._is_viable_auth_token(session.get("auth_token")) and not self._has_viable_credentials(session.get("username"), session.get("password")):
            return {
                "status": "success",
                "connected": False,
                "message": "Session captured but missing API token and fallback credentials"
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

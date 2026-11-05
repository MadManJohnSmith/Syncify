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
        self.settings_file = settings_file
        self.verbose = verbose
        self._cached_token: Optional[str] = None
        self._access_token: Optional[str] = None
        self._storefront: Optional[str] = None

    @property
    def storefront(self) -> Optional[str]:
        """Storefront of the connected account, as reported by `me/storefront`.

        The Rust client uses it for every `/catalog/{storefront}/` request, so it
        must be part of the credentials saved at login.
        """
        return self._storefront
    
    def _log(self, message: str):
        if self.verbose:
            print(f"[Apple Music Auth] {message}")
    
    def get_stored_token(self) -> Optional[str]:
        """Get the stored media-user-token from memory or settings file. Also loads cached dev token."""
        if self._cached_token:
            return self._cached_token
        try:
            if self.settings_file and self.settings_file.exists():
                with open(self.settings_file, 'r') as f:
                    settings = json.load(f)
                    token = settings.get("apple_music_token", "")
                    self._access_token = settings.get("apple_music_dev_token", None)
                    if token:
                        self._cached_token = token
                        return token
        except Exception as e:
            self._log(f"Error reading settings: {e}")
        return None
    
    def save_token(self, token: str, dev_token: Optional[str] = None) -> bool:
        """Save the media-user-token (and optional dev token) in memory (and settings file if configured)."""
        self._cached_token = token
        if dev_token:
            self._access_token = dev_token
        if self.settings_file:
            try:
                settings = {}
                if self.settings_file.exists():
                    with open(self.settings_file, 'r') as f:
                        settings = json.load(f)
                
                settings["apple_music_token"] = token
                if dev_token:
                    settings["apple_music_dev_token"] = dev_token
                
                with open(self.settings_file, 'w') as f:
                    json.dump(settings, f, indent=2)
                
                self._log("Token saved successfully")
                return True
            except Exception as e:
                self._log(f"Error saving token: {e}")
                return False
        return True
    
    def clear_token(self) -> bool:
        """Clear the stored token (logout)."""
        self._cached_token = None
        self._access_token = None
        if self.settings_file and self.settings_file.exists():
            try:
                with open(self.settings_file, 'r') as f:
                    settings = json.load(f)
                settings["apple_music_token"] = ""
                with open(self.settings_file, 'w') as f:
                    json.dump(settings, f, indent=2)
                return True
            except Exception as e:
                self._log(f"Error clearing token: {e}")
                return False
        return True
    
    def _fetch_access_token(self) -> Optional[str]:
        """Fallback: extract the Apple developer JWT from the site using requests.

        FIX 2026-08-25: the owner's network is flagged by Apple's anti-bot
        (empty responses), so this can fail even when the real browser works —
        that is why browser interception is ALWAYS attempted first. Here: real
        UA, timeouts, HTML scanning and up to 6 JS bundles, with per-stage
        diagnostics."""
        import requests as _rq
        H = {"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
             "Accept-Language": "en-US,en;q=0.9"}
        jwt_re = re.compile(r'(eyJhbGciOi[^"\' ]{60,700})')
        try:
            r = _rq.get(f'{self.APPLE_MUSIC_URL}/us/browse', headers=H, timeout=15)
            if r.status_code != 200 or not r.text:
                self._log(f"Apple page not reachable (HTTP {r.status_code}, {len(r.text)} bytes) — network blocked?")
                return None
            m = jwt_re.search(r.text)
            if m:
                self._access_token = m.group(1)
                self._log("Dev token extracted from HTML")
                return self._access_token
            bundles = list(dict.fromkeys(re.findall(r"/assets/[^\"]+[.]js", r.text)))[:6]
            for b in bundles:
                try:
                    rj = _rq.get(f'https://music.apple.com{b}', headers=H, timeout=12)
                    if rj.status_code == 200:
                        mj = jwt_re.search(rj.text)
                        if mj:
                            self._access_token = mj.group(1)
                            self._log(f"Dev token extracted from {b[:40]}…")
                            return self._access_token
                except Exception as ej:
                    self._log(f"Bundle {b[:40]}… failed: {ej}")
            self._log("No JWT in HTML or bundles")
            return None
        except Exception as e:
            self._log(f"Error fetching access token: {e}")
            return None

    def validate_token(self, token: Optional[str] = None) -> Tuple[Optional[bool], str]:
        """
        Validate the media-user-token against Apple Music API.

        Returns a THREE-valued verdict (A-2):
          * True  → the token really works (verified with a 200).
          * False → the provider rejected it (401/403). Truly expired.
          * None  → COULD NOT BE VERIFIED (5xx, 429, timeout, DNS, parse).

        The third value is what was missing: before, any status other than
        200 and any exception fell into the same `else`/`except` and
        returned False, and the consumer translated that to "Token expired". That
        pushed the user to re-login just because Apple had a 503, when the
        session was still perfectly alive.

        Returns:
            (is_valid_or_None, storefront_or_error_message)
        """
        token = token or self.get_stored_token()
        if not token:
            return False, "No token provided"

        # Get access token first
        if not self._access_token:
            self._fetch_access_token()

        if not self._access_token:
            # The developer token was not obtained: this is an environment/network
            # failure, not a rejection of the user's token. Before, this also showed
            # up as False and therefore as "Token expired".
            return None, ("could not obtain the Apple developer token "
                          "(its network looks blocked for direct requests); "
                          "retry — normally the second attempt gets it")

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
                if storefront != "unknown":
                    self._storefront = storefront
                self._log(f"Token valid (storefront: {storefront})")
                return True, storefront
            elif response.status_code in (401, 403):
                # Explicit rejection: here we can indeed say it expired.
                self._log(f"Token invalid: HTTP {response.status_code}")
                return False, f"Invalid token (HTTP {response.status_code})"
            else:
                # 429, 5xx, and any other status: unavailability. Says NOTHING
                # about the user's token.
                self._log(f"Could not verify token: HTTP {response.status_code}")
                return None, (f"Could not verify session "
                              f"(upstream HTTP {response.status_code})")

        except Exception as e:
            # Timeout, DNS, SSL, malformed JSON: not verifiable.
            self._log(f"Validation error: {e}")
            return None, f"Could not verify session ({e})"
    
    async def _read_typed_apple_id(self, context):
        """Read the Apple ID email typed in the sign-in form, when visible.

        Apple exposes no profile API for the `media-user-token`, so the Apple
        ID typed during sign-in is the only identity this flow can capture.
        Best effort: any failure leaves the account generic (the user can
        always rename it from the accounts settings).
        """
        try:
            pages = context.pages
        except Exception:
            return None
        for page in pages:
            try:
                frames = page.frames
            except Exception:
                continue
            for frame in frames:
                try:
                    value = await frame.evaluate(
                        """() => {
                            const EMAIL_RE = /^[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\\.[A-Za-z0-9-]+)*\\.[A-Za-z]{2,}$/;
                            const read = (el) => {
                                if (!el) return null;
                                const value = typeof el.value === 'string' ? el.value.trim() : '';
                                return EMAIL_RE.test(value) ? value : null;
                            };
                            const selectors = [
                                'input#account_name_text_field',
                                'input[type="email"]',
                                'input[name="accountName"]',
                                'input[name="appleId"]',
                                'input[autocomplete="username"]',
                            ];
                            for (const selector of selectors) {
                                const value = read(document.querySelector(selector));
                                if (value) return value;
                            }
                            // Markup-drift safety net: only on Apple pages and only
                            // for fully typed addresses in visible inputs.
                            if (/(^|\\.)apple\\.com$/.test(location.hostname)) {
                                const inputs = document.querySelectorAll('input[type="text"], input[type="email"]');
                                for (const el of inputs) {
                                    if (el.offsetParent === null) continue;
                                    const value = read(el);
                                    if (value) return value;
                                }
                            }
                            return null;
                        }"""
                    )
                except Exception:
                    continue
                if not value:
                    continue
                candidate = str(value).strip()
                if "@" in candidate and "." in candidate.split("@")[-1]:
                    return candidate
        return None

    async def login_with_browser(self, timeout_seconds: int = 300) -> Tuple[bool, str]:
        """
        Open browser for user to log in and capture the media-user-token.
        
        Args:
            timeout_seconds: How long to wait for login (default 5 minutes)
            
        Returns:
            (success, token_or_error_message)
        """
        # Identity captured from the sign-in form; cleared on every attempt so a
        # retry can never persist a previous account's email.
        self._captured_apple_id = None
        try:
            from playwright.async_api import async_playwright
        except ImportError:
            return False, "Playwright not installed. Run: pip install playwright && python -m playwright install chromium"

        # System browser (owner decision 2026-08-25): never the
        # downloadable Chromium from ms-playwright.
        from services.browser_launcher import chrome_launch_kwargs
        
        self._log("Starting browser login flow...")
        
        async with async_playwright() as p:
            # Use a dedicated profile in user temp directory
            import os
            import tempfile
            profile_dir = Path(tempfile.gettempdir()) / "syncify_browser_profiles" / "apple"
            profile_dir.mkdir(parents=True, exist_ok=True)
            user_data_dir = str(profile_dir)
            
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
                # FIX 2026-08-25: the fallback no longer tries Playwright's
                # managed Chromium (binary not downloaded → "Executable doesn't
                # exist"); we ALWAYS resolve a system browser.
                self._log(f"Chrome via channel failed ({e}); using system browser")
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
            captured_apple_id = None
            
            # FIX: closed-window detection compatible with any
            # Playwright version (events instead of is_closed()).
            _window_closed = {"v": False}
            context.on("close", lambda _: _window_closed.update(v=True))
            
            while time.time() - start_time < timeout_seconds:
                if _window_closed["v"]:
                    return False, "You closed the browser window before completing sign-in — please try connecting again."

                # Check for the media-user-token cookie
                cookies = await context.cookies()
                for cookie in cookies:
                    if cookie["name"] == self.TOKEN_COOKIE_NAME:
                        token = cookie["value"]
                        break
                
                if token:
                    self._log("Token captured!")
                    break
                
                # The Apple ID is typed in the sign-in form: capture it while
                # the user is on that page so the account is not generic. Read
                # on every poll and keep the LAST complete address seen: a
                # value read mid-typing ("user@gmail.co") must not win over
                # the final one ("user@gmail.com").
                value = await self._read_typed_apple_id(context)
                if value:
                    captured_apple_id = value

                # Wait a bit before checking again
                await asyncio.sleep(1)
            
            # FIX 2026-08-25 (ordering): the close() used to come BEFORE the reload — with
            # a remembered session the token appeared in ~1s, the browser closed,
            # and the reload ran against a dead context. Now the reload
            # happens FIRST (with a remembered session this gives the player the
            # seconds it needs to emit its developer token) and the close
            # happens at the end of each branch.
            if token and not dev_token_found:
                self._log("Developer token missing; reloading to force API calls…")
                try:
                    await page.reload(wait_until="domcontentloaded", timeout=20000)
                    deadline = time.time() + 15
                    while time.time() < deadline and not dev_token_found:
                        await asyncio.sleep(1)
                except Exception as e2:
                    self._log(f"Reload failed: {e2}")

            if dev_token_found:
                self._access_token = dev_token_found

            await context.close()

            # Never the value itself, only whether it could be captured: this
            # distinguishes "typed but not read" from "never typed".
            self._log("Apple ID email captured: " + ("yes" if captured_apple_id else "no"))

            if token:
                
                # Validate the token
                is_valid, result = self.validate_token(token)
                if is_valid:
                    self._captured_apple_id = captured_apple_id
                    self.save_token(token, self._access_token)
                    return True, token
                elif is_valid is None:
                    # A-2: the token was just captured from a real browser
                    # session, so the API check is a verdict
                    # on OUR verification, not on the token. Discarding it
                    # would force repeating a login that did work (typical
                    # when Apple returns a 5xx right then).
                    # It is saved and the verification error is returned.
                    self._captured_apple_id = captured_apple_id
                    self.save_token(token, self._access_token)
                    self._log(f"Token saved unverified: {result}")
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
        elif is_valid is None:
            # A-2: could not be verified. The session is NOT declared expired: the
            # verification failure is reported so the UI can offer
            # retry instead of forcing a re-login.
            return {
                "connected": False,
                "status": f"Could not verify session: {result}",
                "storefront": None
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
            "message": "Successfully connected to Apple Music" if success else result,
            # Apple exposes no profile API; this is the Apple ID typed at
            # sign-in, when the browser flow could read it. None otherwise.
            "email": getattr(auth, "_captured_apple_id", None),
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

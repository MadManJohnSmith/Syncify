"""System browser resolution for the auth flows (Playwright).

Supports Windows (including installs without Edge / Tiny11 / debloated),
Linux and macOS, with stub/dummy detection and support for Chrome, Brave,
Vivaldi, Opera, Thorium, Chromium and Edge.
"""
import os
import shutil
import subprocess
import sys

_LINUX_CANDIDATES = [
    "/usr/bin/google-chrome-stable",
    "/usr/bin/google-chrome",
    "/usr/bin/brave-browser",
    "/usr/bin/brave",
    "/usr/bin/vivaldi-stable",
    "/usr/bin/vivaldi",
    "/usr/bin/chromium-browser",
    "/usr/bin/chromium",
    "/opt/google/chrome/chrome",
    "/usr/bin/microsoft-edge-stable",
    "/usr/bin/microsoft-edge",
]

_MACOS_CANDIDATES = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    "/Applications/Vivaldi.app/Contents/MacOS/Vivaldi",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Arc.app/Contents/MacOS/Arc",
]


def _is_valid_browser_exe(path: str) -> bool:
    """Check whether an executable file exists and is NOT an empty dummy/stub."""
    if not path or not os.path.exists(path):
        return False
    try:
        # Dummy/stub executables left by uninstalls on Windows are usually 0 KB or < 50 KB.
        # A real compiled browser weighs at least 300 KB - 3 MB.
        size = os.path.getsize(path)
        return size > 150_000
    except OSError:
        return False


def _get_windows_candidates():
    """Look for browsers in the standard Windows folders without assuming Edge exists."""
    candidates = []

    # Allow an explicit override via environment variable
    custom_path = os.environ.get("SYNCIFY_BROWSER_PATH") or os.environ.get("CHROME_PATH")
    if custom_path and _is_valid_browser_exe(custom_path):
        candidates.append(custom_path)

    env_roots = []
    for var in ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"]:
        val = os.environ.get(var)
        if val and val not in env_roots:
            env_roots.append(val)

    # 1. Google Chrome
    for root in env_roots:
        candidates.append(os.path.join(root, "Google", "Chrome", "Application", "chrome.exe"))

    # 2. Brave Browser
    for root in env_roots:
        candidates.append(os.path.join(root, "BraveSoftware", "Brave-Browser", "Application", "brave.exe"))

    # 3. Vivaldi
    for root in env_roots:
        candidates.append(os.path.join(root, "Vivaldi", "Application", "vivaldi.exe"))

    # 4. Thorium
    for root in env_roots:
        candidates.append(os.path.join(root, "Thorium", "Application", "thorium.exe"))

    # 5. Opera & Opera GX
    for root in env_roots:
        candidates.append(os.path.join(root, "Programs", "Opera", "launcher.exe"))
        candidates.append(os.path.join(root, "Programs", "Opera GX", "launcher.exe"))

    # 6. Chromium / Ungoogled Chromium
    for root in env_roots:
        candidates.append(os.path.join(root, "Chromium", "Application", "chrome.exe"))

    # 7. Microsoft Edge (only if the binary is legitimate and not a 0-byte dummy)
    for root in env_roots:
        candidates.append(os.path.join(root, "Microsoft", "Edge", "Application", "msedge.exe"))

    return candidates


def chrome_launch_kwargs() -> dict:
    """Kwargs for `p.chromium.launch*()` using the system browser.

    Verifies real executables, discarding uninstall dummies.
    """
    is_windows = sys.platform.startswith("win") or os.name == "nt"
    is_mac = sys.platform == "darwin"

    # Explicit override
    custom = os.environ.get("SYNCIFY_BROWSER_PATH") or os.environ.get("CHROME_PATH")
    if custom and _is_valid_browser_exe(custom):
        return {"executable_path": custom}

    if is_windows:
        # 1. Check full-path executables and validate size to reject dummies
        for cand in _get_windows_candidates():
            if _is_valid_browser_exe(cand):
                return {"executable_path": cand}

        # 2. Check in PATH, discarding dummies
        for cmd in ["chrome", "brave", "vivaldi"]:
            resolved = shutil.which(cmd)
            if resolved and _is_valid_browser_exe(resolved):
                return {"executable_path": resolved}

        # Check edge in PATH with strict validation
        edge_cmd = shutil.which("msedge")
        if edge_cmd and _is_valid_browser_exe(edge_cmd):
            return {"executable_path": edge_cmd}

    elif is_mac:
        for cand in _MACOS_CANDIDATES:
            if _is_valid_browser_exe(cand):
                return {"executable_path": cand}
    else:
        # Linux
        for cand in _LINUX_CANDIDATES:
            if _is_valid_browser_exe(cand):
                return {"executable_path": cand}
        for cmd in ["google-chrome-stable", "google-chrome", "brave-browser", "chromium", "chromium-browser"]:
            resolved = shutil.which(cmd)
            if resolved and _is_valid_browser_exe(resolved):
                return {"executable_path": resolved}

    # If no valid system browser was found:
    raise RuntimeError(
        "No valid Chromium browser was detected on the system (Chrome, Brave, Vivaldi, Opera, etc.). "
        "If your Windows install does not have Microsoft Edge, please install Google Chrome or Brave, "
        "or set the SYNCIFY_BROWSER_PATH environment variable to your browser's path."
    )


# Desktop URL-handler openers, in preference order per platform. `webbrowser`
# only knows `BROWSER` and `xdg-open`, so on a machine without xdg-utils it
# returns False without launching anything.
_LINUX_URL_OPENERS = ["xdg-open", "gio", "gnome-open", "kde-open", "x-www-browser", "wslview"]
_MACOS_URL_OPENERS = ["open"]
_WINDOWS_URL_OPENERS = ["explorer"]


def _platform_url_openers():
    if sys.platform.startswith("win") or os.name == "nt":
        return list(_WINDOWS_URL_OPENERS)
    if sys.platform == "darwin":
        return list(_MACOS_URL_OPENERS)
    return list(_LINUX_URL_OPENERS)


def open_in_system_browser(url: str) -> bool:
    """Open `url` in the desktop browser. True only if it was actually launched.

    `webbrowser.open()` returns False —without raising— when it finds neither
    `BROWSER` nor `xdg-open`, and Tidal's device code flow cannot continue
    without that tab, so the return value is checked and platform-specific
    openers are tried as fallbacks until one opens the URL.
    """
    import webbrowser

    try:
        if webbrowser.open(url, new=2):
            return True
    except Exception:
        pass

    for opener in _platform_url_openers():
        resolved = shutil.which(opener)
        if not resolved:
            continue
        argv = [resolved, "open", url] if opener == "gio" else [resolved, url]
        try:
            # `explorer.exe` returns 1 even when it opens the window successfully,
            # so any exit status is accepted as long as it is not a launch failure.
            completed = subprocess.run(
                argv,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=10,
            )
            if completed.returncode in (0, 1):
                return True
        except (OSError, subprocess.SubprocessError):
            continue

    return False

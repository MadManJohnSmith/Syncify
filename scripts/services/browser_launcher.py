"""System browser resolution for the auth flows (Playwright).

Supports Windows (including installs without Edge / Tiny11 / debloated),
Linux and macOS, with stub/dummy detection and support for Chrome, Brave,
Vivaldi, Opera, Thorium, Chromium and Edge.
"""
import os
import re
import shutil
import subprocess
import sys

_LINUX_CANDIDATES = [
    "/usr/bin/google-chrome-stable",
    "/usr/bin/google-chrome",
    "/usr/bin/brave-browser",
    "/usr/bin/brave-browser-stable",
    "/usr/bin/brave",
    "/usr/bin/vivaldi-stable",
    "/usr/bin/vivaldi",
    "/usr/bin/chromium-browser",
    "/usr/bin/chromium",
    "/opt/google/chrome/chrome",
    "/opt/brave.com/brave/brave",
    "/usr/bin/microsoft-edge-stable",
    "/usr/bin/microsoft-edge",
    # Snap and Flatpak exports: many distros only expose the browser through
    # these launchers, and the Flatpak ones are tiny wrapper scripts (the size
    # heuristic alone rejected them before the --version probe existed).
    "/snap/bin/chromium",
    "/var/lib/flatpak/exports/bin/org.chromium.Chromium",
    "/var/lib/flatpak/exports/bin/com.google.Chrome",
    "/var/lib/flatpak/exports/bin/com.brave.Browser",
]

_MACOS_CANDIDATES = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    "/Applications/Vivaldi.app/Contents/MacOS/Vivaldi",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Arc.app/Contents/MacOS/Arc",
]

# Version banner of a Chromium-family browser, used to tell a real launcher
# (possibly a small wrapper script) from an uninstall stub.
_BROWSER_VERSION_RE = re.compile(
    r"(Chrome|Chromium|Brave|Edge|Vivaldi|Opera|Thorium)", re.IGNORECASE
)


def _is_valid_browser_exe(path: str) -> bool:
    """Check whether an executable file exists and is a real browser.

    Large binaries are trusted by size. Small files are not rejected outright
    anymore: distros often ship thin wrapper scripts (snap, Flatpak exports,
    Debian's chromium launcher), so they are probed with ``--version`` and
    accepted only when they announce a Chromium-family browser. Windows
    uninstall dummies are 0 KB stubs that produce no such output.
    """
    if not path or not os.path.exists(path):
        return False
    try:
        # Dummy/stub executables left by uninstalls on Windows are usually 0 KB or < 50 KB.
        # A real compiled browser weighs at least 300 KB - 3 MB.
        if os.path.getsize(path) > 150_000:
            return True
    except OSError:
        return False

    if not os.path.isfile(path) or not os.access(path, os.X_OK):
        return False

    try:
        completed = subprocess.run(
            [path, "--version"],
            capture_output=True,
            text=True,
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError):
        return False
    output = f"{completed.stdout or ''} {completed.stderr or ''}"
    return bool(_BROWSER_VERSION_RE.search(output))


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
    # El navegador que Playwright lance hereda este entorno: sin la limpieza
    # moriría al cargar por las librerías del paquete (mismo mal que los
    # abridores de URL).
    scrub_loader_env()
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
        for cmd in [
            "google-chrome-stable",
            "google-chrome",
            "brave-browser",
            "brave",
            "chromium",
            "chromium-browser",
            "vivaldi-stable",
            "microsoft-edge-stable",
        ]:
            resolved = shutil.which(cmd)
            if resolved and _is_valid_browser_exe(resolved):
                return {"executable_path": resolved}

    # If no valid system browser was found:
    raise RuntimeError(
        "No valid Chromium browser was detected on the system (Chrome, Chromium, Brave, "
        "Vivaldi, Edge, Opera or Thorium). Install one of them (on Linux, for example, the "
        "'chromium' or 'brave-browser' package; on Windows, Google Chrome or Edge), or set "
        "the SYNCIFY_BROWSER_PATH environment variable to your browser's executable path."
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


def _appdir_prefixes():
    """Rutas raíz del propio paquete que pueden ensombrecer librerías del sistema."""
    roots = set()
    appdir = os.environ.get("APPDIR")
    if appdir:
        roots.add(os.path.realpath(appdir))
    # Runtime AppImage (APPIMAGE apunta al .AppImage montado en /tmp/.mount_XXXX)
    appimage = os.environ.get("APPIMAGE")
    if appimage:
        roots.add(os.path.realpath(os.path.dirname(appimage)))
    # Extracción manual (--appimage-extract) y ejecutables del bundle
    for probe in (sys.executable, os.path.dirname(sys.executable)):
        if probe:
            roots.add(os.path.realpath(probe))
    return roots


def _is_bundled_path(path: str) -> bool:
    """True si `path` vive dentro del paquete (AppImage montado o extraído)."""
    if not path:
        return False
    real = os.path.realpath(path)
    if "/.mount_" in real or "/squashfs-root/" in real:
        return True
    for root in _appdir_prefixes():
        if real == root or real.startswith(root + os.sep):
            return True
    return False


def scrub_loader_env() -> None:
    """Quita de LD_LIBRARY_PATH/LD_PRELOAD las rutas de librerías del paquete.

    Dentro del AppImage, el runtime exporta LD_LIBRARY_PATH apuntando a las
    copias de Ubuntu 22.04 (libssl, glib, etc.). Los abridores de URL del
    escritorio que este módulo lanza (xdg-open/kde-open, gio) enlazan librerías
    del anfitrión más nuevas y mueren al cargar: "libssl.so.3: version
    'OPENSSL_3.5.0' not found", "gio: undefined symbol
    g_unix_mount_entry_get_options". Peor aún, `webbrowser.open()` lanza el
    hijo sin esperar su salida y devuelve True, así que el flujo de auth cree
    que la pestaña se abrió y se queda sondeando hasta el timeout sin que
    ningún navegador aparezca. Los hijos (abridores y navegadores que
    Playwright lance) deben resolver TODAS sus librerías en el anfitrión: se
    limpia el entorno de este subproceso, que es efímero y ya cargó las suyas.
    """
    for var in ("LD_LIBRARY_PATH", "LD_PRELOAD"):
        raw = os.environ.get(var)
        if not raw:
            continue
        if var == "LD_PRELOAD":
            # LD_PRELOAD separa entradas con espacios, no con ':'
            entries = raw.split(" ")
            kept = [e for e in entries if e and not _is_bundled_path(e)]
        else:
            entries = raw.split(os.pathsep)
            kept = [e for e in entries if e and not _is_bundled_path(e)]
        if kept == entries:
            continue
        if kept:
            sep = " " if var == "LD_PRELOAD" else os.pathsep
            os.environ[var] = sep.join(kept)
        else:
            del os.environ[var]


def open_in_system_browser(url: str) -> bool:
    """Open `url` in the desktop browser. True only if it was actually launched.

    `webbrowser.open()` returns False —without raising— when it finds neither
    `BROWSER` nor `xdg-open`, and Tidal's device code flow cannot continue
    without that tab, so the return value is checked and platform-specific
    openers are tried as fallbacks until one opens the URL.
    """
    # Antes de lanzar CUALQUIER hijo: dentro del AppImage, xdg-open/gio mueren
    # al cargar por el LD_LIBRARY_PATH del paquete y webbrowser.open devuelve
    # True aunque el navegador jamás aparezca (ver scrub_loader_env).
    scrub_loader_env()

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

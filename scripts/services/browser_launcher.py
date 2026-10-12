"""Resolución del navegador del sistema para los flujos de auth (Playwright).

Soporta Windows (incluyendo instalaciones sin Edge / Tiny11 / debloated),
Linux y macOS, con detección de stubs/dummies y soporte para Chrome, Brave,
Vivaldi, Opera, Thorium, Chromium y Edge.
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
    """Verifica si un archivo ejecutable existe y NO es un dummy/stub vacío."""
    if not path or not os.path.exists(path):
        return False
    try:
        # Los ejecutables dummy/stub de Windows desinstalados suelen medir 0 KB o < 50 KB.
        # Un navegador real compilado pesa al menos 300 KB - 3 MB.
        size = os.path.getsize(path)
        return size > 150_000
    except OSError:
        return False


def _get_windows_candidates():
    """Busca navegadores en las carpetas estándar de Windows sin asumir que Edge existe."""
    candidates = []

    # Permite override explícito mediante variable de entorno
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

    # 7. Microsoft Edge (solo si el binario es legítimo y no un dummy de 0 bytes)
    for root in env_roots:
        candidates.append(os.path.join(root, "Microsoft", "Edge", "Application", "msedge.exe"))

    return candidates


def chrome_launch_kwargs() -> dict:
    """Kwargs para `p.chromium.launch*()` usando el navegador del sistema.

    Verifica ejecutables reales, descartando dummies de desinstalación.
    """
    # El navegador que Playwright lance hereda este entorno: sin la limpieza
    # moriría al cargar por las librerías del paquete (mismo mal que los
    # abridores de URL).
    scrub_loader_env()
    is_windows = sys.platform.startswith("win") or os.name == "nt"
    is_mac = sys.platform == "darwin"

    # Override explícito
    custom = os.environ.get("SYNCIFY_BROWSER_PATH") or os.environ.get("CHROME_PATH")
    if custom and _is_valid_browser_exe(custom):
        return {"executable_path": custom}

    if is_windows:
        # 1. Comprobar ejecutables con ruta completa y validar tamaño anti-dummy
        for cand in _get_windows_candidates():
            if _is_valid_browser_exe(cand):
                return {"executable_path": cand}

        # 2. Comprobar en PATH descartando dummies
        for cmd in ["chrome", "brave", "vivaldi"]:
            resolved = shutil.which(cmd)
            if resolved and _is_valid_browser_exe(resolved):
                return {"executable_path": resolved}

        # Comprobar edge en PATH con validación estricta
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

    # Si no se encontró ningún navegador del sistema válido:
    raise RuntimeError(
        "No se detectó un navegador Chromium válido en el sistema (Chrome, Brave, Vivaldi, Opera, etc.). "
        "Si tu Windows no tiene Microsoft Edge instalado, por favor instala Google Chrome o Brave, "
        "o define la variable de entorno SYNCIFY_BROWSER_PATH con la ruta de tu navegador."
    )


# Apertores del gestor de URLs del escritorio, en orden de preferencia por
# plataforma. `webbrowser` sólo conoce `BROWSER` y `xdg-open`, así que en una
# máquina sin xdg-utils devuelve False sin lanzar nada.
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
    """Abre `url` en el navegador del escritorio. True solo si se lanzó de verdad.

    `webbrowser.open()` devuelve False —sin excepción— cuando no encuentra
    `BROWSER` ni `xdg-open`, y el flujo de device code de Tidal no puede seguir
    sin esa pestaña, así que el valor de retorno se comprueba y se degradan
    aperturas explícitas por plataforma hasta que una abre la URL.
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
            # `explorer.exe` devuelve 1 aunque abra la ventana con éxito, así
            # que se acepta cualquier salida siempre que no sea un fallo de arranque.
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

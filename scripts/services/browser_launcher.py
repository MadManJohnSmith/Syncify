"""Resolución del navegador del sistema para los flujos de auth (Playwright).

Soporta Windows (incluyendo instalaciones sin Edge / Tiny11 / debloated),
Linux y macOS, con detección de stubs/dummies y soporte para Chrome, Brave,
Vivaldi, Opera, Thorium, Chromium y Edge.
"""
import os
import sys
import shutil

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

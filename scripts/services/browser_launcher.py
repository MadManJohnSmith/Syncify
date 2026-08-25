"""Resolución del navegador del sistema para los flujos de auth (Playwright).

Decisión 2026-08-25 (propietario): NO depender del Chromium gestionado por
Playwright (`~/.cache/ms-playwright`) porque exige descargar un binario por
versión; usamos el Chrome/Chromium YA INSTALADO en el sistema vía
`channel="chrome"` o `executable_path`.
"""
import os
import shutil

_CANDIDATES = [
    "/usr/bin/google-chrome-stable",
    "/usr/bin/google-chrome",
    "/opt/google/chrome/chrome",
    "/usr/bin/chromium-browser",
    "/usr/bin/chromium",
]


def chrome_launch_kwargs() -> dict:
    """Kwargs para `p.chromium.launch*()` usando el navegador del sistema.

    - `channel="chrome"` cuando hay Chrome disponible (PATH o ubicación estándar).
    - Si no, `executable_path` al primer Chromium encontrado.
    - Nunca devuelve vacío: si no hay navegador del sistema lanza RuntimeError
      accionable en vez de dejar que Playwright busque su binario descargable.
    """
    if (
        shutil.which("google-chrome-stable")
        or shutil.which("google-chrome")
        or shutil.which("chromium")
        or shutil.which("chromium-browser")
        or os.path.exists("/opt/google/chrome/chrome")
    ):
        return {"channel": "chrome"}
    for cand in _CANDIDATES:
        if os.path.exists(cand):
            return {"executable_path": cand}
    raise RuntimeError(
        "No se encontró Chrome ni Chromium del sistema. Instala Google Chrome "
        "(recomendado) o ejecuta `python -m playwright install chromium`."
    )

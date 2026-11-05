#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Probe for the FREE tier of the Spotify Web API (reading one's OWN library).

PURPOSE
=======
Produce a documented VERDICT on whether reading one's own Spotify library
(saved tracks + own playlists) still works for **FREE** accounts
after the November 2024 changes, using your own developer app
(client_id/client_secret). The script changes NOTHING in the account:
it only does bounded reads (?limit=1) and reports totals.

FLOW (OAuth2 authorization-code with localhost redirect)
========================================================
1. Starts an ephemeral http.server on 127.0.0.1:8899 (or the first free port).
2. Opens the browser at https://accounts.spotify.com/authorize with the scopes
   user-library-read playlist-read-private playlist-read-collaborative
   user-read-email, show_dialog=true and a random anti-CSRF `state`
   parameter that is verified on return.
3. Captures ?code= at /callback and exchanges it for an access_token at
   https://accounts.spotify.com/api/token (Basic auth client_id:secret,
   form-urlencoded grant_type=authorization_code). 400/403 errors are
   printed ONLY as status + SANITIZED body.
4. With the token ONLY in memory, sequentially queries GET /me,
   GET /me/tracks?limit=1, GET /me/playlists?limit=1 and, if there is >=1 playlist,
   GET /playlists/{id}/tracks?limit=1.
5. Prints a results table and a final VERDICT, which is also written
   next to this file in spotify_free_tier_probe_result.md.

PRIVACY GUARANTEES (important)
==============================
- NEVER prints complete SPOTIFY_CLIENT_ID or SPOTIFY_CLIENT_SECRET: the
  credentials are read at runtime from the environment or from the repo-root
  `.env` (simple KEY=VALUE lines, comments ignored) and every
  occurrence in messages is MASKED.
- The access_token lives only in memory during the run: it is never
  printed, saved to disk or included in displayed URLs.
- Error bodies are shown truncated and SANITIZED (any pattern
  like token/code/secret/authorization replaced with [REDACTED]).
- The user name is shown truncated to 2 characters + "***"; the email
  requested by scope is never shown.

USAGE
=====
    python3 scripts/maintenance/spotify_free_tier_probe.py

Requires SPOTIFY_CLIENT_ID and SPOTIFY_CLIENT_SECRET in the environment or in `.env`.
Per-request timeout: 15 s. Ctrl+C exits cleanly and always closes the server.

POSSIBLE VERDICTS
=================
- FUNCIONA_CON_FREE                 product=free and all 4 endpoints return 200
                                    with totals.
- FUNCIONA_PARCIAL                  It authenticated but at least one applicable endpoint
                                    failed; the table details which returned 200 and
                                    which failed (status + short reason).
- BLOQUEADO_<reason>                The probe could not be completed (e.g.
                                    BLOQUEADO_CREDENCIALES_INVALIDAS,
                                    BLOQUEADO_QUOTA_EXCEDIDA_O_ACCESO_APP,
                                    BLOQUEADO_HTTP_403_EN_ME,
                                    BLOQUEADO_USUARIO_NEGO_PERMISOS).
- PREMIUM_NO_INFORMATIVO_PARA_FREE  The authenticated account is premium: everything
                                    worked, but it does NOT evidence the FREE tier.

EXIT CODES: 0 favorable/informative verdict, 3 BLOQUEADO_*,
4 configuration error, 130 interrupted by Ctrl+C, 1 unexpected error.
"""

import base64
import json
import os
import re
import secrets
import socket
import sys
import time
import urllib.request
import webbrowser
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib import error as urlerror
from urllib import parse as urlpar

PROBE_VERSION = "1.0.0"
HOST_REDIRECT = "127.0.0.1"
PUERTO_PREFERIDO = 8899
TIMEOUT_REQUEST_SEG = 15
TIMEOUT_ESPERA_NAVEGADOR_SEG = 300

SCOPE_SOLICITADOS = [
    "user-library-read",
    "playlist-read-private",
    "playlist-read-collaborative",
    "user-read-email",
]

URL_AUTORIZAR = "https://accounts.spotify.com/authorize"
URL_TOKEN = "https://accounts.spotify.com/api/token"
URL_API_BASE = "https://api.spotify.com/v1"
ARCHIVO_RESULTADO = "spotify_free_tier_probe_result.md"

NOTA_ENMASCARADO = ("All outputs were MASKED: no complete client_id/secret, "
                    "no tokens, no email or full names.")

_PATRON_FUGA = re.compile(
    r"(?i)\b(access_token|refresh_token|client_secret|authorization|code|token|secret)"
    r"(\"|')?\s*([=:]\s*)"
    r"(\"[^\"]*\"|'[^']*'|[^\s&,}\]]+)"
)


# ---------------------------------------------------------------------------
# Masking / sanitizing helpers
# ---------------------------------------------------------------------------

def sanitizar_texto(texto):
    """Replace values following sensitive keys with [REDACTED].

    Covers both JSON ("access_token":"x") and form-urlencoded (code=x).
    """
    if not texto:
        return ""
    def _reemplazo(coincidencia):
        comilla = coincidencia.group(2) or ""
        return "%s%s%s%s[REDACTED]%s" % (
            coincidencia.group(1), comilla, coincidencia.group(3), comilla, comilla)
    return _PATRON_FUGA.sub(_reemplazo, texto)


def recortar(texto, maximo=240):
    texto = texto or ""
    return texto if len(texto) <= maximo else texto[:maximo] + "..."


def enmascarar_id(valor, n=4):
    """First n characters + *** (for client_id and playlist ids)."""
    valor = str(valor or "")
    if len(valor) <= n:
        return "***"
    return valor[:n] + "***"


def enmascarar_nombre(nombre):
    """Display name reduced to 2 characters + ***."""
    nombre = str(nombre or "").strip()
    if not nombre:
        return "(no name)"
    return nombre[:2] + "***"


def marca_tiempo_utc():
    return time.strftime("%Y-%m-%d %H:%M:%S UTC", time.gmtime())


def parsear_json(cuerpo):
    try:
        return json.loads(cuerpo)
    except ValueError:
        return None


def extraer_motivo_error(datos):
    """Extract a short reason from a Spotify JSON error body."""
    if isinstance(datos, dict):
        err = datos.get("error")
        if isinstance(err, dict):
            return str(err.get("message") or err.get("reason") or "")
        if isinstance(err, str) and err:
            desc = datos.get("error_description") or ""
            return err + (": " + str(desc) if desc else "")
    return ""


def formato_estado(estado):
    return str(estado) if estado is not None else "ERR"


# ---------------------------------------------------------------------------
# Credentials (environment or the repo's .env; values are NEVER shown)
# ---------------------------------------------------------------------------

def raiz_repo():
    # This probe lives in scripts/maintenance/, three levels below the repo root.
    return os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def cargar_env(ruta_env):
    """Read simple KEY=VALUE lines, ignoring comments and blank lines."""
    variables = {}
    try:
        with open(ruta_env, "r", encoding="utf-8") as manejador:
            for linea in manejador:
                limpio = linea.strip()
                if not limpio or limpio.startswith("#"):
                    continue
                if limpio.startswith("export "):
                    limpio = limpio[len("export "):].strip()
                if "=" not in limpio:
                    continue
                clave, _, valor = limpio.partition("=")
                clave = clave.strip()
                valor = valor.strip()
                if len(valor) >= 2 and valor[0] == valor[-1] and valor[0] in "\"'":
                    valor = valor[1:-1]
                if clave:
                    variables[clave] = valor
    except OSError:
        return None
    return variables


def obtener_credenciales():
    """Return (client_id, client_secret) or None if missing. Never reveals them."""
    client_id = os.environ.get("SPOTIFY_CLIENT_ID")
    client_secret = os.environ.get("SPOTIFY_CLIENT_SECRET")
    fuente = "environment variables"
    if not client_id or not client_secret:
        variables = cargar_env(os.path.join(raiz_repo(), ".env"))
        if variables:
            client_id = client_id or variables.get("SPOTIFY_CLIENT_ID")
            client_secret = client_secret or variables.get("SPOTIFY_CLIENT_SECRET")
            fuente = "repo .env"
    faltan = []
    if not client_id:
        faltan.append("SPOTIFY_CLIENT_ID")
    if not client_secret:
        faltan.append("SPOTIFY_CLIENT_SECRET")
    if faltan:
        print("[!] Missing credentials: %s." % ", ".join(faltan))
        print("    Define them in the environment or in simple KEY=VALUE lines of the repo-root .env.")
        return None
    print("[i] Credentials loaded from %s (values HIDDEN: "
          "client_id=%s, client_secret=****)." % (fuente, enmascarar_id(client_id)))
    return client_id, client_secret


# ---------------------------------------------------------------------------
# Generic HTTP (15 s timeout, typed errors, no tracebacks)
# ---------------------------------------------------------------------------

def peticion_http(url, metodo="GET", cabeceras=None, datos=None):
    """Return (status|None, body_text, failure_reason|None)."""
    peticion = urllib.request.Request(url, data=datos, method=metodo)
    for clave, valor in (cabeceras or {}).items():
        peticion.add_header(clave, valor)
    try:
        with urllib.request.urlopen(peticion, timeout=TIMEOUT_REQUEST_SEG) as respuesta:
            estado = getattr(respuesta, "status", None) or 200
            return estado, respuesta.read().decode("utf-8", "replace"), None
    except urlerror.HTTPError as exc:
        try:
            cuerpo = exc.read().decode("utf-8", "replace")
        except Exception:
            cuerpo = ""
        return exc.code, cuerpo, None
    except (socket.timeout, TimeoutError):
        return None, "", "timeout after %ds" % TIMEOUT_REQUEST_SEG
    except urlerror.URLError as exc:
        return None, "", "connection error: %s" % (getattr(exc, "reason", None) or exc)
    except OSError as exc:
        return None, "", "I/O error: %s" % exc


# ---------------------------------------------------------------------------
# Local server for the OAuth redirect
# ---------------------------------------------------------------------------

class ManejadorCallback(BaseHTTPRequestHandler):
    """Handles ONLY /callback; validates the anti-CSRF state and captures ?code=."""

    def log_message(self, formato, *args):  # silence the default log
        pass

    def do_GET(self):
        try:
            self._atender()
        except (BrokenPipeError, ConnectionResetError):
            pass

    def _atender(self):
        sondeo = self.server.estado_probe
        partes = urlpar.urlparse(self.path)
        if partes.path != "/callback":
            self._enviar_html(404, "Spotify Probe",
                              "<p>Unknown route. Waiting for <code>/callback</code>...</p>")
            return
        if sondeo.get("listo"):
            self._enviar_html(200, "Spotify Probe", "<p>The authorization was already processed.</p>")
            return
        params = urlpar.parse_qs(partes.query, keep_blank_values=True)
        estado_recibido = (params.get("state") or [""])[0]
        if params.get("error"):
            sondeo["error_autorizacion"] = (params.get("error") or ["desconocido"])[0]
            sondeo["descripcion_error"] = (params.get("error_description") or [""])[0]
            sondeo["listo"] = True
            self._enviar_html(200, "Authorization rejected",
                              "<p>Spotify returned an authorization error. "
                              "Go back to the terminal to see the verdict.</p>")
            return
        codigo = (params.get("code") or [""])[0]
        if not codigo:
            sondeo["error_autorizacion"] = "sin_codigo"
            sondeo["listo"] = True
            self._enviar_html(400, "Incomplete callback",
                              "<p>The callback did not include <code>?code=</code>.</p>")
            return
        esperado = sondeo.get("estado_csrf", "")
        if not estado_recibido or not secrets.compare_digest(
                estado_recibido.encode("utf-8"), esperado.encode("utf-8")):
            sondeo["error_autorizacion"] = "csrf_state_mismatch"
            sondeo["listo"] = True
            self._enviar_html(400, "Invalid state",
                              "<p>The <code>state</code> parameter does not match "
                              "(possible CSRF). Re-run the probe.</p>")
            return
        sondeo["codigo_autorizacion"] = codigo  # process memory only
        sondeo["listo"] = True
        self._enviar_html(200, "Authorization received",
                          "<p>The probe has captured the authorization code "
                          "(value hidden). You can close this tab.</p>")

    def _enviar_html(self, status, titulo, cuerpo_html):
        pagina = ("<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">"
                  "<title>%s</title></head><body style=\"font-family:sans-serif\">"
                  "<h2>%s</h2>%s</body></html>" % (titulo, titulo, cuerpo_html))
        payload = pagina.encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


def elegir_puerto(preferido):
    """Try 8899; if it is taken, ask the OS for an ephemeral port."""
    for pedido in (preferido, 0):
        zócalo = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            zócalo.bind((HOST_REDIRECT, pedido))
            puerto = zócalo.getsockname()[1]
        except OSError:
            puerto = None
        finally:
            zócalo.close()
        if puerto is not None:
            return puerto
    return None


def construir_url_autorizacion(client_id, redirect_uri, estado_csrf):
    params = {
        "client_id": client_id,
        "response_type": "code",
        "redirect_uri": redirect_uri,
        "scope": " ".join(SCOPE_SOLICITADOS),
        "state": estado_csrf,
        "show_dialog": "true",
    }
    return URL_AUTORIZAR + "?" + urlpar.urlencode(params)


# ---------------------------------------------------------------------------
# Code-for-token exchange (Basic auth; 400/403 errors sanitized)
# ---------------------------------------------------------------------------

def intercambiar_codigo_por_token(client_id, client_secret, codigo, redirect_uri):
    """Return (access_token|None, error_detail|None). Token never printed."""
    basico = base64.b64encode(
        ("%s:%s" % (client_id, client_secret)).encode("utf-8")).decode("ascii")
    cuerpo_form = urlpar.urlencode({
        "grant_type": "authorization_code",
        "code": codigo,
        "redirect_uri": redirect_uri,
    }).encode("utf-8")
    estado, texto, fallo = peticion_http(
        URL_TOKEN,
        metodo="POST",
        cabeceras={
            "Authorization": "Basic " + basico,
            "Content-Type": "application/x-www-form-urlencoded",
        },
        datos=cuerpo_form,
    )
    if fallo:
        print("[!] Code exchange failed: %s" % fallo)
        return None, "network: %s" % fallo
    datos = parsear_json(texto)
    if estado != 200 or not isinstance(datos, dict) or not datos.get("access_token"):
        motivo = extraer_motivo_error(datos) or recortar(sanitizar_texto(texto), 240)
        # Explicit handling of 400/403: ONLY status + SANITIZED body (no tokens).
        print("[!] Token exchange failed. HTTP %s" % formato_estado(estado))
        print("    Body (sanitized): %s" % sanitizar_texto(recortar(motivo)))
        return None, "HTTP %s: %s" % (formato_estado(estado), sanitizar_texto(motivo))
    return str(datos["access_token"]), None


def clasificar_fallo_token(detalle):
    d = (detalle or "").lower()
    if "invalid_client" in d:
        return "BLOQUEADO_CREDENCIALES_INVALIDAS"
    if "invalid_grant" in d:
        return "BLOQUEADO_CODIGO_INVALIDO_O_EXPIRADO"
    if "quota" in d or "403" in d:
        return "BLOQUEADO_QUOTA_EXCEDIDA_O_ACCESO_APP"
    if "timeout" in d or "connect" in d or "i/o" in d:
        return "BLOQUEADO_RED_HACIA_SPOTIFY"
    return "BLOQUEADO_INTERCAMBIO_DE_TOKEN"


# ---------------------------------------------------------------------------
# Read checks against the API
# ---------------------------------------------------------------------------

def consultar_api(token, ruta):
    """Authenticated GET. Returns (status|None, json_data|None, error_reason|None)."""
    estado, cuerpo, fallo = peticion_http(
        URL_API_BASE + ruta, cabeceras={"Authorization": "Bearer " + token})
    if fallo:
        return None, None, fallo
    datos = parsear_json(cuerpo)
    if estado >= 400:
        motivo = extraer_motivo_error(datos) or recortar(sanitizar_texto(cuerpo), 160) \
            or ("HTTP %s" % estado)
        return estado, datos, motivo
    return estado, datos, None


def nueva_fila(endpoint, estado, detalle, aplica=True):
    return {"endpoint": endpoint, "estado": estado, "detalle": detalle, "aplica": aplica}


def fila_fallo(endpoint, etiqueta_num, estado, fallo, datos):
    motivo = fallo or extraer_motivo_error(datos) \
        or ("200 but no 'total' field" if estado == 200 else "HTTP %s" % formato_estado(estado))
    detalle = sanitizar_texto(recortar(motivo, 140))
    print("%s %-40s %s  %s" % (etiqueta_num, endpoint, formato_estado(estado), detalle))
    return nueva_fila(endpoint, estado, detalle)


def ejecutar_chequeos(token):
    """Four sequential reads. Returns (filas, producto, perfil_failure|None)."""
    filas = []

    # [1/4] GET /me — product is THE KEY FIELD of the verdict.
    endpoint = "GET /me"
    estado, datos, fallo = consultar_api(token, "/me")
    if estado != 200 or not isinstance(datos, dict):
        detalle = fallo or extraer_motivo_error(datos) \
            or ("HTTP %s" % formato_estado(estado))
        detalle = sanitizar_texto(recortar(detalle, 140))
        filas.append(nueva_fila(endpoint, estado, detalle))
        print("[1/4] %-40s %s  %s" % (endpoint, formato_estado(estado), detalle))
        return filas, None, {"estado": estado, "motivo": detalle}
    producto = str(datos.get("product") or "(no field)")
    usuario = enmascarar_nombre(datos.get("display_name"))
    filas.append(nueva_fila(endpoint, 200,
                            "product=%s · user=%s · email omitted" % (producto, usuario)))
    print("[1/4] %-40s 200  product=%s user=%s" % (endpoint, producto, usuario))

    # [2/4] GET /me/tracks?limit=1 — total number of saved tracks.
    endpoint = "GET /me/tracks?limit=1"
    estado, datos, fallo = consultar_api(token, "/me/tracks?limit=1")
    if estado == 200 and isinstance(datos, dict) and isinstance(datos.get("total"), int):
        filas.append(nueva_fila(endpoint, 200, "total=%d" % datos["total"]))
        print("[2/4] %-40s 200  total=%d" % (endpoint, datos["total"]))
    else:
        filas.append(fila_fallo(endpoint, "[2/4]", estado, fallo, datos))

    # [3/4] GET /me/playlists?limit=1 — total number of visible playlists.
    endpoint = "GET /me/playlists?limit=1"
    playlists_ok = False
    total_playlists = 0
    primer_playlist_id = None
    estado, datos, fallo = consultar_api(token, "/me/playlists?limit=1")
    if estado == 200 and isinstance(datos, dict) and isinstance(datos.get("total"), int):
        playlists_ok = True
        total_playlists = datos["total"]
        items = datos.get("items")
        if isinstance(items, list) and items and isinstance(items[0], dict):
            primer_playlist_id = items[0].get("id")
        filas.append(nueva_fila(endpoint, 200, "total=%d" % total_playlists))
        print("[3/4] %-40s 200  total=%d" % (endpoint, total_playlists))
    else:
        filas.append(fila_fallo(endpoint, "[3/4]", estado, fallo, datos))

    # [4/4] GET /playlists/{id}/tracks?limit=1 — conditional on having >=1 playlist.
    endpoint = "GET /playlists/{id}/tracks?limit=1"
    if playlists_ok and total_playlists >= 1:
        if primer_playlist_id:
            ruta = "/playlists/%s/tracks?limit=1" % primer_playlist_id
            estado, datos, fallo = consultar_api(token, ruta)
            if estado == 200 and isinstance(datos, dict) and isinstance(datos.get("total"), int):
                detalle = "total=%d (playlist %s)" % (
                    datos["total"], enmascarar_id(primer_playlist_id, n=6))
                filas.append(nueva_fila(endpoint, 200, detalle))
                print("[4/4] %-40s 200  %s" % (endpoint, detalle))
            else:
                filas.append(fila_fallo(endpoint, "[4/4]", estado, fallo, datos))
        else:
            filas.append(nueva_fila(endpoint, None,
                                    "the first page did not include a playlist id", aplica=False))
            print("[4/4] %-40s N/A  (no id on the first page)" % endpoint)
    elif playlists_ok:
        filas.append(nueva_fila(endpoint, None,
                                "the account has no playlists; not applicable", aplica=False))
        print("[4/4] %-40s N/A  (0 playlists)" % endpoint)
    else:
        filas.append(nueva_fila(endpoint, None,
                                "not applicable because /me/playlists failed", aplica=False))
        print("[4/4] %-40s N/A  (previous step failed)" % endpoint)

    return filas, producto, None


def clasificar_fallo_perfil(fallo_perfil):
    estado = fallo_perfil.get("estado")
    motivo = fallo_perfil.get("motivo", "")
    if estado == 401:
        return "BLOQUEADO_TOKEN_RECHAZADO_401", \
            "The token was rejected when querying /me (401)."
    if estado == 403:
        return "BLOQUEADO_HTTP_403_EN_ME", \
            "Spotify responded 403 on /me (possibly a restricted endpoint or an app " \
            "in development mode without allowlisted users). Detail: %s" % motivo
    if estado is None:
        return "BLOQUEADO_RED_HACIA_SPOTIFY", \
            "No HTTP response from api.spotify.com: %s" % motivo
    return "BLOQUEADO_HTTP_%s_EN_ME" % estado, motivo


def calcular_veredicto(producto, filas):
    aplicables = [f for f in filas if f["aplica"]]
    fallidos = [f for f in aplicables if f["estado"] != 200]
    if fallidos:
        return "FUNCIONA_PARCIAL", resumen_fallos(filas)
    if (producto or "").strip().lower() == "free":
        return "FUNCIONA_CON_FREE", None
    return "PREMIUM_NO_INFORMATIVO_PARA_FREE", (
        "The authenticated account is '%s': all endpoints returned 200, "
        "but the result does NOT serve as evidence about the FREE tier." % producto)


def resumen_fallos(filas):
    partes = []
    for fila in filas:
        if fila["aplica"] and fila["estado"] != 200:
            partes.append("%s => %s" % (fila["endpoint"], fila["detalle"]))
    return "; ".join(partes)


def mapear_error_autorizacion(error, descripcion):
    desc = sanitizar_texto(descripcion or "")
    sufijo_desc = (" Detail: " + recortar(desc, 140)) if desc else ""
    if error == "access_denied":
        return "BLOQUEADO_USUARIO_NEGO_PERMISOS", \
            "The user denied consent on the Spotify screen." + sufijo_desc
    if error == "csrf_state_mismatch":
        return "BLOQUEADO_STATE_CSRF_INVALIDO", \
            "The callback's state parameter does not match the one sent " \
            "(possible CSRF or a reused old tab)."
    normalizado = re.sub(r"[^A-Za-z0-9]+", "_", str(error)).strip("_").upper()[:40] or "DESCONOCIDO"
    return "BLOQUEADO_ERROR_AUTORIZACION_%s" % normalizado, \
        "Spotify returned an error during authorization." + sufijo_desc


# ---------------------------------------------------------------------------
# Verdict: printing + Markdown report
# ---------------------------------------------------------------------------

LEYENDA_VEREDICTOS_MD = """
## How to read the verdict

- `FUNCIONA_CON_FREE`: the account is **free** and all 4 endpoints returned 200 with
  totals; reading one's own library still works for FREE.
- `FUNCIONA_PARCIAL`: the account authenticated, but at least one applicable endpoint failed;
  the table distinguishes the 200s from the failures (status + short reason).
- `BLOQUEADO_<reason>`: the probe could not be completed; the reason after the underscore indicates
  the cause (invalid credentials, quota exceeded, HTTP 403/401, user denied permissions,
  timeout, etc.).
- `PREMIUM_NO_INFORMATIVO_PARA_FREE`: the account is premium; everything worked but the
  result does not evidence the behavior of the FREE tier.
""".strip("\n")


def escribir_informe_md(veredicto, filas, producto, motivo):
    ruta = os.path.join(os.path.dirname(os.path.abspath(__file__)), ARCHIVO_RESULTADO)
    lineas = [
        "# Verdict: FREE tier probe — Spotify Web API",
        "",
        "- **Verdict:** `%s`" % veredicto,
    ]
    if motivo:
        lineas.append("- **Reason:** %s" % motivo)
    lineas.extend([
        "- **Account product:** `%s`" % (producto or "(unknown)"),
        "- **Date:** %s" % marca_tiempo_utc(),
        "- **Probe version:** %s" % PROBE_VERSION,
        "- **Requested scopes:** `%s`" % " ".join(SCOPE_SOLICITADOS),
        "",
        "## Results per endpoint",
        "",
        "| Endpoint | Status | Detail |",
        "| --- | --- | --- |",
    ])
    for fila in filas:
        edo = formato_estado(fila["estado"])
        if fila["estado"] is None and not fila["aplica"]:
            edo = "N/A"
        lineas.append("| `%s` | %s | %s |" % (
            fila["endpoint"], edo, fila["detalle"].replace("|", "/")))
    lineas.append("")
    lineas.append(LEYENDA_VEREDICTOS_MD)
    lineas.append("")
    lineas.append("## Privacidad")
    lineas.append("")
    lineas.append("- " + NOTA_ENMASCARADO)
    lineas.append("- The access_token existed only in memory during the run and "
                  "was never written to disk or to this report.")
    lineas.append("- Context: whether reading one's own library still works for "
                  "FREE accounts after Spotify's November 2024 changes.")
    contenido = "\n".join(lineas) + "\n"
    try:
        with open(ruta, "w", encoding="utf-8") as manejador:
            manejador.write(contenido)
        print("[i] Report written to: %s" % ruta)
    except OSError as exc:
        print("[!] Could not write the report to %s (%s)." % (ruta, exc))


def imprimir_y_guardar_veredicto(veredicto, filas, producto, motivo):
    print("")
    print("=" * 66)
    print("FINAL VERDICT: %s" % veredicto)
    if motivo:
        print("Reason: %s" % motivo)
    print("Account product: %s" % (producto or "(unknown)"))
    print("Date: %s | Probe version: v%s" % (marca_tiempo_utc(), PROBE_VERSION))
    print("(Masked outputs: no complete secret, token or personal data.)")
    print("=" * 66)
    escribir_informe_md(veredicto, filas, producto, motivo)


# ---------------------------------------------------------------------------
# Orchestration
# ---------------------------------------------------------------------------

def main():
    print("Spotify FREE tier probe v%s — reading one's own library" % PROBE_VERSION)
    print("Context: November 2024 changes. READS only (?limit=1).")

    credenciales = obtener_credenciales()
    if credenciales is None:
        imprimir_y_guardar_veredicto(
            "BLOQUEADO_FALTAN_CREDENCIALES", [], None,
            "SPOTIFY_CLIENT_ID/SPOTIFY_CLIENT_SECRET were not found in the environment "
            "or in the repo-root .env.")
        return 4
    client_id, client_secret = credenciales

    puerto = elegir_puerto(PUERTO_PREFERIDO)
    if puerto is None:
        imprimir_y_guardar_veredicto(
            "BLOQUEADO_SIN_PUERTO_LIBRE", [], None,
            "No free port was found on 127.0.0.1.")
        return 4
    redirect_uri = "http://%s:%d/callback" % (HOST_REDIRECT, puerto)
    if puerto != PUERTO_PREFERIDO:
        print("[!] Port %d was busy; using %d." % (PUERTO_PREFERIDO, puerto))

    estado_csrf = secrets.token_urlsafe(24)

    try:
        servidor = HTTPServer((HOST_REDIRECT, puerto), ManejadorCallback)
    except OSError as exc:
        imprimir_y_guardar_veredicto(
            "BLOQUEADO_PUERTO_NO_DISPONIBLE", [], None,
            "Could not start the local server on %s:%d (%s)." % (HOST_REDIRECT, puerto, exc))
        return 4
    servidor.timeout = 0.5
    servidor.estado_probe = {"listo": False, "estado_csrf": estado_csrf}

    try:
        url_autorizacion = construir_url_autorizacion(client_id, redirect_uri, estado_csrf)
        print("")
        print("[i] Opening the browser to authorize your developer app...")
        print("[i] If it does not open, copy this URL into your browser:")
        print("    %s" % url_autorizacion)
        print("[i] redirect_uri used: %s" % redirect_uri)
        print("    (it must be registered in your app at developer.spotify.com/dashboard).")
        try:
            abierto = webbrowser.open(url_autorizacion, new=1, autoraise=True)
        except Exception:
            abierto = False
        if not abierto:
            print("[!] Could not open the browser automatically; use the URL above.")
        print("")
        print("[i] Waiting for authorization (max %d s). Ctrl+C to cancel..."
              % TIMEOUT_ESPERA_NAVEGADOR_SEG)

        limite = time.time() + TIMEOUT_ESPERA_NAVEGADOR_SEG
        while not servidor.estado_probe["listo"] and time.time() < limite:
            servidor.handle_request()

        sondeo = servidor.estado_probe
        if not sondeo["listo"]:
            imprimir_y_guardar_veredicto(
                "BLOQUEADO_TIMEOUT_ESPERANDO_AUTORIZACION", [], None,
                "The user did not complete the authorization within %d s." % TIMEOUT_ESPERA_NAVEGADOR_SEG)
            return 3

        if sondeo.get("error_autorizacion"):
            veredicto, motivo = mapear_error_autorizacion(
                sondeo["error_autorizacion"], sondeo.get("descripcion_error"))
            imprimir_y_guardar_veredicto(veredicto, [], None, motivo)
            return 3

        codigo = sondeo.get("codigo_autorizacion")
        if not codigo:
            imprimir_y_guardar_veredicto(
                "BLOQUEADO_SIN_CODIGO_AUTORIZACION", [], None,
                "The callback did not contain ?code=.")
            return 3

        print("[i] Code received (value hidden). Exchanging it for a token...")
        token, detalle_error = intercambiar_codigo_por_token(
            client_id, client_secret, codigo, redirect_uri)
        if token is None:
            imprimir_y_guardar_veredicto(
                clasificar_fallo_token(detalle_error), [], None, detalle_error)
            return 3
        print("[i] Access_token obtained (in memory ONLY; it is never printed).")
        del codigo  # the code is no longer needed after the exchange

        filas, producto, fallo_perfil = ejecutar_chequeos(token)
        if fallo_perfil is not None:
            veredicto, motivo = clasificar_fallo_perfil(fallo_perfil)
            imprimir_y_guardar_veredicto(veredicto, filas, None, motivo)
            return 3

        veredicto, motivo = calcular_veredicto(producto, filas)
        imprimir_y_guardar_veredicto(veredicto, filas, producto, motivo)
        return 0
    finally:
        try:
            servidor.server_close()
        except Exception:
            pass


if __name__ == "__main__":
    try:
        sys.exit(main())
    except KeyboardInterrupt:
        print("\n[!] Run interrupted by the user (Ctrl+C). Exiting cleanly.")
        sys.exit(130)
    except SystemExit:
        raise
    except Exception as exc:  # avoid raw tracebacks in front of the user
        print("[!] Unexpected error: %s: %s" % (type(exc).__name__, sanitizar_texto(str(exc))))
        sys.exit(1)

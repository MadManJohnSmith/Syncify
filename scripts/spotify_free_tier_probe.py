#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Sonda del tier FREE de la API Web de Spotify (lectura de la PROPIA biblioteca).

PROPÓSITO
=========
Emitir un VEREDICTO documentado sobre si la lectura de la propia biblioteca
de Spotify (tracks guardados + playlists propias) sigue operativa para cuentas
**FREE** tras los cambios de noviembre de 2024, usando una developer-app
propia (client_id/client_secret). El script NO modifica nada en la cuenta:
solo hace lecturas acotadas (?limit=1) y reporta totales.

FLUJO (OAuth2 authorization-code con redirect a localhost)
==========================================================
1. Levanta un http.server efímero en 127.0.0.1:8899 (o el primer puerto libre).
2. Abre el navegador en https://accounts.spotify.com/authorize con los scopes
   user-library-read playlist-read-private playlist-read-collaborative
   user-read-email, show_dialog=true y un parámetro anti-CSRF `state`
   aleatorio que se verifica al volver.
3. Captura ?code= en /callback y lo intercambia por un access_token en
   https://accounts.spotify.com/api/token (Basic auth client_id:secret,
   form-urlencoded grant_type=authorization_code). Los errores 400/403 se
   imprimen SOLO como status + body SANITIZADO.
4. Con el token SOLO en memoria consulta secuencialmente GET /me,
   GET /me/tracks?limit=1, GET /me/playlists?limit=1 y, si hay >=1 playlist,
   GET /playlists/{id}/tracks?limit=1.
5. Imprime una tabla de resultados y un VEREDICTO final, que además queda
   escrito junto a este archivo en spotify_free_tier_probe_result.md.

GARANTÍAS DE PRIVACIDAD (importantes)
=====================================
- NUNCA imprime SPOTIFY_CLIENT_ID ni SPOTIFY_CLIENT_SECRET completos: las
  credenciales se leen en runtime desde el entorno o desde el `.env` de la
  raíz del repo (líneas KEY=VALUE simples, comentarios ignorados) y toda
  aparición en mensajes se ENMASCARA.
- El access_token vive únicamente en memoria durante la ejecución: jamás se
  imprime, guarda en disco ni incluye en URLs mostradas.
- Los cuerpos de error se muestran recortados y SANITIZADOS (cualquier patrón
  tipo token/code/secret/authorization reemplazado por [REDACTADO]).
- El nombre de usuario se muestra recortado a 2 caracteres + "***"; el email
  solicitado por scope no se muestra nunca.

USO
===
    python3 scripts/spotify_free_tier_probe.py

Requiere SPOTIFY_CLIENT_ID y SPOTIFY_CLIENT_SECRET en el entorno o en `.env`.
Timeout por petición: 15 s. Ctrl+C sale limpio y siempre cierra el servidor.

VEREDICTOS POSIBLES
===================
- FUNCIONA_CON_FREE                 product=free y los 4 endpoints devuelven 200
                                    con totales.
- FUNCIONA_PARCIAL                  Autenticó pero al menos un endpoint aplicable
                                    falló; la tabla detalla cuáles dieron 200 y
                                    cuáles fallaron (status + razón corta).
- BLOQUEADO_<motivo>                No se pudo completar la prueba (p. ej.
                                    BLOQUEADO_CREDENCIALES_INVALIDAS,
                                    BLOQUEADO_QUOTA_EXCEDIDA_O_ACCESO_APP,
                                    BLOQUEADO_HTTP_403_EN_ME,
                                    BLOQUEADO_USUARIO_NEGO_PERMISOS).
- PREMIUM_NO_INFORMATIVO_PARA_FREE  La cuenta autenticada es premium: todo
                                    funcionó, pero NO evidencia el tier FREE.

CÓDIGOS DE SALIDA: 0 veredicto favorable/informativo, 3 BLOQUEADO_*,
4 error de configuración, 130 interrumpido por Ctrl+C, 1 error inesperado.
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

NOTA_ENMASCARADO = ("Todas las salidas fueron ENMASCARADAS: sin client_id/secret "
                    "completos, sin tokens, sin email ni nombres completos.")

_PATRON_FUGA = re.compile(
    r"(?i)\b(access_token|refresh_token|client_secret|authorization|code|token|secret)"
    r"(\"|')?\s*([=:]\s*)"
    r"(\"[^\"]*\"|'[^']*'|[^\s&,}\]]+)"
)


# ---------------------------------------------------------------------------
# Utilidades de enmascarado / saneo
# ---------------------------------------------------------------------------

def sanitizar_texto(texto):
    """Reemplaza valores que acompañan a claves sensibles por [REDACTADO].

    Cubre tanto JSON ("access_token":"x") como form-urlencoded (code=x).
    """
    if not texto:
        return ""
    def _reemplazo(coincidencia):
        comilla = coincidencia.group(2) or ""
        return "%s%s%s%s[REDACTADO]%s" % (
            coincidencia.group(1), comilla, coincidencia.group(3), comilla, comilla)
    return _PATRON_FUGA.sub(_reemplazo, texto)


def recortar(texto, maximo=240):
    texto = texto or ""
    return texto if len(texto) <= maximo else texto[:maximo] + "..."


def enmascarar_id(valor, n=4):
    """Primeros n caracteres + *** (para client_id e ids de playlist)."""
    valor = str(valor or "")
    if len(valor) <= n:
        return "***"
    return valor[:n] + "***"


def enmascarar_nombre(nombre):
    """Nombre visible reducido a 2 caracteres + ***."""
    nombre = str(nombre or "").strip()
    if not nombre:
        return "(sin nombre)"
    return nombre[:2] + "***"


def marca_tiempo_utc():
    return time.strftime("%Y-%m-%d %H:%M:%S UTC", time.gmtime())


def parsear_json(cuerpo):
    try:
        return json.loads(cuerpo)
    except ValueError:
        return None


def extraer_motivo_error(datos):
    """Extrae una razón corta de un cuerpo JSON de error de Spotify."""
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
# Credenciales (entorno o .env del repo; NUNCA se muestran los valores)
# ---------------------------------------------------------------------------

def raiz_repo():
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def cargar_env(ruta_env):
    """Lee líneas KEY=VALUE simples ignorando comentarios y líneas en blanco."""
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
    """Devuelve (client_id, client_secret) o None si faltan. Nunca los revela."""
    client_id = os.environ.get("SPOTIFY_CLIENT_ID")
    client_secret = os.environ.get("SPOTIFY_CLIENT_SECRET")
    fuente = "variables de entorno"
    if not client_id or not client_secret:
        variables = cargar_env(os.path.join(raiz_repo(), ".env"))
        if variables:
            client_id = client_id or variables.get("SPOTIFY_CLIENT_ID")
            client_secret = client_secret or variables.get("SPOTIFY_CLIENT_SECRET")
            fuente = ".env del repo"
    faltan = []
    if not client_id:
        faltan.append("SPOTIFY_CLIENT_ID")
    if not client_secret:
        faltan.append("SPOTIFY_CLIENT_SECRET")
    if faltan:
        print("[!] Faltan credenciales: %s." % ", ".join(faltan))
        print("    Defínelas en el entorno o en líneas KEY=VALUE del .env de la raíz del repo.")
        return None
    print("[i] Credenciales cargadas desde %s (valores OCULTOS: "
          "client_id=%s, client_secret=****)." % (fuente, enmascarar_id(client_id)))
    return client_id, client_secret


# ---------------------------------------------------------------------------
# HTTP genérico (timeout 15 s, errores tipificados, sin tracebacks)
# ---------------------------------------------------------------------------

def peticion_http(url, metodo="GET", cabeceras=None, datos=None):
    """Devuelve (estado|None, cuerpo_texto, motivo_fallo|None)."""
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
        return None, "", "timeout de %ds" % TIMEOUT_REQUEST_SEG
    except urlerror.URLError as exc:
        return None, "", "error de conexión: %s" % (getattr(exc, "reason", None) or exc)
    except OSError as exc:
        return None, "", "error de E/S: %s" % exc


# ---------------------------------------------------------------------------
# Servidor local para el redirect OAuth
# ---------------------------------------------------------------------------

class ManejadorCallback(BaseHTTPRequestHandler):
    """Atiende SOLO /callback; valida state anti-CSRF y captura ?code=."""

    def log_message(self, formato, *args):  # silencia el log por defecto
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
            self._enviar_html(404, "Probe Spotify",
                              "<p>Ruta desconocida. Esperando <code>/callback</code>...</p>")
            return
        if sondeo.get("listo"):
            self._enviar_html(200, "Probe Spotify", "<p>La autorización ya fue procesada.</p>")
            return
        params = urlpar.parse_qs(partes.query, keep_blank_values=True)
        estado_recibido = (params.get("state") or [""])[0]
        if params.get("error"):
            sondeo["error_autorizacion"] = (params.get("error") or ["desconocido"])[0]
            sondeo["descripcion_error"] = (params.get("error_description") or [""])[0]
            sondeo["listo"] = True
            self._enviar_html(200, "Autorización rechazada",
                              "<p>Spotify devolvió un error de autorización. "
                              "Vuelve a la terminal para ver el veredicto.</p>")
            return
        codigo = (params.get("code") or [""])[0]
        if not codigo:
            sondeo["error_autorizacion"] = "sin_codigo"
            sondeo["listo"] = True
            self._enviar_html(400, "Callback incompleto",
                              "<p>El callback no incluyó <code>?code=</code>.</p>")
            return
        esperado = sondeo.get("estado_csrf", "")
        if not estado_recibido or not secrets.compare_digest(
                estado_recibido.encode("utf-8"), esperado.encode("utf-8")):
            sondeo["error_autorizacion"] = "csrf_state_mismatch"
            sondeo["listo"] = True
            self._enviar_html(400, "State inválido",
                              "<p>El parámetro <code>state</code> no coincide "
                              "(posible CSRF). Vuelve a lanzar el probe.</p>")
            return
        sondeo["codigo_autorizacion"] = codigo  # solo en memoria del proceso
        sondeo["listo"] = True
        self._enviar_html(200, "Autorización recibida",
                          "<p>El probe ya capturó el código de autorización "
                          "(valor oculto). Puedes cerrar esta pestaña.</p>")

    def _enviar_html(self, status, titulo, cuerpo_html):
        pagina = ("<!DOCTYPE html><html lang=\"es\"><head><meta charset=\"utf-8\">"
                  "<title>%s</title></head><body style=\"font-family:sans-serif\">"
                  "<h2>%s</h2>%s</body></html>" % (titulo, titulo, cuerpo_html))
        payload = pagina.encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


def elegir_puerto(preferido):
    """Prueba 8899; si está ocupado pide un puerto efímero al SO."""
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
# Intercambio de código por token (Basic auth; errores 400/403 saneados)
# ---------------------------------------------------------------------------

def intercambiar_codigo_por_token(client_id, client_secret, codigo, redirect_uri):
    """Devuelve (access_token|None, detalle_error|None). Token nunca impreso."""
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
        print("[!] Falló el intercambio del código: %s" % fallo)
        return None, "red: %s" % fallo
    datos = parsear_json(texto)
    if estado != 200 or not isinstance(datos, dict) or not datos.get("access_token"):
        motivo = extraer_motivo_error(datos) or recortar(sanitizar_texto(texto), 240)
        # Manejo explícito de 400/403: SOLO status + body SANITIZADO (sin tokens).
        print("[!] Intercambio de token fallido. HTTP %s" % formato_estado(estado))
        print("    Body (sanitizado): %s" % sanitizar_texto(recortar(motivo)))
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
    if "timeout" in d or "conexi" in d or "e/s" in d:
        return "BLOQUEADO_RED_HACIA_SPOTIFY"
    return "BLOQUEADO_INTERCAMBIO_DE_TOKEN"


# ---------------------------------------------------------------------------
# Chequeos de lectura contra la API
# ---------------------------------------------------------------------------

def consultar_api(token, ruta):
    """GET autenticado. Devuelve (estado|None, datos_json|None, motivo_error|None)."""
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
        or ("200 pero sin campo 'total'" if estado == 200 else "HTTP %s" % formato_estado(estado))
    detalle = sanitizar_texto(recortar(motivo, 140))
    print("%s %-40s %s  %s" % (etiqueta_num, endpoint, formato_estado(estado), detalle))
    return nueva_fila(endpoint, estado, detalle)


def ejecutar_chequeos(token):
    """Cuatro lecturas secuenciales. Devuelve (filas, producto, fallo_perfil|None)."""
    filas = []

    # [1/4] GET /me — product es EL CAMPO CLAVE del veredicto.
    endpoint = "GET /me"
    estado, datos, fallo = consultar_api(token, "/me")
    if estado != 200 or not isinstance(datos, dict):
        detalle = fallo or extraer_motivo_error(datos) \
            or ("HTTP %s" % formato_estado(estado))
        detalle = sanitizar_texto(recortar(detalle, 140))
        filas.append(nueva_fila(endpoint, estado, detalle))
        print("[1/4] %-40s %s  %s" % (endpoint, formato_estado(estado), detalle))
        return filas, None, {"estado": estado, "motivo": detalle}
    producto = str(datos.get("product") or "(sin campo)")
    usuario = enmascarar_nombre(datos.get("display_name"))
    filas.append(nueva_fila(endpoint, 200,
                            "producto=%s · usuario=%s · email omitido" % (producto, usuario)))
    print("[1/4] %-40s 200  producto=%s usuario=%s" % (endpoint, producto, usuario))

    # [2/4] GET /me/tracks?limit=1 — total de tracks guardados.
    endpoint = "GET /me/tracks?limit=1"
    estado, datos, fallo = consultar_api(token, "/me/tracks?limit=1")
    if estado == 200 and isinstance(datos, dict) and isinstance(datos.get("total"), int):
        filas.append(nueva_fila(endpoint, 200, "total=%d" % datos["total"]))
        print("[2/4] %-40s 200  total=%d" % (endpoint, datos["total"]))
    else:
        filas.append(fila_fallo(endpoint, "[2/4]", estado, fallo, datos))

    # [3/4] GET /me/playlists?limit=1 — total de playlists visibles.
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

    # [4/4] GET /playlists/{id}/tracks?limit=1 — condicional a tener >=1 playlist.
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
                                    "la primera página no incluyó id de playlist", aplica=False))
            print("[4/4] %-40s N/A  (sin id en la primera página)" % endpoint)
    elif playlists_ok:
        filas.append(nueva_fila(endpoint, None,
                                "la cuenta no tiene playlists; no aplica", aplica=False))
        print("[4/4] %-40s N/A  (0 playlists)" % endpoint)
    else:
        filas.append(nueva_fila(endpoint, None,
                                "no aplica porque /me/playlists falló", aplica=False))
        print("[4/4] %-40s N/A  (paso anterior falló)" % endpoint)

    return filas, producto, None


def clasificar_fallo_perfil(fallo_perfil):
    estado = fallo_perfil.get("estado")
    motivo = fallo_perfil.get("motivo", "")
    if estado == 401:
        return "BLOQUEADO_TOKEN_RECHAZADO_401", \
            "El token fue rechazado al consultar /me (401)."
    if estado == 403:
        return "BLOQUEADO_HTTP_403_EN_ME", \
            "Spotify respondió 403 en /me (posible endpoint restringido o app " \
            "en modo desarrollo sin usuarios allowlisted). Detalle: %s" % motivo
    if estado is None:
        return "BLOQUEADO_RED_HACIA_SPOTIFY", \
            "No hubo respuesta HTTP de api.spotify.com: %s" % motivo
    return "BLOQUEADO_HTTP_%s_EN_ME" % estado, motivo


def calcular_veredicto(producto, filas):
    aplicables = [f for f in filas if f["aplica"]]
    fallidos = [f for f in aplicables if f["estado"] != 200]
    if fallidos:
        return "FUNCIONA_PARCIAL", resumen_fallos(filas)
    if (producto or "").strip().lower() == "free":
        return "FUNCIONA_CON_FREE", None
    return "PREMIUM_NO_INFORMATIVO_PARA_FREE", (
        "La cuenta autenticada es '%s': todos los endpoints devolvieron 200, "
        "pero el resultado NO sirve como evidencia sobre el tier FREE." % producto)


def resumen_fallos(filas):
    partes = []
    for fila in filas:
        if fila["aplica"] and fila["estado"] != 200:
            partes.append("%s => %s" % (fila["endpoint"], fila["detalle"]))
    return "; ".join(partes)


def mapear_error_autorizacion(error, descripcion):
    desc = sanitizar_texto(descripcion or "")
    sufijo_desc = (" Detalle: " + recortar(desc, 140)) if desc else ""
    if error == "access_denied":
        return "BLOQUEADO_USUARIO_NEGO_PERMISOS", \
            "El usuario denegó el consentimiento en la pantalla de Spotify." + sufijo_desc
    if error == "csrf_state_mismatch":
        return "BLOQUEADO_STATE_CSRF_INVALIDO", \
            "El parámetro state del callback no coincide con el enviado " \
            "(posible CSRF o pestaña vieja reutilizada)."
    normalizado = re.sub(r"[^A-Za-z0-9]+", "_", str(error)).strip("_").upper()[:40] or "DESCONOCIDO"
    return "BLOQUEADO_ERROR_AUTORIZACION_%s" % normalizado, \
        "Spotify devolvió un error durante la autorización." + sufijo_desc


# ---------------------------------------------------------------------------
# Veredicto: impresión + informe Markdown
# ---------------------------------------------------------------------------

LEYENDA_VEREDICTOS_MD = """
## Cómo interpretar el veredicto

- `FUNCIONA_CON_FREE`: la cuenta es **free** y los 4 endpoints devolvieron 200 con
  totales; la lectura de la propia biblioteca sigue operativa para FREE.
- `FUNCIONA_PARCIAL`: la cuenta autenticó, pero al menos un endpoint aplicable falló;
  la tabla distingue los 200 de los fallos (status + razón corta).
- `BLOQUEADO_<motivo>`: no se pudo completar la prueba; el motivo tras el guion indica
  la causa (credenciales inválidas, quota excedida, HTTP 403/401, usuario negó permisos,
  timeout, etc.).
- `PREMIUM_NO_INFORMATIVO_PARA_FREE`: la cuenta es premium; todo funcionó pero el
  resultado no evidencia el comportamiento del tier FREE.
""".strip("\n")


def escribir_informe_md(veredicto, filas, producto, motivo):
    ruta = os.path.join(os.path.dirname(os.path.abspath(__file__)), ARCHIVO_RESULTADO)
    lineas = [
        "# Veredicto probe tier FREE — API Web de Spotify",
        "",
        "- **Veredicto:** `%s`" % veredicto,
    ]
    if motivo:
        lineas.append("- **Motivo:** %s" % motivo)
    lineas.extend([
        "- **Producto de la cuenta:** `%s`" % (producto or "(desconocido)"),
        "- **Fecha:** %s" % marca_tiempo_utc(),
        "- **Versión del probe:** %s" % PROBE_VERSION,
        "- **Scopes solicitados:** `%s`" % " ".join(SCOPE_SOLICITADOS),
        "",
        "## Resultados por endpoint",
        "",
        "| Endpoint | Estado | Detalle |",
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
    lineas.append("- El access_token existió solo en memoria durante la ejecución y "
                  "nunca se escribió en disco ni en este informe.")
    lineas.append("- Contexto: operatividad de la lectura de la propia biblioteca para "
                  "cuentas FREE tras los cambios de Spotify de noviembre de 2024.")
    contenido = "\n".join(lineas) + "\n"
    try:
        with open(ruta, "w", encoding="utf-8") as manejador:
            manejador.write(contenido)
        print("[i] Informe escrito en: %s" % ruta)
    except OSError as exc:
        print("[!] No se pudo escribir el informe en %s (%s)." % (ruta, exc))


def imprimir_y_guardar_veredicto(veredicto, filas, producto, motivo):
    print("")
    print("=" * 66)
    print("VEREDICTO FINAL: %s" % veredicto)
    if motivo:
        print("Motivo: %s" % motivo)
    print("Producto de la cuenta: %s" % (producto or "(desconocido)"))
    print("Fecha: %s | Versión del probe: v%s" % (marca_tiempo_utc(), PROBE_VERSION))
    print("(Salidas enmascaradas: ningún secreto, token o dato personal completo.)")
    print("=" * 66)
    escribir_informe_md(veredicto, filas, producto, motivo)


# ---------------------------------------------------------------------------
# Orquestación
# ---------------------------------------------------------------------------

def main():
    print("Sonda Spotify tier FREE v%s — lectura de la propia biblioteca" % PROBE_VERSION)
    print("Contexto: cambios de noviembre de 2024. Solo LECTURAS (?limit=1).")

    credenciales = obtener_credenciales()
    if credenciales is None:
        imprimir_y_guardar_veredicto(
            "BLOQUEADO_FALTAN_CREDENCIALES", [], None,
            "No se encontraron SPOTIFY_CLIENT_ID/SPOTIFY_CLIENT_SECRET en el entorno "
            "ni en el .env de la raíz del repo.")
        return 4
    client_id, client_secret = credenciales

    puerto = elegir_puerto(PUERTO_PREFERIDO)
    if puerto is None:
        imprimir_y_guardar_veredicto(
            "BLOQUEADO_SIN_PUERTO_LIBRE", [], None,
            "No se encontró un puerto libre en 127.0.0.1.")
        return 4
    redirect_uri = "http://%s:%d/callback" % (HOST_REDIRECT, puerto)
    if puerto != PUERTO_PREFERIDO:
        print("[!] El puerto %d estaba ocupado; usando el %d." % (PUERTO_PREFERIDO, puerto))

    estado_csrf = secrets.token_urlsafe(24)

    try:
        servidor = HTTPServer((HOST_REDIRECT, puerto), ManejadorCallback)
    except OSError as exc:
        imprimir_y_guardar_veredicto(
            "BLOQUEADO_PUERTO_NO_DISPONIBLE", [], None,
            "No se pudo levantar el servidor local en %s:%d (%s)." % (HOST_REDIRECT, puerto, exc))
        return 4
    servidor.timeout = 0.5
    servidor.estado_probe = {"listo": False, "estado_csrf": estado_csrf}

    try:
        url_autorizacion = construir_url_autorizacion(client_id, redirect_uri, estado_csrf)
        print("")
        print("[i] Abriendo el navegador para autorizar tu developer app...")
        print("[i] Si no se abre, copia esta URL en tu navegador:")
        print("    %s" % url_autorizacion)
        print("[i] redirect_uri usado: %s" % redirect_uri)
        print("    (debe estar registrado en tu app en developer.spotify.com/dashboard).")
        try:
            abierto = webbrowser.open(url_autorizacion, new=1, autoraise=True)
        except Exception:
            abierto = False
        if not abierto:
            print("[!] No se pudo abrir el navegador automáticamente; usa la URL de arriba.")
        print("")
        print("[i] Esperando autorización (máximo %d s). Ctrl+C para cancelar..."
              % TIMEOUT_ESPERA_NAVEGADOR_SEG)

        limite = time.time() + TIMEOUT_ESPERA_NAVEGADOR_SEG
        while not servidor.estado_probe["listo"] and time.time() < limite:
            servidor.handle_request()

        sondeo = servidor.estado_probe
        if not sondeo["listo"]:
            imprimir_y_guardar_veredicto(
                "BLOQUEADO_TIMEOUT_ESPERANDO_AUTORIZACION", [], None,
                "El usuario no completó la autorización en %d s." % TIMEOUT_ESPERA_NAVEGADOR_SEG)
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
                "El callback no contenía ?code=.")
            return 3

        print("[i] Código recibido (valor oculto). Intercambiándolo por token...")
        token, detalle_error = intercambiar_codigo_por_token(
            client_id, client_secret, codigo, redirect_uri)
        if token is None:
            imprimir_y_guardar_veredicto(
                clasificar_fallo_token(detalle_error), [], None, detalle_error)
            return 3
        print("[i] Access_token obtenido (SOLO en memoria; nunca se imprime).")
        del codigo  # el código ya no hace falta fuera del intercambio

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
        print("\n[!] Ejecución interrumpida por el usuario (Ctrl+C). Saliendo limpio.")
        sys.exit(130)
    except SystemExit:
        raise
    except Exception as exc:  # evita tracebacks crudos ante el usuario
        print("[!] Error inesperado: %s: %s" % (type(exc).__name__, sanitizar_texto(str(exc))))
        sys.exit(1)

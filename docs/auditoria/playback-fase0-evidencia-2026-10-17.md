# Fase 0 — Prueba dirigida de reproducción local (R-13 / PLAY-TRACE-01): evidencia

**Fecha:** 2026-10-17 · **Host:** CachyOS (Arch), sesión Wayland (prueba por X11/XWayland)
**Versiones:** tauri 2.12.1 · wry 0.57.0 · webkit2gtk-4.1 2.54.1 · GStreamer 1.28.8 · ffmpeg/ffplay del sistema
**Rama:** `fix/windows-release-ffmpeg-pin` · **Prerequisito documental:** `matriz-ajustes.md:508,512` (PLAY-TRACE-01/02)

## 1. Pregunta a resolver

R-13 ([`Errores-Pruebas-2026-10-13.md:126-131`](../Errores-Pruebas-2026-10-13.md)) dejó abierto si el
fallo total de reproducción estaba **en el archivo, en el protocolo `syncify-media://` o en la
decodificación**. La explicación anterior (faltaban `autoaudiosink`/`flacdec`) quedó invalidada al
comprobarse su presencia en el host.

## 2. Método: repro mínimo con las mismas versiones + controles de vía de entrega

En lugar de instrumentar la app real (que solo habría aportado la misma cadena con más variables:
cuenta, cola, DB), se construyó un **repro mínimo aislado** con la **misma combinación
tauri 2.12.1 / wry 0.57.0** que el árbol:

- `register_asynchronous_uri_scheme_protocol("syncify-media", …)` idéntico al de `main.rs`,
  con el handler de `src-tauri/src/commands/playback.rs` **copiado verbatim** (grants, parsing
  de URI, RFC 7233, tope de chunk de 8 MiB, 200/206/403/404/416).
- La misma secuencia del player real: `resolve` → grant → `convertFileSrc` → `<audio>.src` →
  `play()` → seek, con la **misma CSP** de `tauri.conf.json`.
- Fixtures sintéticos (generados con ffmpeg, rutas con acentos/espacios/guiones largos):
  FLAC 16/44.1, FLAC 24/192, MP3 192 kbps, M4A 192 kbps y FLAC de 16.4 MB (> tope de 8 MiB).
- Instrumentación en 4 capas: (1) **protocolo** (stdout del handler: method/uri/range/status/bytes),
  (2) **media element** (eventos, `readyState`, `networkState`, `error.code`, `timeupdates`, seek),
  (3) **GStreamer/WebKit** (`GST_DEBUG=2`), (4) **audio real** (captura `parec` del monitor del sink
  por defecto).
- **Controles de vía de entrega**: el mismo archivo, en la misma webview y ejecución, servido
  (A) por `syncify-media://` tal cual, (B) por `http://127.0.0.1` (servidor local sin rangos),
  (C) por `file://`, y por el custom protocol con formas de respuesta alternativas:
  (D) `200` full-body ignorando `Range`, (E) `206` sin `Content-Range`, (F/G) con `Content-Length`
  explícito.

## 3. Resultado

| Vía de entrega (mismo archivo, misma webview) | Resultado |
|---|---|
| `syncify-media://` (206 + `Content-Range`, idéntico a producción) | **FALLA** — `MEDIA_ERR_SRC_NOT_SUPPORTED` (código 4), eventos `waiting → loadstart → error`; los 5 formatos |
| `syncify-media://` 200 full-body / 206 sin `Content-Range` / ±`Content-Length` | **FALLAN igual** (código 4) |
| `http://127.0.0.1:8123` (mismo archivo, servidor sin rangos) | **REPRODUCE**: `readyState=4`, `playing`, `timeupdates=17`, duración 20 s, **seek a 12 s correcto** (`seeked`, `t=14.99`) |
| `file://` | Falla código 4 (restricción de origen de la página, irrelevante para el diagnóstico) |

Capas de la prueba:

- **Protocolo (Rust): exonerado.** Todas las peticiones del custom protocol llegaron con la URI
  percent-encoded correcta y recibieron `206` con `Content-Range` correcto; cero `403/404`.
  WebKit realiza **una única petición de 1446 bytes** por archivo (`Range: bytes=0-1445`) y
  abandona con `FormatError` — la capa media no llega a pedir el resto del archivo.
- **GStreamer (`GST_DEBUG=2`):** `webkitmediaplayer MediaPlayerPrivateGStreamer.cpp:1617:
  loadingFailed: Loading failed, error: FormatError` — sin error de pipeline GStreamer
  subyacente (no es decodificación: no hay mensaje `ERROR` de GStreamer).
- **Decodificación del sistema: correcta.** `gst-launch-1.0 playbin` y `ffplay` reproducen los
  mismos fixtures sin error, y la captura del monitor confirma señal audible con el control HTTP.

## 4. Veredicto

**El motor media de WebKitGTK no consume media servido por el custom scheme de wry/Tauri.** La
forma de la respuesta HTTP no lo cambia (ni 200/206, ni `Content-Range`, ni `Content-Length`).
No es el archivo, no es el protocolo de Syncify y no es la decodificación: es la **vía
custom-scheme → media** en sí, y es exactamente por donde pasa hoy toda la reproducción local
(`<audio>` + `convertFileSrc('syncify-media')`).

Alcance del veredicto: demostrado en Linux con las versiones de la tabla superior y consistente
con el fallo observado en el AppImage 0.6.1 (que también embarca WebKitGTK). **Windows queda
pendiente de verificación dirigida**; la decisión de arreglo no depende de ella (ver §5).

## 5. Decisión derivada (aprobada 2026-10-17)

Reemplazar la ruta rota por un **motor local nativo en Rust** (rodio + symphonia) que no depende de
WebKit ni GStreamer para el audio, con MPRIS nativo — Fase 1 del plan de reproducción. La ruta
`<audio>` + `syncify-media://` no se parchea: cualquier parche conservaría la dependencia rota y
sería trabajo perdido. Enmienda de alcance: [`DECISION_ALCANCE_BETA.md`](../DECISION_ALCANCE_BETA.md)
(Enmienda 2026-10-17).

## 6. Receta de reproducción del hallazgo

1. Fixtures (ffmpeg): tonos de 20 s a volumen bajo (`sine=frequency=440:duration=20,volume=0.05`)
   en `.flac` (16/44 y 24/192), `.mp3`, `.m4a`, más un `.flac` de ruido blanco de >8 MiB.
2. Repro: crate Tauri con el handler copiado de `playback.rs` + página que ejecuta
   `convertFileSrc` → `<audio>.play()` y registra eventos/errores (instrumentación de las 4 capas).
3. Controles: `python3 -m http.server` para la vía HTTP y variantes del handler para 200/206/CL.
4. Ejecutar con `GST_DEBUG=2` y captura `parec -d <monitor-del-sink> …` en paralelo.
5. Criterio de aceptación del hallazgo: custom protocol = código 4/`FormatError` en todos los
   archivos; HTTP = `readyState 4` + `timeupdates>0` + seek correcto.

Notas de entorno de la corrida: la sesión se lanzó con `GDK_BACKEND=x11` (Wayland crasheaba el
proceso de prueba con «Error 71 (Protocol error)» en este contexto de arranque); la captura de audio
del monitor quedó contaminada por un stream del escritorio ajeno al probe y **no se usa como
evidencia** — la evidencia de reproducción es el estado del elemento y el control HTTP.

## 7. Límites

- No se ejecutó la reproducción dirigida **en la app real versionada** por el usuario (`AppImage
  0.6.1`): el repro replica el handler y las versiones, y la app real comparte la misma vía rota.
- **Windows no verificado** en esta corrida (se hará sobre el motor nativo en Fase 4).
- El veredicto vale para media local; el streaming de proveedores (Fase 2) no depende de
  custom schemes en la arquitectura nativa.

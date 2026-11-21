# WK-01 · composición WebKitGTK: experimento aislado medido (2026-11-21)

Evidencia fechada, **no estado actual**. El estado del carril vive en el issue
[#52](https://github.com/MadManJohnSmith/Syncify/issues/52) y en la cola
[`docs/COLA_WEBKIT_COMPOSICION_ACELERADA.md`](../COLA_WEBKIT_COMPOSICION_ACELERADA.md).

## Qué se midió

Experimento de [#52](https://github.com/MadManJohnSmith/Syncify/issues/52): un
webview mínimo en **proceso hijo** (GTK 3 + WebKitGTK 4.1, la misma pila que el
webview de Tauri) cargando una pantalla fija con un bloque en movimiento, con el
**mismo backend gráfico** que la app y un timeout por corrida.

- Harness: `tools/wk01-composicion.sh`, sonda `tools/wk01/webkit_probe.py`,
  pantalla `tools/wk01/fixture.html`.
- Perfil efímero por corrida (`XDG_DATA_HOME`/`XDG_CONFIG_HOME`/`XDG_CACHE_HOME`
  temporales): no escribe en `~/.local/share/com.syncify.app` ni toca la
  biblioteca. No arranca, reinicia ni mata la instancia de Syncify en uso.
- 18 corridas: 2 backends × 3 rutas × 3 repeticiones, rutas intercaladas dentro
  de cada repetición, misma fixture (480×360) y misma máquina.
- Rutas: `compatible` (`WEBKIT_DISABLE_DMABUF_RENDERER=1` +
  `WEBKIT_DISABLE_COMPOSITING_MODE=1`, lo que fija `src-tauri/src/main.rs`),
  `acelerada` (sin esas variables) y `dmabuf-off` (solo sin renderer DMABuf).
- Señales por corrida: código/señal de salida, ventana visible con píxeles
  (bloque rojo y diferencia entre dos capturas), `requestAnimationFrame`
  del proceso web, frames pintados por GTK (`after-paint`) y
  `webkit_web_view_get_snapshot()` como control de "el contenido se renderiza".

Los artefactos (JSONL por corrida, capturas PNG, `results.json`, `resumen.md`)
quedaron fuera del repo, en
`~/.local/state/syncify-wk01/<etiqueta>/`.

## Entorno medido

NVIDIA GTX 1050 Ti, driver 580.196.07; KDE/Wayland; kernel 7.3.4-1-cachyos;
GTK 3.24.52; WebKitGTK 2.54.3; DISPLAY `:0` (XWayland) y `wayland-1`. Carga
media durante la medición: 5–9 (otros carriles de la Ola 0 activos).

## Resultados

Medianas de las 3 repeticiones por ruta.

| backend | ruta | salida | política efectiva | frames/s (rAF) | paints/s (GTK) | ventana rojo / cambia | snapshot rojo | errores |
|---|---|---|---:|---:|---:|---:|---:|---|
| x11 | compatible | 0,0,0 | never | 58,8 | 58,0 | 3697 / 7518 | 3700 | — |
| x11 | dmabuf-off | 0,0,0 | never | 58,8 | 58,0 | 3697 / 7518 | 3700 | — |
| x11 | acelerada | 0,0,0 | **always** | **—** (0 frames) | **0,0** | **0 / 0** | 3679 | `Failed to create GBM buffer` ×2 por corrida |
| wayland | compatible | 0,0,0 | never | 62,5 | 61,0 | 0 / 0 (captura no disponible) | 3600 | — |
| wayland | dmabuf-off | 0,0,0 | never | 62,5 | 61,3 | 0 / 0 (captura no disponible) | 3600 | — |
| wayland | acelerada | 0,0,0 | **always** | 58,8 | 58,2 | 0 / 0 (captura no disponible) | 3600 | — |

Lectura de las señales:

- `frames/s (rAF)` **no prueba presentación**: mide producción de frames en el
  proceso web, no que la ventana muestre algo. Por eso el veredicto exige
  píxeles de la ventana o `after-paint`, y el `snapshot` se usa solo como
  control de que el contenido se renderiza.
- x11/acelerada: la ventana nunca se pinta (0 frames rAF, 0 paints), el
  `snapshot` **sí** contiene el bloque (3679 píxeles rojos) y el UI process
  emite dos `Failed to create GBM buffer of size 480x360: Invalid argument`.
  El contenido se renderiza en el proceso web y **no llega a presentarse**.
  Reproducido 3/3.
- wayland/acelerada: el `after-paint` acompaña al rAF (~58/s) como en la ruta
  compatible (~61/s). La captura de ventana no es posible en Wayland (devuelve
  negro también en la ruta compatible), así que allí no hay prueba de píxel.
- La ruta `dmabuf-off` resolvió la política efectiva a `never`, igual que la
  compatible: en WebKitGTK 2.54.3 no es un punto medio entre las dos rutas.

## Veredicto de esta máquina

- **Backend X11/XWayland (el que usa la app en desarrollo):** la ruta acelerada
  **no compone/presenta**: ventana en blanco, sin frames pintados, con fallo de
  GBM. La compatible presenta contenido a ~58 frames/s. **No habilitar la ruta
  acelerada sobre X11 en este equipo.**
- **Backend Wayland (el que usa la app empaquetada en esta sesión):** la ruta
  acelerada pinta frames a ~58/s, sin errores de EGL/GBM; **compone**, aunque sin
  prueba de píxel por falta de captura en Wayland.
- No se reprodujo ningún abort de EGL (`Could not create default EGL display`):
  el fallo observado en X11 es ventana en blanco, no caída del proceso.

## Alcance y lo que falta

- El veredicto aplica a **esta** máquina (la de los carriles de la Ola 0). El
  script es reutilizable en cualquier otra (`tools/wk01-composicion.sh`,
  `/usr/bin/python3` con PyGObject del sistema); **no se ejecutó en el Dell
  Latitude 5420**, que sigue sin medir.
- Medición de composición, no de rendimiento: la carga de la máquina durante la
  corrida fue 5–9, así que las diferencias de fps entre rutas no se interpretan
  como mejora ni regresión.
- La sonda no es la app: reproduce su pila de webview y su backend, no sus
  vistas ni su carga. Ningún cambio de producto acompañó a esta medición.

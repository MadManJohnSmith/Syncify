---
name: syncify-screenshots
description: "When the user wants to update, refresh or retake the Syncify app screenshots for the GitHub frontpage/README — 'actualiza las capturas', 'refresh the screenshots', 'nuevas capturas de la app', 'la UI cambió, actualiza la portada', 'screenshots for the README'. Also when a UI change made the current docs/screenshots stale."
metadata:
  version: 1.0.0
---

# Syncify — capturas reales de la app para la portada de GitHub

Las imágenes de la portada (`docs/screenshots/*.png`, usadas por `README.md` y
`README.es.md`) son capturas de la aplicación **corriendo de verdad con la
biblioteca real** — nunca mocks ni imágenes generadas. Este skill estandariza
cómo renovarlas.

## Comando principal

```bash
# Set completo de marketing (dashboard, library, downloads, lyrics)
python3 scripts/capture_screenshots.py

# Solo algunas vistas
python3 scripts/capture_screenshots.py --only library lyrics

# Incluye el asistente de 4 pasos (SOLO si el onboarding está pendiente:
# data dir fresco o localStorage sin syncify_onboarding_completed)
python3 scripts/capture_screenshots.py --onboarding
```

El script lanza `target/debug/syncify-tauri` forzado a XWayland, navega con
los atajos Ctrl+1..8 y clics sintéticos (XTEST vía python-xlib), captura la
ventana por id con ImageMagick y optimiza a 1600px dentro de
`docs/screenshots/`. Al final cierra la app que él mismo lanzó.

## Prerrequisitos

1. Sesión X11/XWayland (`echo $DISPLAY` no vacío; en Wayland puro no funciona).
2. Binario debug: `cargo build -p syncify-tauri`.
3. `pip install --user --break-system-packages python-xlib` (Arch/CachyOS).
4. ImageMagick 7 con soporte X11 (`magick import`).
5. La biblioteca local debe tener datos reales (las capturas son el producto).
6. Ninguna otra instancia de Syncify corriendo.

## Qué captura cada receta — y qué NUNCA se publica

| Receta | Vista | Por qué vende |
|---|---|---|
| `dashboard` | Hero: resumen de biblioteca, fuentes por servicio, stats de descarga | Prueba de escala real |
| `library` | Tabla completa (portadas, badges de servicios, scores META) | El catálogo unificado |
| `lyrics` | Lector de letras sincronizadas con reproductor (abre un track con "Synced") | La cascada de letras |
| `downloads` | Cola con hilos, reintentos, badges de calidad | El motor de descargas |
| `services` *(onboarding)* | Los 6 servicios "Connected" | Un login por servicio |

**Descartadas siempre, aunque existan:**
- **Accounts**: expone el Client ID real y estados de reconexión.
- **Migrate**: muestra los 6 servicios como "Not Connected" (su chequeo de auth
  no refleja las sesiones activas — si eso se arregla algún día, reconsiderar).
- **Settings**: muestra rutas personales del usuario.
- Cualquier captura con emails, tokens o datos de cuenta visibles.

## Después de capturar (checklist)

1. **Verificar cada PNG visualmente** (Read) — nada de toasts, cursores
   capturados, overlays a medio cerrar o paneles vacíos.
2. Si cambió el set de imágenes, actualizar la galería en `README.md` y
   `README.es.md`: hero a ancho completo tras los badges + tabla 2×2 con
   captions orientados a beneficio (EN y ES espejados).
3. Optimización ya hecha por el script (1600px, strip); PNG debe quedar
   <400KB por imagen.
4. Commit + push a `syncify-app` + fast-forward a `main`
   (`git push origin syncify-app:main`).

## Si el script falla — receta manual

1. `GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 ./target/debug/syncify-tauri &`
2. Activar la ventana con python-xlib (`configure(stack_mode=Above)` +
   `set_input_focus`). **XTEST no mueve el puntero** — usar
   `root.warp_pointer(x, y)`; los clics XTEST ButtonPress/Release sí funcionan
   tras el warp.
3. Captura exacta: `magick import -window 0x<ID> out.png` (1920x1020, píxel =
   coordenada local; la ventana queda en root y=+28 → root = local + 28).
4. Navegación solo con teclado: Ctrl+1 Library, Ctrl+2 Downloads, Ctrl+3
   Metadata, Ctrl+4 Lyrics, Ctrl+5 Accounts, Ctrl+6 Migration, Ctrl+8
   Settings, Ctrl+K paleta de comandos (buscar "playlists").
5. El onboarding aparece si el localStorage del origen de producción no tiene
   `syncify_onboarding_completed`/`syncify_onboarding_complete`="true"
   (utf-16le); sembrar `localstorage/tauri_localhost.localstorage` a mano NO
   funciona (WebKit migró de store) — completar el asistente por clics.

## Mantenimiento

- Las coordenadas de clic son *window-local* para la ventana 1920x1020 y
  viven en `scripts/capture_screenshots.py` (`TOAST_X`, `ONBOARDING_WALK`,
  `RECIPES`). Si la UI cambia de layout, re-verificarlas: capturar, mirar la
  imagen, ajustar. Las capturas del onboarding se toman en los pasos 1 y 4
  del walk (`ONBOARDING_SHOTS_ON_STEP`).
- Si se edita este SKILL.md, sincronizar ambas copias:
  `cp .agents/skills/syncify-screenshots/SKILL.md ~/.agents/skills/syncify-screenshots/SKILL.md`

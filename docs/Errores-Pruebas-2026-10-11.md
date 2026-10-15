# Errores y peticiones detectados en pruebas manuales — 2026-10-11

Registro de lo que se observó usando la aplicación, agrupado por pantalla. El orden
de este archivo no implica prioridad. Las entradas Q-04, Q-06, Q-07 y Q-12 están
marcadas **HECHO** con cambios de código y pruebas automatizadas; las demás siguen
en la cola de triaje y verificación. Estas pruebas no sustituyen la comprobación
en una sesión gráfica real. El resto sigue el mismo proceso que
`Errores-Pruebas-2026-10-08.md` (triaje → matriz → plan → workflow).

Origen: pruebas manuales del usuario (2026-10-11), sobre AppImage en Linux, con cuentas
logueadas en Spotify, Qobuz y Tidal (varias cuentas por servicio, incluidas las 4 de Qobuz
  que muestran `REAUTH` en la captura).

---

## Global / layout

- **Q-01 — La barra de estado tapa elementos de la aplicación.** La `status-bar` fija abajo
  (`fixed bottom-0`) se solapa con controles que viven en la zona inferior, por ejemplo el
  botón del drawer de navegación (`#app-nav-drawer > nav > button`). No hay separación real
  entre el contenido con scroll y la barra: los clics aciertan en la barra, no en el
  elemento que el usuario ve debajo. Revisar el z-index/espaciado inferior de todos los
  overlays flotantes (nav drawer, batch-bar de descargas) respecto a `.status-bar`.
  **Arreglado el caso del lateral (12-oct-2026):** el `<nav>` del lateral ahora
  reserva `padding-bottom: calc(var(--status-bar-h, 48px) + 8px)`, igual que
  `#page-content`, de forma que "Now Playing" se puede desplazar por encima de
  la barra inferior aunque esta crezca. Cubierto por `LayoutSpacing.spec.ts`
  (8/8), suite UI completa 997/997 y build en verde. El caso general de otros
  overlays flotantes queda pendiente de su propia comprobación visual.

## Cuentas (Accounts)

- **Q-02 — No se pueden diferenciar las cuentas por su correo o nombre de usuario.** En
  Qobuz y Tidal todas las tarjetas se llaman igual ("Qobuz User", "tidal User") y no hay
  forma de saber cuál es cuál; solo Spotify muestra el identificativo real
  (`Usuario (usuario@example.com)`). La tarjeta activa también dice "Reconnect"
  junto al badge `ACTIVE`, lo que contradice la segunda captura, donde esa misma cuenta está
  en `REAUTH`. Hay que pintar la identidad real (login/email, o un alias que el usuario pueda
  poner) en cada fila de cada servicio, y no repetir el nombre del proveedor como nombre de cuenta.
- **Q-03 — Las sesiones caducan solas en menos de 20 minutos.** Tras iniciar sesión en todas
  las cuentas, en no más de ~20 min Qobuz entra en `Reconnect Required` con el aviso "The
  linked account can no longer access Qobuz. Reconnect to keep syncing." mientras las
  descargas/sync siguen corriendo (el log muestra `UPDATE albums SET total_tracks` activo).
  Parece que el token se refresca mal o no se refresca: el estado `ACTIVE` y el aviso de
  reautenticación conviven. Mejorar la gestión de sesiones para que duren lo más posible en
  estado válido: refresco proactivo antes de expirar, reintento transparente con el token
  renovado, y no marcar `REAUTH` cuando la cuenta sigue siendo usable.

## Descargas

- **Q-04 — El botón de descargar no da ningún feedback de estado.** El botón de la batch-bar
  (`#page-content > div > div.batch-bar...z-40 > div > button.px-3.5...cursor-pointer`) no
  indica si la descarga empezó, si sigue en curso o si falló: el usuario no sabe si el clic
  funcionó. Falta estado explícito en el botón (`idle` → `enviando` → `encolado/descargando`
  → `error`), deshabilitarlo mientras se envía, y un mensaje de error visible si el comando
  falla.
  **HECHO (12-oct-2026).** El botón de la batch-bar ahora tiene estado de envío: `isEnqueuingSelection` (`LibraryView.vue`) se activa al pulsar y se libera en un `finally`; el botón se deshabilita (`disabled` + `aria-busy`), cambia su icono y su texto a "Enqueuing…", y un segundo clic no lanza otra petición. Los toasts existentes se conservan. Cubierto por test: idle → pendiente (doble clic bloqueado) → resuelto.

## Migrar (Migrate)

- **Q-05 — La tarjeta de Migrate desperdicia espacio.** La tarjeta queda centrada con mucho
  hueco vacío a los lados, y se pintan dos barras que no muestran información (vacías).
  Recuperar el ancho útil y quitar o rellenar esas barras.

## Metadata

- **Q-06 — El aviso "1 issues" no dice cuál es el problema.** En la vista de metadatos de una
  pista, la insignia roja "⚠ 1 issues" no es accionable: no se ve de qué issue se trata.
  Mostrar el detalle (qué campo, qué falla, qué se puede hacer) sin tener que abrir otro
  sitio.
  **HECHO (12-oct-2026).** `calculateIssues` deriva de `missingMetadataFields` (`MetadataView.vue`), que devuelve los nombres de los campos ausentes. Bajo el badge del detalle se pinta "Missing: …" con `data-testid="metadata-issues-detail"`, visible sin hover. Las filas de lista tienen altura fija por scroll virtual, por eso el detalle se muestra solo en la ficha de la pista.

## Lyrics

- **Q-07 — Se ven las etiquetas de tiempo fuera del modo edición.** En la lista de pistas las
  etiquetas LRC (`<00:00.75>`) se muestran aunque no se esté editando. Ocurre con las letras
  sincronizadas por palabra o sílaba ("Kugou Karaoke KRC", que abre `<00:00.75>I <00:01.03>...`);
  las de tipo línea simple no lo hacen. La vista de solo lectura debe renderizar palabras
  limpias y scroll/resaltado sincronizado, no el marcado crudo.
- **Q-08 — El tipo de letra no se informa bien.** La pista muestra solo `Synced` o `None`, sin
  distinguir línea / palabra / sílaba. En la ficha coincide: "Synced · word" para una KRC.
  Falta exponer el tipo real (`line`, `word`, `syllable`) en la UI y usarlo para decidir
  cómo se pinta y cómo se resalta.
- **Q-09 — El modo edición no representa bien las letras por palabra o sílaba.** El editor
  muestra filas con `<00:22.74>I'm <00:23.05>...` en una sola línea de texto concreto plano,
  sin estructura por token ni tiempos por token; para KRC no se pueden editar los tiempos por
  palabra correctamente. Hace falta un modelo de edición por token (palabra + timestamp) para
  word/syllable, y no tratar todo como LRC de línea.
  **HECHO (12-oct-2026).** `stripWordTimestamps` (`LyricsView.vue`) quita los marcadores `<mm:ss.xx>` de palabra **solo** en la representación de lectura. `line.text` no se modifica: la serialización al guardar conserva el texto crudo y la sincronización por palabra.

## Reproducción (Playback)

- **Q-10 — La app se congela al reproducir y luego se queda en negro (fallo de audio
  identificado; causa de la pantalla negra aún por confirmar).** Al pulsar play la interfaz
  deja de responder; pasado un rato la ventana se queda negra, pero el proceso sigue vivo
  (la consola y el indicador de la bandeja del SO muestran actividad). El registro muestra
  que falta `autoaudiosink` en GStreamer, pero no demuestra por sí solo por qué la ventana
  termina negra.
  **Evidencia de la consola (2026-10-12, host del usuario):**

  ```
  06:07:56.189  INFO syncify_tauri::commands::playback: resolve_playback_source: track_id=5918
  06:07:56.190  DEBUG sqlx::query: SELECT file_path, file_format FROM downloads WHERE track_id = ? → 1 fila
  06:07:56.19x  GStreamer element autoaudiosink not found. Please install it
  06:07:56.202  (WebKitWebProcess:445990): GLib-GObject-CRITICAL **: invalid (NULL) pointer instance
  06:07:56.202  (WebKitWebProcess:445990): GLib-GObject-CRITICAL **: g_signal_connect_data: assertion 'G_TYPE_CHECK_INSTANCE (instance)' failed
  ```

  El backend resuelve bien: encuentra el archivo local (fila en `downloads`, `track_id=5918`)
  y devuelve la ruta. **El fallo está en el proceso WebKit/GStreamer, no en Rust**: el
  elemento `autoaudiosink` no existe y el intento de conectarlo devuelve una instancia NULL,
  que es justo lo que parte el WebProcess. Detrás está el diseño ya documentado: la
  reproducción es un `<audio>` único servido por el protocolo propio
  `syncify-media://` (`ui/src/composables/usePlayer.ts:5,153,687,1103`), y WebKitGTK delega
  el audio en GStreamer.

  **Verificado en esta sesión (solo lectura):**

  - El host del usuario hoy tampoco lo tiene: `pacman -Qq | grep gstreamer` devuelve solo
    `gstreamer` y `vlc-plugin-gstreamer`; `gst-inspect-1.0 autoaudiosink` responde
    "No such element or plugin". Faltan los paquetes de plugins (`gst-plugins-base/good/bad`,
    `gstreamer1.0-alsa`/`-pulseaudio`), que en Arch viven en otro paquete.
  - El CI de Linux ya contempla exactamente este síntoma y su causa
    (`.github/workflows/build-linux.yml:191-299`): instala
    `gstreamer1.0-plugins-base/good/bad` + alsa/pulseaudio y verifica con `gst-inspect-1.0`
    que `autoaudiosink` y `autovideosink` existen antes de empaquetar, porque sin plugins de
    sink "el AppImage se publica sin salida de audio". Es decir: **el requisito de
    `autoaudiosink` ya es conocido y está resuelto en el pipeline de AppImage, pero eso no
    cubre a quien ejecuta la app instalada/desde el repo sin esos paquetes**, que es el caso
    de esta prueba.
  - La UI no avisa del fallo real: `usePlayer.ts:623` escucha el evento `error` del elemento y
    en varios sitios el `play()` se traga con `.catch(() => { isPlaying.value = false })`
    (`:810`, `:955`), así que el usuario no ve por qué no suena nada.

  **Implicación para la cola (aún sin empezar):** al planificar Q-10 hay que separar dos
  cosas — (a) el requisito de entorno/dependencia (plugins de sink de GStreamer, con su
  verificación y mensaje claro si faltan, igual que hace el CI), y (b) el diagnóstico de UI
  para que un fallo de audio se muestre como error legible en lugar de congelar la vista.
  La **causa de congelación/pantalla negra sigue sin explicación**: el log no muestra ni
  panic ni stack overflow en el lado Rust, así que hay que reproducirlo con los plugins
  instalados para saber si el síntoma visual sobrevive al arreglo del audio o si era una
  consecuencia suya. No está empezado.

---

## Global / layout (segunda tanda, 12-oct)

## Descargas → calidad (Tidal)

- **Q-15 — Las descargas de Tidal salían en m4a cuando se pedía la máxima calidad.
  HECHO (12-oct-2026), con un matiz honesto.** Había dos fugas, no una:
  1. `parse_tidal_playback_manifest` (`crates/syncify-tidal-downloader/src/lib.rs`) trataba
     el contenedor DASH como prueba de 24-bit hi-res, así que un manifest AAC mal etiquetado
     podía registrarse como FLAC 24-bit. Ahora hi-res solo se marca si Tidal **declara**
     `audioQuality: HI_RES_LOSSLESS/HI_RES`; el contenedor y el tier pedido ya no sustituyen
     al códec.
  2. La UI forzaba `allowFallback: true` en `LibraryView.vue` (`handleDownload`,
     `handleDownloadBestQuality`, lote), lo que desactivaba el rechazo de degradación
     incluso con "skip on downgrade" configurado. Ahora la petición respeta la calidad
     elegida en Ajustes (`maxQuality` → `high|lossless|hires`) y solo permite degradación si
     el ajuste lo permite. El menú "From Tidal" pasó de un único ítem fijo en 16/44.1 a
     elegir **Hi-Res FLAC (24-bit) / FLAC (16/44.1) / AAC 320 (m4a)**.
  Cubierto por tests nuevos: AAC sobre DASH nunca se reporta como FLAC, un pedido HiRes que
  responde AAC queda como `m4a`/lossy y se rechaza en política estricta, y la etiqueta
  declarada HI_RES_LOSSLESS sí se marca hi-res. Verdes: `syncify-tidal-downloader`
  (19/19), `syncify-core-domain` (71/71), UI completa (993/993) y test de integración
  Tidal/Tauri (19/19).
  **Pendiente de verificar:** que Tidal devuelva realmente FLAC para estos tracks concretos
  con la cuenta del usuario. El código ya no falsea la etiqueta, pero la calidad que el
  servicio entregue depende de la cuenta y del catálogo; eso solo se puede confirmar
  descargando con la sesión real. No está hecho.

## Descargas → cuentas y suscripciones

- **Q-16 — Qobuz podía descargar un preview de 30 s desde una cuenta sin streaming.
  PARCIALMENTE HECHO (12-oct-2026), con una salvedad.** Qobuz autentica igual una cuenta
  cuyo plan no incluye streaming, y a esas cuentas `track/getFileUrl` les devuelve una URL
  válida para un preview de 30 segundos. El código pedía la primera cuenta cuyas
  credenciales resolvieran, sin mirar la suscripción.
  Lo hecho:
  - Migración `0095_qobuz_streaming_entitlement.sql`: columna `streaming_entitled` por cuenta
    (1 = puede hacer streaming, 0 = autenticada sin streaming, -1 = desconocido/legacy).
  - `get_service_fallback_account_ids` ahora ordena por entitlement: con streaming →
    desconocido → sin streaming. **Nunca excluye una cuenta**: un valor desconocido pesa más
    que "sin streaming" precisamente para no romper cuentas previas.
  - El login de Qobuz consulta `user/getUserData` y guarda el estado real. Las credenciales
    inválidas siguen yendo al final, como antes.
  - La vista de Cuentas muestra una advertencia clara cuando una cuenta no tiene streaming,
    para que un archivo de 30 s se reconozca en vez de sorprender.
  - El validador existente (`validate_qobuz_audio_payload`) sigue rechazando el preview después
    de bajarlo; no lo toqué.
  Verificado: 24/24 tests de `accounts` (incluidos dos nuevos del orden por entitlement),
  370/370 de `syncify-tauri --lib`, 996/996 de la UI y build de UI en verde.
  **Salvedad honesta:** la consulta de `user/getUserData` se hace al iniciar sesión y, para las
  cuentas que aún están en `-1` (las conectadas antes de esta columna), una sola vez durante la
  descarga. No hay refresco periódico: si una suscripción caduca, el valor guardado sigue
  diciendo "puede hacer streaming" hasta el siguiente inicio de sesión. Lo protege a medias el
  validador, que sigue rechazando el preview después de bajarlo.
  *Sin empezar: el refresco periódico del entitlement.*

- **Q-17 — Otros servicios: auditar si necesitan la misma comprobación de suscripción.**
  Resultado de la auditoría, sin código tocado:
  - **Tidal**: no sirve previews de 30 s. Su modo de fallo es la degradación silenciosa a
    lossy/AAC, que es lo que ya se corrigió en TIDAL-QUAL.
  - **Spotify, SoundCloud, YT Music**: no son fuentes de descarga. El orquestador nativo solo
    cablea Qobuz y Tidal (`src-tauri/src/download/orchestrator.rs:123-131`); SoundCloud además
    tiene su propia comprobación de `streamable`/`policy`.
  - **Deezer**: **sí necesita la misma revisión**. El puente detecta el plan solo por
    `web_hq`/`web_lossless` (`scripts/services/deezer_service.py:206-211`) y degrada la calidad
    sin validar que el archivo recibido no sea un preview; no lee el estado real de la
    suscripción en ningún sitio. *Sin empezar — anotado para revisión.*

- **Q-11 — No se puede iniciar sesión en otra cuenta de Spotify mientras se está
  sincronizando.** El flujo de añadir cuenta se bloquea durante la sincronización; hace falta
  saber si es un bloqueo deliberado (exclusión mutua con la importación) o un estado que no
  se libera. *Sin empezar — anotado para triaje.*
- **Q-12 — Eliminar de la barra lateral la franja superior del lateral. HECHO.** El bloque
  `#app-nav-drawer > div` (fila de 64px con `border-b`, cuyo único contenido era el botón de
  colapsar) se eliminó de `ui/src/App.vue` a petición del usuario, porque con la hamburguesa
  de la cabecera ya sobra. Ahora el `<aside id="app-nav-drawer">` arranca directamente en el
  `<nav aria-label="Vistas">`. Los 8 tests de `ShellCascaron.spec.ts` se actualizaron para
  exigir la forma nueva —el primer hijo del lateral es `nav`, y `nav-sidebar-collapse` no
  existe— y el colapso se prueba a través de `nav-drawer-toggle`. Suite en verde: 8/8.
  El toggle `toggleSidebarCollapsed` sigue vivo y lo usa la hamburguesa.
- **Q-13 — La sincronización duplica demasiado.** El ámbito de duplicados al sincronizar
  (importar) es mucho mayor de lo que debiera: la misma pista/entrada reaparece al repetir
  una sincronización. Falta mirar la clave de deduplicación real (identidad
  `track_sources(service_id, service_track_id)`, `ON CONFLICT` y la re-chequeo de
  "¿ya existe?" antes de insertar) y los lotes que se reenvían. *Sin empezar — anotado
  para triaje.*
  **Pista:** el mismo síntoma de recargas durante la importación que describe Q-14 pasa
  por `debouncedReloadLibrary()`; conviene mirar ambas juntas porque comparten las
  recargas, aunque Q-13 habla de filas duplicadas y Q-14 de filas que no se renderizan.
  *Sin empezar.*

- **Q-14 — La biblioteca se queda mostrando unas pocas pistas durante la importación.**
  Mientras se importan pistas y se navega por Library, a veces solo se renderizan unas
  pocas filas y no se puede avanzar. Cambiar a otra pestaña y volver recupera la vista.
  *Sin empezar — anotado para triaje.*

  **Pistas del código, no causa confirmada:** `loadLibrary()` vuelve a cargar la primera
  página de 100 pistas (`ui/src/views/LibraryView.vue:1517-1540`); eventos
  `LIBRARY_UPDATED`, `IMPORT_COMPLETE` y `SYNC_COMPLETE` disparan recargas con debounce
  (`:2160-2168`, `:2701-2709`). `loadMore()` depende de `hasMore` y del evento de scroll
  (`:1543-1565`, `:1622-1663`), mientras el render de filas depende del `scrollTop`, la
  altura visible y el rango virtual (`:2318-2345`). Hay que reproducir el fallo y medir
  esos estados antes de atribuirlo a una carrera o al scroller: el log y el relato aún
  no permiten decidir cuál de esos mecanismos se atasca.

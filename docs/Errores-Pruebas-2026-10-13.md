# Errores y peticiones detectados en pruebas manuales — 2026-10-13

Registro de lo que se observó usando la aplicación el 13-oct-2026, agrupado por pantalla.
El orden no implica prioridad. Sigue el mismo proceso que
`Errores-Pruebas-2026-10-11.md` (triaje → matriz `docs/auditoria/matriz-ajustes.md` → plan → workflow).
El detalle de cada ajuste aplicado está pre-registrado y verificado en la sección
«Ajustes detectados tras el respaldo de 2026-10-13» de la matriz (regla R1).

Origen: pruebas del usuario sobre la rama de respaldo de 2026-10-13 en Linux, con canciones ya
borradas del disco, colas de descarga reales y cuenta de Spotify/Qobuz/Tidal conectadas.

Estado de esta sesión: **3 ajustes aplicados y verificados con pruebas automatizadas**
(DL-DISK-01, DL-UX-02, TRAY-OPEN-01); **4 frentes siguen abiertos** y quedan registrados como
DL-SVC-01, DL-SVC-02, DL-SVC-03 y PLAY-TRACE-02. Ninguno de estos cambios se ha comprobado en
una sesión gráfica real con cuentas reales, salvo donde se dice lo contrario.

---

## Biblioteca (Library)

- **L-13 — Las pistas borradas del disco seguían detectadas como descargadas.** Tras borrar
  todo el audio descargado y pulsar «Clear All» en la pestaña de descargas, al volver a
  encolar pistas la aplicación decía «ya descargado» y no descargaba nada.
  **HECHO (13-oct-2026), solo para el guardrail de encolado.** `check_queue_guardrail`
  (`src-tauri/src/commands/queue.rs:230-272`) confiaba en que `downloads.file_path` no
  estuviera vacío y leía una sola fila con `LIMIT 1`. La fila puede sobrevivir al fichero
  (borrado, o movimiento interrumpido), así que la cola saltaba la pista para siempre. Ahora
  se consultan **todas** las filas equivalentes (por `track_id`, ISRC normalizado, o
  título+duración±2 s+artista) y solo devuelve `AlreadyDownloaded` si alguna tiene un
  fichero regular real en disco. Corrige también el orden de las filas: antes una fila
  ausente delante bloqueaba aunque otra equivalente tuviera el fichero.
  Cubierto por `queue_audit_and_guardrails_test` (4/4) y `batch_100_pilot_test` (1/1)
  ([DL-DISK-01]).

  **Límites conocidos, no resueltos:**

  1. La **biblioteca** sigue derivando «descargado» de `d.file_path IS NOT NULL` sin mirar el
     disco (`src-tauri/src/commands/library.rs:202-206`), así que la vista de biblioteca aún
     puede mostrar una pista como presente cuando su archivo no existe. La reconciliación
     explícita `perform_reconcile_library_physical_state`
     (`src-tauri/src/commands/library.rs:3892` (cuerpo: 3892-3900)) ya existe y no se modificó: **no se borra
     ningún archivo ni ninguna fila**; las filas huérfanas quedan para que el usuario decida.
  2. La rama de título+duración sigue pudiendo equiparar dos ediciones distintas con el mismo
     ISRC normalizado. Ese comportamiento es anterior a este cambio; los tests nuevos no lo
     cubren y sigue abierto.

  Evidencia de partida (auditoría de solo lectura sobre la BD real, 2026-10-13): de **180**
  filas en `downloads`, **85 rutas no tenían archivo** y 95 sí. De los ficheros presentes,
  **91 tenían cabecera `fLaC` real** (todos registrados como Qobuz) y **4 eran M4A/AAC con
  cabecera `ftyp`** (los cuatro registrados como Tidal).

## Descargas (Downloads)

- **D-13 — Las tarjetas de fallo ocupaban demasiado espacio y daban información repetida.**
  En pantalla mediana solo cabían dos tarjetas de fallo, y cada una repetía el mismo texto
  tres veces (razón de decisión, aviso de error y detalles expandidos).
  **HECHO (13-oct-2026).** La fila queda compacta (portada, título, insignia de causa,
  servicio, artista • álbum y el mensaje final). La franja de metadatos, la caja de decisión
  de calidad siempre visible y el aviso de error separado se eliminaron: todo eso está ahora
  dentro de los detalles expandibles, junto con intentos, procedencia (Origin/Effective),
  calidad pedida/recibida y resultado. La expansión ahora sí funciona: usaba
  `item.showDetails`, escrito sobre un objeto que un `computed` rehacía en cada pase, así que
  el panel se cerraba solo al llegar cualquier actualización; el estado pasó a un
  `ref<Set<number>>` (`expandedFailedDetails`, `ui/src/views/DownloadsView.vue:1249-1259`),
  el mismo patrón que ya usaban las filas completadas.
  Cubierto por `DownloadsView.spec.ts` (29/29) ([DL-UX-02]).

- **D-14 — «Attempts: 99» en tarjetas que no se reintentaron 99 veces.**
  **HECHO (13-oct-2026).** `retry_count = 99` es el centinela con el que el worker marca un
  fallo terminal/poner en cuarentena (`src-tauri/src/worker.rs:911` y `:2070`), frente al
  incremento real de `retry_count + 1` de los fallos transitorios (`:873`). La tarjeta ya no
  muestra «Attempts: 99» cuando el valor es el centinela; el panel de detalles dice
  «Stopped after a terminal failure (attempt count unavailable)».

- **D-15 — Las causas de fallo estaban mal etiquetadas.** Una fila con
  `SourceIdentityMissing: No equivalent Tidal source found...` aparecía con la insignia
  «Requires authentication», y otras como «Stale source / 404`, contradiciendo el motivo real.
  **HECHO (13-oct-2026).** `classifyFailureReason` (`ui/src/api/queue.ts:519-547`)
  comprobaba tipos de error sobre el texto concatenado del mensaje principal y de
  `last_error`, de modo que una frase como «Considered a 401-like token…» o un 404 mencionado
  en la explicación reetiquetaba la causa. Ahora los errores de identidad
  (`ambiguous_source`, `SourceIdentityMissing`, `identityconflict`, `multiple matches`) se
  clasifican primero y solo contra el mensaje principal; `SourceIdentityMissing` se muestra
  como **Source identity missing**, sin insignia de autenticación y sin acción de fallback.
  Esas filas ofrecen «Review source» (abre los detalles) en lugar de un «Retry» genérico que
  repetiría el mismo fallo. Cubierto por `queue.spec.ts` (17/17).

- **D-16 — La etiqueta de servicio al encolar no es prueba de la cuenta que descargará.**
  **ABIERTO** ([DL-SVC-01]). Una pista etiquetada como Qobuz podía fallar igualmente porque
  la etiqueta solo indica el identificador de catálogo de la fuente importada, no que una
  cuenta concreta pueda obtener el audio completo. Además `download/tidal.rs` sigue pasando
  `account_id: None` a `services/tidal_pipeline.rs`, que puede elegir otra cuenta distinta de
  la prevista. Falta separar en la UI la etiqueta heredada de la cuenta que realmente
  descargará y verificar que el resolutor sondee y catalogue candidatos Qobuz antes de caer
  en una fuente Tidal importada.

- **D-17 — Calidad real de Tidal frente a la pedida.** **ABIERTO** ([DL-SVC-02]). Los cuatro
  ficheros Tidal presentes en el disco del usuario son M4A/AAC con cabecera `ftyp` honesta y
  formato anotado en la base de datos: **no son FLAC disfrazado**. Lo que no está
  establecido es si esa degradación se permitió a propósito o si la política debería
  rechazarla; eso exige comprobar contra Tidal la calidad servida frente al techo pedido.

- **D-18 — Spotify «service unavailable» y fallos de Qobuz sin causa concreta.**
  **ABIERTO** ([DL-SVC-03]). En la base de datos real aparecen filas con
  `Unsupported or unavailable service: spotify`, y el usuario observa fallos de Qobuz
  etiquetados como Qobuz. Falta explicar cada causa concreta sin debilitar la identidad de
  edición ni el techo de calidad.

## Icono de bandeja (System tray)

- **T-13 — No había opción clara para abrir la app desde el icono.**
  **HECHO (13-oct-2026), con alcance limitado.** El primer elemento del menú cambiaba entre
  «Hide Syncify» y «Show Syncify» según el estado de la ventana, así que no siempre había una
  acción de abrir. Se añadió un primer elemento fijo **«Open Syncify»** (id `open`,
  `src-tauri/src/tray.rs:440`) que despacha `show_main_window` (mostrar + restaurar +
  enfocar) tanto con la ventana visible como oculta. La opción de ocultar se conserva.
  Cubierto por `system_tray_test` (11/11).

  **Límite:** esto solo cubre abrir **desde el menú contextual** del icono. El clic izquierdo
  sobre el icono sigue alternando visibilidad (`src-tauri/src/tray.rs:529-539`), y el cierre
  a bandeja y el arranque minimizado no se tocaron. Si lo que se quería era un solo clic que
  siempre abra (nunca oculte), eso es un comportamiento distinto y no está implementado.

## Reproducción (Playback)

- **R-13 — Todas las pistas fallan al reproducir.** **ABIERTO** ([PLAY-TRACE-01],
  actualizada como PLAY-TRACE-02 para el ruido de consola). La explicación anterior (faltaban
  los elementos de GStreamer `autoaudiosink` y `flacdec`) **ya no sirve**: en el host actual
  ambos elementos y los paquetes `gst-plugins-base/good/bad` están presentes. Hace falta una
  reproducción real que separe si el fallo está en el archivo, en el protocolo
  `syncify-media://` o en la decodificación, antes de tocar código.

- **R-14 — La consola de desarrollo inunda de información y omite el error accionable.**
  **ABIERTO** ([PLAY-TRACE-02]). Reducir el ruido informativo y elevar solo el error útil,
  sin perder la pista que hoy sí sirve para diagnosticar.

---

## Registro de pruebas de esta sesión

- `cargo test -p syncify-tauri --test queue_audit_and_guardrails_test --quiet` → 4 pasados.
- `cargo test -p syncify-tauri --test batch_100_pilot_test --quiet` → 1 pasado.
- `cargo test -p syncify-tauri --test system_tray_test --quiet` → 11 pasados.
- `npx vitest run` (UI completa, `ui/`) → 122 ficheros, 1020 pruebas, 0 fallos.
- `cargo check --workspace --quiet` → limpio.
- `cargo fmt --all -- --check` y `git diff --check` → limpios.
- Build de producción de la UI (`npm run build`) → correcto.
- **No ejecutado:** suite completa de Rust; prueba de extremo a extremo con descargas reales;
  comprobación en sesión gráfica real con cuentas reales.

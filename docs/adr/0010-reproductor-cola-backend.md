# ADR-0010 — Reproductor: cola en el backend y descubrimiento en proveedores

**Estado:** aceptado · **Fecha:** 2026-11-05 · **Enmienda:** 2026-11-05 (ADR-0014)
**Origen:** ronda de grilling del flujo REPRODUCTOR.
**Spec:** #130 · **Tickets:** #131–#149.

## Contexto

Hoy la cola, el historial, los modos y la persistencia viven en la UI
(`ui/src/composables/usePlayer.ts`), y el motor nativo solo transporta una pista
(`src-tauri/src/commands/player.rs`). Esa división impide dos cosas que el flujo
necesita: gapless/crossfade, porque el motor no conoce la pista siguiente, y
reproducción con la ventana cerrada en bandeja, porque la webview es la que
decide el avance.

El ADR-0002 excluía la búsqueda externa y el descubrimiento (veredicto S194). La
enmienda del 2026-10-17 metió el streaming de proveedores (el motor resuelve Qobuz
y Tidal) y la decisión 5 de este ADR revoca S194 para la búsqueda de catálogo; lo
que sigue fuera es navegar el catálogo completo.

La búsqueda actual es solo local, con `LIKE` sobre SQLite
(`src-tauri/src/commands/search.rs`). Las rejillas de álbumes y artistas favoritos
tienen backend y envoltorios (`src-tauri/src/commands/favorites.rs`,
`ui/src/api/library.ts`) pero ninguna vista.

La cola en localStorage no escala a decenas de miles de entradas ni es accesible
desde Rust para MPRIS, bandeja o avance sin webview.

## Decisión

1. **El backend es dueño de la cola y del estado de reproducción.** Cola, historial,
   pista actual, posición, modos, volumen y shuffle se persisten en SQLite. La UI
   pasa a ser una vista sobre comandos y eventos; MPRIS y la bandeja leen del mismo
   estado sin depender de la webview.

2. **La cola admite entradas efímeras.** Una entrada puede ser un track de la
   biblioteca (`track_id`) o un Provider item que no está en ella
   (`service_id` + `service_track_id`). La entrada efímera no crea `Track` ni
   `TrackSource` al encolarse.

3. **Promoción al acreditar la escucha.** Tras 30 s reales de reproducción, una
   entrada efímera se resuelve con el matcher de identidad existente: si hay
   `Track`, se enlaza; si no, se crea con su `TrackSource`. La entrada pasa a ser
   de biblioteca y `record_local_play` acredita `play_count` una sola vez.
   La promoción es idempotente.

4. **El crédito de escucha vive en el motor.** Rust acumula los 30 s y llama a
   `record_local_play`; la UI deja de cronometrar.

5. **Se revoca S194 para la búsqueda de catálogo.** `SearchView` gana una pestaña
   de proveedores: seis servicios en paralelo, cuenta preferida por servicio
   (D09, #121), resultados de pistas, álbumes y artistas, ficha de álbum con
   pistas y ficha de artista con álbumes. Acciones: reproducir, encolar y descargar
   según la capacidad real del proveedor. Navegar el catálogo completo sigue fuera.

6. **Favoritos tiene vista propia.** Un ítem nuevo en el sidebar con rejillas de
   álbumes y artistas; el álbum reproduce, encola, descarga o abre ficha; el artista
   abre su detalle. Sin top-tracks inventados.

7. **Se descarta `syncify.playerState.v1`.** No hay migración de la cola vieja: la
   primera versión con cola en SQLite arranca con cola e historial vacíos.

8. **Cerrar la ventana sigue el ajuste close-to-tray.** Si está activo, la
   reproducción continúa en bandeja con la cola del backend; si no, la app termina.

## Enmienda 2026-11-05 — adopción de ADR-0014

La ronda de grilling con los informes de Symfonium y Audirvāna (ADR-0014) añade
estas decisiones, sin cambiar las anteriores:

1. **Contrato de eventos.** La UI recibe una instantánea completa al montar la
   webview, al reconectar MPRIS y al abrir la bandeja; después, deltas para
   cambios de estado (`TrackChanged`, `QueueChanged`, `PlaybackStatus`,
   `PositionTick`, `VolumeChanged`, `ModesChanged`). `PositionTick` no se persiste.
   Si la UI y el backend discrepan, la instantánea manda.

2. **Shuffle estable.** La cola conserva su orden; shuffle genera un orden de
   reproducción aparte, persistido. Al apagar shuffle se vuelve al orden de cola,
   así la restauración es reproducible.

3. **Preflight de entitlement.** Antes de encolar o reproducir una entrada efímera
   se consulta `streaming_entitled`, `credentials_invalid` y `credential_state`
   de la cuenta. Si no puede sonar, la entrada se marca "reconectar" en vez de
   fallar a mitad.

4. **Prefetch y caducidad.** La siguiente entrada se resuelve antes de que termine
   la actual. Un 401/403/410 en el stream provoca una sola re-resolución/refresh y,
   si falla, un error tipado; no se reintenta como si fuera transitorio.

5. **Errores de stream tipados.** `player:stream-state` lleva `errorKind`
   (`auth|expired|network|catalog|unsupported`), `serviceName`, `accountId` y
   `statusCode`; la UI puede distinguir sesión caducada, red y catálogo. Cuando
   corresponda, se emite `auth-state-updated`/`invalidated`.

6. **Búsqueda de proveedores con estado.** Los resultados se agrupan por servicio y
   muestran `connected`/`expired`/`preview-only`/`unavailable`; las acciones
   reproducir/encolar/descargar dependen de la capacidad real. Un servicio caído no
   se presenta como "sin resultados".

7. **Entrada local o stream.** La entrada de cola lleva si es local, descargable o
   solo-stream; el reproductor prefiere el archivo descargado. Es el mismo dato que
   el preflight necesita.

Quedan **fuera** de este ADR, por ser post-beta: los servidores personales
(Plex/Jellyfin, solo lectura) y el casting UPnP/Chromecast. Ver ADR-0014.

## Consecuencias

- `usePlayer.ts` se reduce a una vista; el contrato IPC crece y la puerta
  `dead_commands_hygiene_test.rs` exige invocador para cada comando nuevo.
- Nueva migración con tablas de cola, historial y estado; no se edita ninguna
  migración aplicada.
- La promoción necesita tests de idempotencia y de identidad (ISRC, evidencia).
- La búsqueda de proveedores exige adaptadores nativos para seis servicios;
  Apple Music y Spotify necesitan búsqueda de listas, no solo `search_track`.
- Depende de D09 (#121) para la cuenta preferida y del flujo de descargas
  (#112–#128) para la acción de descarga.
- ADR-0002 queda enmendado: la búsqueda externa deja de estar excluida.
- La enmienda de 2026-11-05 adopta el contrato de eventos, shuffle estable,
  preflight, prefetch y error tipado de ADR-0014; no cambia la decisión de
  descartar `syncify.playerState.v1`.

## Alternativas descartadas

- **Cola en la UI con SQLite como simple almacén.** No resuelve gapless/crossfade
  ni el avance con la webview cerrada.
- **Importar el Provider item al encolar.** Contamina la biblioteca con
  previsualizaciones.
- **Solo tracks de biblioteca.** El descubrimiento no puede reproducir nada nuevo.
- **Mantener localStorage.** No escala ni es accesible desde Rust.
- **Enmienda menor del ADR-0002 sin ADR nuevo.** El cambio de dueño de la cola y
  la promoción de identidad son decisiones estructurales, no un ajuste de alcance.


# Player Fase 1 — MediaSession y spike de MPRIS (Linux)

> **Idioma:** español. Los títulos de sección están en inglés y el cuerpo
> sigue en español; no hay traducción de este documento todavía. El índice
> que marca su estado está en `docs/README.en.md`.

**Estado de revisión:** 2026-10-25 — re-revisión contra el árbol (verificación dirigida de símbolos y comportamiento): existe implementación MPRIS nativa en Rust (`src-tauri/src/services/mpris.rs`, `MprisService::start` en el setup de `src-tauri/src/main.rs`, bus `org.mpris.MediaPlayer2.syncify` en `/org/mpris/MediaPlayer2`); el spike WebKitGTK→D-Bus pasa a registro histórico. Añadida la sección del motor nativo (Fase 2: `PlayerState`, `player_load_track`, ticker, `services/stream_cache.rs`). Sin verificar en sesión gráfica real: la comprobación D-Bus end-to-end del panel MPRIS (prueba manual abajo) y la reproducción de audio real (README §"Verification and limitations").

> **Actualización 2026-10-19 — la ruta de audio de este documento está retirada.**
> El cuerpo describe la fase 1 original (elemento HTML Audio + `resolve_playback_source` +
> protocolo `syncify-media://`). El reproductor actual carga el audio en un motor nativo
> (`commands::player_load` / `commands::player_load_track`, en `src-tauri/src/commands/player.rs`)
> y sí soporta streaming de proveedores. `resolve_playback_source` y el protocolo
> `syncify-media://` **se retiraron** (C-19): no quedaba invocador que poblara sus
> concesiones, así que se borraron con sus tests. La fuente de verdad del estado de reproducción sigue siendo
> `ui/src/composables/usePlayer.ts`. El resto del documento se conserva como registro
> histórico de la fase 1.

> **Actualización 2026-11-17 — P01 (#132): el backend decide la siguiente pista y
> acredita la escucha.** Desde #131 (P00) el agregado de cola vive en SQLite
> (`player_queue_entries` + `player_state`, migración 0109) y desde P01 el backend
> también decide el avance: al fin natural de una pista (`player:ended`), el ticker
> lanza `services::playback_advance::spawn_auto_advance`, que con la decisión pura
> `decide_auto_advance` (pendientes primero; `repeat one` repite la actual;
> `repeat all` reconstruye el ciclo con `perform_rebuild_cycle`; shuffle es el orden
> persistido) transiciona el agregado con los `perform_*` de `commands/playback_queue.rs`
> y carga la pista en el motor, notificando por el evento `player:advanced`
> (`TauriEvents.PLAYER_ADVANCED`). La UI ya NO avanza al `ended` ni cronometra: el
> backend acumula los 30 s reales en `PlayerCore` (`services/playback_session.rs`,
> `listen_step`, umbral `LISTEN_THRESHOLD_SEC`) y acredita llamando a
> `perform_record_local_play` una vez por sesión — el comando IPC `record_local_play`
> **se retiró** (ya no hay invocador en la UI); la semántica del writer no cambia.
> Las secciones «Contrato de escucha», «Alcance de la fase 1» y la lista de decisiones
> describen el comportamiento PRE-P01 de la UI y quedan como registro histórico del
> reparto de responsabilidades anterior.

**Último cambio:** re-revisión 2026-10-25 — MPRIS nativo (`services/mpris.rs`) y motor nativo Fase 2 documentados; el registro de la fase 1 queda intacto. Antes: fase 1 del player — archivos: `ui/src/composables/usePlayer.ts`, `ui/src/components/StatusBar.vue` (mini reproductor, desde PL-08) y `ui/src/components/player/FullscreenPlayer.vue`, `ui/src/api/library.ts` (`recordLocalPlay`), vistas consumidoras (`AlbumDetailView.vue`, `ArtistDetailView.vue`, `PlaylistView.vue`, `LibraryView.vue`).
**Leer antes de modificar el reproductor actual:** `ui/src/composables/usePlayer.ts` (proyección de estado), `src-tauri/src/commands/player.rs` (`player_load_track`, motor nativo), `src-tauri/src/services/playback_session.rs` (sesión, acumulación de 30 s, `LISTEN_THRESHOLD_SEC`), `src-tauri/src/services/playback_advance.rs` (decisión y avance automático, #132), `src-tauri/src/commands/playback_queue.rs` (agregado de cola, migración 0109) y `src-tauri/migrations/0091_local_play_stats.sql` (schema de escuchas). La antigua resolución de fuentes que aparece más abajo es contexto histórico, no una ruta que deba reutilizarse.
**Archivos core:**
- `ui/src/composables/usePlayer.ts` (player, cola, modos, persistencia, MediaSession; desde #132 NO cronometra ni avanza al ended)
- `ui/src/components/StatusBar.vue` (mini reproductor y controles globales; PL-08 borró el `NowPlayingBar.vue` duplicado) + `ui/src/components/player/FullscreenPlayer.vue` (panel expandido/cola)
- `src-tauri/migrations/0091_local_play_stats.sql` (schema `play_count`/`last_played`/`playback_listens`)
- `src-tauri/src/commands/playback.rs` (`perform_record_local_play`: INSERT-primero, idempotente por `listen_session_id`; llamado por el ticker del backend desde #132 — ya no es comando IPC)
- `src-tauri/src/services/playback_advance.rs` (#132: `decide_auto_advance`, `run_auto_advance`, `spawn_auto_advance`, evento `player:advanced`)
- `src-tauri/src/services/mpris.rs` (MPRIS nativo: `MprisService::start`, interface `org.mpris.MediaPlayer2` + `.Player`)
- `src-tauri/src/services/stream_cache.rs` (sesión de streaming progresivo de la Fase 2)

---

## Alcance de la fase 1

Player HTML Audio para archivos locales ya descargados, vía `resolve_playback_source` (concesión en memoria + protocolo `syncify-media`). **Sin streaming de proveedores y sin motor nativo.** Incluye:

- Volumen acotado `[0,1]` y mute independiente, persistentes.
- `next()`/`previous()` (previous reinicia a 0 si la posición supera 3 s; si no, pop del historial).
- `shuffle` y `repeat` (`off | all | one`) persistentes. Desactivar shuffle **restaura el orden base** de los pendientes (el orden pre-barajado se persiste junto a la permutación); no se conserva el barajado.
- Cola visible con entradas de identidad propia (`entryId`, duplicados permitidos): reordenación por drag & drop y por controles de teclado (subir/bajar), añadir (menú contextual «Add to Play Queue» en Library) y vaciar. `clearQueue`/`removeEntry` solo tocan pendientes, nunca la pista en curso.
- Restauración de track/cola/posición tras reinicio: **siempre** vuelve a resolver con `resolve_playback_source` (nunca se persisten ni reusan URLs de archivo, `file_path` ni el DTO completo; la clave `syncify.playerState.v1` solo guarda `entryId`/`trackId`, posición, modos, volumen y pendientes de escucha). La restauración valida solo la pista actual; cada pendiente se valida justo antes de sonar y las filas con IDs desaparecidos se descartan al vuelo. Queda **pausado**: leer localStorage no reproduce nada.
- Acreditación de escucha: umbral de **30 segundos de reproducción real** (ver abajo), con `play_count`/`last_played` en la migración 0091 y el comando `record_local_play` con writer idempotente por token de sesión.

## Contrato de escucha (30 s reales)

- En cada `timeupdate` elegible (`!audio.paused && readyState >= 3`) se acumula `min(Δwallclock, ΔcurrentTime / playbackRate)`, con ambos deltas finitos y ≥ 0; un tick inválido no acredita pero **tampoco borra** el acumulado, y la línea base se re-ancla en cada tick elegible. Así pausas, buffering (`waiting`/`stalled`), seeks repetidos (p. ej. las líneas de Lyrics) y cambios de velocidad no fabrican ni pierden tiempo; **nunca** se usa `currentTime >= 30` como criterio.
- Cada reproducción lógica crea un `listen_session_id` UUID v4 canónico nuevo; pausa, seek y cambio de velocidad lo conservan; cambio de pista, `stop()` y cada vuelta de repeat-one crean uno nuevo. La posición restaurada tras reinicio **no acredita** tiempo previo.
- Al cruzar 30,0 s efectivos se envía `record_local_play(trackId, listenSessionId)` **una sola vez** por sesión. Reintentos: errores transitorios (DB ocupada, IPC caído) reintentan el mismo token con backoff y tope de 5 intentos; errores definitivos (pista borrada/UUID inválido, clasificados por mensaje) descartan el pendiente sin reintentar; un pendiente corrupto en localStorage (no-UUID) se descarta al cargar.

## MediaSession: implementación real

- Feature detection por símbolo: `navigator.mediaSession`, el constructor `MediaMetadata`, y **cada método por separado** (`setActionHandler`, `setPlaybackState`, `setPositionState`). Sin MediaSession el player funciona igual (degradación limpia, probado en Vitest con jsdom, que no lo implementa).
- Metadatos al confirmar el inicio; `playbackState` desde `play`/`pause`/`ended`/error; `setPositionState` solo con valores finitos; cleanup en `stop()`; los handlers (`play`, `pause`, `previoustrack`, `nexttrack`, `seekto`) delegan en las mismas acciones del player.

## Motor nativo (Fase 2): el reproductor vigente

El motor HTML Audio descrito arriba ya no es el camino activo: el audio se carga en
un motor nativo Rust (rodio + symphonia) gestionado desde el setup de
`src-tauri/src/main.rs`:

- `PlayerState` (`src-tauri/src/commands/player.rs`): estado del motor, gestionado en el arranque.
- `player_load` / `player_load_track` (mismo fichero): carga de pista, con streaming
  progresivo de proveedores vía `src-tauri/src/services/stream_cache.rs`
  (`StreamSession` / `StreamCacheState`, caché temporal Qobuz/Tidal).
- `spawn_player_ticker` (mismo fichero, llamado una vez en `setup`): tick periódico de
  progreso y detección de fin de pista.
- `player_mpris_update`: alimenta los metadatos y modos del panel MPRIS (ver sección siguiente).

La fuente de verdad del estado de reproducción en la UI sigue siendo
`ui/src/composables/usePlayer.ts`. Sin verificación de escritorio real: la
reproducción de audio en sesión gráfica y la prueba D-Bus del panel MPRIS
(README §"Verification and limitations").

## Spike MPRIS (Linux): qué se comprobó y qué no

**Actualización 2026-10-25:** la hipótesis WebKitGTK→D-Bus quedó superada: hay
implementación MPRIS nativa en Rust en `src-tauri/src/services/mpris.rs`
(`MprisService::start`, arrancada en el setup de `src-tauri/src/main.rs`), que expone
el bus `org.mpris.MediaPlayer2.syncify` en `/org/mpris/MediaPlayer2` con el interface
`org.mpris.MediaPlayer2.Player`, sin depender del reflejo de MediaSession de
WebKitGTK. Lo que **sigue sin verificar** es la comprobación D-Bus end-to-end en
sesión gráfica real: la prueba manual de abajo sigue siendo su definición, y el nombre
de bus esperado es `org.mpris.MediaPlayer2.syncify`. Lo que sigue es el registro
histórico del spike.

Honestidad del inventario (encargo previo de esta fase), conservada literalmente: se comprobaron **símbolos de MediaSession en una ventana WebKitGTK de prueba** (`pkg-config --modversion webkit2gtk-4.1` → `2.52.6`), **no** MPRIS de Syncify. **No hay sesión GUI ni D-Bus ejecutados de Syncify.** Re-verificación de este encargo (2026-10-06): `pkg-config --modversion webkit2gtk-4.1` → `2.52.6`; `busctl --user list` no mostró ningún propietario `org.mpris.MediaPlayer2.*`.

**No se afirma integración MPRIS verificada.** La cadena esperada es: la app (WebKitGTK) expone `navigator.mediaSession` y WebKitGTK la refleja en D-Bus como `org.mpris.MediaPlayer2.*`; hasta ejecutar la prueba manual es una hipótesis de la plataforma.

### Prueba manual pendiente (definición)

1. Lanzar Syncify en sesión gráfica y reproducir una pista descargada.
2. `busctl --user list | grep org.mpris.MediaPlayer2` — debe aparecer un nombre `org.mpris.MediaPlayer2.<algo>`.
3. Consultar `Identity`, `PlaybackStatus`, `Metadata` y `Position` del interface `org.mpris.MediaPlayer2.Player` (`busctl --user get-property <nombre> /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player <propiedad>`).
4. Enviar `PlayPause` (`busctl --user call <nombre> /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player PlayPause`) y verificar pausa/reanudación en la app; repetir con `Seek`.

## Decisiones de producto registradas (fase 1)

- Pistas de duración < 30 s **no cuentan** reproducción jamás: decisión aceptada, no descuido (alternativa rechazada en fase 1).
- `playback_listens` crece una fila por escucha **sin purga** en fase 1; la migración 0091 lo declara y deja la purga por antigüedad como trabajo futuro.
- El backup portátil **no exporta** `play_count`/`last_played` (DTO de `src-tauri/src/commands/backup.rs` sin estos campos): decisión documentada, no olvido.
- Desactivar shuffle **restaura el orden base** de los pendientes (no conserva el barajado).
- Repeat describe solo qué pasa al agotarse la cola: la cola **avanza siempre** al `ended` mientras haya pendientes, en cualquier modo (`off` detiene al agotarse; `all` reconstruye el ciclo; `one` reinicia la pista actual con token nuevo y `currentTime = 0`).
- `stop()` desmonta todo (incluida la cola) y persiste ese estado vacío: la restauración tras reinicio aplica al pausar/cerrar, no tras un stop explícito.

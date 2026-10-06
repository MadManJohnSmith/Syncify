# Player Fase 1 — MediaSession y spike de MPRIS (Linux)

> **Idioma:** español. Los títulos de sección están en inglés y el cuerpo
> sigue en español; no hay traducción de este documento todavía. El índice
> que marca su estado está en `docs/README.en.md`.

**Estado de revisión:** 2026-10-06 — fase 1 implementada en UI (cola con identidad, modos persistentes, acreditación de 30 s, restauración tras reinicio). MediaSession integrado con feature detection y degradación limpia. **MPRIS NO verificado en GUI**: no se ejecutó sesión gráfica ni D-Bus de Syncify; la prueba manual queda definida abajo y no debe darse por hecha hasta ejecutarse.

**Último cambio:** fase 1 del player — archivos: `ui/src/composables/usePlayer.ts`, `ui/src/components/NowPlayingBar.vue`, `ui/src/api/library.ts` (`recordLocalPlay`), vistas consumidoras (`AlbumDetailView.vue`, `ArtistDetailView.vue`, `PlaylistView.vue`, `LibraryView.vue`).
**Leer antes de modificar:** `ui/src/composables/usePlayer.ts` (única fuente de verdad del estado de reproducción), `src-tauri/src/commands/playback.rs` (`resolve_playback_source`, `record_local_play` — lado Rust, propiedad de su implementador), `src-tauri/migrations/0091_local_play_stats.sql`.
**Archivos core:**
- `ui/src/composables/usePlayer.ts` (player, cola, modos, acreditación 30 s, persistencia, MediaSession)
- `ui/src/components/NowPlayingBar.vue` (controles globales y panel de cola)
- `src-tauri/migrations/0091_local_play_stats.sql` (schema `play_count`/`last_played`/`playback_listens`)
- `src-tauri/src/commands/playback.rs` (`record_local_play`: INSERT-primero, idempotente por `listen_session_id`)

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

## Spike MPRIS (Linux): qué se comprobó y qué no

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

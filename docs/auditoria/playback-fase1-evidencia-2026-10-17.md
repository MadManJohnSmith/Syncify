# Fase 1 — Motor de reproducción nativo + MPRIS: evidencia

**Fecha:** 2026-10-17 · **Host:** CachyOS (Arch), sesión Wayland (pruebas por X11/XWayland)
**Versiones:** tauri 2.12.1 · rodio 0.23 · symphonia 0.5.5 · zbus 5.12 · Rust estable del sistema
**Rama:** `fix/windows-release-ffmpeg-pin` · **Antecedente:** `playback-fase0-evidencia-2026-10-17.md`

## 1. Pregunta a resolver

La Fase 0 demostró que la ruta `<audio>` + `syncify-media://` no reproduce en WebKitGTK
(`FormatError`, código 4) y decidió sustituirla por un motor nativo. La Fase 1 debía responder:
¿el motor nativo reproduce de verdad **todos** los formatos locales con transporte correcto, y la
app puede controlarlo (y ser controlada por el escritorio vía MPRIS) sin depender de WebKit?

## 2. Método

1. **Crate `crates/syncify-player`** (sin Tauri): rodio 0.23 + symphonia 0.5.5, decodificación
   completamente en Rust. Verificación dirigida con `examples/verify_transport.rs`:
   carga pausada, resume, pause-hold, recarga del mismo fichero, relevo a otro fichero, stop.
2. **Captura de audio real**: `parec` del monitor del sink por defecto (`--latency-msec=50
   --process-time-msec=10`), mantenido despierto con una fuente nula de `ffmpeg`; análisis de
   energía por bandas (440 Hz para el fixture A) con ajuste de offset captura↔marca.
3. **Integración Tauri**: comandos `player_load/pause/resume/stop/seek/set_volume/set_rate` +
   `player_mpris_update`; ticker de 250 ms con eventos `player:tick`/`player:ended` marcados con
   `seq` para descartar eventos de cargas sustituidas.
4. **MPRIS nativo**: `src-tauri/src/services/mpris.rs` (zbus, sólo Linux) sirve
   `org.mpris.MediaPlayer2.syncify`; el estado/posición lo alimenta el ticker, los metadatos la UI;
   las acciones vuelven al frontend como `player:mpris-action`.
5. **E2E de MPRIS con la app real aislada**: `XDG_DATA_HOME` propio + `dbus-run-session` (bus
   privado, sin tocar la sesión del usuario), BD sembrada con dos pistas sintéticas (tonos de 440 y
   880 Hz, 20 s) y `localStorage` sembrado con la sesión restaurada. Acciones ejecutadas con
   `busctl` y señales `PropertiesChanged` capturadas con `dbus-monitor`.

## 3. Resultado

### 3.1 Motor y transporte

Todas las comprobaciones de `verify_transport` **PASS**: `load(autoplay=false)` deja el motor
`Paused` en 0 sin audio; `resume()` avanza; la pausa mantiene la posición (deriva 0); recargar el
mismo fichero reinicia desde 0; el relevo a otro fichero decodifica sin reiniciar el proceso; `stop`
detiene. La captura confirma el tono de 440 Hz **sólo** en las ventanas de reproducción y silencio
digital en pausa/stop.

**Bug encontrado y corregido (rodio 0.23):** tras `stop()` + `append()` el `Player` conserva una
posición latcheada (hasta ~5 ms) y la primera lectura de la carga nueva no es 0. El motor ejecuta
`try_seek(Duration::ZERO)` después de `append`; verificado con el ejemplo dirigido (la carga
pausada reporta posición 0 y la reproducción arranca desde 0).

### 3.2 Integración con la app

- `cargo check -p syncify-tauri` y `cargo clippy -p syncify-tauri` sin warnings; `cargo fmt` limpio.
- La restauración de sesión carga la pista en pausa en la posición persistida y no acredita escucha
  hasta que el usuario reanuda; `stop()` limpia `isPlaying` y los metadatos MPRIS.
- Limitación conocida: `setRate` usa `set_speed` de rodio, que **cambia el tono** (no es
  `audio.playbackRate`); queda documentado, no se aborda en esta fase.

### 3.3 MPRIS end-to-end (instancia aislada)

| Comprobación | Resultado |
|---|---|
| Registro `org.mpris.MediaPlayer2.syncify` | OK · `Identity = "Syncify"` |
| Restauración pausada | `PlaybackStatus=Paused`, `Position=2.000 s`, metadatos de 9001 completos (`trackid`, título, artista, álbum, `artUrl`, `length=20 s`) |
| `Play` por D-Bus | `Playing`; `PropertiesChanged` emitido |
| `Pause` por D-Bus | `Paused`; posición retenida (4.215 s) |
| `Seek(+2 s)` | 4.215 → **6.215 s** (exacto) |
| `SetPosition(10 s)` | **10.758 s** tras 0.76 s de reproducción |
| `SetPosition` con `trackid` ajeno | **ignorado** (la posición siguió avanzando: 11.741 s) |
| `Next` | `Playing`, metadatos de **9002** («Prüba Dos»), posición 1.645 s |
| `Stop` | `Stopped`, metadatos vacíos (`trackid /org/syncify/track/0`) |
| Audio tras `Next` | energía estrecha de **880 Hz sólo** en la ventana de la pista B |

La captura del E2E compartió el sink con audio ajeno del escritorio (Brave), que enmascaró el tono
de 440 Hz de la pista A; el tono de 880 Hz de la pista B (banda limpia) demuestra que `Next` produjo
audio real por el sink. La verificación de audio del transporte (440 Hz vs silencio) se hizo en el
arnés aislado (§3.1).

### 3.4 Bug de contrato encontrado en la app (corregido)

`TrackMetadata` en Rust serializa con `#[serde(rename_all = "camelCase")]` (`trackId`,
`artistName`, `filePath`…), pero `normalizeTrackMetadata` en `ui/src/api/library.ts` leía sólo
`snake_case`. Consecuencia: `projectMetadata().id = 0` en la restauración y en cada
`getTrackMetadata`, lo que propagaba un id 0 a la persistencia (`currentTrackId`), a MPRIS
(`mpris:trackid /0`) y a los listens, y dejaba `filePath` indefinido en el reproductor a pantalla
completa.

Corregido con el patrón `pick` (acepta ambas grafías) + dos tests de regresión en
`ui/src/__tests__/api/library.spec.ts`. Evidencia en vivo sobre la instancia `tauri dev` real: antes
`/org/syncify/track/0`; después `/org/syncify/track/4711` con metadatos completos.

### 3.5 Suite

- `vue-tsc --noEmit`: 0 errores.
- `vitest run`: **125 ficheros / 1064 tests verdes**.
- Rust: `clippy` sin warnings, `fmt --check` limpio.

## 4. Veredicto

La reproducción local pasa a un motor **100 % Rust** que no depende de WebKit, GStreamer ni
descargas en runtime, y responde a MPRIS como un reproductor nativo del escritorio: estado,
metadatos, posición, seek absoluto/relativo, siguiente pista y parada, con el audio verificado en el
sink real. La ruta antigua `<audio>`/`syncify-media://` queda registrada pero **sin uso**; su
limpieza (comando `resolve_playback_source` y protocolo) se hará en la Fase 4.

## 5. Receta de reproducción

1. Motor: `cargo run -p syncify-player --example verify_transport -- <fixtureA> <fixtureB>` y
   captura con `parec` (ver `verify-transport.sh` / `analyze_transport.py` del arnés).
2. E2E MPRIS: sembrar `localStorage` y la BD aislada (`/tmp/opencode/mpris-e2e/seed_localstorage.py`),
   lanzar la app bajo `dbus-run-session` y ejecutar `run.sh` / `run_positions.sh`; analizar la
   captura con `analyze_audio.py`.
3. Comprobación manual rápida: `busctl --user get-property org.mpris.MediaPlayer2.syncify
   /org/mpris/MediaPlayer2 org.mpris.MediaPlayer2.Player PlaybackStatus` y `Metadata`.

## 6. Límites

- **Windows no verificado** en esta fase (se hará sobre el motor nativo en Fase 4); MPRIS es
  exclusivo de Linux y el servicio es un no-op en otras plataformas.
- El streaming de proveedores (Fase 2) y la cascada ISRC de Spotify (Fase 3) no forman parte de esta
  evidencia.
- El E2E usó pistas sintéticas; no se reprodujo la biblioteca personal del usuario.
- Los scripts del arnés viven en `/tmp/opencode` (efímero): si se quieren conservar como
  reproducibles del repo, hay que moverlos a `docs/auditoria/` o `tools/`.

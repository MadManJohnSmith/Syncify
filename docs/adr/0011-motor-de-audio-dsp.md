# ADR-0011 — Motor de audio: gapless, crossfade, ReplayGain, AGC y EQ paramétrico

**Estado:** aceptado · **Fecha:** 2026-11-05 · **Enmienda:** 2026-11-05 (ADR-0014)
**Origen:** ronda de grilling del flujo REPRODUCTOR.
**Spec:** #130 · **Tickets:** #131–#149.

## Contexto

`crates/syncify-player/src/lib.rs` envuelve rodio y expone transporte de una pista:
cargar, pausar, reanudar, buscar, volumen y ritmo. No hay DSP ni conocimiento de la
pista siguiente.

ReplayGain se calcula al descargar y se guarda en columnas de `tracks`
(`src-tauri/migrations/0083_loudness_replaygain_columns.sql`,
`src-tauri/src/download/audio_inspector.rs`), pero la reproducción nunca lo lee.
El ajuste `replay_gain_mode` de la UI es de etiquetado, no de reproducción.

## Decisión

1. **El DSP vive en el motor nativo.** Nada de Web Audio ni de procesar en la UI:
   gapless, crossfade, ganancia, AGC y EQ se implementan en `syncify-player`.

2. **Gapless siempre activo.** El motor precarga la pista siguiente de la cola del
   backend y la encadena sin silencio. Si el prefetch de un stream falla, hay hueco
   y se registra; nunca se finge continuidad.

3. **Crossfade opcional.** 0–12 s, desactivado por defecto; solo entre pistas de la
   cola, no en seeks, pausas ni paradas manuales.

4. **Modo de ganancia de reproducción separado del de etiquetado.** Tres modos:
   `off`, `track` y `album`. La ganancia sale de las columnas de `tracks`
   (`replaygain_track_gain` / `replaygain_album_gain`) y se aplica en el motor con
   un target LUFS propio de reproducción; un limiter de pico evita clipping.

5. **AGC es un modo alternativo, no un complemento.** Nivelación dinámica con
   target LUFS propio y limiter; excluyente con ReplayGain. Por
   defecto está apagado. No se combinan dos normalizaciones.

6. **EQ paramétrico configurable.** De 1 a 10 bandas, cada una con frecuencia, Q y
   ganancia; plano por defecto. Se aplica a todo lo que suena, local o stream.

7. **Los ajustes de reproducción se persisten en SQLite.** No en localStorage;
   la UI los edita y el motor los lee al cargar y al cambiar.

## Enmienda 2026-11-05 — adopción de ADR-0014

1. **EQ con tipo de banda.** Cada banda lleva tipo (`peak`, `low_shelf`,
   `high_shelf`, `low_pass`, `high_pass`), frecuencia, Q y ganancia; plano por
   defecto; 1–10 bandas. El modelo y la persistencia admiten el tipo desde el
   principio, aunque la primera UI solo exponga peak.

2. **Indicador de clipping.** El motor publica el pico post-proceso y el estado del
   limiter; la UI puede avisar sin adivinar.

3. **Orden de cadena fijo.** EQ → ganancia (ReplayGain/AGC) → limiter → salida. Se
   documenta y no se reordena desde la UI; el limiter atrapa los boosts del EQ y de
   la ganancia.

4. **Ganancia de álbum real.** Se calcula con suma de energía cuando el álbum está
   completo; si no, `NULL` y el modo álbum cae a `track` con aviso. Se elimina el
   valor fabricado `track + 0.70 dB`.

5. **Calidad de salida.** El motor reporta sample rate, bit depth, canales y si
   hubo resample; `player:stream-state` publica la calidad de fuente y la de salida.
   No se presenta la de fuente como si fuera la del dispositivo.

6. **Gapless por backend.** Se prueba en ALSA/PipeWire/WASAPI/CoreAudio; si el
   prefetch falla, se registra el hueco, no se finge continuidad. Crossfade sigue
   siendo diseño propio; Audirvāna no lo implementa.

7. **Fuera de alcance:** crossfeed, balance, convolver, VST3, MQA y DSD.

## Consecuencias

- Los tests del motor usan audio sintético; la reproducción real y MPRIS siguen
  siendo verificación manual (B-02).
- El prefetch de streams convive con URLs de proveedor que caducan; el fallo debe
  ser explícito.
- AGC, limiter y EQ añaden CPU; hay que medir en la carga de 30k.
- Pistas sin ReplayGain en BD no reciben ganancia; no se inventa un valor. Tampoco
  se inventa la ganancia de álbum: si el álbum no está completo, queda `NULL` y el
  modo álbum cae a `track`.
- La enmienda de 2026-11-05 adopta tipos de banda, clipping, orden fijo, ganancia
  de álbum real y calidad de salida (ADR-0014).
- La migración de ajustes de reproducción es nueva; no se edita ninguna aplicada.

## Alternativas descartadas

- **Web Audio GainNode.** La ruta de audio es nativa; la webview no toca el audio.
- **Reusar el `replay_gain_mode` de descarga.** Etiquetar y reproducir son
  decisiones distintas.
- **ReplayGain + AGC combinados.** Impredecible y difícil de explicar.
- **EQ gráfico de 10 bandas.** Se prefirió paramétrico configurable.

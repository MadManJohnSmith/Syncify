# ADR-0024 — Sesión de reproducción: una puerta para resolver, cachear y gobernar

**Estado:** aceptado · **Fecha:** 2026-11-06
**Origen:** parte P6 del informe de arquitectura, cerrada en grilling con inventario verificado.
**Spec:** PR de P6 (rama `p6/playback-session`). **Tickets:** ninguno (coexiste con #130, P00–P18).

## Contexto

`player_load_track` (`src-tauri/src/commands/player.rs:302-433`) es el verdadero
orquestador del audio: SQL directo a `downloads` para decidir local, resolución de
streaming con verificación de cabecera dentro del resolver, caché de streams con
prime/frontera/generación, spawns de descarga progresiva y DASH, y espera del prime
(≤45 s). La posición y el fin de pista se gobiernan desde un ticker de 250 ms que
además alimenta MPRIS. Hay dos generaciones superpuestas (`core.seq` en backend,
`transitionGen` en la UI). El motor (`crates/syncify-player`) no tiene ni un test;
la orquestación del comando tampoco. Los parciales de caché no son reanudables ni se
invalidan al descargar la pista, y un fallo de red a mitad mata la reproducción sin
re-resolución (ADR-0010.enm.4 ya exige una).

## Decisión

- **Una puerta, una pista.** `PlaybackSession` (Rust) resuelve local → caché completa
  → streaming (progresivo/DASH), prepara el audio y gobierna transporte, frontera,
  tick y fin de pista. Estrictamente una-pista: sin hooks de cola ni prefetch; el
  `player:ended` y `player:tick` salen igual que hoy. La cola, el avance automático y
  el crédito de escucha no viven en la sesión (los mueve #130/P00–P01).
- **Generación única.** La sesión es la única dueña de la generación; la UI abandona
  `transitionGen` y descarta por la generación de la carga. Los eventos de una
  generación superada se descartan igual que hoy.
- **Semántica de corte conservada por fuente** (local corta antes de resolver;
  streaming mantiene la anterior mientras resuelve) y anotada como deuda: el
  crossfade (#130/P06) rediseñará las transiciones.
- **Errores con reintento acotado.** Un seek tras la frontera espera acotado y
  re-resuelve una sola vez si la descarga se estanca; un fallo 401/403/410 a mitad de
  descarga re-resuelve una sola vez (ADR-0010.enm.4). Los errores son tipados dentro
  de la sesión; el payload de `player:stream-state` no cambia (`errorKind` llega con
  la P-series).
- **Parciales reanudables.** El streaming progresivo reanuda por Range desde el byte
  escrito; DASH trunca al segmento completo previo y continúa; el parcial inválido se
  descarta. La caché de una pista se invalida cuando ya está descargada en
  `downloads`. La reanudación de parciales en la cola de descargas queda fuera.
- **Sin cambios de contrato IPC.** Comandos, eventos y payloads quedan como están;
  `player_load` (por ruta, sin invocador en producción) se elimina con comando,
  wrapper y mocks en el mismo cambio (puerta `dead_commands_hygiene`).
- **Caracterización delante.** El motor se caracteriza con audio sintético por el
  crate real (promesa de ADR-0011 aún sin cumplir) y la orquestación con fakes, antes
  de mover nada.

## Consecuencias

- El comando `player_load_track` queda delgado: delega en la sesión; el ticker pasa a
  consumir `session.tick()` y sigue alimentando MPRIS y los eventos.
- Añadir una fuente o cambiar la política de prime/frontera es tocar un módulo con
  tests, no el comando.
- Puerta de aceptación reforzada: suite completa y gates del repo, más capturas
  objetivas (checksum del FLAC de caché vs descargado, duración vs STREAMINFO) y
  escucha en vivo guiada con `tools/prueba-limpia.sh`; el README sigue declarando que
  el audio real no está verificado en CI.
- Deuda declarada: semántica de corte divergente, `errorKind` en `player:stream-state`,
  prefetch/gapless, restore sin red (todo de #130) y reanudación de parciales en la
  cola de descargas.

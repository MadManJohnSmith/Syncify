# ADR-0017 — Letras: motor v2 (crate, selección con mejora, caché negativa y sondas)

**Estado:** propuesto · **Fecha:** 2026-11-05
**Origen:** ronda de grilling del flujo LETRAS.
**Spec:** #176 · **Tickets:** #178–#194.

## Contexto

El motor vive en `download/lyrics.rs` (3.337 líneas) con los 12 fetchers inline. La
resolución para en la primera **Fuente de letras** aceptable por prioridad fija; un
`not_found` se vuelve a sondear en cada intento (solo caché en memoria, sin TTL ni
invalidación). Las credenciales viven en variables de entorno (`SPOTIFY_SP_DC`),
`test_lyrics_provider` rechaza las 12 fuentes Rust y solo prueba el puente Python
legacy (`lyrics_bridge.py`, `lyrics_service.py`), que convive con el motor sin usarlo
el motor. Las colas batch no son cancelables: cerrar el diálogo no detiene el bucle.

## Decisión

1. **Crate `syncify-lyrics-engine`.** Un fichero por fuente, política y selección en
   el crate; `download/lyrics.rs` queda como fachada fina. Las **Sondas** Rust viven
   ahí y las reutilizan el pipeline de descargas y la resolución on-demand.

2. **Selección secuencial con mejora.** Para en la primera fuente aceptable (como
   hoy), pero si el resultado queda por debajo del objetivo (`min_sync_level`),
   sigue sondeando las siguientes para mejorar; para al alcanzar el máximo disponible
   o agotar la lista.

3. **Caché negativa persistente.** Registro del fallo de resolución por (pista,
   fuente) con caducidad; los barridos se la saltan hasta que expire. El resultado
   positivo no se cachea: se persiste como letra/sidecar.

4. **Nativo-si-existe, compitiendo por calidad.** Si el proveedor de streaming tiene
   endpoint propio de letras (caso Tidal), se sondea en su pipeline y compite con el
   motor genérico: gana el candidato de mayor **Nivel de sincronía**, luego idioma
   preferido, luego prioridad de fuente. Deezer/SoundCloud siguen sin letras. El
   endpoint de Tidal se verifica con un spike con criterio de éxito antes de
   implementar.

5. **Credenciales en ajustes de la app.** UI por fuente (sp_dc de Spotify; usertoken
   Musixmatch opcional), guardadas con el patrón local de la app; `SPOTIFY_SP_DC`
   sigue valiendo como fallback. La sonda distingue falta de credencial de fuente
   caída.

6. **Retirada del puente Python de letras.** `lyrics_bridge.py`, `lyrics_service.py`
   y sus tests se eliminan; `test_lyrics_provider` se reescribe como sonda Rust.

7. **Barridos sobre cola cancelable.** Auto-fetch al importar y barrido de reintentos
   corren en la cola batch con progreso y cancelación real (el evento
   `lyrics-fetch-progress` se reutiliza).

## Consecuencias

- Migración nueva para el registro de caché negativa; no se edita una aplicada.
- Las rutas batch actuales se reescriben sobre la cola cancelable.
- La evidencia de red de `docs/LYRICS_PROVIDER_MATRIX.md` se re-verifica con un
  spike previo (E-01) y queda mantenida por las sondas en campo.
- Los tests del puente legacy se retiran junto con él.

## Alternativas descartadas

- **Fan-out best-of-N.** Más peticiones contra rate-limits de LRCLIB/Musixmatch y
  fuentes de scraping; latencia de la ventana entera.
- **Secuencial puro.** Re-sondea fallos conocidos y no mejora resultados bajos.
- **Mantener el puente Python.** Dos motores vivos y el botón de test muerto para
  todo lo configurable.
- **Credenciales solo por entorno o en keyring.** El usuario de la app no puede
  configurarlas, o se añade superficie por plataforma desalineada con los demás
  secretos locales.

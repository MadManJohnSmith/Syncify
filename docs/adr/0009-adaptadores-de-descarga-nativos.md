# ADR-0009 — Descargas nativas por servicio: alcance de destinos en beta

**Estado:** propuesto · **Fecha:** 2026-11-05
**Origen:** ronda de grilling del flujo de descargas. Amplía el alcance del ADR-0005:
«todos los servicios» significa todos los que declaran descarga, con adaptador nativo
en Rust.
**Spec:** #111 · **Tickets:** #112–#128.

## Contexto

El seed de servicios declara descarga para `qobuz`, `tidal`, `deezer` y `soundcloud`;
`spotify` y `apple_music` quedan en `supports_download = 0`
(`src-tauri/migrations/0002_normalize_schema.sql:261-266`). La migración 0064 fija
SoundCloud en `lossy` (`0064_pipeline_hardening_and_integrity.sql:193-194`).

En el árbol solo Qobuz, Tidal y Amazon tienen adaptador Rust
(`src-tauri/src/download/qobuz.rs`, `tidal.rs`, `amazon.rs`). Amazon no aparece en el
seed y su adaptador usa la API de terceros DoubleDouble, que fija `bit_depth: 24` y
`sample_rate: 48000` sin validar la entrega (`amazon.rs:57-60`, `:244-257`). Deezer y
SoundCloud solo tienen puentes Python (`scripts/services/deezer_service.py`,
`soundcloud_service.py`) que el camino de descarga no invoca; `download_bridge.py` no
se usa desde Rust.

La beta necesita descargar cualquier pista de la biblioteca, no solo las que tienen
fuente Qobuz o Tidal. El propietario decidió ampliar los destinos y portar los puentes
Python a Rust, sin dejar dos implementaciones vivas.

El orquestador, sin embargo, trata Deezer y SoundCloud como *catalog-only*
(`orchestrator.rs:2824-2833`) mientras la cola filtra por `supports_download = 1`
(`queue.rs:1292-1300`): la capacidad declarada y la real noo coinciden. El puerto Python
tampoco es una base fiable: la ruta SoundCloud de `download_bridge` no llama
`authenticate()` y falla con `session=None`; Deezer no tiene tests de descarga. Ambos se
usan como especificación, no como comportamiento correcto que copiar.

## Decisión

- Los destinos de descarga de beta son **Qobuz, Tidal, Deezer, SoundCloud y Amazon**,
  todos con adaptador nativo en Rust.
- Deezer y SoundCloud se portan desde los servicios Python. Los caminos de descarga
  Python se retiran; se conservan auth, metadata y fingerprint.
- Amazon se mantiene como adaptador Rust, pero debe validar el payload y declarar la
  calidad realmente entregada. Si no puede, el ítem falla la verificación.
- Spotify y Apple Music **no** son destinos de descarga: aportan identidad, biblioteca
  y disponibilidad. Una pista solo de Spotify se enruta por SongLink a un destino con
  adaptador; si no hay, se marca «sin proveedor».
- Cada adaptador declara su matriz de formatos y calidades. La verificación física
  (formato, bit depth, sample rate, duración) manda sobre lo anunciado.
- La preferencia de destino del job ordena; no encierra. Si el destino preferido no
  puede, se pasa al siguiente según evidencia (ADR-0005).
- El piso de bitrate es por proveedor: SoundCloud 128, Deezer 320 (128 en cuentas free)
  y la matriz de cada destino. El piso global de 320 no se aplica en duro y la caída se
  declara; nunca se etiqueta lossless un MP3 de 1288.
- MP3 se etiqueta con un escritor ID3v2 + portada compartido; FLAC sigue con
  `syncify-flac-writer`. Sin escritor MP3 no se cumple la promesa de ficheros exportables.
- `services.supports_download` refleja el adaptador real: Deezer y SoundCloud pasan a 1
  cuando su adaptador aterriza; hasta entonces, la cola no los ofrece como descargables.

## Consecuencias

- La beta crece: cinco adaptadores que mantener, con auth y cuotas distintas.
- Deezer y SoundCloud requieren investigación de API y auth; el puente Python es la
  especificación de comportamiento, no una dependencia de ejecución.
- Amazon deja de ser un camino opaco: su calidad se valida o no se acepta.
- El resolver necesita la capacidad por servicio (formatos y calidad) además del
  derecho por cuenta.
- Cada adaptador debe cubbrir en tests:  payload inválido, downgrade de calidad, auth
  caducada y rate limit.
- Dependencias nuevas: `blowfish` y parser HLS (`m3u8` o equivalente), más un escritor
  ID3; el workspace hoy solo tiene `aes-gcm`, `md5` y escritor FLAC.
- Deezer es el camino crítico por `license_token`, `TRACK_TOKEN` y Blowfish-CBC;
  SoundCloud lo es por HLS y la normalización de OAuth.

## Preguntas abiertas

- Matriz exacta de formatos y calidades de Deezer y SoundCloud; la fija el puerto.
- ¿Amazon se queda en beta o detrás de un flag si la verificación no es fiable?
- Orden por defecto de destinos: prioridad configurada, con Qobuz y Tidal delante.

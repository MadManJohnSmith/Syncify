> Notas de la versión 0.6.2 (tag `v0.6.2`, 2026-11-05). Histórico; las
> limitaciones vigentes del proyecto se declaran en `README.md`
> §"Verification and limitations".

Syncify v0.6.2 — reproducción en streaming, identidad de edición y orquestador de sync

## Novedades

### ▶️ Reproducción sin descargar (Qobuz/Tidal)
El reproductor resuelve la fuente por `quality_score`, verifica la cabecera real
del stream (Content-Type, magia FLAC/MP3/M4A, Content-Length y STREAMINFO) y
**rechaza previews de 30 s** aunque la API devuelva el `format_id` pedido. La
cascada de cuentas por servicio prefiere entitlement conocido y cuentas activas;
un candidato lossy de duración completa queda en reserva mientras se prueban las
demás fuentes. El caché progresivo escribe a un fichero temporal, soporta seek
pendiente y emite `player:stream-state` para el spinner de buffering. La URL
firmada no sale del backend.

### 🎵 Tidal lossless real
Tidal entrega FLAC dentro de ISOBMFF; el fMP4 creciente no se puede reproducir ni
buscar de forma fiable. La nueva crate `syncify-fmp4-flac` convierte el init y
los segmentos DASH a **FLAC nativo que crece**, copiando frames sin recodificar
(verificado bit-perfect a 24/192). La etiqueta de calidad se construye desde el
`dfLa` real, no desde lo que declara el manifiesto. Las descargas de Tidal dejan
de invocar ffmpeg: el mismo conversor hace el remux en Rust puro.

### 🔎 Cascada ISRC y desambiguación
La cascada incorpora Spotify → Qobuz/Tidal lossless y, cuando hay varias
candidatas, desambigua por la **duración más cercana**. La cascada es honesta y
reproducible: registra la fuente y la calidad obtenidas, no las solicitadas.

### 🖼️ Reproductor grande
- Booklet: los sidecars se resuelven contra el disco (`booklet.pdf` o cualquier
  PDF del álbum, incluido el padre de `Disc N`) y se abren con el visor del
  sistema mediante allowlist.
- Metadatos completos: album artist, sello, UPC, fecha, pista/total, MBIDs,
  formato/bits/kHz/tamaño, fecha de descarga y tags crudos del contenedor.
- Carátula ~50 % mayor y sin superposiciones.
- Letras por línea: el parser consume los tags `<mm:ss.xx>` del LRC enriquecido.
- Carátulas animadas WebP y MP4, con fallback.
- Etiqueta de calidad fuera del arte y con la calidad real del stream verificado.
- En WebKitGTK los sidecars se sirven como `blob:` porque `asset://` no se pinta
  como subrecurso desde una página http(s); `asset://` queda como fallback.

## Descargas e importación

- **Matriz de fallback calidad×servicio**, sesión/HI_RES de Tidal y refresco de
  fuentes obsoletas de Qobuz.
- **Identidad verificada**: una edición solo se bloquea si su título, artista o
  duración contradicen la pista canónica; un id de catálogo que resuelve a otra
  grabación se rechaza en vez de entregar audio ajeno.
- **Reintento por cuenta ante preview**: un sample de 30 s o un lossy servido
  para un lossless ya no condena el ítem; se prueba la siguiente cuenta de la
  misma edición, con la primaria primero.
- **Resolución viva por ISRC**: si el resolver no encuentra fuente descargable y
  hay ISRC, se ejecuta una resolución verificada (Tidal → Qobuz), se persiste la
  fuente y se recalcula el plan. Sin coincidencia verificada no se inventa nada.
- **ISRC case-insensitive** y upsert dirigido en `track_sources` (RSQ-01..03).
- **Favoritos multi-edición**, paginación de Tidal deduplicada y una fila por
  release en la biblioteca.
- **Migración 0101**: nombre visible de cuenta elegido por la persona; el login y
  el refresco automático no lo pisan, pero un correo descubierto después sí se
  guarda como identidad.

## Identidad y ediciones

- El guardián **ED-04** ya no confunde formato, empaquetado ni un título sin
  texto útil con otra familia de álbum. El ledger cubre la ausencia de evidencia
  en vez de contradecirla.
- **ED-04b — apariciones**: la llegada de otro álbum se registra como aparición
  en vez de mover la grabación en silencio. La pista muestra las apariciones en
  el modal de metadatos y permite confirmarlas o descartarlas desde la pista.
- El diagnóstico clasifica los rechazos ED-04 para decidir con datos; el
  contador cuenta ítems y el motivo del artista queda explícito.
- Una edición que solo cambia de caja en el ISRC se resuelve con índice NOCASE;
  el relleno de ISRC no pisa el de otra pista y el alta reutiliza el existente.

## Sincronización y robustez

- **Orquestador de sync**: `perform_sync_service_with_emitter_inner` pasa de
  4 546 a 958 líneas (−79 %). Los cinco brazos (SoundCloud, Deezer, Spotify,
  TIDAL y Qobuz) viven en funciones propias, extraídas sobre tests de
  caracterización. Límite conocido: ningún test de integración recorre los brazos
  contra sus proveedores; la equivalencia se apoya en el compilador, los
  contratos offline y la suite completa.
- **Frontera de escritura SQLx**: `crate::db::write_or_log` centraliza la
  política de escritura que falla y no se propaga, con `tracing::warn!` y
  contexto. Migrados 130 de los 225 silenciosos reales; el ratchet baja de 243 a
  113 y solo puede bajar.
- **Keyring persistente en Windows**, con migración y read-back verificado.
- **Hardening cross-platform**: navegadores, pines, rutas de los puentes, health
  check, autostart y fuentes. El AppImage limpia `LD_LIBRARY_PATH` antes de abrir
  el navegador de auth; el build de Windows vuelve a tener FFmpeg disponible.
- La bandeja difiere el tooltip para no panickear dentro del menú.
- El cierre del sync se informa también con cero errores.
- Un artista vacío ya no bloquea guardar la pista.
- La búsqueda de Tidal usa el mercado de la cuenta, no `US` fijo.
- Se elimina el subsistema de reproducción retirado (`resolve_playback_source` y
  el protocolo `syncify-media://`) y el registro del comando huérfano; el
  selector de concurrencia queda conectado a su comando real.

## Interfaz

- La sección de ediciones explica qué hace y qué no cambia.
- Las apariciones de una pista se ven en el modal de metadatos y se confirman o
  descartan desde la pista.
- El tab de cuentas estrena un prototipo de variantes **solo en desarrollo**
  (`import.meta.env.DEV`): no entra en el bundle de producción.

## Documentación y agentes

- Las decisiones sueltas pasan a `docs/adr/0001..0009`, con punteros en las rutas
  antiguas para que las citas históricas resuelvan.
- `docs/agents/` define el tracker, las etiquetas de triage y las reglas de
  consumo de la documentación.
- `AGENTS.md` y `GLOSSARY.md` se reescriben sobre el vocabulario vigente;
  `docs/HANDOFF.md` deja el relevo vivo y `tools/prueba-limpia.sh` la prueba
  manual con logs aislados.
- La puerta `test_docs_consistency.py` valida ahora también los ADR (estado y
  rutas citadas).

## Lo que esta versión **no** afirma

- **No hay medición de tasa de éxito válido sobre cuentas reales.** Las
  verificaciones automatizadas de esta versión son offline y con datos
  sintéticos; el porcentaje real de descargas completadas sigue sin ejercicio.
- La puerta automatizada **no ejercita GUI nativa, audio real, bandeja, MPRIS ni
  la app instalada**. Las comprobaciones en vivo citadas en los commits son
  evidencia de desarrollo, no de esta puerta.
- La selección de edición `ED-03`/`ED-04` permanece sin activarse de forma
  general.
- El target Windows/MSVC se compila en CI, pero no se verificó manualmente en
  esta estación.

## Verificación del candidato

| Gate | Resultado |
|---|---|
| `cargo test --locked --no-fail-fast` | 2820 pasan, 0 fallan, 13 ignorados |
| `cargo check --workspace --all-targets --locked` | pasa |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | sin warnings |
| `cargo fmt --all -- --check` | pasa |
| `cargo test --test batch_50_audit_test -- --test-threads=1` | 2/2 |
| `npm run test:run` (UI) | 126 archivos / 1083 tests |
| `python3 -m unittest discover -s scripts/tests` | 311 pasan (2 saltados) |
| Coherencia documental | 23/23 |

Ningún gate usa cuentas, base de datos ni música personales.

---

Syncify v0.6.2 adds progressive Qobuz/Tidal streaming, real Tidal lossless
(FLAC-in-ISOBMFF converted to native FLAC without ffmpeg), ISRC cascade and
duration disambiguation, verified download identity, per-account preview retry,
ED-04b album appearances, a five-arm sync orchestrator extraction and a
centralised SQLx write boundary. It also carries the ADR/documentation
reorganisation and the agent workflow docs. There is still no measured
valid-success rate against real accounts: every automated check in this release
is offline and synthetic.

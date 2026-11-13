# Documentación del Proyecto Syncify

[English](README.en.md)

Esta carpeta centraliza las especificaciones funcionales y decisiones de diseño
de arquitectura del proyecto. El índice declara qué documento es la referencia
vigente de qué y qué ficheros son solo evidencia histórica, de modo que nadie
trate una instantánea fechada como estado actual.

**¿Vas a trabajar en el repo?** `AGENTS.md`, en la raíz, es el mapa de entrada
para agentes: superficies, comandos deterministas, las puertas que no se
relajan y las minas. Empieza por ahí. Este índice no duplica su contenido.

## Cómo está organizada

| Capa | Dónde | Qué es |
|---|---|---|
| Mapa de entrada | `AGENTS.md`, `GLOSSARY.md` (raíz) | Por dónde empezar y vocabulario de dominio |
| Specs duraderas | `features/`, `adr/`, `APP_FLOW.en.md`, `design-system.md` | Lo que el sistema es hoy |
| Configuración de agentes | `agents/` | Tracker, etiquetas de triage y reglas de consumo de esta documentación |
| Registros vivos | esta carpeta: Deuda, colas `COLA_*.md`, planes | Dónde está el trabajo y la deuda |
| Histórico | `historico/` | Instantáneas fechadas: evidencia, **no** estado |
| Rastro de auditoría | `auditoria/` | Bitácoras por carril y el ledger `matriz-ajustes.md` |

## Especificaciones de Módulos y Arquitectura Vigente

- **`features/`**: especificaciones funcionales de los subsistemas principales
  (9 ficheros). Cada una abre con un sello `**Estado de revisión:**` que dice
  cuándo se contrastó contra el árbol:
  - `auth-flow.md`: flujos de autenticación OAuth, PKCE y scraping de sesión,
    reconciliación multi-cuenta.
  - `authentication.md`: la sonda de diagnóstico solo-debug del login de
    Deezer (cookie jar, sin valores) de AUTH-WV-01.
  - `download-pipeline.md`: arquitectura del orquestador, cola y los motores
    nativos por proveedor.
  - `track-browser.md`: la maquinaria de listas de pistas — kernel de ventana
    virtual y composable de biblioteca (ADR-0025).
  - `library-import.md`: algoritmos de importación, deduplicación e identidad
    canónica.
  - `metadata-enrichment.md`: enriquecimiento con MusicBrainz, Last.fm y
    AcoustID, y el worker de fondo.
  - `playlist-system.md`: sincronización y persistencia de listas de
    reproducción.
  - `settings-system.md`: configuración jerárquica y persistencia en SQLite.
  - `PLAYER_PHASE1_MPRIS.md`: reproductor integrado — MediaSession, motor
    nativo (`crates/syncify-player`) y el servicio MPRIS nativo
    (`src-tauri/src/services/mpris.rs`). La comprobación D-Bus de extremo a
    extremo en sesión gráfica real **no** se ha ejecutado nunca en este repo.
- **`adr/`**: decisiones de arquitectura y alcance, numeradas por fecha. Son el
  único registro de decisiones; los antiguos `DECISION_*.md` son punteros de tres
  líneas que se conservan para que resuelvan las citas fechadas.
  - `0001-identidad-de-tracks.md`: la clave canónica
    `track_sources(service_id, service_track_id)` y las columnas de identidad.
  - `0002-alcance-beta.md`: qué queda fuera del beta y con qué condición vuelve;
    enmienda del carril de reproducción.
  - `0003-seleccion-de-edicion.md`: decisión beta contra la descarga de una
    edición incorrecta; contratos de identidad ED-01…ED-04 y pruebas de
    aceptación.
  - `0004-encolado-no-bloqueante.md`: la cola de descargas no bloquea la interfaz y
    escala a decenas de miles de pistas (propuesto).
  - `0005-enrutado-de-descargas-por-cuenta.md`: una descarga se enruta a una cuenta con
    derecho, reutilizando la evidencia de entregas anteriores (propuesto).
  - `0006-contrato-de-sincronizacion.md`: el contrato de sync — modo configurable con
    «solo importar» por defecto, cuarentena, disparo, recorrido y cobertura (propuesto).
  - `0007-cuentas-multiples-y-sesiones.md`: varias cuentas por servicio sin cuenta
    activa, biblioteca agregada, perfil de webview por cuenta y ciclo de vida (propuesto).
  - `0008-modelo-de-biblioteca.md`: alcance, pestañas, panel de detalle y selección
    masiva sobre la biblioteca agregada (propuesto).
  - `0009-adaptadores-de-descarga-nativos.md`: destinos de descarga en beta; Qobuz,
    Tidal, Deezer, SoundCloud y Amazon con adaptador nativo en Rust; Spotify y Apple
    Music quedan como catálogo (propuesto).
  - `0010-reproductor-cola-backend.md`: cola y estado de reproducción en el backend,
    entradas efímeras y búsqueda de catálogo en proveedores (aceptado).
  - `0011-motor-de-audio-dsp.md`: gapless, crossfade, ReplayGain, AGC y EQ
    paramétrico en el motor nativo (aceptado).
  - `0012-playlists-entidad-y-escritura.md`: playlist local sin cuenta, fuentes
    remotas, duplicados, tombstones y push bidireccional opt-in (propuesto).
  - `0013-playlists-escala-y-smart.md`: paginación sin tope de 500, smart playlists
    v2 con AND/OR, orden y límite, y portadas por mosaico (propuesto).
  - `0014-adopcion-hallazgos-symfonium-audirvana.md`: adopción selectiva de los
    hallazgos de Symfonium y Audirvāna; contrato de tags, cola, streaming y
    límites de alcance (aceptado).
  - `0015-migracion-colecciones-y-destinos.md`: favoritos, álbumes, artistas y
    playlists; capacidad declarada por destino y revisión de dudosos (propuesto).
  - `0016-migracion-ejecucion-y-undo.md`: idempotencia, reanudación, lotes con
    rate limit, journal y undo best-effort (propuesto).
- **`APP_FLOW.en.md`**: recorrido de extremo a extremo del flujo actual (en
  inglés): de conectar una cuenta a una descarga verificada en disco.
- **`design-system.md`**: los 10 temas (4 estándar + 6 de servicio), tokens
  CSS y reglas anti-FOUC.

## Matrices y planes vivos

- **`MATRIZ_PARIDAD_IMPORTACION.md`**: matriz viva de compatibilidad por
  servicio. Es la referencia de lo que cada proveedor puede entregar de
  verdad; léela antes de creerte cualquier promesa de importación global.
- **`LYRICS_PROVIDER_MATRIX.md`**: catálogo de las 12 fuentes de letras
  configurables por usuario y su capa de prioridad (antes
  `LYRICS_16_PROVIDER_MATRIX.md`: la cascada fija de 16 estrategias que
  describe ya no existe).
- **`PLAN_UNIFICACION_IMPORTACION.md`**: plan maestro de paridad de
  importación entre los 6 servicios (en ejecución, con estado por fase).
- **`COLA_FLUJO_DESCARGAS_PRE_BETA.md`**: cola viva de cronología y fallback
  del flujo de descargas (DLX-01…06, DFL-01: nada aplicado aún).
- **`COLA_RENDIMIENTO_FRONTEND_2026-10-13.md`**: cola viva de rendimiento de
  interfaz (FE-PERF-01…04; FE-PERF-02 aplicado en código).
- **`COLA_WEBKIT_COMPOSICION_ACELERADA.md`**: hipótesis abierta de
  composición WebKitGTK con sus invariantes de compatibilidad.
- **HANDOFF.md**: relevo vivo del 2026-11-05; estado por flujo y siguiente paso (descargas).
- **`PLAN_MEJORA_POST_BETA.md`**: lista de espera del post-beta (no cola de
  trabajo). Ojo: sus IDs `C-01…C-06` y `D-01/D-02` son **propios** y no
  coinciden con los de la Deuda.

## Estado actual — qué está hecho y qué no

**`Deuda_Tecnica_y_UX.md`** es el registro vivo de la deuda conocida: deuda
abierta (D-01…D-08), riesgos aceptados, alcance condicionado (lo que este
registro **no** declara como pendiente) y la tabla de cerrados (C-01…C-22).

Los cierres de relevo están fechados y son evidencia, no estado: tras el
cierre del **2026-10-11** se abrieron colas nuevas de incidencias de pruebas
manuales y el árbol sigue en desarrollo. En corto, para quien lea las páginas
públicas: Rust estaba en verde el 2026-10-11; la repetición del 2026-10-12
previa al lanzamiento pasó la suite UI (993 pruebas en 121 archivos;
registro local no versionado `auditoria/handoff-vitest-rerun.txt`) y la suite
Python (279 pruebas), y el build y los tipos del frontend quedaron OK. Son
resultados históricos, no una afirmación sobre esta revisión. La sesión
gráfica real, la red en vivo y el target Windows/MSVC no se han ejercitado,
así que las portadas públicas no declaran la app lista para publicar.

El rastro interno (español) de por qué se hizo cada cambio:

- **`auditoria/matriz-ajustes.md`**: el ledger R1 que gobierna cualquier
  ajuste nuevo («ningún ajuste sin fila previa»). Es el estado canónico de
  casi todos los ítems de colas y bitácoras.
- **`auditoria/triaje.md`** y **`auditoria/cierre-handoff-2026-10-11.md`**: el
  triaje de huérfanos y el cierre verificable del relevo, con su lista
  explícita de lo que **no** verificó.
- **`auditoria/*-bitacora.md`**: las bitácoras por carril detrás de cada
  arreglo.
- **`auditoria/CONSOLIDATED_PLAN.md`** y **`auditoria/AUDITORIA-PILARES-2026-10-18.md`**:
  plan OAuth+rendimiento (ya ejecutado en lo esencial, con errata en su
  cabecera) y la auditoría integral por pilares.

## Histórico — `historico/`

Registros fechados que se conservan como evidencia. Ninguno es estado actual;
todos llevan un banner «Estado actual» que dice qué quedó resuelto después y
qué sigue abierto solo en ese fichero:

- `PLAN-Correccion-168.md`: plan v2 de los 168, ya consumido; retractaciones
  de la v1 y trazabilidad ID→carril.
- `PENDIENTES.md`: acta de cierre del backlog v0.4.1 (sin trabajo abierto).
- `INFORME_HALLAZGOS_OMITIDOS.md`: qué se descartó deliberadamente y por qué.
- `Errores-Pruebas-2026-10-08.md`, `-11.md`, `-13.md` y `-13-flujo-limpio.md`:
  los registros de pruebas manuales y sus colas (espacios de IDs G-, L-, Q-,
  DSC-, DL-SVC-, MF-…).
- `perf-probe-2026-10-12.md` y `perf-run-2026-10-12.md`: mediciones de
  rendimiento backend (el índice de expresión sobre ISRC sigue sin hacerse).
- `RELEASE_NOTES_v0.6.0.md`, `RELEASE_NOTES_v0.6.1.md` y
  `RELEASE_NOTES_v0.6.2.md`: notas de las releases etiquetadas `v0.6.0`,
  `v0.6.1` y `v0.6.2`.

## Espacios de identificadores

Los IDs se repiten entre documentos con significados distintos. Cita siempre
ID **y fichero**:

| Prefijo | Fuente definitoria | Colisión a vigilar |
|---|---|---|
| `D-01…D-08`, `C-xx`, `A-01` | `Deuda_Tecnica_y_UX.md` | `C-12/13/14` figuran dos veces en la tabla de cerrados; no existe `D-05` |
| `C-01…C-06`, `D-01/D-02` | `PLAN_MEJORA_POST_BETA.md` | **Colisiona** con la Deuda: significan cosas distintas |
| `Q-01…Q-17` | `historico/Errores-Pruebas-2026-10-11.md` | `Q-01` también existe en `auditoria/triaje.md` con otro significado |
| `G-`, `L-`, `DSC-`, `FS-`, `AQ-`, `PR-`, `MET-`, `LY-`, `LOG-`, `M-`, `P-`, `PL-`, `BP-`, `DUP-`, `SYM-`, `BPM-` | `historico/Errores-Pruebas-2026-10-08.md` + `auditoria/triaje.md` + `auditoria/matriz-ajustes.md` | `L-`/`D-` reaparecen en el registro del 10-13 |
| `DLX-`, `DFL-` | `COLA_FLUJO_DESCARGAS_PRE_BETA.md` | — |
| `FE-PERF-` | `COLA_RENDIMIENTO_FRONTEND_2026-10-13.md` | — |
| `DL-SVC-`, `DL-DISK-`, `DL-UX-`, `TRAY-OPEN-`, `PLAY-TRACE-` | `historico/Errores-Pruebas-2026-10-13.md` | `D-16/D-17/D-18` colisionan con la Deuda |
| `MF-*` | `historico/Errores-Pruebas-2026-10-13-flujo-limpio.md` | — |
| `ED-01…ED-04` | `adr/0003-seleccion-de-edicion.md` | — |
| `A1…A5`, `B1…B5` | `auditoria/CONSOLIDATED_PLAN.md` (histórico) | Visualmente parecido a `A-01` de la Deuda |
| `SYNC-AUD-0xx` | cabecera de `scripts/tests/test_docs_consistency.py` | No existe informe aparte que los enumere |

## Diagramas y assets

- `diagrams/`: diagramas HTML explorables, cada uno con su build inglesa
  (`*.en.html`) y española (`*.html`): [architecture](diagrams/architecture.en.html) ·
  [download journey](diagrams/download-journey.en.html) ·
  [library care](diagrams/library-care.en.html) ·
  [user journey](diagrams/user-journey.en.html).
- `screenshots/`, `design-mockups/`, `logo-candidatos/`: assets de portada y
  diseño, no documentación.

## READMEs de directorio

- `crates/README.md` (raíz de `crates/`): qué hace cada crate del workspace.
- `scripts/maintenance/README.md`: herramientas de mantenimiento de scripts.
- `src-tauri/binaries/README.md`: binarios externos empaquetados.
- `ui/public/fonts/README.md`: fuentes locales servidas sin red.

Vuelta a las portadas públicas: [`../README.md`](../README.md) (inglés) y
[`../README.es.md`](../README.es.md) (español).

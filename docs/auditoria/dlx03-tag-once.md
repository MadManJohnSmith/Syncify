# DLX-03 — Etiquetado y verificación sin trabajo repetido

**Issue:** [#44](https://github.com/MadManJohnSmith/Syncify/issues/44)

**Rama de evidencia:** `feat/ola1-44-dlx03-tag-once`

**Medición:** 2026-11-24, offline, sin cuentas ni red reales.

## Método

`src-tauri/tests/dlx03_tag_once_test.rs` reproduce la secuencia anterior y la
candidata sobre el mismo FLAC sintético. Un comentario VORBIS de 8 MiB hace
visible el coste de cada parser; las llamadas se agrupan con
`DownloadPhaseTracker` y se comparan con `TimelineArtifact::compare_runs`, es
decir, por **unión de intervalos**, nunca por suma.

Los contadores son deterministas. Los milisegundos de abajo son una corrida
ilustrativa, no una promesa de rendimiento ni una medición p50/p95.

## Inventario antes/después

| Camino | Antes | Después | Lecturas eliminadas |
|---|---:|---:|---:|
| Tags de Tidal tras escribir | 3: `apply_and_verify`, verificación duplicada y auditoría de portada | 2: `apply_and_verify` y auditoría | 1 |
| Resultado final (verificación/métricas) | 8: post-promoción (1), proveedor (1), gateway (1), orquestador (1), gate tags+duración (2), calidad estricta (1), ledger (1) | 2: proveedor (1) y tags del gate (1) | 6 |

Ejemplo de unión de intervalos en la corrida del 2026-11-24:

| Camino | Baseline | Candidata | Delta |
|---|---:|---:|---:|
| Resultado final | 22 ms | 8 ms | −14 ms |
| Tags de Tidal | 32 ms | 23 ms | −9 ms |

La evidencia no cambia: `TagVerification`, duración, bit depth, sample rate,
canales, bitrate, tags de identidad y portada coinciden entre baseline y
candidata. El test también borra el fichero tras la primera reconciliación y
comprueba que gateway, orquestador y gate de calidad siguen usando la instantánea.

## Garantías conservadas

- El gate final sigue abriendo y verificando los tags, la duración y la portada
  exigida; no se sustituye evidencia física por valores anunciados.
- Calidad estricta y ledger consumen la misma instantánea del gate. Sin
  evidencia física, el rechazo sigue siendo cerrado.
- La auditoría de portada de Tidal se mantiene. Solo desaparece la segunda
  verificación de tags, que antes era no fatal y repetía el veredicto.
- Qobuz y Tidal reutilizan STREAMINFO ya validado. El etiquetado no modifica ese
  bloque y la promoción de Qobuz conserva su comprobación de tamaño/hash.
- Si el trimmer de silencio modifica el fichero, invalida la instantánea antes
  del gate; el worker vuelve a inspeccionar una sola vez el resultado final.
- No cambia selección de identidad/edición, política de calidad, rutas, migraciones
  ni contratos IPC.

## Límites

La medición es offline y no ejercita proveedores reales, GUI, Windows ni
rendimiento de red. Las llamadas ajenas a esta redundancia —reparación de
portada, sincronización MusicBrainz posterior al ledger o manifiestos de lote—
no se eliminan.

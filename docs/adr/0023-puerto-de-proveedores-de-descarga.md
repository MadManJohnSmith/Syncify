# ADR-0023 — Puerto de proveedores de descarga

**Estado:** aceptado · **Fecha:** 2026-11-06
**Origen:** parte P5 del informe de arquitectura, cerrada en dos rondas de grilling.
**Spec:** PR de P5 (rama `p5/download-provider`). **Tickets:** ninguno.

## Contexto

El orquestador (`src-tauri/src/download/orchestrator.rs`) conocía Qobuz, Tidal y Amazon
concretos: guardaba tres descargadores, despachaba por string y tenía ramas por
proveedor en cada flujo. D01 entregó un gateway (`provider_gateway.rs`) con el trait
`DownloadProvider`, `ProviderRequest`, `ProviderError` y adaptadores, pero solo lo
consumían los tests: producción seguía llamando a `self.qobuz/tidal/amazon`.

Hechos verificados que condicionaron la decisión:

- `DownloadRequest` tiene 30 campos y los descargadores usan 22 (Qobuz), 20 (Tidal) y 9
  (Amazon); el «contexto reducido» que proponía la ronda 1 no existía en el código y
  reconstruirlo dentro del adaptador introduciría defaults silenciosos.
- La provenance (origin/effective/fallback/match) y la reconciliación de calidad se
  repetían en cada flujo; `ProviderDelivery::from_result` copiaba bit depth y sample
  rate del resultado, no del fichero (Amazon declara 24/48 fijos).
- Amazon es URL-relay, lossy-only, sin cuenta ni fila en `services`; su error se
  clasificaba como `CrossProviderFallback` porque `OperationType` no tenía variante.
- SongLink resuelve enlaces y produce candidatos; no descarga.
- `account_id` no es vinculante: los descargadores re-resuelven cuenta.

## Decisión

- **El puerto es el gateway de D01, adoptado con `ProviderRequest` completo**
  (`DownloadRequest` + `account_id` + `ProviderRouting`). El contexto estrecho queda
  aplazado hasta que los descargadores se reescriban; se declara como deuda.
- **El adaptador posee descarga + normalización + provenance + reconciliación de
  calidad** (`download/providers/{qobuz,tidal,amazon}.rs`, detrás de traits de motor
  pequeños que los tests sustituyen sin red). `ProviderRouting` lleva los valores que
  el orquestador ya calcula (origin service/track id, effective track id, match
  method/confidence, fallback reason) y `ProviderCapabilities` declara
  `supports_lossless` (Qobuz/Tidal sí, Amazon no) para que el enrutado no codifique el
  caso Amazon.
- **El orquestador conserva la política**: selección de flujo, fallback Qobuz↔Tidal,
  auth, progreso y SongLink. Toda descarga de proveedor pasa por el registro
  (`resolve_provider`); el progreso y sus eventos no se mueven.
- **Errores:** `ProviderError` se clasifica una vez en la frontera con
  `classify_operation_error` y el orquestador decide auth, fuente no disponible,
  calidad y preview desde `ErrorTaxonomy`. Los checks de string que el clasificador
  compartido no desambigua (entity lookup 401/403, `NetworkExhausted`, marcadores de
  credencial) se conservan anotados. Los descargadores no cambian su contrato de error.
- **Amazon es proveedor de primera clase**, con URL-relay encapsulada y
  `OperationType::DownloadAmazon`.
- **SongLink queda fuera de P5**: es un resolvedor, no un proveedor; se documenta como
  seam aparte.
- **`qobuz`/`tidal` siguen como campos del orquestador** para la búsqueda de identidad
  (ISRC/MBID/AcoustID) y comparten sus `Arc` con los motores del registro; `amazon` se
  elimina como campo (no tiene estado).

## Consecuencias

- Añadir un proveedor (Deezer, SoundCloud; ADR-0009) es escribir un adaptador con su
  motor y registrarlo; el despacho de descarga deja de ser un `match` por string.
- La provenance y la calidad viven en un solo sitio (`apply_routing` +
  `quality_reconcile`); los flujos del orquestador se reducen a routing y fallback.
- Tests: motores grabadores para los adaptadores (`provider_adapters_test`), overrides
  del registro para el despacho (`provider_dispatch_test`) y los 12 tests de
  fallback/identidad como red de seguridad.
- Deuda declarada: contexto estrecho, cuenta vinculante, errores tipados en los
  descargadores, checks de string anotados, el clasificador de re-resolución
  (`classify_retry_failure`), unificar `service_priority` con `service_preferences`,
  SongLink como puerto propio y `ProviderDelivery` duplicando campos de
  `DownloadResult`.

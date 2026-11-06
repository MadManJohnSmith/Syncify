# ADR-0022 — Máquina de estados de la cola de descargas

**Estado:** aceptado · **Fecha:** 2026-11-06
**Origen:** parte P4 del informe de arquitectura, cerrada en ronda de grilling.
**Spec:** PR de P4 (rama `p4/download-job-state`). **Tickets:** ninguno.

## Contexto

El estado de un job de descarga no es un tipo: es `download_queue.status`, una columna
TEXT con CHECK de cinco valores (`queued`, `downloading`, `complete`, `failed`,
`cancelled`; migración 0004) consultada en una veintena de sitios repartidos entre
`commands/queue.rs`, `commands/library.rs`, `download/orchestrator.rs` y `worker.rs`.

Consecuencias verificadas en el árbol:

- **Cancelar** solo hacía `UPDATE ... status = 'cancelled'`: el `CancellationToken`
  que `download_track` acepta nunca se cableaba en producción (se pasaba `None`), la
  transferencia en vuelo continuaba hasta el final y su resultado se descartaba por el
  `WHERE status = 'downloading'`. Cancelar era «esperar y descartar».
- `mark_failed` y `mark_downloading` escribían **sin predicado de estado**: podían
  pisar una fila ya cancelada.
- **Cuatro barridos** `downloading → queued` distintos (worker, `operation_recovery`
  ×2, `integrity`) hacían de recuperación de arranque.
- **Tres políticas de reintento**: tope 5 en el reintento masivo, centinela
  `retry_count = 99` para «permanente», y el claim del worker incrementaba sin tope.
- La columna `failure_class` (migración 0103) existía y **nunca se usaba**; la
  taxonomía de fallos ya vivía en `crates/syncify-core-domain/src/errors.rs`.
  `next_attempt_at` tampoco se usaba en ningún sitio.
- `download_jobs` (0102) es un **agregado distinto** con su propio vocabulario
  (`running`, `completed`, `partial`, `cancelled`, `failed`): solo se escriben
  `running` y `failed`, y un test fija que un job exitoso queda en `running`. Queda
  fuera de esta decisión.

## Decisión

- **La máquina es un tipo puro** en `crates/syncify-core-domain`:
  `JobState { Queued, Downloading, Complete, Failed, Cancelled }`, `JobEvent
  { Claim, Complete, Fail(FailureClass, reason), Cancel, ManualRetry, RecoveryReset,
  ScheduleRetry(Instant) }` y `transition()` que devuelve `IllegalTransition` en las
  aristas inexistentes. Los strings serializados son **los cinco actuales**: contrato
  con BD, CHECK y UI intacto.
- **`RetryVerdict`**: `Permanent` o `RetryAt(instante)` con backoff exponencial
  1m → 5m → 15m → 1h → tope 4h y cinco intentos automáticos
  (`MAX_AUTOMATIC_ATTEMPTS`). Una `FailureClass` terminal fuerza `Permanent` sin
  espera. Un `failed` con `RetryAt` **sigue siendo `failed`** durante la espera
  (`next_attempt_at` persistido) y el claim lo admite cuando vence: el usuario ve el
  fallo, no un «queued» engañoso.
- **El reintento manual** es la transición `Failed → Queued` con `retry_count = 0`,
  `next_attempt_at = NULL` y errores limpios; se permite siempre, incluidas las
  clases terminales (el adaptador volverá a fallar rápido; no hay bucle).
- **Toda escritura de estado es un `UPDATE ... WHERE id = ? AND status = <from>`**
  (y `job_id` cuando aplica). Perder la carrera es un error observable, no un
  overwrite: `mark_failed` ya no puede pisar una fila cancelada.
- **Cancelación real**: registro `job_id → CancellationToken`; cancelar hace el
  `UPDATE` condicional a `cancelled` y después `token.cancel()`. Los adaptadores
  consultan el token en su bucle de transferencia y devuelven `Cancelled`; el staging
  parcial se conserva (`resumable`) para reanudar. Un job cancelado no se recupera ni
  se reintenta automáticamente.
- **Recuperación de arranque**: una única transición `RecoverySweep`
  (`downloading → queued`) aplicada en un solo sitio; `failed` y `cancelled` no se
  tocan.
- **`failure_class` pasa a estar viva**: la transición a `Failed` persiste la clase
  junto al estado y al mensaje, de modo que tras un reinicio el veredicto de
  reintento se calcula desde la fila.
- **UI**: el tracker deja de mapear `Cancelled → "failed"`; el estado `cancelled`
  llega tal cual y el tipo TypeScript lo admite.

## Consecuencias

- Desaparecen los ~20 predicados sueltos (los comandos de lectura usan los helpers de
  `JobState`), los tres presupuestos de reintento, los cuatro barridos y el centinela
  99.
- `next_attempt_at` y `failure_class` dejan de ser columnas muertas; el claim filtra
  por vencimiento.
- `download_jobs` sigue con su vocabulario y sus campos muertos (`completed`,
  `partial`, contadores): deuda documentada, fuera de P4.
- Riesgo alto asumido con caracterización previa: claim, cancelación, reintento y
  recuperación quedan fijados por tests antes de mover el código, y la matriz de
  transiciones legales/ilegales se prueba en el crate puro sin BD.

# ADR-0016 — Migración: ejecución idempotente, reanudación y undo

**Estado:** propuesto · **Fecha:** 2026-11-05
**Origen:** ronda de grilling del flujo MIGRACIÓN.
**Spec:** #164 · **Tickets:** #165–#174.

## Contexto

El job actual recorre ítems en serie con pausas de 25 ms y sin clave de idempotencia;
`retry_failed_items` reintenta fallidos sin comprobar si ya se escribieron. Al cerrar
la app, un job `running` queda colgado. No hay journal de lo creado ni undo.

La migración de 30k favoritos con llamadas de red no cabe en un bucle secuencial fijo.

## Decisión

1. **Idempotencia por ítem.** Clave `(job, colección, id de origen, destino)`; antes de
   escribir se comprueba si ya existe en destino y se marca `preexisting` o `created`.

2. **Lotes con concurrencia limitada.** Presupuesto por proveedor y backoff adaptativo
   ante 429/5xx; nunca un fan-out sin techo.

3. **Reanudación automática.** Los estados de ítem son durables; al arrancar, un job
   `running` se reanuda desde el último ítem no terminal, sin repetir escrituras.

4. **Journal y undo best-effort.** Se registra cada ítem creado con su id de destino;
   el undo solo quita lo creado por el job, nunca lo preexistente, y reporta lo que el
   proveedor no permita deshacer.

5. **Cancelar para tras el lote actual.** El job queda reanudable; cancelar no deja
   ítems a medias ni borra lo ya creado.

6. **Progreso por lote y sin escrituras silenciosas.** Cada ítem deja estado y motivo;
   la UI muestra creados, existentes, fallidos y omitidos.

## Consecuencias

- Migración nueva para clave de idempotencia, estado durable y journal; no se edita una aplicada.
- El undo depende de que el proveedor tenga remove; si no, se reporta.
- Los jobs existentes se marcan como históricos; no se reanudan a ciegas.
- Los tests usan gateway falso; el E2E real va en tests #[ignore].
- El coste de 30k se mide en la aceptación.

## Alternativas descartadas

- **Secuencial con delay fijo.** No escala a 30k ni aprovecha la API.
- **Sin undo.** El propietario pidió best-effort.
- **Rollback completo.** Las APIs de destino no lo permiten.
- **Sin reanudación.** Un cierre de app no debe perder el job.

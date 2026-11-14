# Modo de ejecución

Cómo se ejecuta el trabajo en este proyecto. Decisión del propietario, no
preferencia del agente. Relacionadas: [`entorno.md`](entorno.md) y
[`protocolo-pr.md`](protocolo-pr.md).

## Inline es el modo preferente

- **2026-11-13, decisión del propietario**: implementación inline con el
  modelo del hilo del jefe de gabinete; ese día los dos carriles
  (AUTH-WV-01 y `mb_dump_import`) se implementaron inline y fusionaron
  limpios. Fuente: `docs/historico/HANDOFF-2026-11-13.md` §Plan de
  continuidad.

## Por qué no delegar (evidencia del 2026-11-13)

- **Codex `gpt-6-luna`, 2/2 carriles fallidos**: ambos turnos terminaron
  tras inspeccionar, sin commits ni PR («completado sin entregar»). El run 2
  —con contrato endurecido— entregó commit y PR pero su autoinforme dijo lo
  contrario en ambos sentidos (declaró «sin commit ni PR» existiendo
  `2ee84a3` y PR #283). Fuente:
  `docs/historico/retro-sesiones-2026-11-13.md` §Sesión 2 y actualización.
- **El sistema de tareas reporta «completado», no «entregado»**: un turno
  que se cierra tras inspección pasa salvo que el contrato haga del informe
  sin commits un fallo explícito. Misma fuente.
- **Este proveedor no admite delegado en background**: el 2026-11-13, seis
  fallos seguidos `3008 user concurrency limit exceeded` (la cuenta ya la
  ocupa el hilo padre). Fuente: `docs/historico/HANDOFF-2026-11-13.md`
  §Restricciones operativas.

## Si se delega igualmente

Contrato endurecido (aplicado con éxito en el run 2 del 2026-11-13):

1. Entrega mínima aceptable = commits + PR abierto (base `dev`) + puertas
   corridas con resultado verbatim. «Un informe sin PR es un fallo del
   encargo».
2. Verificación de estado al despertar: `git log dev..rama` y
   `gh pr list`; el estado verificado manda sobre el autoinforme.
3. Continuar en el hilo del delegado (conserva la inspección pagada) mejor
   que relanzar limpio.

## Escalado pendiente

- **2026-11-13**: segunda ocurrencia de «completado sin entregar» alcanza
  el criterio de escalado acordado — ticketar junto a #277 como hueco de
  plataforma distinto («run que termina sin el artefacto contratado»). No
  ticketado aún a la espera del comportamiento inline; re-abrir si vuelve a
  ocurrir. Fuente: retro del 2026-11-13 §Criterio de escalado.

## Orden de carriles fijado el 2026-11-13

A (claims del README, #264 punto 5) → B (serie SB, spec #195) → C (#178
LY00, parte sin credenciales) → programas grandes por spec, uno en vuelo
cada vez. El inventario completo y los bloqueados no se copian aquí: viven
en `docs/historico/HANDOFF-2026-11-13.md` y el estado canónico en el
tracker y `docs/auditoria/matriz-ajustes.md`.

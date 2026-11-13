# Retro de sesiones de agente — 2026-11-13

Evidencia fechada, no estado actual. Segunda retro del proyecto, acotada a dos
fallos de delegación de la semana (precedida por
`docs/historico/retro-sesiones-2026-11-11.md`).

## Sesión 1 · Delegado DLX-01 muerto por cuota

El delegado de DLX-01 (issue #42) murió con error 1005 del proveedor
(cuota) el 2026-11-13 00:32 UTC, antes de producir commits. Su worktree quedó
en `9804e33` con cero trabajo propio y se retiró; registrado en
`docs/historico/HANDOFF-2026-11-12.md`. Fallo de plataforma (run muerto sin
trabajo propio), del mismo familia que los hallazgos escalados en el issue
#277: no añade causa nueva en el repo.

## Sesión 2 · Delegado AUTH-WV-01 «completado» sin entrega (2026-11-13)

Delegado de implementación (issue #63, Codex `gpt-6-luna`, effort high) en
worktree. La tarea terminó con estado `completed`, pero el informe reveló que
**no implementó nada**: se cerró tras inspeccionar (dos hallazgos reales y
útiles: Tauri expone API de cookies; la puerta `dead_commands_hygiene_test`
exige invocador literal en la UI) sin convertirlos en código, sin commits
(worktree verificado en el commit base de `dev`) y sin PR.

### Hallazgo central

El sistema de tareas reporta *completado*, no *entregado*. Un turno que se
cierra tras inspección pasa por completo salvo que el contrato de entrega
haga del informe sin commits un fallo explícito. La verificación al despertar
(comprobar `git log dev..rama` y la lista de PRs antes de actuar) funcionó y
es obligatoria, pero detecta el daño tarde: ya se quemó un ciclo de
delegación completo.

### Lo aplicado

- **Continuación en el mismo hilo del delegado** en vez de un relanzamiento
  limpio: conserva la inspección ya pagada en su contexto.
- **Contrato de entrega endurecido** en el mensaje de continuación: entrega
  mínima aceptable = commits + PR abierto (base `dev`) + puertas corridas con
  resultado verbatim; «un informe sin PR es un fallo del encargo». Cláusula
  candidata a estándar en todo brief de implementación.
- La cláusula del brief original «elige la mínima y no pidas confirmación»
  se mantuvo; no fue la causa. La causa plausible es el cierre de turno del
  modelo tras una fase de exploración, sin barrera contractual que lo
  impidiera.

### Criterio de escalado

Si el patrón «completado sin entregar» se repite (segunda ocurrencia), se
ticketará junto a #277 como hueco de plataforma distinto: no es un run
muerto (el run termina), es un run que termina sin el artefacto contratado.

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

## Actualización del mismo día · continuación del delegado (run 2) y toma del carril

El run 2 con el contrato endurecido terminó honesto (declaró no alcanzar el
mínimo) pero con el autoinforme erróneo en ambos sentidos: decía «no hay
commit ni PR» y **sí había** commit (`2ee84a3`, 5 ficheros, +74 líneas) y PR
abierto (#283), con timestamps posteriores a su borrador de informe. Además
su guion de puertas siguió corriendo como proceso huérfano tras el fin del
turno (clippy/check vivos contra el target compartido; al matarlos, la
secuencia avanzó de paso). Lecciones:

- **El estado verificado manda sobre el autoinforme.** Antes de actuar sobre
  un informe de delegado: `git log dev..rama`, `gh pr list`. Los dos primeros
  informes de esta lane divergieron del árbol real (uno por defecto, otro por
  exceso de pesimismo).
- **Los procesos huérfanos de un run terminado pueden bloquear el lock de
  `/tmp/syncify-target`** — causa plausible del bloqueo que el run 2 reportó
  (contendría también con cualquier puerta que el checkout padre corra en
  paralelo, p. ej. el hook pre-push de un push de docs). Contramedida: al
  retomar un carril, `pgrep cargo` antes de gatear; los briefs de delegación
  deberían usar target privado por worktree cuando el padre puede gatear a la
  vez.
- **El worktree de un delegado nace sin entorno**: sin `ui/node_modules` ni
  `.venv` (gitignored, sin script de setup). Enlazar los del checkout padre
  funciona cuando el PR no toca lockfiles (verificado aquí). Candidato a
  estándar: enlazar o instalar el entorno en la preparación del worktree.
- **El contrato endurecido mejoró la honestidad, no la calidad del código.**
  La review del diff de 74 líneas encontró cuatro defectos antes de las
  puertas: import duplicado de `ref` (TS2300), `import.meta.env` en
  expresión de plantilla (error de compilación Vue), tipo inventado
  `tauri::UrlParseError` (E0425) y move de `cookies` tomado prestado (E0505),
  más una línea fuera de fmt. Los arregló el jefe de gabinete en el worktree;
  clippy/check/fmt/1149 tests UI/430 Python/38 de docs en verde tras ellos.
  La suite Rust completa quedó corriendo en background al cierre de este
  apunte; su resultado, en el handoff del carril.

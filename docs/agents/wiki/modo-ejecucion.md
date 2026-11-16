# Modo de ejecución

Cómo se ejecuta el trabajo en este proyecto. Decisión del propietario, no
preferencia del agente. Relacionadas: [`entorno.md`](entorno.md) y
[`protocolo-pr.md`](protocolo-pr.md).

## Por issue, un subagente por issue (vigente)

- **2026-11-14, decisión del propietario**: el trabajo pasa a hacerse por
  issue, cada issue con su subagente dedicado; ya no implementación inline
  por el hilo del jefe de gabinete. Suplanta la decisión del 2026-11-13
  (inline preferente): nueva evidencia del propietario, decisión suya, no
  del agente. La sesión del 2026-11-14 validó la infraestructura de
  carriles: SB01 y SB03 viajaron limpios por rama + PR con puertas
  locales completas antes de empujar. Fuente:
  `docs/historico/HANDOFF-2026-11-14.md` §Cambio de modo.
- Riesgos del modo delegado y mitigaciones (no re-litigar): fallback de
  «completado sin entregar» escala a #277 como hueco de plataforma;
  este proveedor puede rechazar delegado en background (`3008`) y se
  delege en primer plano o con otro modelo vía `orchestrator_capabilities`;
  dos carriles que comparten fichero mina se serializan o usan
  `isolation: worktree`. Fuente: misma sección del handoff.
- **2026-11-15/16, dos fallos nuevos de infraestructura de delegación**:
  (1) un delegado murió por cuota (`1005 exceed quota limit`) dejando WIP
  sin commit — el reintento se lanzó limpio, sin reutilizar el WIP;
  (2) tras una recuperación de sesión del harness, el id de modelo
  heredado cambió de `builtin:zai-start-plan\GLM-5.3-Flash` a
  `account:zai-start-plan\GLM-5.3-Flash` y el agente rechazó el segundo;
  el catálogo (`orchestrator_capabilities`) ya no anuncia el `builtin:` —
  la mitigación fue relanzar con un modelo hermano del mismo plan
  (`account:zai-start-plan\GLM-5.2`). Fuente: fallos de los delegados
  SB05 #201 y sus reintentos en la sesión del 2026-11-14/16.

## Histórico: inline preferente (supersado)

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

### Progreso fechado

- **2026-11-14**: carril A cerrado (PR #286, rebaja de §«Verification and
  limitations») y SB00 fusionado el día anterior (#287, `ea62b39`).
  Fuente: `git log dev`.
- **2026-11-14**: SB01 (#199) fusionado por squash en PR #288 (`d5976c6`
  en `dev`); issue cerrado manualmente citando el SHA. La puerta de
  claims del AGENTS.md cazó el objetivo de test nuevo (319→320). El
  carril venía a medio hacer de una sesión anterior (cambios sin
  commit); se completó inline, con 4 objetivos de test de
  caracterización actualizados al contrato de superficie única. Fuente:
  PR #288. Siguiente del orden: SB03 (#198).
- **2026-11-14**: SB03 (#198) fusionado por squash en PR #289 (`b16d70e`
  en `dev`); issue cerrado manualmente citando el SHA. Los checks de
  CI quedaron verdes antes del merge (Rust 29m47s / 47m59s). Siguiente
  del orden: SB04 (#200), primer issue delegado bajo el modo nuevo.
  Fuente: PR #289 y `git log dev`.
- **2026-11-14**: LY00 (#178) fusionado por squash en PR #290 (`7efbd20`
  en `dev`); issue cerrado manualmente citando el SHA. Primera
  delegación de investigación completada bajo el modo nuevo: solo la
  matriz, cero código del motor, puerta Python verbatim en el informe.
  Las 4 fuentes con credenciales quedaron como tarjeta al propietario.
  Fuente: PR #290 y `git log dev`.
- **2026-11-14**: re-derivación de la spec #36 (mitad delegable) fusionada
  por squash en PR #291 (`788c195` en `dev`); issues de ejecución
  #58–#61 cerrados citando el SHA y D-04 cerrada en el ledger de la
  Deuda. La mitad de verificación de red (brazos SoundCloud y Apple
  Music) sigue condicionada a credenciales del propietario (D-01).
  Fuente: PR #291, `docs/Deuda_Tecnica_y_UX.md` §D-04.

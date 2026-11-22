# Modo de ejecución

Cómo se ejecuta el trabajo en este proyecto. Decisión del propietario, no
preferencia del agente. Relacionadas: [`entorno.md`](entorno.md) y
[`protocolo-pr.md`](protocolo-pr.md).

## Reglas vigentes

**Actualizado 2026-11-19.** El estado canónico es el tracker de GitHub y el
handoff del día; esta página solo fija lo que no debe re-litigarse.
*(Superado el 2026-11-22 en lo del «handoff del día»: el estado se re-deriva
del tracker y del árbol según `docs/HANDOFF.md`; los handoffs son registros
fechados en `docs/historico/`.)*

Los bloques van en orden cronológico; **prevalece el más reciente** (hoy el
del 2026-11-22, al final). Lo que un bloque posterior contradice queda marcado
«superado» en su sitio, sin borrarlo.

- **Modo por issue con subagentes.** Cada issue viaja en su carril con un
  subagente dedicado; el jefe de gabinete orquesta, no implementa inline salvo
  para rescatar un WIP.
- **Cupo real.** 2 carriles GLM locales + 1 remoto = 3 GLM, más 1 deepseek
  local. El cuarto GLM muere con `3008`: la cola rota, no se fuerza.
  *(Superado el 2026-11-22: el límite de concurrencia/3008 deja de aplicar
  con el cambio de modelo; ver bloque 2026-11-22.)*
- **Remotos aparcados (2026-11-19).** agentvm1 no entregó ningún carril
  (`Internal error` en los tres); la cola es local hasta nuevo aviso.
  *(Superado el 2026-11-22: la cola se reparte entre PC-A y PC-B.)*
- **Reparto de superficies.** Los carriles simultáneos no comparten fichero;
  si lo necesitan, se serializan o usan un worktree aislado.
- **Target por checkout.** `CARGO_TARGET_DIR=$HOME/.cache/syncify-target-$(basename "$PWD")`,
  fuera de tmpfs; nunca el `/tmp/syncify-target` compartido.
  *(Precisado el 2026-11-22: en carriles, un target persistente por carril,
  no por worktree; ver bloque 2026-11-22.)*
- **Contrato de entrega.** Commits + PR con base `dev` + puertas corridas con
  salida verbatim. Un «completed» sin commits y PR es un fallo del encargo.
  *(Superado el 2026-11-21/22 en la base del PR: el PR de carril va contra
  la rama del lote `wave/ola-N-batch-M`, no contra `dev`; ver
  [`protocolo-pr.md`](protocolo-pr.md).)*
- **Revisión con artefacto.** La revisión independiente publica
  `gh pr review --comment` (hallazgos, fecha y veredicto) antes del merge; sin
  ese comentario el claim no cuenta.
- **Módulo sin llamador.** No cierra su issue hasta que la adopción aterrice o
  exista un follow-up enlazado (precedente: `account_refresh`, #342).
- **Docs directos a `dev` con CI de docs.** Los cambios de wiki/docs viajan
  directos a `dev`; `.github/workflows/docs-check.yml` corre las tres puertas
  de `scripts/tests/`.

**Bloque fechado 2026-11-21 (Ola 0, Lote 2).** Los lotes de Ola 0 viajan por
**oleadas de ≤3 carriles concurrentes en una sola máquina** (gallifrey):
TARDIS está enlazada y accesible pero sin el proyecto Syncify registrado, y
agentvm1 sigue inaccesible (ver `entorno.md`). Los delegados son todos
`opencode-go/deepseek-v4.1-flash` (provider `opencode`) en `full-access`
(orden del dueño ese día), un worktree por carril vía delegación con
`isolation: worktree`. *(Superado el 2026-11-22: «una sola máquina» y
«≤3 concurrentes» → ≤3 por PC en dos PCs; el modelo de delegados cambia y
con él el límite 3008; los worktrees nuevos van a carpeta limpia. Ver
bloque 2026-11-22.)* Los PRs de carril van contra `wave/ola-0-batch-<n>` y
el lote cierra con un PR consolidado a `main`; cada PR de carril lleva
revisión delegada independiente con artefacto `gh pr review --comment` antes
de integrarse. Anti-3008: commit + push en cada micro-hito (caracterización,
núcleo, puertas). Fuente: handoff del despacho del Lote 2, versionado el
2026-11-22 como `docs/historico/HANDOFF-2026-11-21.md`.

**Lección fechada 2026-11-21:** para saber si un carril interrumpido dejó
commits sin pushear, no te fíes del ref remoto local `origin/<rama>` — puede
no existir aunque el delegado ya pusheó (falso «0 sin push» medido en el
carril #69, que sí había pusheado). Verifica con `git ls-remote origin
<rama>`. Al relanzar un carril muerto: pushear/preservar la rama, capturar el
WIP sin commit como patch en la carpeta de estado del dueño, liberar el
worktree muerto y re-delegar con `git checkout -b <rama> origin/<rama>`.

**Bloque fechado 2026-11-22 (decisiones del dueño; prevalece sobre los
anteriores).** Fuente: decisión del dueño en la sesión del 2026-11-22.

- **Techo de concurrencia.** El límite de concurrencia del proveedor
  (`3008`) ya no condiciona el despacho: se usará otro modelo de delegados.
  El techo pasa a ser **≤3 carriles por PC**, con dos PCs (PC-A y PC-B):
  **6 carriles** como máximo, limitados en la práctica por los locks y por
  las colisiones de fichero, no por el proveedor. Supera el cupo GLM/deepseek
  del 2026-11-19 y el «≤3 en una sola máquina» del 2026-11-21.
- **Seis carriles con afinidad de máquina.** PC-A: `lane:core-sync`,
  `lane:downloads`, `lane:sql-migration`. PC-B: `lane:ui-vue`, `lane:lyrics`,
  `lane:ci-linux`. Un issue se despacha en la máquina de su carril.
- **Locks exclusivos por ola.** `lock:sql-migration`, `lock:ipc-surface`,
  `lock:service-rs` y `lock:orchestrator-rs`: como mucho **un issue activo
  por lock** a la vez. Un issue con lock ocupado espera aunque su carril esté
  libre.
- **Prioridad.** `prio:P0` aceleradores de CI › `prio:P1` desbloqueadores de
  fan-out y del gate beta #264 › `prio:P2` hojas aisladas › `prio:P3`
  integración pesada.
- **Protocolo de despacho.**
  1. Reclamo atómico antes de crear el worktree:
     `gh issue edit <N> --add-label in-progress`. Un issue ya marcado
     `in-progress` no se toma.
  2. Worktrees nuevos en una carpeta limpia; los ~24 worktrees viejos de
     `~/.t3/worktrees/Syncify/` quedan intactos.
  3. Entorno por carril: `CARGO_TARGET_DIR` persistente
     `~/.cache/syncify-target-lane-<carril>`, reutilizado entre issues del
     mismo carril (nunca `/tmp`), y
     `PKG_CONFIG_PATH=/usr/lib/openssl-3/pkgconfig`.
  4. Micro-commits con push por hito: caracterización → núcleo → batería
     verde.
  5. Ningún PR entra a la rama del lote sin la batería completa en local y
     una code-review con artefacto (`gh pr review --comment`).
  6. Con 4–6 PRs verdes en el lote (o un hito clave), PR consolidado a
     `main`, y se arranca el lote siguiente **sin esperar** al CI de
     Windows del consolidado.
  7. Cada issue del lote lleva un comentario «Contrato de cola».
- **Flujo de integración** del lote y protección de `main`:
  [`protocolo-pr.md`](protocolo-pr.md) §«Flujo por lotes».

## Histórico

Las entradas fechadas del 2026-11-13 al 2026-11-19 (decisiones del
propietario, capacidad medida, fallos de delegación y progreso de carriles) se
conservan completas en
[`docs/historico/wiki-modo-ejecucion.md`](../../historico/wiki-modo-ejecucion.md).

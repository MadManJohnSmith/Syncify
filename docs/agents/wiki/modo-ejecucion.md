# Modo de ejecución

Cómo se ejecuta el trabajo en este proyecto. Decisión del propietario, no
preferencia del agente. Relacionadas: [`entorno.md`](entorno.md) y
[`protocolo-pr.md`](protocolo-pr.md).

## Reglas vigentes

**Actualizado 2026-11-19.** El estado canónico es el tracker de GitHub y el
handoff del día; esta página solo fija lo que no debe re-litigarse.

- **Modo por issue con subagentes.** Cada issue viaja en su carril con un
  subagente dedicado; el jefe de gabinete orquesta, no implementa inline salvo
  para rescatar un WIP.
- **Cupo real.** 2 carriles GLM locales + 1 remoto = 3 GLM, más 1 deepseek
  local. El cuarto GLM muere con `3008`: la cola rota, no se fuerza.
- **Remotos aparcados (2026-11-19).** agentvm1 no entregó ningún carril
  (`Internal error` en los tres); la cola es local hasta nuevo aviso.
- **Reparto de superficies.** Los carriles simultáneos no comparten fichero;
  si lo necesitan, se serializan o usan un worktree aislado.
- **Target por checkout.** `CARGO_TARGET_DIR=$HOME/.cache/syncify-target-$(basename "$PWD")`,
  fuera de tmpfs; nunca el `/tmp/syncify-target` compartido.
- **Contrato de entrega.** Commits + PR con base `dev` + puertas corridas con
  salida verbatim. Un «completed» sin commits y PR es un fallo del encargo.
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
`isolation: worktree`. Los PRs de carril van contra `wave/ola-0-batch-<n>` y
el lote cierra con un PR consolidado a `main`; cada PR de carril lleva
revisión delegada independiente con artefacto `gh pr review --comment` antes
de integrarse. Anti-3008: commit + push en cada micro-hito (caracterización,
núcleo, puertas). Fuente: handoff del despacho del Lote 2 en
`~/.local/state/syncify/`.

## Histórico

Las entradas fechadas del 2026-11-13 al 2026-11-19 (decisiones del
propietario, capacidad medida, fallos de delegación y progreso de carriles) se
conservan completas en
[`docs/historico/wiki-modo-ejecucion.md`](../../historico/wiki-modo-ejecucion.md).

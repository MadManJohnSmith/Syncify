# Entorno de esta máquina

Gotchas medidos, con la evidencia que los sostiene. El mapa vivo del
entorno es `AGENTS.md` (comandos, target fuera del repo, entorno inmutable);
aquí solo queda lo aprendido con fecha. Relacionadas:
[`modo-ejecucion.md`](modo-ejecucion.md) y [`protocolo-pr.md`](protocolo-pr.md).

## Cargo fuera del hook pre-push

- **2026-11-13**: `PKG_CONFIG_PATH=/usr/lib/openssl-3/pkgconfig` sigue siendo
  obligatorio para cualquier cargo fuera del hook pre-push. Las
  exportaciones específicas de máquina no van en `tools/prepush-check.sh`.
  Fuente: `docs/historico/HANDOFF-2026-11-13.md` §Gotcha de entorno.

## `CARGO_TARGET_DIR` compartido entre checkouts

- **2026-11-13, medido**: los binarios de test hornean `CARGO_MANIFEST_DIR`
  en compilación. Tras borrar un worktree, los binarios reutilizados desde
  el target compartido fallan con `NotFound` en fixtures.
  `crates/syncify-fmp4-flac/tests/fmp4_flac.rs`: 2 fallos así, verificados
  con `strings` sobre el binario y resueltos con `touch` + rebuild (2/2
  ok). En CI (checkout fresco) no ocurre. Contramedidas: reconstruir los
  binarios afectados tras mover o borrar un checkout que compartió target,
  o target privado por checkout. Fuente: handoff del 2026-11-13 §Gotcha de
  entorno.

## tmpfs 12G y targets privados en paralelo

- **2026-11-16, medido**: con dos carriles en paralelo con target privado
  por delegado, tmpfs (12G) se llenó a mitad de `cargo test` (error 28, `No
  space left on device`); el target privado del carril D-06c llegó a 8,5G.
  Contramedidas aplicadas: (1) cuando dos carriles gatean a la vez, el
  segundo usa target privado **fuera de tmpfs**
  (`/home/alan/.cache/syncify-target-<carril>`; 509G libres en `/home`);
  (2) liberar el target privado de un carril que ya gateó verde (sus
  binarios además hornean rutas de un checkout que muere con el merge);
  (3) el delegado D-06c liberó `/tmp/syncify-target` compartido (inactivo,
  regenerable por diseño) para seguir gateando: la primera puerta con el
  default recompilará. Fuente: informe del delegado D-06c (PR #296,
  `deb5183`) y `df` medido en la sesión del 2026-11-16.

## Lock del target y procesos huérfanos

- **2026-11-13**: los procesos huérfanos de un run terminado pueden
  bloquear el lock de `/tmp/syncify-target` (clippy/check vivos contra el
  target compartido tras el fin del turno). Contramedida al retomar un
  carril: `pgrep cargo` antes de gatear; los briefs de delegación deberían
  usar target privado por worktree cuando el padre puede gatear a la vez.
  Fuente: retro del 2026-11-13, actualización del mismo día.

## OOM con dos suites cargo simultáneas

- **2026-11-17, medido**: con dos carriles locales gateando a la vez
  (cargo test de uno + clippy de otro), el kernel mató a un delegado con
  «Killed: out of memory» a mitad de gate; el WIP quedó íntegro sin
  commit y el carril se relanzó retomándolo. Contramedida estructural
  desde entonces: toda puerta cargo de carril local va envuelta en
  exclusión mutua — `flock -w 3600 /home/alan/.cache/syncify-gate.lock
  <comando>` — y el brief prohíbe correr dos comandos cargo pesados en
  paralelo dentro del mismo carril. Fuente: muerte del delegado A86
  (2026-11-17) con el cargo test del carril L03 aún vivo.
- **2026-11-17, segunda muerte misma noche**: el flock NO protege contra
  un carril cuyo brief es anterior a la regla (L03 gatea sin lock) —
  A86 murió OOM de nuevo por clippy mientras el cargo test de L03
  seguía; la máquina estaba presionada (swap 16G/23G usada, 23G RAM).
  Contramedida endurecida en los briefs: espera activa ANTES de cada
  comando cargo — bucle `pgrep -x cargo && sleep 300` hasta que no
  quede ningún cargo en la máquina (máx ~50 min), además del flock.
  Fuente: dos muertes OOM del carril A86 en la sesión del 2026-11-17;
  `free -h` y `pgrep -a cargo` medidos.

## Symlinks de entorno y staging

- **2026-11-13, casi-error capturado antes de empujar**: los patrones de
  `.gitignore` con barra final ignoran directorios, no symlinks;
  `git add -A` en un worktree con `.venv` / `ui/node_modules` enlazados
  commiteó los symlinks. Corregido con `git rm --cached` + amend. Regla:
  tras enlazar entorno en un worktree, staging explícito por fichero,
  nunca `-A`. Fuente: retro del 2026-11-13.

## Worktrees de delegado sin entorno

- **2026-11-13**: el worktree de un delegado nace sin `ui/node_modules` ni
  `.venv` (gitignored, sin script de setup). Enlazar los del checkout padre
  funciona cuando el PR no toca lockfiles (verificado en PR #283).
  Candidato a estándar: enlazar o instalar el entorno en la preparación del
  worktree. Fuente: retro del 2026-11-13.

## `/tmp` es efímero

- Regla global del propietario (vigente el 2026-11-13): handoffs y
  artefactos de sesión a `~/.local/state/<proyecto>/`; `/tmp` se pierde con
  cada reinicio. El target de Rust vive en `/tmp/syncify-target` por
  decisión de `AGENTS.md` (llenar el disco del repo es el mal menor
  aceptado): tras un reinicio, la primera puerta recompila.

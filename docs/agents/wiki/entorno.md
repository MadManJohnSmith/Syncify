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

## Lock del target y procesos huérfanos

- **2026-11-13**: los procesos huérfanos de un run terminado pueden
  bloquear el lock de `/tmp/syncify-target` (clippy/check vivos contra el
  target compartido tras el fin del turno). Contramedida al retomar un
  carril: `pgrep cargo` antes de gatear; los briefs de delegación deberían
  usar target privado por worktree cuando el padre puede gatear a la vez.
  Fuente: retro del 2026-11-13, actualización del mismo día.

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

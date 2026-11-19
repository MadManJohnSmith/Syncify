# Modo de ejecución — histórico

Contenido fechado de [`docs/agents/wiki/modo-ejecucion.md`](../agents/wiki/modo-ejecucion.md)
hasta el 2026-11-19. Se conserva completo como evidencia; las reglas
vigentes viven en la página de la wiki y el estado canónico sigue en el
tracker y en los handoffs.

---

# Modo de ejecución

Cómo se ejecuta el trabajo en este proyecto. Decisión del propietario, no
preferencia del agente. Relacionadas: [`entorno.md`](../agents/wiki/entorno.md)
y [`protocolo-pr.md`](../agents/wiki/protocolo-pr.md).

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
- **2026-11-15/16, tres fallos nuevos de infraestructura de delegación**:
  (1) un delegado murió por cuota (`1005 exceed quota limit`) dejando WIP
  sin commit — el reintento se lanzó limpio, sin reutilizar el WIP;
  (2) tras una recuperación de sesión del harness, el id de modelo
  heredado cambió de `builtin:zai-start-plan\GLM-5.3-Flash` a
  `account:zai-start-plan\GLM-5.3-Flash` y el agente rechazó el segundo;
  el catálogo (`orchestrator_capabilities`) ya no anuncia el `builtin:`;
  (3) el modelo hermano `account:zai-start-plan\GLM-5.2` también fue
  rechazado con el mismo error — dos rechazos seguidos apuntan a la
  cuenta del proveedor, no al modelo. Mitigación aplicada: la del
  handoff («mismo encargo con otro proveedor») — Codex `gpt-6.1-sol`
  (evitando `gpt-6-luna`, el fallido del 2026-11-13), esfuerzo alto.
  Ojo al precedente «completado sin entregar» de Codex: verificar el
  estado real al despertar (`git log dev..rama`, `gh pr list`).
  Fuente: fallos de los delegados SB05 #201 y sus reintentos en la
  sesión del 2026-11-14/16.

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
- **2026-11-14/15**: SB04 (#200) fusionado por squash en PR #292
  (`ea9dcb6` en `dev`); también fusionados D-06a (#37, PR #293
  `52fdd44`) y D-08a (#40, PR #294 `261a640`). Fuente: `git log dev`.
- **2026-11-16**: D-06b (#38) fusionado por squash en PR #295
  (`b8b949c` en `dev`, checks Rust ubuntu 30m15s / windows 33m20s
  verdes); issue #38 cerrado manualmente citando el SHA. El programa
  #32 queda con sub-issues 3/5 (pendientes D-06c #39 y D-08b #41).
  Fuente: PR #295, tracker.
- **2026-11-16, decisión de rescate de WIP**: los tres reintentos de
  SB05 del 2026-11-15 murieron sin commits (cuota y rechazos de
  modelo), pero el worktree del primer delegado conservaba ~848
  líneas sin commit de implementación de #201. La decisión anterior
  («reintento limpio, sin reutilizar el WIP») quedó sin efecto al
  morir también sus reintentos: el WIP se audita y reutiliza como
  punto de partida (commits + rebase sobre `b8b949c`, con conflicto
  seguro porque D-06b tocó `commands/backup.rs`). Lección: antes de
  descartar WIP de un delegado muerto, medir su tamaño y calidad; la
  regla «sin reutilizar» era para un reintento inmediato, no para un
  carril tras tres muertes seguidas. Fuente: diff del worktree
  t3-4a866099, `git log dev`.
- **2026-11-16, decisión del propietario sobre el modelo de delegación**:
  los delegados usan el mismo modelo de la sesión principal
  (`builtin:zai-start-plan\GLM-5.3-Flash`) con esfuerzo `high` en vez de
  `max`; suplanta la mitigación del 2026-11-15/16 (Codex `gpt-6.1-sol`)
  como política vigente. El primer lanzamiento del día con Codex se
  canceló por esta orden. El proveedor aceptó el id `builtin:` sin el
  rechazo del 15/16. Fuente: orden del propietario en la sesión del
  2026-11-16.
- **2026-11-16**: sesión con dos carriles en paralelo por orden del
  propietario (máx. 2 subagentes simultáneos, paralelización
  prioritaria): SB05 #201 (retomando el WIP en
  `feat/sb05-manifest-v2-import`) y D-06c #39 (worktree privado
  nuevo). Ambos con el modelo fijado ese día y target privado por
  carril (`/tmp/syncify-target-sb05`, `/tmp/syncify-target-d06c`).
  Fuente: orden del propietario en la sesión del 2026-11-16.
- **2026-11-18, cambio de modelo de delegación (vigente)**: los
  subagentes pasan a `opencode-go/deepseek-v4.1-flash` (proveedor
  OpenCode). Suplanta la decisión del 2026-11-16
  (`builtin:zai-start-plan\GLM-5.3-Flash`): esa madrugada el proveedor
  ZCode agotó cuota (`1005 exceed quota limit`) y murieron los tres
  delegados en vuelo — L04 llegó a PR #313, P01 y #87 dejaron commits o
  WIP en sus worktrees. Fuente: orden del propietario en la sesión del
  2026-11-18 y estado de los hilos de delegación.
- **2026-11-18**: L04 (#101) fusionado por squash en PR #313
  (`e27cfaf`); issue cerrado citando el SHA. P01 (#132) abrió PR #314,
  rebasado sobre `dev` y con claims re-derivados (338 objetivos cargo;
  140 ficheros / 1199 tests UI). #87 quedó con la rama remota
  `feat/87-profile-restoration` (WIP + fix del claim; suite reauth 5/5)
  a la espera de gates y PR. Fuente: `gh`, `git log`.
- **2026-11-16**: D-06c (#39) fusionado por squash en PR #296
  (`b27b611` en `dev`, CI 8/8); issue cerrado citando el SHA. El
  programa #32 queda 4/5. Fuente: PR #296, tracker.
- **2026-11-16**: SB05 (#201) fusionado por squash en PR #297
  (`ee5b3fd` en `dev`, CI 8/8); issue cerrado citando el SHA. El
  delegado rescató el WIP, lo rebasó limpio y corrigió un defecto real
  de dominio: el artista centinela `'Unknown Artist'` del export se
  trata como «sin artista» (espejo del trigger `0079`) — antes
  cualquier manifest con álbum sin artista era irrestaurable. El
  programa #195 queda con solo SB06 #202 pendiente, lanzado ese mismo
  día como segundo carril junto a D-08b #41. Fuente: PR #297, tracker.
- **2026-11-16**: D-08b (#41) fusionado por squash en PR #298
  (`56f3154` en `dev`, CI 8/8); issue cerrado citando el SHA y el spec
  #32 cerrado 5/5 (programa de deuda accionable terminado). La
  costura de entorno `SYNCIFY_QOBUZ_API_BASE` en `services/qobuz.rs`
  siguió el molde documentado `SYNCIFY_DEEZER_API_BASE`; el test
  objetivo se verificó en ejecución real (3 passed en el target del
  delegado). Fuente: PR #298, tracker.
- **2026-11-16**: SB06 (#202) fusionado por squash en PR #299
  (`9526aa6` en `dev`, CI 8/8); issues #202 y el spec padre #195
  cerrados citando el SHA. Programa AJUSTES/BACKUP completo (6/6).
  Fuente: PR #299, tracker.
- **2026-11-16, decisión de serialización**: el spec siguiente #96
  (biblioteca agregada, 14 sub-issues #97-#110) toca ui/src, ficheros
  que el carril en vuelo FE-PERF-01 #49 también edita: por el
  contrato del handoff se serializa — #96 no arranca hasta cerrar
  #49, y dentro de #34 los hermanos #50/#51 (desbloqueados por #49)
  irán en paralelo solo si no comparten fichero. Fuente: orden del
  propietario de paralelización con máx. 2 + reparto de superficies
  del handoff del 2026-11-14.
- **2026-11-16**: FE-PERF-01 (#49) fusionado por squash en PR #300
  (`afd087d` en `dev`, CI 8/8); issue cerrado citando el SHA. El
  programa #34 queda 1/3. Lectura honesta de la medición registrada en
  el PR: con BD vacía y `out-in` conservado el tiempo pared no baja
  (~300 ms); lo verificado por mecanismo es el fin del remontaje
  (47 mount → 39 cached) y de las llamadas repetidas. La sonda
  `navigationProbe` y el harness `tools/nav-perf.sh` quedan en el
  árbol para los hermanos #50/#51 (el plugin de vite.config es
  dev-only, no reparto de chunks). Fuente: PR #300, tracker.
- **2026-11-16**: investigación del spec #96 completada por delegación
  (solo lectura); informe versionado en
  `docs/historico/investigacion-biblioteca-96-2026-11-16.md` como
  referencia de los 14 carriles (L00-L13, #97-#110). Tres decisiones
  del propietario de ese día: (1) retirar por completo
  `auto_resolve_duplicates` al integrar #107; (2) pertenencia de #97
  como tabla nueva + nota en el glosario (delegada en el agente, sin
  usuarios existentes que proteger); (3) la serie arranca con #98
  (L01) contra fixtures en cuanto el programa #34 cierre, sin esperar
  a #64/#65-#68. FE-PERF-03 #50 y FE-PERF-04 #51 lanzados en
  paralelo ese mismo día (worktree privado cada uno, zonas excluidas
  cruzadas en los briefs). Fuente: informe versionado, orden del
  propietario en la sesión.
- **2026-11-16**: FE-PERF-03 (#50) fusionado por squash en PR #302
  (`8c59672` en `dev`, CI 8/8); issue cerrado citando el SHA. El
  programa #34 queda 2/3 (solo #51, PR #301 en CI). El triaje de
  Q-13/Q-14 quedó resuelto: los issues #265/#266 (que ya existían como
  tickets del checklist #264) se enlazaron como sub-issues de #96 con
  comentarios cruzados en #97/#110; la afirmación del informe de
  investigación de que «no eran issues de GitHub» quedó corregida con
  nota de mismo día en el informe versionado. Fuente: PR #302, tracker.
- **2026-11-16**: FE-PERF-04 (#51) fusionado por squash en PR #301
  (`0684c43` en `dev`, CI 8/8); issues #51 y el spec padre #34
  cerrados citando los tres SHAs (afd087d / 8c59672 / 0684c43).
  Programa de rendimiento de UI completo 3/3. La sonda
  navigationProbe y tools/nav-perf.sh quedan como infraestructura
  reutilizable. Con el cierre de #34 arrancó la serie #96 según la
  decisión del propietario: L01 #98 delegado contra el esquema actual
  (módulo propio extraído de library.rs como pieza habilitante del
  paralelismo posterior; pertenencia de L00 detrás de un borde claro;
  piezas que la requieren deshabilitadas con motivo). Fuente: PR
  #301, tracker.
- **2026-11-16**: L01 (#98) fusionado por squash en PR #303
  (`d317c01` en `dev`, CI 8/8); issue cerrado citando el SHA. Serie
  #96: 1/14. Con el carril liberado por la orden del propietario
  («siempre 2 subagentes») se adelantó el raíz del programa #64
  (SyncRun #65) y el del #83 (login webview #84) a la cola de
  carriles.
- **2026-11-16, rebase de carril por el orquestador**: el PR #304
  (SyncRun #65) quedó CONFLICTING al abrirse antes del merge de #303
  (ambos tocaban el registro de AGENTS.md). El jefe de gabinete hizo
  el rebase mecánico directamente en el worktree del delegado
  (conflicto único de claims: 325 vs 327 desde base 324 → resuelto a
  328; el código de ambos carriles se combinó sin conflicto) y
  force-push con lease. La nueva head es `8706f91`; CI re-corre.
  Lección: el rebase mecánico de un carril terminado es gestión de
  carril, no implementación — no requiere delegado si el conflicto es
  de una línea. Fuente: git log de la rama rebasada.
- **2026-11-17**: SyncRun (#65) fusionado por squash en PR #304
  (`3774eae` en `dev`, CI 8/8 tras el rebase de claims del 16);
  issue cerrado citando el SHA. Programa #64: 1/18.
- **2026-11-17, cuota GLM agotada de nuevo**: los dos carriles en
  vuelo (L02 #99 y A84 #84) murieron con `1005 exceed quota limit`
  el mismo turno — el patrón del 15/16 (dos fallos seguidos apuntan
  a la cuenta del proveedor, no al modelo). Mitigación aplicada otra
  vez: relanzados con Codex `gpt-6.1-sol`, esfuerzo alto, RETOMANDO
  la inspección pagada — L02 con su WIP sin commit (migración
  0107_library_fts_v2.sql + integración) y A84 con su implementación
  ya commiteada (2 commits, 2.088 líneas; murió solo en claims,
  puertas y PR). La orden del propietario de usar el modelo de la
  sesión (2026-11-16) queda suplantada mientras la cuota siga
  agotada; volver a GLM cuando el propietario lo indique o la cuota
  se recupere. Fuente: fallos 1005 de los delegados en la sesión del
  2026-11-17.
- **2026-11-17**: SyncRun (#65) fusionado por squash en PR #304
  (`3774eae` en `dev`, CI 8/8 tras el rebase de claims del 16);
  issue cerrado citando el SHA. Programa #64: 1/18.
- **2026-11-17, cuota GLM agotada de nuevo y orden reiterada**: los
  dos carriles en vuelo (L02 #99 y A84 #84) murieron con `1005
  exceed quota limit` el mismo turno — patrón del 15/16. El
  orquestador aplicó la mitigación cross-provider de la wiki (Codex
  `gpt-6.1-sol`), y el propietario la revocó en el mismo momento:
  «los subagentes usarán el mismo modelo que la sesión padre, o sea
  tú, pero en high» — la orden del 2026-11-16 queda reiterada como
  política aunque la cuota falle; los Codex lanzados se cancelaron
  sin zombis y ambos carriles se relanzaron con GLM-5.3-Flash
  thought high, RETOMANDO la inspección pagada (L02: WIP sin commit
  con migración 0107_library_fts_v2.sql; A84: implementación ya
  commiteada en 2 commits, 2.088 líneas — murió solo en claims,
  puertas y PR). La mitigación cross-provider queda disponible pero
  NO se aplica sin orden explícita del propietario. Fuente: fallos
  1005 y orden del propietario en la sesión del 2026-11-17.
- **2026-11-17, run «completed» sin informe y sin PR (D06 #118)**:
  el delegado de D06 en agentvm1 terminó con estado `completed`
  pero su última salida era un mensaje de progreso truncado
  («Ahora el nuevo módulo de identidad de fichero en download/:») a
  mitad de edición, 0 PRs y sin rama creada: el run cortó sin
  entregarse.
  Lección: `completed` del runner no es entrega — el informe final
  con PR es el criterio; leer el contenido del hilo antes de tratar
  una terminación como éxito. Rescate: relanzado retomando el WIP
  sin commit del checkout compartido (mismo patrón que P00/L03: la
  inspección pagada se recupera). Fuente: lectura del hilo del
  delegado D06 (agentvm1) en la sesión del 2026-11-17.
- **2026-11-17**: D07 (#119, contrato IPC de jobs) fusionado por
  squash en PR #307 (`a6f4778` en `dev`, CI 8/8); issue cerrado
  citando el SHA; unwatch y limpieza de carril (worktree, rama,
  target privado). Su cuerpo documentó el patch de 3 líneas de
  `worker.rs` que D08 (#120) debe aplicar al partir. Fuente:
  `gh pr view 307` y `gh issue close 119` en la sesión del
  2026-11-17.
- **2026-11-17, 3008 con 5 carriles llenos**: el relanzamiento de A86
  (3er intento, esperando bloqueante el flock del carril L03) murió
  con `3008 user concurrency limit exceeded` — el límite de turns
  simultáneos de la cuenta GLM se dispara con 5 delegados + hilo
  padre (ya pasó con P00 y L03 el mismo día). Política aplicada:
  NO relanzar de inmediato (sería otro 3008); relanzar al liberarse
  una ranura (el fin de L03 la libera) y mantener 4 delegados hasta
  que el carril recuperado entregue. El trabajo de A86 sigue
  blindado en 5 commits (`dc9af8e`), solo faltan gates, rebase y PR.
  Fuente: fallo del delegado a86-retry-2 y diagnóstico de carriles
  en la sesión del 2026-11-17.
- **2026-11-17**: P00 (#131, cola persistente en SQLite) fusionado
  por squash en PR #309 (`5707d00` en `dev`, CI 8/8 tras el
  re-run del flaky de `completed_download_primes_at_eof`) y PL00
  (#153, modelo de playlist local) en PR #310 (`6719409`, CI 8/8
  tras rebase + resolución de claims: el fichero nuevo de suite de
  migración sube los objetivos a 335, medidos por la puerta; el
  flaky de red de lyrics lrclib re-corrido). Issues cerrados con
  SHA; carriles limpiados (worktrees, ramas, targets privados).
  Programas: #130 1/18, #152 1/11. La lección del claim mezclado
  (334 de dev vs 335 del árbol combinado) confirma que la puerta
  mide, no la aritmética manual. Fuente: `gh pr merge`/`gh issue
  close` de #309 y #310 en la sesión del 2026-11-17.
- **2026-11-17, cortes de run en agentvm1 y checkout compartido
  inviable para el mismo módulo**: D06 (×2) y D05 (×2) murieron con
  estado `completed` y último mensaje de PROGRESO (sin informe ni
  PR). Evidencia: el run de D06-retry-2 quedó EN COLA 73 min por la
  concurrencia de cuenta (pedido 19:46, arrancó 20:59) y cortó a
  los ~30 min clasificando hunks. El checkout compartido de
  agentvm1 entrelaza los WIP de carriles que tocan el mismo módulo
  (D05/D06 ambos en download/, hunks mixtos en worker.rs) y el WIP
  roto de un hermano (lyrics.rs de LY01 a mitad) rompe las puertas
  de los demás. Decisión: el D-programa pasa a la cola LOCAL con
  worktree aislado (donde L03/A86/P00/PL00 avanzaron sin
  incidentes); agentvm1 como máximo un carril del mismo módulo a la
  vez. Fuente: lecturas de hilos d06-retry-2 y d05-relanzado en la
  sesión del 2026-11-17.
- **2026-11-17**: L03 (#100, cabecera/URL/pestañas de biblioteca)
  fusionado por squash en PR #311 (`916f811` en `dev`, CI 8/8, 140
  ficheros / 1200 tests UI); issue cerrado citando el SHA; carril
  limpiado. Programa #96: 3/14. A86 entregó PR #312 (head
  `4a2ddd7`, 6 commits, rebase sobre P00+PL00 con claims a 336
  objetivos medidos; clippy destapó 4 errores del WIP rescatado y
  se arreglaron en commit nuevo; `#[expect(dead_code)]` NO funciona
  para API pendiente de consumo futura: el lint no dispara en el
  objetivo lib — quedó `allow(dead_code)` documentado, no comando).
  El WIP entrelazado de D05/D06 se exporta a ramas wip/* con
  clasificación solo por ficheros (sin hunks) para retomarlo en
  carriles locales aislados. Fuente: informes de l03-retry-3 y
  a86-retry-3, `gh pr merge 311` en la sesión del 2026-11-17.
- **2026-11-18, once fusiones y tres límites de infraestructura medidos**:
  L04 #101 (#313), P01 #132 (#314), Packaging Windows #246 (#315), Toolchain
  #249 (#319), D08 #120 (#318), LY02 #183 (#316), P02 #133 (#317), A02 #93
  (#321), Packaging Linux #245 (#320), PL01 #154 (#322) y Portable Windows
  #247 (#324). L01 #97 quedó en PR #323. Fuente: `git log dev` y merges de
  la sesión del 2026-11-18.
- **2026-11-18, ZCode limita a UNA sesión concurrente**: `3008 user
  concurrency limit exceeded` no es solo «no background»: al relanzar #165
  mientras #247 estaba en vuelo, el proveedor mató #247 a mitad de clippy.
  Regla: un solo carril Flash a la vez; el siguiente entra cuando el
  anterior cierra. Fuente: fallos de #165-retry y #247 en la sesión.
- **2026-11-18, agentvm1 sigue sin ZCode autenticado**: sesiones nuevas y
  reenvíos con `mode: auto` a la misma sesión fallan con
  `provider_not_configured · Provider authentication failed`; solo
  `acpRegistry_zcode` figura habilitado allí y `mode: restart` exige un turno
  vivo. `delegate_task` con `isolation: worktree` se rechaza en entornos
  vinculados (`invalid_request`): corren en el checkout del proyecto. Fuente:
  reintentos de #102/#165/#247 y `orchestrator_capabilities` del 2026-11-18.
- **2026-11-18, `/tmp` es un tmpfs de 12 GB y el target compartido lo llenó
  al 100%**: `cargo test` y el smoke de recuperación murieron con ENOSPC
  (`StorageFull`); varios delegados cortaron a la vez. Mitigación aplicada:
  `CARGO_TARGET_DIR` por carril bajo `~/.cache` y `TMPDIR` en disco. Fuente:
  log del job de #183 y del delegado de #249.
- **2026-11-18, worktree nuevo sin `ui/dist`**: `tauri::generate_context!`
  panickea (`proc macro panicked` en `main.rs:1386`) y parece un fallo de
  clippy del binario; se resuelve con `npm --prefix ui run build` antes de
  las puertas. Fuente: push fallido de #247 y su rescate.
- **2026-11-18, flake de L04 destapado por la suite completa**: el fixture de
  `library_track_detail_test` compartía un fichero temporal entre tests en
  paralelo; corregido con contador atómico en el nombre. Fuente: job Rust de
  #316 y commit `6910d43`.
- **2026-11-18, dead_code de Windows por código Linux-only**: gatear MPRIS
  obliga a gatear también `player_control` y la cadena de transporte manual
  (`run_manual_*`, `perform_set_shuffle`, métodos de `PlayerState`); `-D
  warnings` en Windows los marca uno a uno. Fuente: jobs Windows de #317 y
  commits `9d3988c`/`807cac3`.
- **2026-11-18/19, segunda tanda fusionada**: A03 #89 (#326), M00 #165
  (#325), A05 #91 (#327), P04 #135 (#328), D04 #116 (#329), L05 #102
  (#330) y D09 #121 (#331); L07 #104 quedó en PR #332. Fuente: `git log
  dev` y merges de la sesión.
- **2026-11-18, un solo hijo vivo a la vez con deepseek**: con varios
  carriles abiertos, cada notificación de fin de uno interrumpe el turno
  del padre y tumba a los demás (cascada de `interrupted`). Con un único
  hijo y sin PRs abiertos, el carril llega a entregar. Regla operativa:
  rotar de uno en uno, y no lanzar el siguiente hasta cerrar el anterior.
  Fuente: interrupciones encadenadas de #102/#116/#135/#91/#121.
- **2026-11-19, capacidad de carriles ampliada (vigente)**: orden del
  propietario — hasta **dos subagentes** con
  `opencode-go/deepseek-v4.1-flash` y **dos** con
  `builtin:zai-start-plan\GLM-5.3-Flash` a la vez. Suplanta la rotación
  de un solo hijo del 2026-11-18. Condición operativa que no se relaja:
  reparto de superficies sin ficheros compartidos (los claims de
  `AGENTS.md` se resuelven al fusionar, como siempre) y vigilancia de la
  cascada de interrupciones; si reaparece, se vuelve a rotación. Fuente:
  orden del propietario en la sesión del 2026-11-19.
- **2026-11-18, WIP rescatable tras interrupción**: los hijos cortados
  suelen dejar commits o cambios sin commitear en su worktree; revisar
  `git status`/`git log` antes de relanzar. Así se terminaron #133, #120,
  #154, #247, #91, #135 y #116 sin repetir el trabajo. Fuente: rescates
  de la sesión.
- **2026-11-19**: L08 #105 fusionado por squash en PR #334 (`22d92a6`),
  issue cerrado citando el SHA. Tres commits (WIP, claims medidos y
  reintento de paginación del detalle); la revisión independiente no
  encontró bloqueantes y el pulido menor quedó en #336. Cifras medidas:
  345 objetivos cargo, 150 ficheros / 1275 tests UI, 476 Python. Antes
  se fusionó #335 (`924d169`), el repin de ffmpeg que desbloqueó el job
  Rust de Windows. Fuente: `git log dev`, PRs #334/#335.
- **2026-11-19, dependencias reales de la serie #96**: #106
  (cuarentena) no era ejecutable: `sync_quarantine` no existe en el
  árbol y #72 sigue abierto (depende de T02-T04 y #71). #108 depende de
  #77 y #109 de la serie D. Carril desbloqueado elegido para el segundo
  turno: #110 (L13), que solo depende de L01-L04. Lección: re-derivar
  el grafo del tracker antes de seguir el orden de la cola del handoff.
  Fuente: cuerpos de #106/#108/#109/#110/#72 y `grep sync_quarantine`
  del 2026-11-19.

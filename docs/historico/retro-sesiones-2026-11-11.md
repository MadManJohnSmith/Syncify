# Retro de sesiones de agente — 2026-11-11

Evidencia fechada, no estado actual. Primera retro del proyecto: revisión de
las 17 sesiones previas (~3.300 ítems; los 12 hilos grandes leídos por cuatro
lectores en primer plano, los 5 pequeños a directo). Origen: la sesión de
cierres del 10-11 acumuló once mensajes del usuario pidiendo el estado de
trabajo lanzado en segundo plano.

## El hallazgo central

Los carriles de trabajo se lanzaron como `Agent` nativo en background en vez
de `delegate_task`. Esos avisos de fin despiertan al proveedor fuera de un
turno de T3: el trabajo ocurre (el PR #270 se creó a las 22:18:01) pero no
deja rastro en el hilo ni notificación que llegue. El agente citó la regla
que lo prohibía (ítem 149 del hilo de cierres) y la violó tres veces más.
Consecuencias medidas: 11 mensajes de estado del usuario, un agente muerto en
silencio con 9 ficheros sin commit (descubierto por un `SendMessage` fallido)
y CI que quedó en verde menos de un minuto después de cerrar el turno en 4
casos. Del mismo familia: dos promesas de vigilia incumplidas (AppImage, 69
min de silencio; resultado de la CI de #252, tras tres «continue»), un
«reanudado» reportado cuando la reanudación ya había fallado, y una delegación
zombi sin cancelar 2 h 20 min.

## Qué funcionaba (contraejemplos del mismo proyecto)

- `watch_pull_request` armado en el mismo turno con el merge ya aprobado:
  tres merges autónomos sin preguntas del usuario.
- Delegaciones bloqueantes (subagente de inventario, revisores en primer
  plano): cero preguntas de estado.
- Delegado con orden explícita «no cierres tu turno, espera activamente».

## Lo aplicado

- Fuera del repo (machine-specific, misma lógica que el hook de prepush):
  hook `PreToolUse` de ZCode que bloquea `Agent` con
  `run_in_background: true`, y cinco líneas de steering en el AGENTS.md
  global (cierre de turno con vigilia verificada, no afirmar background sin
  verificar el lanzamiento, cancelar zombis al despertar, artefactos a
  `~/.local/state/<proyecto>/` en vez de `/tmp` — el reinicio del día 11
  perdió dos worktrees y los logs de tormenta).
- `scripts/tests/test_agents_claims.py`: puerta que mide el árbol contra las
  cifras del AGENTS.md. Los 318 objetivos cargo se reprodujeron por
  derivación (311 de integración + 7 paquetes con tests unitarios) antes de
  gatearlos. La cifra `~113` no se duplica: se verifica contra `BASELINE` de
  `scripts/tests/test_silent_write_ratchet.py`. La puerta nació validada: su
  primer fallo real fue el propio recuento tras el ff-only (329 sobre árbol
  viejo → 404 medido en el fresco), y re-medida otra vez a 428 tras el squash
  de #275 — el mecanismo funcionando.
- `tools/doctor.sh` + puntero en la puerta previa al push. Primera salida ya
  útil: marca el único hueco real de la máquina (pkg-config resuelve OpenSSL
  4.1.0, que `openssl-sys` rechaza).
- Issue #277: los hallazgos que son de plataforma (runs muertos por cuota sin
  aviso, wakes sin aterrizar en el hilo, zombis) para escalar o aceptar.

## Queda fuera, a propósito

- El recuento de tests de la UI (1142): vitest materializa `it.each` en
  ejecución; no hay conteo hermético que no sea correr la suite.
- El «(2 skipped)» de la suite Python: los skips son condicionales
  (skipIf/skipUnless) y dependen de la plataforma.
- Mientras tanto, la línea de recuentos del AGENTS.md seguirá chicoteando en
  cada merge de PRs con tests (el propio #275 la sube 399→423 sobre su base):
  la puerta convierte ese conflicto en un re-measure de un comando, no en un
  rebase a ciegas.

## Incertidumbres

- La actividad de los hilos grandes se leyó por muestreo (mensajes completos,
  tramos de actividad en los puntos de delegación); un uso puntual de
  herramientas se pudo escapar.
- El hook está probado por stdin (exit codes), no aún en vivo dentro de una
  sesión; la primera sesión con un `Agent` en background lo ejercitará.

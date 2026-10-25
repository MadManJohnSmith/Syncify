# Pendientes heredados de los workflows de agente

> **Estado actual (2026-10-25):** acta de cierre del backlog v0.4.1 (veredicto del propio fichero: sin trabajo abierto). No es una lista de pendientes; el trabajo vivo está en `docs/Deuda_Tecnica_y_UX.md`, las colas `docs/COLA_*.md` y `docs/PLAN_MEJORA_POST_BETA.md`.


**Cerrado el:** 2026-10-24. **Origen:** dos guiones de agente del workflow v0.4.1
que vivían en la raíz del workspace, fuera del repo, y que se retiraron en la limpieza
de esa fecha (registro recuperable en el manifiesto de papelera del workspace).

## Veredicto: no queda trabajo abierto

La rama `fix/backlog-v041` (`1e772ad`) es ancestro de `dev`
(`git merge-base --is-ancestor` lo confirma) y fue borrada del remoto junto a otras
11 ramas ya fusionadas. El backlog que describía está implementado.

Comprobación por muestreo de cinco puntos críticos:

| Punto del backlog | Esperado | Estado |
|---|---|---|
| Veto `partialCatalogServices` eliminado | sin veto en la UI de cuentas | `ui/src/views/AccountsView.vue:1525` lo documenta como eliminado |
| Logs históricos | comando `read_log_history` | `src-tauri/src/logs_history.rs` |
| Bandeja → abrir ajustes | evento `tray-open-settings` | `src-tauri/src/tray.rs:794`, `ui/src/App.vue:762` |
| Arranque automático | `tauri-plugin-autostart` | `src-tauri/Cargo.toml:20` |
| Corrección de la auditoría de 73 ítems | commit de corrección | `f2bfb8a` |

## Los seis carriles (para referencia histórica)

| Carril | Título |
|---|---|
| `auth-sync` | Sync completo SoundCloud y Apple Music |
| `sistema` | Logs históricos, bandeja, arranque y pausas |
| `dialogos` | Diálogos unificados, asistente de migración y descargas |
| `navegacion` | Navegación, onboarding y barra de progreso global |
| `biblioteca` | Semántica de BD, cola y biblioteca en el backend |
| `python` | Puentes Python con credenciales y contrato reales |

## Qué no se conservó

Ninguno de los dos ficheros se conserva. Eran guiones operativos de agente (instrucciones
paso a paso para otro agente), no documentación del producto: sus resultados ya están en
`dev` y en `docs/auditoria/`. Sus nombres y rutas quedaron en el manifiesto de papelera del
workspace, desde donde son recuperables.

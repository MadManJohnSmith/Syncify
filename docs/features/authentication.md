# Deezer debug cookie probe

**Estado de revisión:** 2026-11-13 — fichero creado con AUTH-WV-01 (issue #63, PR #283), revisado contra el árbol de ese PR: el comando `debug_deezer_cookie_probe` es `#[cfg(debug_assertions)]` en `src-tauri/src/commands/auth.rs`, su invocador en debug es el botón de `ui/src/views/settings/SettingsAdvanced.vue` gated por `import.meta.env.DEV` (via wrapper en `ui/src/api/accounts.ts`). No verificado en escritorio real: la corrida con login vivo de Deezer la hace el operador (evidencia esperada del issue #63); GUI y red viva siguen sin ejercitarse en CI.

## Deezer debug cookie probe

In a development build, open Settings → Advanced → Debug and choose “Run Deezer cookie probe”. Complete Deezer login in the isolated diagnostic webview. The result is an in-app evidence report listing cookie names and HttpOnly, Secure, and SameSite flags, plus the HttpOnly status of `arl`; cookie values are never included. Closing the window early produces no report. The probe command is compiled only with Rust debug assertions and does not participate in account authentication.

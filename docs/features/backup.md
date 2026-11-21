# Backup: manifest v2 — export, import y reautenticación

> **Idioma:** español. El índice que marca su estado está en `docs/README.md`
> (versión inglesa del índice: `docs/README.en.md`).

**Estado de revisión:** 2026-11-16 — re-derivación contra el árbol al cerrar la
serie SB de ajustes/backup (cierre #202): comportamiento verificado contra
`src-tauri/src/commands/backup.rs`, `src-tauri/src/settings_defaults.rs` y la
suite `src-tauri/tests/backup_manifest_v2_test.rs` (8 tests). Entrega: SB04
#200 (PR #292, export) y SB05 #201 (PR #297, import). Decisión de diseño:
`docs/adr/0020-backup-manifest-v2.md`.

**Leer antes de modificar:** `docs/adr/0020-backup-manifest-v2.md`,
`GLOSSARY.md` §Ajustes y backup (entradas «Manifest de backup» y «Copia
pre-mutación»), `src-tauri/src/commands/backup.rs`.
**Archivos core:**
- `src-tauri/src/commands/backup.rs` (comandos `export_library`/`import_library`, manifest, sandbox y checksum)
- `src-tauri/src/settings_defaults.rs` (`SETTINGS_KV_KEYS`, catálogo del reset que acota la KV del manifest)
- `src-tauri/src/commands/settings.rs` (`is_sensitive_setting_key`, exclusión de secretos)
- `ui/src/views/settings/SettingsBackup.vue` (superficie de usuario)
- `src-tauri/tests/backup_manifest_v2_test.rs` (contrato export/import)

---

## Qué es y qué no es

El backup de usuario es **manual**: dos comandos (`export_library`,
`import_library`) expuestos en la pestaña Backup de Ajustes. No hay
programador, ni historial, ni backup automático (ADR-0020 decisión 3). La copia
pre-mutación interna (`VACUUM INTO` en
`src-tauri/src/commands/library.rs` y
`src-tauri/src/services/catalog_identity_repair.rs`) **no es backup de
usuario**: no sale del equipo, no se lista en la UI y no se ofrece como tal
(entrada «Copia pre-mutación» de `GLOSSARY.md`).

## Manifest v2 (contrato)

El manifest es JSON con checksum SHA-256 (campo `checksum_sha256`, calculado
sobre la serialización sin checksum) y `version` de formato `"2.0.0"`
(desde SB04; el v1 `"1.0.0"` sigue importándose).

**Contenido:**

1. **Biblioteca como v1:** tracks, álbumes, artistas y playlists, sin cambios
   respecto al manifest original; el audio **no** viaja en el manifest (cada
   pista conserva su flag de descarga para reproducirlo después).
2. **Ajustes (`BackupSettingsDto`):** las 11 superficies tipadas
   (`service_preferences`, `sync_settings`, `service_sync_settings`,
   `quality_preferences`, `folder_settings`, `duplicate_settings`,
   `audio_processing_settings`, `lyrics_provider_settings`, `lyrics_config`,
   `metadata_preferences`, `advanced_settings`) más la KV de ajustes de
   usuario.
3. **Metadatos de cuenta (`BackupAccountDto`):** servicio, identidad externa,
   nombre visible, email y `is_active`. La consulta de export no toca
   `credentials_json` ni columnas de autenticación.

**Exclusión de secretos (contrato testeado).** La KV solo viaja si la clave
está en el catálogo del reset (`SETTINGS_KV_KEYS` de
`src-tauri/src/settings_defaults.rs`) **y** no es sensible
(`is_sensitive_setting_key` en `src-tauri/src/commands/settings.rs`):
tokens de sesión, claves de API (Last.fm, Spotify) y credenciales de fuentes
de letras se quedan fuera. El test de contrato siembra secretos en claro y
cifrados y recorre el manifest entero; en el lado import, un manifest hostil
que traiga `session_token` o `spotify_client_secret` no los escribe.

**Versión de esquema honesta.** `schema_version` se deriva del estado real del
esquema (mayor migración aplicada, vía `detect_schema_version` en
`src-tauri/src/commands/migration.rs`), no de una constante.

## Import (restauración)

- **Fallo seguro ante versiones (SB05).** Dos puertas antes de escribir nada:
  un manifest de un major futuro (solo se soportan 1.x y 2.x,
  `MAX_SUPPORTED_MANIFEST_MAJOR`) y un manifest de esquema más nuevo que la BD
  destino se rechazan con diagnóstico claro y **cero escrituras** — ni
  biblioteca, ni ajustes, ni cuentas.
- **Ajustes por superficie:** las 11 superficies tipadas se restauran con
  upsert por su clave natural; solo se escriben columnas que la tabla destino
  conoce — una columna futura o inyectada (p. ej. un `credentials_json`
  inyectado) se descarta, no se fabrica. La KV de ajustes pasa la misma
  selección doble del export.
- **Cuentas sin sesión:** los metadatos se aplican (servicio, identidad
  externa, nombre, email); `credentials_json` queda NULL en las cuentas
  nuevas y no se toca jamás en las existentes. `ImportLibraryResult` devuelve
  `accounts_awaiting_reauth` para pedir login por cuenta al terminar; el
  resultado del import muestra el aviso en la UI. Mover de máquina reproduce
  biblioteca y ajustes; las cuentas piden reautenticar (cláusula 10 y 11 de
  ADR-0007).
- **Manifest v1 aceptado:** los campos v2 son `Option` y no viajan en
  manifests v1; el import de v1 se comporta como siempre.
- **Artista centinela:** el export fabrica el nombre `'Unknown Artist'` para
  álbumes y pistas sin artista enlazado, y la migración
  `src-tauri/migrations/0079_canonical_artists_unification.sql`
  (`trg_artists_reject_garbage_ins`) lo rechaza como basura al restaurar; el
  import trata esos nombres como «sin artista»: no crea la fila ni enlaza
  `album_artists`/`track_artists`. Sin este tratamiento, cualquier manifest
  con un álbum sin artista era irrestaurable.
- **Se conserva del v1:** transacción atómica `BEGIN IMMEDIATE` (todo o nada),
  idempotencia del reimport, checksum SHA-256, sandbox de rutas
  (`validate_safe_backup_export_path_with_bases` /
  `validate_safe_backup_import_path_with_bases`) y límite de tamaño de
  importación (`MAX_BACKUP_IMPORT_BYTES`, 64 MiB). El audio sigue fuera del
  manifest.

## Superficie de usuario

`ui/src/views/settings/SettingsBackup.vue`: export con un clic (ruta por
defecto *Syncify_Backup_TIMESTAMP.json* junto a las bases permitidas) e import
con ruta + confirmación previa; el resultado del import (`message` de
`ImportLibraryResult`) persiste en `localStorage` y avisa de los ajustes
restaurados y de las cuentas que esperan reautenticación.

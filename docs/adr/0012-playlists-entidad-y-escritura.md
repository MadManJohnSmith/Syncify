# ADR-0012 — Playlists: entidad local, fuentes remotas y escritura bidireccional

**Estado:** propuesto · **Fecha:** 2026-11-05
**Origen:** ronda de grilling del flujo PLAYLISTS.
**Spec:** #152 · **Tickets:** #153–#163.

## Contexto

Hoy `playlists.account_id` es NOT NULL y las playlists locales cuelgan de la cuenta 1
(`src-tauri/src/commands/library.rs`), con `service_playlist_id = local_<nombre>`:
renombrar cambia la identidad. `playlist_tracks` impone `UNIQUE(playlist_id, track_id)`
y no admite la misma pista dos veces.

El borrado de una cuenta hace CASCADE sobre sus playlists; la importación rehace la
pertenencia y deja posiciones del proveedor (Apple). `playlist_sources.snapshot_id`
está muerto. La gestión vive en `ui/src/views/PlaylistView.vue` y
`src-tauri/src/commands/playlists.rs`.

Solo Qobuz y Tidal tienen crear/añadir playlist en Rust; ningún proveedor tiene
update, remove ni reorder. La canonicalización y los modos de conflicto están en
#74-#76 (flujo de sync), no implementados.

## Decisión

1. **La playlist es una entidad local canónica.** `account_id` pasa a nullable; las
   locales no pertenecen a ninguna cuenta y llevan un UUID estable, así que renombrar
   no cambia su identidad. Las fuentes remotas viven en `playlist_sources`.

2. **Se permiten pistas repetidas.** Se reconstruye `playlist_tracks` con unicidad por
   `(playlist_id, position)`; la posición local es canónica 0..n-1 y la del proveedor
   queda como evidencia. Quitar una pista quita una ocurrencia.

3. **Borrado con tombstone.** Borrar una playlist importada la marca `deleted_at` para
   que el sync no la recree; el borrado local es duro tras confirmar. Al borrar una
   cuenta se borran sus playlists importadas; las locales nunca.

4. **Escritura bidireccional opt-in para Qobuz y Tidal.** Crear, añadir, actualizar,
   quitar y reordenar donde la API lo permita; si una operación no existe, se muestra
   como no disponible. El push ocurre al guardar, con cola de reintentos y estado
   visible; nunca hay escritura silenciosa.

5. **El push respeta los modos de #76.** Solo se empuja en `editable` o `gana local`;
   `gana proveedor` es solo lectura. La canonicalización y la fusión son de #74-#76.

6. **El borrado remoto no se propaga por defecto.** Una acción explícita con
   confirmación puede borrar en el proveedor. Si el proveedor hace desaparecer la
   playlist, la local queda huérfana: se conserva, se avisa y no se empuja.

7. **Publicar una playlist local.** En Qobuz/Tidal crea la remota, enlaza la fuente y
   conserva la identidad local; a partir de ahí puede ser bidireccional.

8. **El pull ocurre en el sync de cuenta y en el refresco manual.** Sin watcher
   periódico.

## Consecuencias

- La migración reconstruye `playlists` y `playlist_tracks`; no se edita ninguna
  migración aplicada.
- Depende de #74-#76 para canónicas y modos; los tickets de push quedan bloqueados.
- Qobuz/Tidal pueden no exponer update/remove/reorder; la UI lo declara.
- Las pistas repetidas pueden colapsar al empujar; se avisa.
- El export M3U conserva orden y repeticiones.
- Las smart playlists no guardan pertenencia; no les afecta el modelo de duplicados.

## Alternativas descartadas

- **Mantener las locales en la cuenta 1.** Mezcla datos de usuario con una cuenta real.
- **Mantener `UNIQUE(playlist_id, track_id)`.** Una playlist real puede repetir pista.
- **Solo importar.** El propietario pidió bidireccional.
- **Push sin los modos de #76.** Dos fuentes de verdad y conflictos silenciosos.
- **Borrar el remoto al borrar el local.** Un clic local no debe destruir catálogo.

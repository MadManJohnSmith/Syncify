# ADR-0015 — Migración de biblioteca: colecciones y destinos por capacidad

**Estado:** propuesto · **Fecha:** 2026-11-05
**Origen:** ronda de grilling del flujo MIGRACIÓN.
**Spec:** #164 · **Tickets:** #165–#174.

## Contexto

La migración actual (`src-tauri/src/commands/migration.rs`) mueve favoritos de pista y
playlists seleccionadas entre seis destinos. `MigrationOptions` fija umbral,
`skip_unmatched`, `create_playlists`, `merge_existing` y `download_matched`
(`src-tauri/src/models.rs`).

Los destinos declaran soporte de creación de playlist en código, pero no hay matriz de
capacidad por colección: `add_favorite` existe para los seis; álbumes y artistas solo
para algunos; playlists solo Qobuz/Tidal.

El informe de pruebas de 2026-10-13 (MF-BULK-15) pide alcance, confirmación, dry-run,
reanudación, idempotencia, batching y cuotas para migrar una biblioteca completa.

`ui/src/views/MigrationView.vue` tiene una ruta que fuerza `create_playlists: false`.
ADR-0003 fija la selección de edición.

## Decisión

1. **La migración cubre cuatro colecciones.** Favoritos de pista, álbumes favoritos,
   artistas seguidos y playlists. Un job puede llevar varias; cada ítem declara su tipo.

2. **Capacidad por destino y colección.** La UI solo ofrece lo que el destino declara
   por escrito; lo no soportado se muestra como no disponible, no se intenta y falla.
   Matriz inicial: Qobuz y Tidal, las cuatro; Spotify, pistas/álbumes/artistas;
   Apple Music, pistas/álbumes; Deezer y SoundCloud, solo pistas. Playlists solo Qobuz/Tidal.

3. **Matching y revisión.** Se reutiliza el matcher y ADR-0003; el umbral decide, los
   dudosos se revisan antes de empezar y el resto va automático. `skip_unmatched` y el
   match manual siguen disponibles.

4. **Playlists en destino.** Se crean nuevas por defecto y se puede fusionar por nombre;
   las pistas repetidas se envían en orden y el destino puede colapsarlas. Si el destino
   no crea playlists, la colección no se ofrece.

5. **Dry-run obligatorio y auditoría.** El preview cuenta por colección y no escribe;
   el preflight comprueba esquema, credenciales y capacidades, y el post-run deja un
   informe con creados, existentes, fallidos y omitidos.

6. **Cuentas explícitas o por defecto.** El origen y el destino se validan como pareja;
   sin cuenta de origen, la lectura es de todo el servicio.

## Consecuencias

- Nueva migración para tipos de ítem y capacidades; no se edita ninguna aplicada.
- La matriz de capacidades vive en código y tiene tests por destino.
- Las playlists con duplicados pueden perder ocurrencias en destinos que no las admiten.
- Los destinos sin álbumes/artistas no fallan en runtime: no se ofrecen.
- El preview de 30k debe ser paginado y cancelable.
- La ruta de MigrationView que fuerza `create_playlists: false` se elimina.

## Alternativas descartadas

- **Solo favoritos y playlists.** El propietario pidió álbumes y artistas.
- **Intentar todo en todos los destinos.** Fallos de capacidad disfrazados de errores.
- **Un job por colección.** Multiplica estados y cuentas.
- **Sin dry-run.** Un job de 30k sin preview es una apuesta.

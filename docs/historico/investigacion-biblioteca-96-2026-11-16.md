# Investigación — arranque del spec #96 (biblioteca agregada a escala)

Registro fechado del 2026-11-16: informe de la delegación de
investigación del spec #96 y decisiones del propietario sobre las tres
preguntas que elevó. Evidencia de planificación, **no** estado actual:
el árbol que cita es `a99ffd6` (pre FE-PERF-03/04); re-derivar los
punteros antes de usarlos. El informe es la referencia de los 14
carriles de la serie (#97-#110, L00-L13).

Decisiones del propietario (2026-11-16, sesión del jefe de gabinete):

1. **Fusión automática de duplicados (`auto_resolve_duplicates`)**:
   retirar por completo al integrar #107 (L10) — el comando y sus dos
   llamadas UI desaparecen; la fusión manual con registro y deshacer
   de #107 cubre el caso.
2. **Pertenencia de #97 (L00)**: decisión delegada en el agente («lo
   que sea mejor, no hay que preocuparse por usuarios existentes»).
   Resuelta: **tabla nueva + nota en GLOSSARY.md** — `library_items`
   (0067) es infraestructura del flujo de migración mbdump con
   semántica propia («mejor fuente» por release); mezclarla con la
   pertenencia runtime acoplaría dos dominios.
3. **Calendario**: la serie #96 arranca con **#98 (L01) contra el
   fixture determinista de #97** en cuanto el programa #34 cierre,
   sin esperar a #64/#65-#68 (el spec ya prevé partes deshabilitadas
   con su motivo). Reordena en la práctica el orden #96→#64 del
   handoff del 2026-11-14.

---

# Informe de investigación — arranque del spec #96 (biblioteca agregada a escala)

Árbol verificado: `HEAD == origin/dev == a99ffd6`. Sin ficheros creados; solo lectura.

## 1. Extracción de los 14 sub-issues

Numeración interna usada por los propios issues: **L00=#97 … L13=#110** (confirmada por las notas cruzadas: #99 cita "L13" para la caché, #109 cita "L07" para cobertura de álbum).

| # | Rol | Entrega | Dependencias explícitas | Superficie estimada |
|---|---|---|---|---|
| **#97 L00** | **Raíz.** Pertenencia por cuenta/track/colección + `tracks.library_added_at` materializado + índices mínimos + fixtures compartidos | Nueva tabla de pertenencia (o alineación de `library_items` de 0067), migración nueva con prueba de upgrade | Blocked by **#65-#68** (adaptadores de sync); #64 contrato; #83/#88 base | `src-tauri/migrations/` nueva, `src-tauri/src/commands/library.rs`, importer/sync (`service.rs`), `GLOSSARY.md` |
| **#98 L01** | Consulta `query_library(query)` — una fila por `tracks.id`, filtros AND/OR, contadores, orden en servidor, paginación offset/limit (default 100, max 200) | Función con pool + wrapper fino; retira `get_library`/`get_favorite_tracks`/`get_duplicate_tracks` de la vista | L00, #88 | `library.rs` (extraer a módulo propio), `ui/src/api/library.ts` |
| **#99 L02** | Búsqueda FTS ampliada (género + ISRC), bm25 ponderado, ISRC exacto, debounce 250 ms UI | Migración FTS nueva (no editar 0019) + triggers | L01 | migración nueva, `library.rs`, `LibraryView.vue` (debounce/token) |
| **#100 L03** | Cabecera con 5 chips, estado en URL, armazón de pestañas `/library/albums|artists|duplicates` | Solo armazón; chips de cuarentena/duplicados deshabilitados si #72/#73 faltan | L01 | `LibraryView.vue`, `ui/src/main.ts` (rutas), `api/library.ts` |
| **#101 L04** | Panel lateral de detalle; `get_library_track_detail` en una llamada; deep link `?track=<id>` | Comando nuevo + panel | L01; secciones cuarentena/duplicados esperan #72/#73 | `library.rs`, `LibraryView.vue` |
| **#102 L05** | Selección 2 modos (ids / «todo el filtro» + exclusiones), vista previa, `run_library_action` en bloques de 500, eventos agregados | Runner de lotes (descarga, favorito, añadir a playlist) | L01 | `library.rs` (o módulo nuevo), `api/library.ts`, `LibraryView.vue` |
| **#103 L06** | Edición de metadatos por lotes con bloqueo manual de #80; nunca ISRC/MusicBrainz/ids proveedor | Extensión de `run_library_action` | **L05 + #80** | `library.rs` |
| **#104 L07** | Pestaña álbumes (`query_library_albums`), cobertura de descarga, detalle vía `/album/:id` | | L01 + L03 | `library.rs`, vista nueva |
| **#105 L08** | Pestaña artistas (`query_library_artists`), detalle >5 pistas, colaboraciones | `top_tracks` pasa a primera página | L01 + L03; revisar `fetch_artist_appearances` | `library.rs`, `ArtistDetailView.vue` |
| **#106 L09** | Cuarentena en la biblioteca (modos `exclude`/`only`/`include`), chip, restauración por pista | Lee `sync_quarantine` de #72, no detecta | **L01, L04, #72** | `library.rs`, panel |
| **#107 L10** | Duplicados: clústeres ISRC∪huella, diff, fusión manual + registro + deshacer; **retira la fusión automática** | Vista `/library/duplicates` + registro de fusiones nuevo | **L01, #73** | `library.rs` (sustituye a `auto_resolve_duplicates` en la ruta de biblioteca), migración nueva (registro), vista |
| **#108 L11** | Playlists y apariciones: badge «en playlist», filtro, añadir idempotente; apariciones de #77 alimentan pertenencia | | **L00, L01, #77** | `library.rs`, panel |
| **#109 L12** | Descargas y calidad: 5 estados de badge, filtro, cobertura compartida con L07, borrar audio por pista | Reutiliza la cola existente | L01 + L05 | `library.rs`, `LibraryView.vue` |
| **#110 L13** | Rendimiento: caché por clave, dedup de peticiones, invalidación por eventos, virtualización, **prueba de regresión del caso 319 llamadas** | Endurecimiento transversal | L01, L02, L03, L04 | `LibraryView.vue`, `useTrackBrowser.ts`/`useVirtualWindow.ts`, `api/library.ts` |

## 2. Mapa de superficies del árbol vigente

**Lista virtualizada y orden en el renderer (lo que L01/L13 retiran):**
- `ui/src/views/LibraryView.vue:2264-2275` — `compareTracks()`: el `switch` **no tiene caso `dateAdded`** aunque `sortBy` por defecto es `'dateAdded'` (:1258, opción :1345). Es el comparador muerto del spec.
- :2291-2351 — `filteredTracks` computed: filtros en JS (flac/mp3/downloaded/multiSource/favorites/quality) y **la búsqueda sustituye la lista** (`searchQuery.value.trim() ? searchResults.value : tracks.value`, :2292). Optimización de prefijo ordenado (:2255-2349) que L13 deja obsoleta.
- :2353-2372 — ventana virtual con kernel compartido `useVirtualWindow` (overscan, throttle 120 ms). L13 lo conserva.
- Virtualizador: `ui/src/composables/useVirtualWindow.ts`.

**Paginación y carga:**
- `ui/src/composables/useTrackBrowser.ts` — `loadLibrary`/`loadMore`/`hasMore` (:100-130); el buscador FTS del navegador (:135 `toFtsPrefixQuery`, :240-241 `loadMore` solo con query).
- Llamada al backend: `LibraryView.vue:1601` (`libraryApi.getLibrary(offset, limit)`); watcher de búsqueda con debounce propio :1720.

**Selección («seleccionar todo» roto a escala):**
- `useTrackBrowser.ts:169-171` — `selectAll` "REEMPLAZA con los ids de las filas actuales": selecciona solo la página cargada. L05 lo sustituye por el modo «todo el filtro» + exclusiones.

**Búsqueda:**
- Backend: `search_tracks` en `src-tauri/src/commands/library.rs:1651` sobre `library_fts` (FTS5, migración `0019_library_search_index.sql`:19-23, triggers :45-100 que solo cubren título/artista/álbum — sin género ni ISRC).
- En la vista, la búsqueda reemplaza la lista (ver :2292) en vez de combinarse: exactamente lo que L01/L02 corrigen.

**Cabecera/contadores:**
- No hay cabecera con total de pistas distintas ni chips; la barra actual muestra contadores de cola/sumisión (`LibraryView.vue:64-112`) y `selectedCount`.
- `get_library_stats` (`library.rs:661`) lee la vista SQL `library_stats` — el spec prohíbe alimentar la cabecera con ella.
- `get_library` cuenta `SELECT COUNT(*) FROM tracks` y devuelve **todas** las filas de `tracks`, sin pertenencia (`library.rs:387-514`; `COUNT(*)` en :400; orden fijo por título :497). `limit` máx hoy 500 (:393); L01 lo baja a 200.

**Detalle de pista / artista / álbum:**
- `get_artist` con `top_tracks LIMIT 5` (`library.rs:867-880`) y `fetch_artist_appearances` (:904). Consumidor: `ArtistDetailView.vue:160-167,223-233`. L08 lo reemplaza.
- No existe panel lateral en LibraryView ni `get_library_track_detail` — todo nuevo (L04).
- Rutas: `ui/src/main.ts:16` solo `/library`; `/album/:id` y `/artist/:id` ya existen (:35-36). `/library/albums|artists|duplicates` no existen (L03 las añade).

**Fecha añadida / duplicados / cuarentena:**
- `get_duplicate_tracks` agrupa **por título + artista primario** (`library.rs:518-546`) — el anti-patrón que L10 prohíbe.
- Fusión automática: `auto_resolve_duplicates` (`library.rs:3608`), registrado en `main.rs:1302`; además `merge_level2_3_duplicates_inner` (:3614-3615, con `#[allow(dead_code)]`). Consumidores UI: `api/library.ts:307-312` (opción `autoResolveDuplicates` del sync local, **off por defecto**, comentario explícito de que modifica toda la biblioteca) y `api/dashboard.ts:138`. L10 cambia este contrato. **Decisión del propietario 2026-11-16: retirar por completo (ver cabecera).**
- Claves de identidad para L10: `tracks.isrc` y `acoustid_fingerprint` ya existen (`0003_enhancements.sql`:9,34 — índice parcial existente sobre la huella).
- **Cuarentena: no existe nada.** Ni `sync_quarantine` ni cualquier tabla de cuarentena en migraciones; #72 está OPEN. L09 y el chip de L03 deben nacer deshabilitados.
- Espejo de migración: `library_items` un-fila-por-track con mejor fuente (`0067_migration_schema_alignment.sql`:11-32, continuado por 0086/0087) — **decisión del propietario 2026-11-16: L00 usa tabla nueva, no este espejo (ver cabecera).**
- Pertenencia hoy: `library_entries` (`0002_normalize_schema.sql`, solo favoritos) y `playlist_tracks` (`0006_playlists.sql`); la SQL de `get_library` ya lee ambas (:411-416). No hay pertenencia de álbumes ni artistas — la brecha central de L00.

**Playlists truncadas a 500:**
- `PlaylistView.vue:829-830` `PLAYLIST_PAGE_SIZE = 500` ("Backend per-page cap (`limit` is clamped to 500 in get_playlist_tracks)") y :1407. Fuera de alcance del spec; la biblioteca solo muestra pertenencia.

**Favoritos expandidos por release:**
- `get_favorite_tracks` / `FAVORITE_ROWS_CTE` (`library.rs:2088,2134-2160`) — "expanded per-release rows". L00/L01 los retiran de la vista.

**Incidente 319 llamadas:**
- Registrado en `docs/COLA_RENDIMIENTO_FRONTEND_2026-10-13.md:22` (319 llamadas a `get_library` en 4 min, una cada 0,8 s durante scroll infinito). La prueba de regresión de L13 lo acota.

## 3. Orden interno propuesto

```
Externos abiertos: #65-#68 ──► #97 (L00)
                   #72 ──► #106 (L09)     #73 ──► #107 (L10)
                   #77 ──► #108 (L11)     #80 ──► #103 (L06)
                   #83/#88 ──► base de #97/#98

Fase 0   L00 #97                    (raíz; bloqueada por adaptadores #65-#68)
Fase 1   L01 #98                    (serializable obligatorio: define el contrato)
Fase 2   L02 #99   ║ L03 #100 ║ L04 #101 ║ L05 #102     (4 en paralelo, ver reglas)
Fase 3   L07 #104 ║ L08 #105        (paralelo entre sí; necesitan L01+L03)
         L06 #103                   (cuando L05 + #80)
         L09 #106 (L01+L04+#72), L10 #107 (L01+#73), L11 #108 (L00+L01+#77)
         L12 #109 (L01+L05)
Fase 4   L13 #110                   (cierre transversal; depende L01-L04 y adopta L07/L08/L10)
```

**Sub-issue raíz: #97 (L00).** Todo el grafo baja de él: L01 depende de L00, y L07/L08/L11 lo citan explícitamente para la cuenta por colección. En paralelo, L01 puede esbozarse contra fixtures, pero no puede darse por completa sin la pertenencia real (su propio criterio: "los planes de consulta entran por los índices de L00").

**Serializables obligatoriamente por fichero compartido:**
- **L00 → L01**: contrato y fixture compartido; L01 define `LibraryQuery` que todos los demás consumen. No paralelizar.
- **L01 → (L02, L04, L05, L07, L08, L09, L10, L11, L12)**: todos extienden el mismo módulo de consulta. Su arranque es serial tras L01, aunque luego diverjan.
- **L05 → L06 y L12**: comparten runner `run_library_action`, bloques y eventos. L06/L12 no arrancan antes de L05.
- **L04 → L09**: el chip y el panel de cuarentena se montan sobre el panel de L04.
- **L13 al final**: toca `LibraryView.vue`, `useTrackBrowser.ts` y `api/library.ts` que L02-L04 habrán modificado; hacerla antes sería rebase constante.

**Paralelismo realista bajo la regla del repo (dos carriles solo si no comparten fichero):**
- Fase 2 es el máximo de paralelismo sano: **L02** (migración FTS + Rust) en un carril y **L03** (LibraryView.vue + main.ts) en otro, porque L02 es casi todo Rust/SQL y L03 casi todo UI. **L04 y L05** rozan ambos ficheros (`library.rs` + `LibraryView.vue`); si se abren como segundo par, ejecutarlos en serie entre sí o con carriles separados por convención estricta.
- **L07 y L08** son paralelizables entre sí si el módulo de consulta extraído en L01 deja superficies separadas (`query_library_albums` vs `query_library_artists`); comparten fixture, no ficheros.
- **Advertencia**: `library.rs` ya tiene 8.035 líneas. La instrucción de L01 de "extraer la consulta a un módulo" es la pieza que habilita todo el paralelismo posterior; sin ella, cada par choca en el mismo fichero y todo se serializa de facto.

**Nota de calendario (2026-11-16):** el programa #34 (rendimiento de UI) sigue OPEN con FE-PERF-03/04 en vuelo; #49 (FE-PERF-01, KeepAlive) ya está CLOSED y fusionado. **Decisión del propietario: la serie arranca con L01 contra fixtures en cuanto #34 cierre (ver cabecera).**

## 4. Riesgos y minas

- **Ficheros mina**: `library.rs` (8.035 líneas — grande pero *no* está en la lista de "evitar" del AGENTS.md, que señala `commands/service.rs` ~9,4k y `download/qobuz.rs` ~4,1k). L00 tocará `service.rs` a través del sync de #64: ahí aplican las minas del repo — tests de caracterización delante. `main.rs` (registro del `generate_handler!`) se toca en casi toda fase; el gate de comandos muertos (`tests/dead_commands_hygiene_test.rs`) castiga retirar `get_library`/`get_favorite_tracks`/`get_duplicate_tracks` sin verificar consumidores: `get_favorite_tracks` y `search_tracks` tienen consumidores fuera de LibraryView que hay que auditar antes de retirar.
- **Sondeo de `get_library` (319 llamadas)**: la causa registrada es el patrón de la vista (recargas por eventos + scroll), no el coste de la consulta. `debouncedReloadLibrary` (`LibraryView.vue:2158`, eventos :2773-2780) y los watchers de búsqueda/filtros son el objetivo de la prueba de regresión de L13. Riesgo: que L13 se perciba como "optimización de SQL" cuando es de orquestación de vista.
- **Q-13/Q-14 vs sub-issues — se solapan parcialmente, no se sustituyen.** Aclaración: Q-13/Q-14 **no son issues de GitHub** (#13/#14 son build Linux, MERGED); viven en `docs/historico/Errores-Pruebas-2026-10-11.md`:228 y :239:
  - **Q-13 (el sync duplica demasiado)**: solapa con #97 (L00) — la clave de dedup en importación (`track_sources(service_id, service_track_id)`, `ON CONFLICT`) y con el criterio de L00 "los favoritos dejan de expandirse por release". Pero Q-13 apunta al *importador local*, no al sync por cuenta de #64: L00 no lo cierra por sí solo. Q-13 debería asignarse/triajarse hacia #97 o al spec #64 explícitamente, o quedará huérfano.
  - **Q-14 (la biblioteca renderiza pocas filas durante la importación)**: solapa de lleno con **#110 (L13)** — mismas pistas de código citadas en 2026-10-11 (`loadLibrary` recarga primera página, eventos con debounce, render dependiente del rango virtual). La prueba de regresión de L13 es el vehículo natural para cerrarlo. Riesgo: si L13 se hace antes de que la vista pase a `query_library`, la prueba se escribe contra la arquitectura vieja y se tira.
  - Conclusión: **complementan** — L00/L13 cambian la arquitectura que Q-13/Q-14 diagnosticaron; el triaje de ambos registros debería engancharse a #97 y #110 para no perderlos.
- **Fusión automática (L10)**: retirar `auto_resolve_duplicates` afecta a `api/library.ts:307-312` y `dashboard.ts:138`, con tests existentes (`test_auto_resolve_duplicates_by_isrc` y otros en `library.rs:3060+` y `tests/duplicates_level2_3_merge_test.rs`). **Decisión del propietario 2026-11-16: retirar por completo (ver cabecera).**
- **D-06 (escrituras silenciosas)**: L00 (sync masivo de pertenencias), L05 (bloques de 500) y L06 citan el ratchet explícitamente; cualquier `let _ = sqlx::query(...)` nuevo sube el contador prohibido (suelo 19, ratchet automático desde #39).
- **Migraciones**: L00 (tabla nueva + índices + `library_added_at`), L02 (rebuild de FTS sobre 30k pistas) y L10 (registro de fusiones) crean migraciones. Ninguna edita aplicadas ni reutiliza números (minas del AGENTS.md); L00 exige prueba de upgrade sobre base existente, no solo `:memory:`.
- **`useVirtualWindow` es reciente (ADR-0025)**: L13 dice "mantener el virtualizador existente si cumple" — no introducir dependencia nueva.

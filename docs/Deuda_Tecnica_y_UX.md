# Deuda técnica y de UX

**Estado:** registro vivo de la deuda conocida (abierta, aceptada y cerrada).
**Última revisión:** 2026-10-25 (nota de trazabilidad de esta cabecera).

Registro vivo de la deuda conocida del proyecto. Es el documento al que apuntan
`docs/PLAN_UNIFICACION_IMPORTACION.md` (Fase 4, entregable 3) y el núcleo
`sync_soundcloud_likes_with_engine` de `src-tauri/src/commands/service.rs`.

> Hasta `b9559da` este archivo se citaba desde el código y desde la documentación
> pero **no existía en el árbol**: `workspace/` está en `.gitignore` y ningún
> archivo bajo esa ruta llega a un checkout ni a CI (misma causa raíz que
> SYNC-AUD-058 y SYNC-AUD-065). Esta es la versión versionada.
>
> Nota de trazabilidad (2026-10-25): el commit `b9559da`, citado en esta
> cabecera y como evidencia de C-01/C-02, **no es resoluble en la historia
> alcanzable del repositorio** (probablemente se perdió en una reescritura).
> La evidencia de esas dos entradas queda sin anclaje de commit; el
> comportamiento que describen está vigilado por las puertas de CI.

## Cómo se registra una entrada

Cada entrada dice **qué se observó**, **dónde** y **qué la cerraría**. Una
entrada sin observación reproducible no entra aquí. Los riesgos aceptados se
declaran como tales y no se reopened por reflejo.

Última revisión: auditoría de pilares y reparación posterior (2026-10-19) —
se abren D-05 a D-08 (subsistema de reproducción retirado, escrituras silenciosas,
contrato de identidad por edición y funciones monstruo) y se cierran C-12 a C-18.
Antes: auditoría 73 ítems, entrada A-01 reescrita contra el árbol (2026-10-05) — el par `app_id`/`app_secret` está versionado en Python y en
Rust, no solo el `app_id`. Antes de eso: ítem 9.1 del plan post-auditoría
(2026-10-02), que cierra D-02 y redefine D-01 y D-04 a la luz de las fases 4 y
8. Y antes: `b9559da` + reparaciones de SYNC-AUD-062 a SYNC-AUD-071.

## Riesgos aceptados

### A-01 — El par `app_id`/`app_secret` público de Qobuz está en el árbol

- **Observado**: el bundle de identificadores de aplicación que usa el cliente
  open-source de Qobuz está versionado **en los dos lados**: en Python,
  `scripts/services/qobuz_service.py` (líneas 56 y 63) y en el núcleo Rust,
  `src-tauri/src/services/qobuz.rs` (líneas 19-20,
  `QOBUZ_APP_ID_FALLBACK` y `QOBUZ_APP_SECRET_FALLBACK`). El `app_id` público
  es `798273057`; el `app_secret` emparejado no se reproduce aquí para no
  darle una segunda copia.
- **Decisión**: exposición **aceptada y documentada** (SYNC-AUD-071). Ambos
  valores son identificadores de la aplicación pública de Qobuz, no
  credenciales personales de ningún usuario: sin ellos la API responde 400
  «Invalid or missing app_id». Lo que este riesgo **no** cubre son las
  credenciales personales del usuario (token/login), que viajan por el
  cifrado de la app.
- **Prioridad de resolución**: las variables de entorno `QOBUZ_APP_ID` y
  `QOBUZ_APP_SECRET` ganan sobre los literales de respaldo
  (`src-tauri/src/services/qobuz.rs`, líneas 30-38;
  `scripts/services/qobuz_service.py`, líneas 95-96), de modo que un operador
  puede rotar el par sin tocar el árbol.
- **Lo que no se acepta**: credenciales personales de usuario versionadas, o
  una petición firmada con el secreto vacío cuando no hay ninguno
  (SYNC-AUD-062).
- **Vigilancia**: `scripts/tests/test_docs_consistency.py`
  (`AcceptedQobuzAppIdRiskTests`) exige que esta entrada siga nombrando el
  `app_id` que el árbol lleva y que los dos lados compartan el mismo valor
  público; `src-tauri/tests/secrets_isolation_security_test.rs` y
  `src-tauri/tests/qobuz_credentials_leak_test.rs` tratan la exposición
  como material vigilado.
- **Cierra**: decisión del propietario, no trabajo pendiente. Se revisita solo si
  Qobuz rota el identificador.

## Deuda abierta

### D-01 — Fase 3 implementada; falta verificarla contra las APIs reales

- **Ejecución**: [#56](https://github.com/MadManJohnSmith/Syncify/issues/56) y [#57](https://github.com/MadManJohnSmith/Syncify/issues/57) (spec [#36](https://github.com/MadManJohnSmith/Syncify/issues/36)).

- **Observado**: la Fase 3 del plan de unificación ya no está bloqueada en código.
  El brazo `"soundcloud"` de `perform_sync_service_with_emitter` pasa cada like por
  `sync_soundcloud_likes_with_engine` → `EnrichmentEngine`, con identidad canónica,
  artista de `publisher_metadata`, álbum, portada y `added_at` del like
  (`tests/soundcloud_unified_engine_test.rs`); el brazo `"apple_music"` importa
  canciones, álbumes y playlists con storefront desde credenciales y cursor `next`
  (`tests/apple_music_storefront_pagination_and_migration_test.rs`). Ambos están
  cubiertos contra un servidor mock, no contra la API real.
- **Lo que sigue sin poder verificarse** es el comportamiento de esos dos brazos
  contra los servicios reales, porque no hay con qué llamar: ni la OAuth app de
  SoundCloud ni el developer token JWT de Apple Music están en el árbol, y no pueden
  estarlo.
- **Cierra**: con las credenciales del propietario — la OAuth app de SoundCloud y el
  developer token de Apple Music—. Es una comprobación, no una implementación.

## Alcance condicionado: lo que este registro NO declara como pendiente

La entrada anterior (D-01) está **registrada, con su condición de cierre, y no
bloquea una entrega**. No es un defecto: es alcance condicionado. Un cierre correcto
lo declara como tal y no como trabajo pendiente, porque confundirlo convierte un
bloqueo conocido en una promesa que el proyecto no puede cumplir. (D-04, que
compartía esta sección hasta el 2026-11-14, se cerró ese día: ver su entrada.)

| ID | Condiciona | Entrada externa que la resuelve |
|---|---|---|
| D-01 | Verificación end-to-end de los brazos SoundCloud y Apple Music contra sus APIs reales | Credenciales del propietario (OAuth app y developer token JWT) |

Lo que este registro **no** afirma en ningún caso: que la funcionalidad del producto
esté completa. La verificación end-to-end de SoundCloud y Apple Music sigue sin estar
hecha, y `docs/MATRIZ_PARIDAD_IMPORTACION.md` lo dice en cada fila afectada (⚠️ y ⛔).

- **Cierra**: la declaración misma. Cada entrada se reabre cuando llegue su entrada
  externa, y se convierte en deuda abierta solo si aparece trabajo que el equipo sí
  pueda hacer.

## Deuda abierta accionable (auditoría de pilares, 2026-10-19)

Las entradas de esta sección **no** están condicionadas a una entrada externa: son
trabajo que el equipo puede hacer ya. Por eso no aparecen en la tabla de alcance
condicionado, cuyo contrato es declarar solo lo que espera a un tercero.

### D-06 — Escrituras SQLx silenciosas

- **Ejecución**: [#37](https://github.com/MadManJohnSmith/Syncify/issues/37), [#38](https://github.com/MadManJohnSmith/Syncify/issues/38) y [#39](https://github.com/MadManJohnSmith/Syncify/issues/39) (spec [#32](https://github.com/MadManJohnSmith/Syncify/issues/32)).

- **Observado**: 243 `let _ = sqlx::query(...)` en producción (125 UPDATE, 82 INSERT,
  11 DELETE). Una escritura puede fallar y la transacción confirmar sin rastro. Reparto:
  `src-tauri/src/services/enrichment.rs` 45, `src-tauri/src/commands/service.rs` 28,
  `src-tauri/src/services/qobuz.rs` 21.
- **Corrección medida al migrar (2026-10-19)**: de esos 243, **18 terminan en
  `.await.map_err(..)?`**, o sea que propagan el error y nunca fueron silencios (10 en
  `commands/favorites.rs`, 8 en `services/tidal.rs`). Los silenciosos reales eran **225**.
- **Frontera creada**: `crate::db::write_or_log(contexto, futuro)` en `src-tauri/src/db.rs`
  concentra la política (registra con `tracing::warn!` y devuelve `Option`), con sus tests.
  Las escrituras que deban abortar tendrán su variante cuando exista el primer llamador.
- **Progreso**: 130 de los 225 silenciosos migrados a `crate::db::write_or_log` —
  `commands/library.rs` 18, `services/enrichment.rs` 45 (los caminos calientes
  `apply_to_database` 25 y `enrich_and_persist_sync_track` 19), `commands/favorites.rs` 9,
  `services/tidal.rs` 9, `services/qobuz.rs` 21 y `commands/service.rs` 28. Los cinco
  ficheros de mayor riesgo del plan quedan cubiertos; el ratchet baja de 243 a 113 (patrón
  bruto; 95 silenciosos + 18 que propagan) y solo puede bajar.
- **Progreso (lote D-06a, servicios de proveedor, 2026-11-14, [#37](https://github.com/MadManJohnSmith/Syncify/issues/37))**:
  42 puntos migrados a `crate::db::write_or_log`, ninguno propaga (son escrituras
  best-effort dentro de bucles de importación/merge/hidratación donde abortar cambiaría
  el comportamiento observable): `services/musicbrainz.rs` 13 (`resolve_ghost_artists` 6,
  `hydrate_stub_albums` 7), `services/apple_music.rs` 10, `services/spotify.rs` 8,
  `services/track_matcher.rs` 5, `services/deezer.rs` 4, `services/tidal_pipeline.rs` 1
  y `crypto.rs` 1 (el flag de migración de credenciales, ya no fatal por diseño).
  Spotify tenía un octavo punto en forma multilínea (`let _ =` con la query en la línea
  siguiente) que la expresión del ratchet no contaba; migrado igualmente. Medido con la
  expresión del ratchet: 99 sitios antes del lote, 58 después; el suelo registrado baja
  de 113 a 58.
- **Hecho (2026-11-14, lote comandos/worker/caché, [#38])**: los 44 puntos restantes
  fuera de los servicios de proveedor pasan por la frontera o propagan, revisados uno a
  uno: `commands/settings.rs` 3 y `commands/playlists.rs` 1 propagan con `?` (los
  hermanos de su misma transacción ya abortaban con `map_err(..)?`; perder la escritura
  y confirmar igual era exactamente el daño de D-06), y con `write_or_log` —
  `services/operation_recovery.rs` 10 (reconciliación de arranque y caminos canónicos),
  `commands/backup.rs` 6 (restauración: upserts `INSERT OR IGNORE` y favoritos
  best-effort), `worker.rs` 5, `import_cache.rs` 4, `commands/enrichment.rs` 3,
  `commands/accounts.rs` 3, `main.rs` 3 (arranque: purga y siembra de credenciales),
  `commands/queue.rs` 2, `services/enrichment.rs` 2, `commands/lyrics.rs` 1 y
  `commands/migration.rs` 1. Sin cambio de comportamiento observable fuera del log.
  Nota de recuento: la cifra del issue (ajustes 10, cola 5, dashboard 2) se midió antes
  de SB00/SB01; al llegar a `origin/dev` settings tenía 3, queue 2 y dashboard ya
  propagaba. El ratchet (`scripts/tests/test_silent_write_ratchet.py`, métrica de una
  línea sobre todo `src-tauri/src`) baja de 113 a 60 con este lote y solo puede
  seguir bajando.
- **Lo que la frontera destapó (2026-10-19)**: en la primera prueba manual,
  `context="enrich_and_persist_sync_track"` registró 51 fallos que antes eran
  invisibles: `UNIQUE constraint failed: tracks.isrc`. Corregido en `c7325a7`
  (ver D-07). Es el argumento de la frontera: no cambia el comportamiento, pero
  convierte un fallo mudo en una línea de log accionable.
- **Cierra**: migrar el resto por lotes y seguir bajando el baseline; el ratchet
  automático es [#39](https://github.com/MadManJohnSmith/Syncify/issues/39). Con los
  dos lotes de 2026-11-14 (#37 servicios de proveedor y #38 comandos/worker/caché)
  fusionados, la métrica del ratchet mide **19** apariciones: 18 que ya propagan
  (`commands/favorites.rs` 10, `services/tidal.rs` 8) y 1 dentro de un bloque
  comentado de `main.rs`, sin efecto en ejecución. El suelo registrado baja de 113 a
  19 (medido 2026-11-14 sobre el árbol con ambos lotes; solo puede seguir bajando).
  Desde [#39](https://github.com/MadManJohnSmith/Syncify/issues/39) el ratchet
  (`scripts/tests/test_silent_write_ratchet.py`) es automático: su `BASELINE` (19,
  fecha de medición 2026-11-14) debe coincidir con esta cifra — el test la sincroniza
  — y el patrón cubre también la query en la línea siguiente al `let _ =`.

### D-08 — Dos funciones monstruo

- **Ejecución**: [#40](https://github.com/MadManJohnSmith/Syncify/issues/40) y [#41](https://github.com/MadManJohnSmith/Syncify/issues/41) (spec [#32](https://github.com/MadManJohnSmith/Syncify/issues/32)).

- **Observado**: `perform_sync_service_with_emitter_inner`
  (`src-tauri/src/commands/service.rs:3285`) mide 4 556 líneas y
  `resolve_token_for_account` (`src-tauri/src/download/qobuz.rs:417`) 2 860; el censo del
  hub da mediana 23 líneas y máximo 4 559.
- **Medido de nuevo (2026-10-19, tras migrar D-06)**: la función ocupa las líneas
  3291-7836 de `src-tauri/src/commands/service.rs`, **4 546 líneas**, con 214 `if`, 88
  `match`, 40 `for` y 122 `.await`; los brazos se distinguen por servicio (qobuz 16
  comparaciones, tidal 13, spotify 10, deezer 9, soundcloud 4).
- **Primer brazo a extraer**: soundcloud, el más pequeño y el que menos comparte con los
  demás. Antes de mover una línea: tests de caracterización del orquestador (secuencia de
  progreso emitida, recuento por servicio, mapeo de errores y qué escribe cada brazo).
- **Hecho (2026-10-19)**: tests de caracterización del orquestador (3 contratos sin red:
  `requires_auth` terminal, emisor opcional y determinismo) y el brazo SoundCloud extraído
  entero a `sync_soundcloud_arm` (249 líneas) con un `ArmOutcome` de 19 acumuladores:
  el `match` pasa a ser una llamada y un volcado explícito. La precondición de credenciales
  vive en `soundcloud_credentials`, función pura con 5 tests, y el brazo tiene su contrato
  (sin fases activas no emite ni toca nada). `perform_sync_service_with_emitter_inner` baja
  de **4 546 a 4 330 líneas**.
- **Hecho (2026-10-19, segundo brazo)**: Deezer extraído a `sync_deezer_arm` (591 líneas).
  Aquí el patrón se generalizó: `ArmOutcome` crece a 26 contadores y el orquestador siembra
  el struct entero y lo vuelca por asignación, sin sumas parciales; los `Vec` de errores y
  avisos se mueven y se devuelven (el brazo corre en exclusiva, nadie más los toca mientras
  tanto). `album_expansion_metrics` entra tal cual porque el brazo lo incrementa por campo.
  El orquestador baja a **3 809 líneas** (desde 4 546: un 16 % menos) y el límite de 8
  argumentos se marca con `allow(clippy::too_many_arguments)` en el brazo, no en el
  orquestador.
- **Límite conocido**: ningún test de integración recorre los brazos con red real; la
  equivalencia se apoya en el compilador, en el contrato de cada brazo (autenticación sin
  red, sin fases no se toca nada) y en la suite completa. El quinto brazo (Qobuz) ya tiene
  su reproducción de red contra proveedor simulado (2026-11-16, abajo); los otros cuatro
  siguen sin ella.
- **Hecho (2026-10-19, tercer brazo)**: Spotify extraído a `sync_spotify_arm` (841 líneas).
  El struct crece con `reported_favorites`/`reported_albums`/`reported_playlists`
  (`Option<i64>`, los recuentos que declara el proveedor) y el call site de Deezer se alineó
  para sembrarlos también: un struct con campos nuevos no puede dejar inicializadores
  desactualizados. `service_name` volvió a aparecer en una firma sin usarse y se retiró igual
  que en Deezer: el aviso del compilador es el que manda, no mi inventario. El módulo de
  tests pasa a `sync_arm_contracts` porque ya cubre tres brazos (8 contratos).
  El orquestador baja a **3 057 líneas** (desde 4 546: −33 %).
- **Hecho (2026-10-19, cuarto brazo)**: TIDAL extraído a `sync_tidal_arm` (1 098 líneas), el
  brazo con más fases. `ArmOutcome` llega a 33 campos con `albums_unavailable`,
  `tracks_unavailable`, `tracks_expansion_deferred` y `tracks_expansion_failed`. Antes de
  escribirlo hubo un casi-accidente que merece quedar: la línea `availability_checked,`
  aparece también en el literal de `ServiceSyncResult` del final del orquestador, así que
  insertar «detrás de la primera coincidencia» habría corrompido ese struct; el `assert` del
  script lo paró antes de escribir y la inserción quedó restringida a literales
  `ArmOutcome {` sin `..Default::default()`. `service_name` volvió a sobrar y se retiró.
  El orquestador baja a **2 054 líneas** (desde 4 546: −55 %).
- **Hecho (2026-10-19, quinto y último brazo)**: Qobuz extraído a `sync_qobuz_arm`
  (1 237 líneas), el más grande. `ArmOutcome` queda en 34 campos con `purchases_total`.
  `perform_sync_service_with_emitter_inner` baja a **958 líneas**: de 4 546 a 958 (−79 %),
  sin cambiar el contrato del orquestador (los 3 tests de caracterización y los 10 contratos
  de brazo siguen verdes).
- **Validado en la app real (2026-10-19)**: un sync completo de los cinco servicios
  recorrió los cinco brazos sin panic ni cuelgue, con los tiempos que registra el
  orquestador: Tidal 670 s (enriquecimiento 348 s, persistencia 124 s), Qobuz 81 s,
  Apple Music 29 s, Deezer 4 s, SoundCloud 1,4 s. Tidal importó 16 087 de 17 492
  favoritos y 27 832 de 27 832 pistas de playlist.
- **Estado de D-08**: los cinco brazos extraídos (soundcloud 249, deezer 591, spotify 841,
  tidal 1 098, qobuz 1 237). Lo que queda **no** es mover líneas: el `match` ya es una
  llamada y un volcado por brazo, y el siguiente trabajo real es decidir si `ArmOutcome` se
  convierte en el contador único del orquestador (hoy se siembra y se vuelca por brazo) y
  escribir la caracterización con red que hoy falta (secuencia de progreso completa y
  recuento por servicio contra un proveedor simulado).
- **Hecho (2026-11-14, D-08a, #40)**: contador único unificado en PR #205 (fusionado
  2026-11-05). `ArmOutcome`
  pasó a llamarse `SyncOutcome` y ya no se siembra ni se vuelca por brazo: el orquestador
  crea una instancia (`SyncOutcome::default()` en `src-tauri/src/commands/service.rs`, en
  `perform_sync_service_with_emitter_inner`), los cinco brazos (y `apple_music` embebido) la
  mutan in situ por `&mut`, y el resto de la función lee de ella; desaparecieron los 153
  volcados campo a campo y el saco de snapshots. El contrato queda documentado en el doc
  comment del propio struct. La caracterización con red sigue pendiente (issue #41).
- **Hecho (2026-11-16, D-08b, #41)**: caracterización de la red del quinto brazo
  (`sync_qobuz_arm`) contra un proveedor simulado en proceso, sin red:
  `src-tauri/tests/qobuz_sync_arm_simulated_provider_test.rs` levanta un servidor HTTP en
  `127.0.0.1:0` y apunta el cliente Qobuz a él con la costura de entorno
  `SYNCIFY_QOBUZ_API_BASE` (mismo molde que `SYNCIFY_DEEZER_API_BASE`), con migraciones
  reales sobre SQLite `:memory:` y credenciales con token almacenado. Fija la petición que
  el brazo envía (`favorite/getUserFavorites?type=tracks&offset=0&limit=50`, firmada con
  `app_id`/`request_sig`), los contadores del `SyncOutcome` único (2 favoritos → 2 pistas,
  2 `library_entries`, `account_import_counts` con lo declarado y lo importado), la
  secuencia completa de fases, la idempotencia de la segunda pasada y el golden de las
  cadenas de i18n que viajan en el payload de los eventos de progreso: la comparación es
  exacta por plantilla y una cadena traducida en el payload hace fallar el test (prueba
  negativa incluida). La caracterización con red de los otros cuatro brazos sigue
  pendiente (D-01, credenciales).
- **Cierra**: un brazo por commit, cada uno con su test de caracterización verde antes y
  después, y el orquestador reducido a un bucle corto sobre los brazos.

### D-09 — Reintento DASH re-descarga sobre un temporal a medio escribir

- **Observado (2026-11-20, review de PR #349, hallazgo preexistente)**: en la rama DASH de
  `download_audio_payload_with_progress` (`crates/syncify-tidal-downloader/src/lib.rs`), el
  `File::create` del temporal está fuera del bucle de reintentos: si `write_all` falla a
  medias en el intento N (p. ej. disco lleno), el intento N+1 re-descarga el segmento y
  añade sin truncar. El control de completitud aterrizado en #349 es de longitud (bytes en
  disco == bytes consumidos) y no detectaría el solape; el remux posterior lo tumbaría casi
  siempre con `RemuxError`, así que el defecto visible es un fallo de descarga, no
  corrupción silenciosa. La estructura de reintentos es idéntica antes y después de #349:
  el PR no la empeora, y sus garantías nuevas (libro de consumo en orden, purga del
  temporal en fallo de segmento/init) no cubren este camino.
- **Corrección que lo cerraría**: truncar o recrear el temporal al inicio de cada intento,
  con un test que reproduzca el fallo de escritura a medias. Cobertura simétrica opcional:
  un test offline con servidor local para `PayloadIncomplete` en la rama directa (los
  criterios de #45 eran DASH y esa rama solo tiene tests live `#[ignore]`).
- **Cierra**: el truncado por intento con su test rojo→verde.

### D-10 — Preview y confirmación del wizard de migración no conocen `collections`

- **Observado (2026-11-21, review y re-check de PR #350, registrado como limitación en su cuerpo)**: tras M01 (#166), el job puede llevar `collections` con pistas y álbumes, pero el preview (`migration.rs:254`) no recibe colecciones y el wizard sigue mostrando `total_tracks` como «N items previewed» y «migrating N tracks»: un job solo-álbumes confirma con métricas de pistas. Además `pairKey` (`MigrationView.vue:1906`) no incluye la selección: se puede previsualizar con favoritos, volver atrás, añadir álbumes y el gate sigue considerando el preview «actual». ADR-0015 §5 (dry-run por colección) pide el preview por colección y sigue pendiente. Copia in-app que insinúa de más: el paso de revisión dice que «las listas se espejan de las pistas transferidas», cierto en el motor, pero el wizard nunca envía `playlist_ids`, así que hoy seleccionar Playlists no crea ninguna lista.
- **Corrección que lo cerraría**: preview por colección (ADR-0015 §5), `pairKey` con la selección en la clave, métricas de confirmación por tipo, y condicionar o corregir la copia del espejo de listas.
- **Alcance ampliado (2026-11-21, M02 #167)**: el job también puede llevar artistas seguidos y el preview sigue sin conocerlos: un job solo-artistas confirma con `total_tracks = 0` y la copia de revisión no distingue las tres colecciones de ítems. El preview por colección debe cubrir pistas, álbumes y artistas.
- **Cierra**: el preview por colección aterrizado con su test, y la copia sin promesas que el wire no cumple.

### D-11 — Fila `pending` residual si la cancelación llega dentro de una fase de colección

- **Observado (2026-11-21, re-check de PR #350)**: el fix de #350 (`&& !cancelled` en `migration.rs:2449`) impide ABRIR la fase de álbumes con el job ya cancelado, pero si la cancelación llega DENTRO de la fase, `run_album_collection` sigue insertando la fila `pending` del primer álbum antes de mirar el estado en la primera iteración. Comportamiento preexistente que la review no pidió cambiar; deja un ítem `pending` en un job cancelado.
- **Alcance ampliado (2026-11-21, M02 #167)**: la fase de artistas (`run_artist_collection`) reproduce el molde exacto y hereda la misma ventana: sus filas `pending` se insertan antes de la primera comprobación de cancelación. El test `test_artist_phase_stops_when_the_job_is_cancelled` pincha el comportamiento actual (fila `pending` residual) a propósito; el arreglo de esta deuda debe cubrir las dos fases a la vez.
- **Corrección que lo cerraría**: comprobar `cancelled` antes de cada inserción (o capturar el estado al entrar en la iteración), con un test determinista del mismo molde que el de la fase ya arreglada (trigger sobre la primera fila).
- **Cierra**: la comprobación por inserción con su test rojo→verde, en álbumes y artistas.

### D-12 — Estado obsoleto del detalle de artista al cambiar de artista in-place

- **Observado (2026-11-21, re-check de PR #351)**: tras aterrizar la recarga real
  por parámetro (#351: `watch(() => route.params.id)` en `ArtistDetailView.vue`),
  quedan dos ventanas sin protección:
  1. `loadArtist` no tiene secuenciación: con una navegación A→B rápida, la
     respuesta que llega última pinta el artista aunque la URL sea otra, y el
     estado persiste hasta la siguiente navegación. `loadArtistCollection` además
     relee `artistId.value` en cada página: cambiar de artista a mitad de la
     enumeración de Download All/Shuffle puede mezclar pistas de dos artistas en
     la misma cola.
  2. `isFavorite` nunca se reinicia ni se carga en `loadArtist`, mientras el
     backend sí persiste el favorito por artista (`src-tauri/src/commands/favorites.rs`):
     tras A→B el corazón conserva el estado del artista anterior y en el primer
     montaje no refleja el backend.
- **Corrección que lo cerraría**: el patrón barato ya presente en el propio
  fichero (`tracksRequestSeq`): un `artistRequestSeq` para `loadArtist` y
  `loadArtistCollection`, capturar el id al iniciar cada acción de cola, y
  reiniciar (o leer) `isFavorite` en `loadArtist`, con un test de carrera A→B.
- **Cierra**: la secuenciación + captura de id + `isFavorite` con su test de carrera.

### D-13 — `download_bridge.py` sobrevive como tombstone tras D14

- **Observado (2026-11-26, #126)**: D14 retiró los caminos de descarga de
  `scripts/services/` (Qobuz, Tidal, Deezer y SoundCloud) y dejó
  `scripts/download_bridge.py` como tombstone que responde con error y exit 1.
  El fichero no puede borrarse en un carril Python/docs porque dos suites Rust
  exigen su ruta y su contrato de entorno:
  `src-tauri/tests/python_module_hygiene_test.rs`
  (`test_legitimate_production_bridges_exist`) y
  `src-tauri/tests/qobuz_credentials_leak_test.rs`
  (`test_qobuz_bridges_keep_env_override_contract`).
- **Corrección que lo cerraría**: en un carril que toque `src-tauri/**`, quitar
  `download_bridge.py` de la lista de puentes requeridos, retirar el caso de
  override de Qobuz (su sujeto ya no existe) y borrar el fichero junto con sus
  entradas en las listas de empaquetado y en el guardián
  `scripts/tests/test_python_download_retirement.py`.
- **Cierra**: el fichero y sus referencias desaparecen del árbol y las suites
  Rust siguen verdes.

## Cerrado en esta revisión

| ID | Qué se cerró | Evidencia |
|---|---|---|
| C-01 | Los tests de `batch_50_audit_test` se colgaban por el pool SQLite en memoria compartida, no por el paralelismo | SYNC-AUD-060, `b9559da` |
| C-02 | Los tests de `batch_50_audit_test` escribían fuera de su `TempDir` | SYNC-AUD-061, `b9559da` |
| C-03 | La cabecera de `.github/workflows/ci.yml` describía una Deuda A12 ya cerrada | SYNC-AUD-063 |
| C-04 | El README apuntaba a la rama `syncify-graphical` (29 commits por detrás) | SYNC-AUD-064 |
| C-05 | El README citaba un archivo bajo `workspace/`, ruta no versionada | SYNC-AUD-065 |
| C-06 | El puente Python de Qobuz firmaba con secreto vacío | SYNC-AUD-062 |
| C-07 | `sanitize_flac_pictures` descartaba portada sin reportarlo | SYNC-AUD-066 |
| C-08 | Este registro no existía pese a estar citado desde el código y los docs | SYNC-AUD-067 |
| C-09 | La matriz publicaba gates de suite obsoletos | SYNC-AUD-068 |
| C-10 | El plan declaraba "pendiente de ejecución" con F0–F2 y F4 ya ejecutadas | SYNC-AUD-069 |
| C-11 | Los docs de `docs/features/` no declaraban su estado de revalidación | SYNC-AUD-070 |
| C-12 | `sanitize_flac_pictures` no se ejecutaba en la app y el bloque PICTURE irreparable se perdía sin recuperación (D-03) | Ítem 5.3 del plan post-auditoría: `sanitize_flac_pictures_with_recovery` + `salvage_cover_to_jpeg` en `crates/syncify-flac-writer`, `services::flac_cover_sanitizer` con las dos rutas de D-03 (re-encode en el host y sidecar externo `<track>.unrecovered-cover.<sha8>.<ext>`) y registro en `repair_history`; wired en las cinco rutas de escritura FLAC de producción (post-promoción del pipeline Tidal, finalización de descarga Qobuz, re-tag de enriquecimiento, editor manual de tags y el fallback sin BD del descargador Tidal), con `tests/flac_cover_sanitizer_recovery_test.rs` como guardia |
| C-13 | `sync_favorites` solo cubría 3 de los 6 servicios (D-02) | Fase 8 del plan post-auditoría, ítem 8.3: `perform_sync_favorites` tiene los seis brazos (`tidal`, `qobuz`, `spotify`, `deezer`, `soundcloud`, `apple_music`), cada uno paginando completo y persistiendo en `favorites` y en el catálogo vía `persist_favorite_track_via_engine`; `tests/favorites_service_arms_test.rs` los cubre contra servidor mock |
| C-14 | La narrativa de `docs/features/` afirmaba capacidades que el código ya no tiene (DO-4, DO-5) | Ítem 9.1: `metadata-enrichment.md` re-derivado sin Spotify Audio Features y con el worker real (MusicBrainz), `playlist-system.md` re-derivado con la importación real por servicio, README sin «apariciones» ni la cifra de migraciones, `LYRICS_PROVIDER_MATRIX.md` sin la ruta `legacy/syncify-cli` |
| C-12 | `cargo fmt --all -- --check` estaba en rojo (9 ficheros) | `ff11167`; equivalencia probada normalizando whitespace y comas |
| C-13 | `cargo clippy -D warnings` en rojo: 2 lints, no 1 (clippy para por crate) | `3174642`; el segundo apareció al arreglar el primero |
| C-14 | El selector de hilos de la biblioteca invocaba un comando inexistente y el error se tragaba | `f5e2317`; vitest 1 failed/44 → 45 passed |
| C-15 | La puerta IPC no vigilaba comandos que la UI invoca y no existen, ni veía `logs_history::*` | `f5e2317`; test nuevo + parser generalizado |
| C-16 | Dos objetivos de test rojos por fixtures que usaban `account_id: 1` sin crear la cuenta | `41b2f08`; 4/4 y 5/5 verdes |
| C-17 | El README afirmaba 9 tests ignorados y el árbol tiene 13 | `1e35108`; puerta que compara la cifra en ambos idiomas |
| C-18 | No había mapa de entrada para un agente | `b4a9588`; `AGENTS.md` escaneado por `scripts/tests/test_docs_consistency.py` |
| C-19 | El subsistema de reproducción retirado seguía registrado (puerta de higiene en rojo) | `resolve_playback_source`, el protocolo y sus tests borrados; `playback.rs` 628 → 194 líneas; puerta 9/9, suite 1 rojo menos |
| C-20 | Misma grabación en otra edición pasaba sin revisión (ED-04 inalcanzable en ese camino) | Guarda antes de escribir: álbum canónico ≠ entrante sin evidencia de release → `needs_review`; suite 2740/0/13. **Corregida el 2026-10-19**: la comparación solo normalizaba espacios y mayúsculas y rechazaba ediciones legítimas; `misma_familia_de_album` las unifica (`46e0a57`) |
| C-21 ✔ | El alta de pista con un ISRC ya presente se perdía en silencio (`UNIQUE constraint failed: tracks.isrc`: 51 avisos el 2026-10-19 y 65 el 2026-10-20, sobre 736 descargas) | `INSERT_PISTA_SQL` con `ON CONFLICT DO NOTHING` (sin objetivo: cubre el UNIQUE BINARY de la columna y el índice NOCASE parcial de 0064:186) y resolución por `isrc COLLATE NOCASE`; completa lo que falte sin mover la pista de álbum; test rojo→verde con el 2067 real (variante de caja) |
| C-22 ✔ | La app se cerraba al abrir el menú de bandeja durante una descarga: `RefCell already borrowed` en `tray-icon-0.25.1` (`set_tooltip` dentro del despacho del evento), panic en el hilo principal y cierre sin aviso | `0e53dad`: el tooltip se publica con `run_on_main_thread`, fuera del despacho; lib 478/0, suite 2779/0/13. Verificación en runtime (menú abierto con descarga activa) pendiente del usuario |

Nota sobre esta tabla: los IDs **C-12, C-13 y C-14 aparecen dos veces**
(filas 12–14: flac/sync_favorites/docs; filas 15–17: fmt/clippy/selector de
hilos). La segunda tanda reutilizó los números durante la fase de puertas
rojas de 2026-10-18. No se renumeran para no romper referencias externas; el
texto de cada fila desambigua. Tampoco existe D-05: la numeración abierta
salta de D-04 a D-06.

### D-07 — Cerrada, con corrección de rumbo tras la prueba manual (2026-10-19)

La guarda de ED-04 se validó primero en la app real, no solo en la suite: un sync
completo de los cinco servicios (guion `syncify-prueba-limpia.sh`, logs en
`~/.local/state/syncify-manual-test`) dejó **16 766 errores de ítem** en seis
pasadas (Tidal 7 644, Qobuz 5 098, Spotify 4 023, SoundCloud 1) y los favoritos de
Tidal se quedaron en **16 087 de 17 492** (92 %). Los motivos dominantes eran la
guarda nueva, con pares que son la misma grabación en dos ediciones:

    «Nothing But Thieves»          <- «Nothing But Thieves (Deluxe)»
    «Moral Panic»                  <- «Moral Panic (The Complete Edition)»
    «1»                            <- «1 (Remastered 2015)»
    «Hybrid Theory (DMD Album + 3 Bonus Tracks)» <- «Hybrid Theory (Bonus Edition)»

La comparación normalizaba solo espacios y mayúsculas, así que cualquier sufijo de
edición contaba como álbum distinto y, como `album_provider_track_id` no llega en
estas rutas, el resultado era `needs_review` para todo. `misma_familia_de_album`
quita ahora los sufijos de edición (Deluxe, Remastered, Anniversary, Expanded,
Complete, Bonus, Reissue, Collector, Legacy/Definitive y sus equivalentes en
español; entre paréntesis, corchetes o tras « - ») y solo entonces compara. Siguen
yendo a revisión los lanzamientos distintos de verdad («Jazz Dance» vs «60s Stars»,
recopilatorios «The Essentials» vs «60s Essential Hits»). Dos tests fijan ambos
lados con los pares reales del log, en ambas direcciones (`46e0a57`).

La misma prueba destapó un fallo preexistente que la frontera de escritura de D-06
hizo visible: 51 altas de pista perdidas por `tracks.isrc` UNIQUE. Se corrigió con
`ON CONFLICT(isrc)` que reutiliza la fila y no la mueve de álbum (`c7325a7`).

**Tercera prueba manual (2026-10-21).** Con los tres arreglos dentro, sync + descargas
durante 51 minutos: **447** descargas finalizadas, **0** avisos de
`UNIQUE constraint failed: tracks.isrc` y **0** avisos de la frontera de escritura
(`escritura SQLx fallida`), es decir, ninguna escritura perdida en toda la sesión; **0**
panics y la app viva al terminar. El reparto de errores de ítem queda así: identidad del
ítem 1 702 (724 Tidal, 721 Spotify, 257 Qobuz) y el resto, unos 6 080, son la guarda ED-04
sobre lanzamientos que siguen siendo distintos o variantes de formato que el guardián no
normaliza: «The Best Of 1990-2000 & B-Sides» frente a «The Best Of 1990-2000» (88 casos
sumando las tres fuentes), «Happier Than Ever (Explicit)» frente a «Happier Than Ever»,
«Absolution XX Anniversary» frente a «Absolution» (el marcador va sin paréntesis) y
«BLUE LIPS ([lady wood phase II])» frente a «BLUE LIPS».

**Normalizacion ED-04 medida (2026-10-21).** De los pares que quedaron en revision, la
mayoria solo difiere en formato y ya no va a `needs_review`: `(Explicit)`, `(Explicit
Version)`, `XX Anniversary`, `(2016 Rerecorded Version)`, corchetes y puntuacion. Los
cuatro casos del log estan fijados en tests, y se mantiene fuera a proposito lo que si
distingue lanzamientos (`& B-Sides`, `([lady wood phase II])`).

**Segunda vuelta (2026-10-22, tarde).** Top real de la pasada anterior: 6 pares de
formato y empaquetado normalizados (corchetes dobles, `- 192 kHz`, `+`, `(DMD Album)`,
`(Archive Collection)`, `(Int'l Only DMD w/ Altered iLiner)`). Se mantienen en revision los
tres que no son formato: `& B-Sides` (88), `RUSH! (ARE U COMING?)` y `Euphoria Morning` vs
`Euphoria Mourning`. Pendiente la pasada real para medir.

**Medido el 2026-10-22.** Con ED-02a dentro, **0** «identity collision» y **0**
contradicciones: la identidad dejo de bloquear. La cifra de errores comparable es
5 664 → **5 575** (−89) al anadir la normalizacion de formato; el −2 118 que se cito antes
mezclaba pasadas con conjuntos de items distintos y queda retirado.

**Clasificacion instrumentada (2026-10-22).** El album de llegada no se guarda cuando el
item falla, asi que el rechazo ED-04 se clasifica en el momento (`MismoTexto`, `Subtitulo`,
`Conjunto`, `Empaquetado`, `Distinto`) y el sync informa el histograma por servicio. La
proxima pasada real da el reparto con el que decidir la politica, sin adivinar por el top de
motivos.

**Resuelto y medido con ED-04b (2026-10-22).** Pasada real posterior: **8** errores de
item frente a 5 575 (−99,9 %), y los 8 son otra causa legitima («Rejected garbage or empty
artist name»), no ED-04. Se registraron **5 917** apariciones (Qobuz 2 876, Spotify 1 857,
Tidal 1 184), sin `UNIQUE` y sin panics. Se ven en la UI con `ediciones_de_pista`, dentro del
modal de metadatos.

**Corregido el 2026-10-22.** El contador de adopciones de ED-02a marcaba 21 142 porque
sumaba intentos: el item aparece en favoritos, album y playlist y cada aparicion vuelve a
adoptarse. Ahora cuenta items unicos por pasada (conjunto `(service_id, service_track_id)`).

**Ojo con lo que mide.** No es una cifra de progreso: cuenta exactamente los items que solo
existen en el ledger, y ED-04b los mantiene ahi a proposito (la aparicion no escribe
`track_sources`). Tras la pasada del 2026-10-22 hay 10 050 items en esa situacion, asi que el
contador rondara esa cifra, no los 2 213 de antes de ED-04b.

Los 8 errores restantes de la pasada eran «Rejected garbage or empty artist name», el aborto
crudo del trigger de la migracion 0079. Ahora se detectan antes de insertar y el motivo es
accionable: «el artista del proveedor está vacío o es «Unknown Artist»; no se importa».

**Reparto medido (pasada del 2026-10-22, 16 633 intentos clasificados).**
`distinto` 59,6 %, `empaquetado` 26,2 %, `subtitulo` 10,8 %, `conjunto` 2,5 %,
`mismo_texto` 0,9 %. El histograma cuenta intentos (un item aparece en favoritos, album y
playlist), no items unicos: ~3x los 5 575 errores. Las muestras del log muestran el patron
dominante: la grabacion ya vive en un album de estudio y llega desde un recopilatorio, banda
sonora o playlist («The Best Of Crystal Waters» vs «Storyteller», «Pop Internacional 2022»,
«Twisted Sister: Essentials», «Lucky (Apple Original Series Soundtrack)»). Las 148 de
`mismo_texto` eran un fallo real: familias vacias comparadas como distintas, corregido.

**El techo, medido.** Las 5 575 restantes son rechazos de ED-04: la grabacion existe en un
album y llega con otro sin evidencia de release. Ademas la adopcion no persiste, porque
ED-04 revierte la transaccion y la evidencia adoptada se pierde: el sync informo 5 043
adopciones en la segunda pasada y en la BD siguen 2 213 items solo en el ledger.

**Cierre inesperado (2026-10-21T00:44:48Z).** Con descargas en marcha y el menú de
bandeja abierto, la app murió con `RefCell already borrowed` en `tray-icon` desde
`update_tray_icon_state` (`tray.rs:895`): `set_tooltip` no puede ejecutarse mientras la
bandeja despacha un evento. Diferido con `run_on_main_thread` (`0e53dad`). El manifiesto se
había escrito de forma atómica justo antes y las escrituras de BD van en transacción, así
que no hubo estado a medias; la recuperación de arranque informó `recovered=0`.

**Segunda prueba manual (2026-10-20).** Repetido el sync con el arreglo de ED-04
aplicado: los errores de ítem bajan de 16 766 a **7 864** (Tidal 3 834, Spotify 1 773,
Qobuz 2 257) y el motivo dominante deja de ser la guarda ED-04 para pasar a ser la
**colisión de identidad del ítem** (620 + 497 + 597), que sigue yendo a `needs_review`
a propósito. Los ED-04 que quedan son lanzamientos realmente distintos (recopilatorios,
bandas sonoras, «B-Sides», reediciones) o variantes de formato que el guardián no debe
unificar a ciegas.

La misma prueba destapó que el arreglo del ISRC seguía incompleto, y por dos motivos
encadenados. Primero, la tabla tiene
**dos** restricciones sobre la columna (`tracks.isrc` UNIQUE BINARY y
`idx_tracks_isrc_unique` NOCASE parcial, 0064:186) y el `ON CONFLICT(isrc)` solo veía la
primera, así que un ISRC enriquecido que solo cambiaba de caja seguía perdiendo la fila:
65 de 736 descargas. Segundo, el 2067 persistía porque el camino que fallaba no era el
alta sino **dos `UPDATE tracks SET … isrc = COALESCE(isrc, ?)`** (líneas 3037 y 3109) que
rellenan el ISRC que falta: cuando otra fila ya lo tenía (misma grabación, otra caja), el
UPDATE chocaba y se perdía el relleno. Ahora el ISRC solo se asigna si ninguna otra fila
lo tiene (`NOT EXISTS … COLLATE NOCASE`), el alta usa `ON CONFLICT DO NOTHING` sin
objetivo y resuelve por `isrc COLLATE NOCASE`, y dos tests cubren ambos caminos.

### D-02 — Cerrada, y con qué alcance exacto

La entrada D-02 pedía que `sync_favorites` cubriera los 6 servicios, no que dejara de
funcionar para 3. Está cerrada porque los seis brazos existen y están cubiertos:

| Servicio | Canciones | Álbumes | Artistas |
|---|---|---|---|
| Tidal | paginado (offset/limit 100) | una página (0, 100) | una página (0, 100) |
| Qobuz | paginado | una página (0, 100) | una página (0, 100) |
| Spotify | paginado (50) | una página (0, 50) | una página: `get_followed_artists(None, 50)`, sin iterar cursor |
| Deezer | paginado | paginado | paginado |
| SoundCloud | likes paginados por `next_href` | — (la API no lo expone) | — (la API no lo expone) |
| Apple Music | biblioteca paginada por `next` | biblioteca paginada por `next` | — (sin rama de artistas) |

Las diferencias de la tabla son del código, no del documento: la fase 8 dio a
`sync_favorites` un brazo por servicio, no una paginación uniforme por colección.

Los seis escriben la fila en `favorites` **y** la persisten en el catálogo por
`persist_favorite_track_via_engine`, que es el mismo camino que usa el motor
unificado; SoundCloud y Apple Music además conservan el atributo que solo ellos
traen (`publisher_metadata` y `date_added`). El brazo inexistente devuelve
`Unsupported service for favorites sync` en vez de fingir una importación vacía.

**Lo que la verificación de esta entrada NO cubre**: el comportamiento de los brazos
`deezer`, `soundcloud` y `apple_music` contra sus APIs reales. Los tests de
`tests/favorites_service_arms_test.rs` usan un servidor mock en proceso. Esa
verificación end-to-end es la parte de D-01, y depende de credenciales del
propietario.

### Observabilidad del sync — el cierre ya siempre se informa

**Resuelto el 2026-10-22.** El resumen `[sync] N item error(s) for <servicio>` solo se
imprimía cuando `errors` no estaba vacío. Eso hizo leer una pasada terminada como una pasada
colgada: Tidal cerró con cero errores y no dejó línea, y se interpretó como cuelgue durante dos
pasadas seguidas (comprobado en el log: `adoptados del ledger: 4671 en tidal` a las 01:56 y
ninguna línea de error después). Ahora la línea se emite siempre: `0 item error(s) … servicio
completo` a nivel info, o el resumen de motivos a nivel warn.

Lección para la auditoría: una señal que solo existe en el caso malo no sirve para distinguir
«bien» de «no terminó». Toda fase que se quiera observar necesita su línea de cierre.

## D-03 — Cerrada, y con qué límite exacto

La entrada D-03 pedía una ruta de recuperación antes de declarar cerrado el descarte
del bloque PICTURE. Se implementaron **las dos** opciones que la dejaban como decisión
de producto, no una:

1. **Re-encode en el host** — `salvage_cover_to_jpeg` decodifica el payload dañado con
   `-err_detect ignore_err -fflags +discardcorrupt` sobre un archivo temporal con la
   extensión que corresponde a su contenedor, y el fotograma reconstruido vuelve al
   bloque PICTURE (`FlacPictureSanitizeReport::recovered_blocks`).
2. **Sidecar externo** — si ni la ruta estricta ni la tolerante producen un fotograma,
   los bytes originales se conservan junto al FLAC como
   `<track>.unrecovered-cover.<sha256[..8]>.<ext>` (direccionado por contenido, así que
   una segunda pasada no duplica nada y dos portadas perdidas del mismo track no se
   pisan). Los sidecars canónicos de Symfonium no se tocan.

Lo que **no** se promete: que un payload que ningún decodificador del host puede leer
vuelva a ser una imagen. El límite real es explícito y queda registrado, nunca mudo: si
ambas rutas fallan, el bloque sale del contenedor, la causa (incluida la ruta del
sidecar) viaja en `FlacPictureSanitizeReport::dropped_unrepairable_blocks`, en las
`actions` y en el `details_json` de la fila de `repair_history`, y el usuario la ve en
el modal de historial de reparaciones.


## D-04 — Cerrada el 2026-11-14

La entrada pedía re-derivar del código los cuatro documentos de `docs/features/`
que seguían con narrativa de diseño sin verificar (`auth-flow.md`,
`download-pipeline.md`, `library-import.md` y `settings-system.md`). Se re-derivaron
completos contra el árbol con la misma disciplina que los dos ya revisados
(metadata-enrichment y playlist-system): símbolos y comportamiento verificados, sello
«Estado de revisión» con fecha 2026-11-14 en cada uno. Evidencia: PR #291 (`788c195`);
issues de ejecución #58–#61 cerrados citando el SHA. Lo que queda de la spec #36 es
la verificación de red de los brazos SoundCloud y Apple Music, que vive en D-01
condicionada a credenciales del propietario.

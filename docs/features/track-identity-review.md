# Revisión y resolución de identidad de pistas

**Estado de revisión:** 2026-11-23 — T08b (#383) sobre la mitad de identidad de
#73 (migración `0115`). Verificado contra el árbol: comandos IPC, wrapper de UI,
vista `/reviews`, merge transaccional y tests contra BD `:memory:` migrada. Sin
verificar en escritorio real: el flujo completo con una cola de revisión nacida
de un sync vivo contra proveedores.

## Qué resuelve

El sync decide la identidad de un ítem antes de tocarlo
(`src-tauri/src/services/track_identity.rs`, ADR-0006 decisión 16):

1. **ISRC primero.** Un ISRC idéntico enlaza automáticamente.
2. **Huella después.** Sin ISRC, artista + título + duración dentro de ±2 s
   enlazan. El título por sí solo nunca enlaza.
3. **Lo ambiguo no se fusiona.** Si la huella encuentra más de un Track local,
   el sync importa el ítem como Track propio y deja la evidencia congelada en
   `track_identity_reviews` + `track_identity_review_candidates`
   (`src-tauri/migrations/0115_track_identity_reviews.sql`).

Esta página documenta la superficie humana que cierra esa cola: listar las
revisiones `pending`, comparar los dos lados y resolverlas.

## Modelo de datos (0115, sin migración nueva)

`track_identity_reviews` guarda:

| Columna | Papel |
|---|---|
| `service_id`, `service_track_id` | identidad del ítem del proveedor (ADR-0001) |
| `account_id` | cuenta que lo trajo; `NULL` si ya no existe |
| `reason` | vocabulario del núcleo; hoy solo `fingerprint_multiple_candidates` |
| `status` | `pending` → `resolved_merged` / `resolved_separate` / `dismissed` |
| `incoming_*` | título, artista, álbum, duración e ISRC del ítem, congelados |
| `resolved_track_id` | Track canónico elegido; solo en `resolved_merged` |
| `resolved_at` | obligatorio en cualquier estado distinto de `pending` |

`track_identity_review_candidates` es la evidencia local congelada: un Track
por fila con `duration_delta_ms` (la distancia a la duración del ítem). Si el
Track cambia después, la revisión sigue mostrando lo que se comparó.

El índice único parcial de 0115 solo cubre `pending`: cerrar una revisión deja
abrir otra para el mismo `(service_id, service_track_id, reason)` — el sync
puede volver a encontrar el mismo ítem ambiguo en otro barrido. Las decisiones
cerradas no se reabren solas.

## IPC

Los dos comandos viven en `src-tauri/src/commands/track_identity_reviews.rs` y
delegan en `src-tauri/src/services/track_identity_review.rs`:

| Comando | Contrato |
|---|---|
| `list_track_identity_reviews` | devuelve solo `pending`, de la más reciente a la más antigua, con `incoming_*`, candidatos, cuenta/servicio y `linked_track_id` |
| `resolve_track_identity_review(review_id, resolution, track_id?)` | cierra la revisión; `track_id` es obligatorio y debe ser un candidato congelado solo para `merged` |

La UI entra por `ui/src/api/trackIdentityReviews.ts`; ninguna vista llama a
`invoke` directamente (puerta `src-tauri/tests/dead_commands_hygiene_test.rs`).
La ruta es `/reviews` y el enlace del sidebar es «Reviews»
(`ui/src/views/IdentityReviewsView.vue`).

## Qué hace cada resolución

- **`resolved_merged`** (el Track elegido pasa a `resolved_track_id`): pliega el
  Track que hoy representa al ítem en el elegido, dentro de una transacción
  `BEGIN IMMEDIATE`. Conserva:
  - `track_sources`: si el elegido no ocupa la ranura del servicio, la fila se
    repunta; si ya la ocupa, el ítem extra se liga por el ledger de ediciones
    (`edition_provider_items` + `edition_item_tracks`, la misma ruta 2 del
    motor). La ranura legacy del elegido no se pisa.
  - `library_entries`: `is_liked` e `is_purchased` se OR-ean; si el elegido ya
    tenía `added_at` se conserva, y si no se rellena con el del importado. Un
    favorito del Track importado no se pierde aunque el elegido tuviera su
    propia fila.
  - `library_memberships`: se mueven por la clave única de 0112; en conflicto
    se conserva `provider_item` del elegido y su `added_at`, rellenándolo desde
    el importado solo si falta o es «1970-01-01».
  - `playlist_tracks`: se repunta la fila (el modelo de 0110 admite la misma
    pista varias veces por playlist).
  - `edition_item_tracks`, `track_artists`, `track_credits`, letras y filas
    dependientes con clave única: se insertan o se repuntan, y luego se borra
    la fila del Track importado; si ambas pistas tienen la misma clave (p. ej.
    la misma letra en el mismo formato), gana la fila del Track elegido.
  - `tracks.library_added_at` se recalcula con
    `services::library_membership::refresh_track_library_added_at`.
  - Después, la pista importada se borra **solo si** ninguna tabla con FK a
    `tracks` conserva una fila suya. Si queda algo que el merge no sabe
    fusionar (p. ej. una descarga en ambos lados), la transacción entera se
    revierte y el error dice qué tabla lo bloquea: no hay CASCADE silencioso.
- **`resolved_separate`**: confirma que son grabaciones distintas. No toca
  ninguna pista.
- **`dismissed`**: la evidencia dejó de aplicar (el ítem ya no está, por
  ejemplo). No toca ninguna pista.

La lista de tablas que referencian `tracks` es un contrato verificado por test
en `track_identity_review::TRACK_REFERENCE_TABLES` contra
`pragma_foreign_key_list`: una migración futura que añada otra FK obliga a
decidir qué hace el merge con sus filas antes de que la suite vuelva a verde.

## Garantías y límites

- **Sin acción explícita no se fusiona ni se borra nada.** La vista exige
  seleccionar un candidato y confirmar el merge en un segundo paso.
- **Una revisión cerrada no se reabre.** El `UPDATE` filtra
  `status = 'pending'`; resolver dos veces devuelve error y no escribe.
- **El merge no elige por su cuenta.** Si `track_sources` liga el ítem a más de
  un Track, la fusión falla con un mensaje accionable en vez de escoger uno.
- **No hay migración nueva.** `0115` ya reserva `status`, `resolved_track_id` y
  `resolved_at`. Si el diseño necesitara otra columna, el número siguiente es
  `0116`; `0115` no se edita.

## Pruebas

- `src-tauri/src/services/track_identity_review.rs` (módulo `tests`): BD
  `:memory:` migrada de verdad con `sqlx::migrate!("./migrations")`; cubre
  listado con evidencia, `separate`/`dismissed` sin efectos, no reapertura,
  nueva revisión `pending` tras cerrar, merge con fuentes/pertenencias/
  favoritos/playlists, candidato no congelado, ítem ausente, conflicto de
  descargas y la lista de FKs.
- `ui/src/__tests__/api/trackIdentityReviews.spec.ts`: nombres de comando,
  normalización y argumentos camelCase.
- `ui/src/__tests__/views/IdentityReviewsView.spec.ts`: estados de carga, error
  y vacío; comparación lado a lado; merge en dos pasos; `separate`/`dismissed`
  sin `track_id`; error del backend visible.

P7 · TrackBrowser: kernel de ventana virtual y navegador de pistas

## Qué

Parte **P7 · TrackBrowser** del informe de arquitectura (ficha 5; grilled con inventario verificado; ADR-0025). La maquinaria de listas de las tres vistas grandes se concentra: muere el triple clon de ventana virtual y los dos mapeadores de biblioteca.

- **Kernel `useVirtualWindow`** (`ui/src/composables/`, 180 líneas): una ventana virtual para las tres vistas, con **paridad estricta** — las divergencias de los clones quedan congeladas como configuración explícita (rowHeight 56/64/60, overscan 10/5/10, footer de 48 px solo en Library, throttle de scroll de 120 ms solo en Library, reset y viewport vacío distintos). Cero cambios visuales.
- **Composable `useTrackBrowser`**: para las listas basadas en biblioteca (`LibraryView`, `MetadataView`). La vista declara la **fuente** (paginada de biblioteca con triple elección en caliente, búsqueda FTS con paginador propio y query prefijando `*` por palabra, o página fija de 500) y su **mapeo**; el composable da filas, paginación + `has_more` con la optimización de prefijo ordenado intacta, y **selección por ids**.
- **Selección unificada**: los ids viven en el composable; LibraryView migra su flag en fila (el contador derivado no puede replicar el estado huérfano que el flag dejaba tras una recarga — única cifra visible que cambia, solo en ese estado incoherente; de paso el checkbox de fila ya sí actualiza el contador, bug preexistente muerto).
- **La cola no entra al contrato**: `DownloadsView` usa el kernel y conserva su lógica (mutación en vivo por eventos, drag de filas, secciones; R11 intacto).
- **Backend alineado** (decisión 5): `quality_bucket` entra en `search_tracks` y `get_favorite_tracks` con la expresión CASE de `get_library`; el filtro de calidad de Library deja de degradar en FTS y favoritos. Tres tests de paridad contra los literales del dashboard, uno por fuente.
- **Higiene IPC** (decisión 6): wrapper `enrichGenreLastfm` nuevo (los otros 4 existían), los 9 invokes crudos de MetadataView/LibraryView migrados y los 2 bypasses de `invokeCommand` de DownloadsView fuera.
- **Limpiezas**: tipo `'year'` muerto de `groupBy`, demos hardcodeadas en `expandedGroups`, comentario S194 (citaba el protocolo `syncify-media` retirado) y los cuatro comentarios de paridad con líneas muertas, reescritos apuntando al kernel.

## Decisiones cerradas con el propietario (grill, una ronda de 7)

| # | Decisión | Elección |
|---|---|---|
| 1 | Alcance | Kernel de ventana virtual para las 3 vistas + composable de biblioteca para Library/Metadata; la cola sobre el kernel |
| 2 | Divergencias mecánicas | Paridad estricta: config por vista; homogenizar queda como deuda |
| 3 | Selección | Por ids dentro del composable; Library migra |
| 4 | Agrupación | Fuera del composable; limpiezas puntuales |
| 5 | Backend | Alinear `quality_bucket` en FTS y favoritos; `metadata_score` de FTS queda como deuda |
| 6 | Higiene IPC | Incluida (5 wrappers, 9 crudos + 2 bypasses) |
| 7 | Puerta | Suite + caracterización + pasada visual humana antes del merge |

## Deuda declarada en ADR-0025

`metadata_score` de FTS divergente (sin exclusiones), agrupación no virtualizada (problema de diseño propio si se virtualiza), toast local de MetadataView no unificado con `useToast` (lleva el fix MT-02), homogenización de OVERSCAN/footer/throttle/reset.

## Verificación (local, todas en verde)

| Puerta | Resultado |
|---|---|
| `cargo test --locked --no-fail-fast` | 2987 passed / 0 failed / 13 ignored |
| `batch_50_audit_test -- --test-threads=1` | 2/2 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | OK |
| `cargo fmt --all -- --check` | OK |
| `npm --prefix ui run build` | OK |
| `npm --prefix ui run test:run` | 129 ficheros / 1130 tests |
| Python gates (`scripts/tests`) | 311 OK (2 skipped) |
| Caracterización nueva | `useVirtualWindow.spec` 20 tests + `useTrackBrowser.spec` 22 tests |

## Puerta visual (antes del merge)

Pasada humana por las tres vistas (biblioteca con filtros/orden/paginación/agrupación, metadatos, cola de descargas) validando que nada se movió. Resultados: TBD.

## Base `dev`

Parte de `origin/dev` (`74c5a03`). El ADR usa el 0025, libre en ambas ramas.

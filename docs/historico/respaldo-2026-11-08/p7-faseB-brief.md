# Brief — P7 Fase B: kernel de ventana virtual y fin del triple clon

Contrato: **ADR-0025** decisiones 1-2 (kernel compartido, **paridad estricta**).
Hechos: **/tmp/p7-inventory.md** §2 (las tres copias con sus divergencias exactas),
§12 (comentarios de paridad muertos). Base: `470a73d` en `/tmp/syncify-p7`
(rama `p7/track-browser`, Fase A ya mergeada en la rama). Fase UI pura: no toques
Rust ni comandos.

## Entregable principal

`ui/src/composables/useVirtualWindow.ts` — el kernel que las tres vistas consumen.
Requisitos de contrato:

- Configuración explícita por vista con **paridad estricta**: la salida de cada
  vista debe ser hoy exactamente la de su copia actual. Del inventario §2:
  - LibraryView: `rowHeight: 56`, `overscan: 10`, **footer de 48 px** en la altura
    total cuando no hay más páginas, **throttle de scroll de 120 ms con lectura de
    layout dentro del throttle**, reset del scroll si el offset queda fuera del
    total.
  - MetadataView: `rowHeight: 64`, `overscan: 5`, sin footer, sin throttle, reset
    del scroll al cambiar búsqueda/filtro/orden (el view dispara el reset: expón
    una función `reset()`; el watch actual pasa a llamarla).
  - DownloadsView: `rowHeight: 60`, `overscan: 10`, sin footer, sin throttle de
    scroll, sin reset, y los ítems visibles necesitan su `absoluteIndex` adjunto.
- Expón lo mínimo que las vistas necesitan: handler de scroll, altura total,
  índices de ventana (start/end), offset, y una función de reset. La lectura de
  layout (scrollTop/viewport) y el throttle viven en el kernel cuando la vista los
  tenía; la vista no debe duplicarlos.
- **NO confundir**: `PROGRESS_THROTTLE_MS = 250` de DownloadsView es de eventos de
  progreso, no de scroll — queda en la vista, fuera del kernel.
- `jumpToPosition` de DownloadsView y `scrollToTrack` de MetadataView son APIs del
  contenedor de scroll de la vista; el kernel solo necesita exponer lo que esas
  funciones consumen (p. ej. el elemento/altura total) — decide tú la frontera
  mínima que evite duplicación sin cambiar comportamiento.

## Migración (las tres vistas)

- LibraryView (computeds 2471-2499, handler 1717-1762, constantes 1666-1667) →
  kernel con su config. Cuidado: la ventana virtual consume resultados de FTS o de
  biblioteca según `searchQuery` (2409) — el kernel recibe el array, no la fuente.
- MetadataView (computeds 1834-1855, handler 1857-1863, reset 1874-1879,
  scrollToTrack 1865-1872, constantes 1411-1413) → kernel con su config.
- DownloadsView (computeds 1728-1762, handler, constantes 1283-1285) → kernel con
  su config; `absoluteIndex` sigue adjuntándose en la vista (R11: la cola no es ref
  profundo — no romper `DownloadsViewQueue.spec.ts`).
- Los comentarios de paridad muertos (LibraryView 1662-1665 y 1673-1676,
  MetadataView 1411 y 1832) se reescriben apuntando al kernel (sin líneas muertas).
- No toques filtros, orden, paginación, selección ni agrupación (Fase C). La fase B
  solo sustituye la mecánica de ventana virtual.

## Caracterización (obligatoria antes de borrar)

Spec nuevo `ui/src/__tests__/composables/useVirtualWindow.spec.ts`: tests que
capturan el comportamiento de las TRES configs (índices, altura total con/sin
footer, throttle sí/no, reset sí/no, overscan) con arrays y scroll simulados. Si un
test exige un comportamiento y la vista real hoy hace otro, gana la vista real:
corregir el test, no la vista.

## Puertas

- `npm --prefix ui run build`
- `npm --prefix ui run test:run` — 127+ ficheros, 1088 tests mínimo (los specs de
  vistas NO deben necesitar cambios salvo mocks del contenedor de scroll; si alguno
  los necesita, documéntalo en el informe).

## Commits

Un commit: `refactor(ui): kernel de ventana virtual y fin del triple clon
(ADR-0025, P7)`. Mensaje en español, estilo del repo.

## Restricciones

- Solo `/tmp/syncify-p7`, rama `p7/track-browser`. No pushees.
- No toques el worktree principal, otros worktrees ni el target de Rust (fase UI).
- No toques `useTrackBrowser` (Fase C) ni nada de Rust (Fase A ya hecha).

## Informe

1. Forma final del kernel (API) y config de cada vista.
2. Qué specs necesitaron cambios y por qué.
3. Resultados de puertas (cifras), hash del commit.
4. Cualquier divergencia real descubierta que la paridad haya obligado a conservar.

# Issue tracker: GitHub

Los issues y las specs de este repo viven como issues de GitHub, en
`MadManJohnSmith/Syncify`. Usa la CLI `gh` para todas las operaciones.

## Convenciones

- **Crear un issue**: `gh issue create --title "..." --body "..."`. Usa un heredoc
  para cuerpos multilínea.
- **Leer un issue**: `gh issue view <number> --json number,title,body,labels,comments`.
- **Listar issues**: `gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'`
  con los filtros `--label` y `--state` que correspondan.
- **Convertir un issue en sub-issue de otro**: `gh issue create --parent <parent> ...`,
  o después `gh issue edit <parent> --add-sub-issue <child>` (`gh` 2.94+). En `gh`
  más antiguo: `gh api --method POST repos/<owner>/<repo>/issues/<parent>/sub_issues -F sub_issue_id=<child-db-id>`
  (id de base de datos, como en **Bloqueos** abajo). Sin sub-issues, pon
  `Part of #<parent>` al principio del cuerpo del hijo.
- **Comentar un issue**: `gh issue comment <number> --body "..."`
- **Aplicar / quitar etiquetas**: `gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **Cerrar**: `gh issue close <number> --comment "..."`

El repo se infiere de `git remote -v`; `gh` lo hace solo cuando se ejecuta dentro
de un clon.

## Qué NO es el tracker

Las colas `docs/COLA_*.md`, `docs/PLAN_MEJORA_POST_BETA.md` y
`docs/Deuda_Tecnica_y_UX.md` son seguimiento dentro del repo, no issues. Si un
issue cierra trabajo anotado ahí, nómbralo (fichero y registro `D-xx`/`C-xx`) en
el cuerpo; no muevas ni reescribas esos ficheros desde un issue.

## Pull requests as a request surface

**PRs as a request surface: no.** _(Cambia a `yes` si este repo trata PRs externas
como peticiones de funcionalidad; `/triage` lee este flag.)_

Cuando esté en `yes`, las PRs pasan por las mismas etiquetas y estados que los
issues, con los equivalentes `gh pr`:

- **Leer una PR**: `gh pr view <number> --comments` y `gh pr diff <number>` para el diff.
- **Listar PRs externas para triage**: `gh api --paginate 'repos/{owner}/{repo}/pulls?state=open' --jq '.[] | select(.author_association | IN("OWNER","MEMBER","COLLABORATOR") | not) | {number, title, author: .user.login, author_association, labels: [.labels[].name]}'`.
- **Comentar / etiquetar / cerrar**: `gh pr comment`, `gh pr edit`, `gh pr close`.

## Cuando un skill dice "publica en el issue tracker"

Crea un issue de GitHub.

## Cuando un skill dice "trae el ticket relevante"

Léelo como en **Leer un issue**.

## Wayfinding

Lo usa `/wayfinder`. El **mapa** es un único issue con **issues hijos** como tickets.

- **Mapa**: un issue con la etiqueta `wayfinder:map` que contiene el cuerpo
  Notes / Decisions-so-far / Fog. `gh issue create --label wayfinder:map`.
- **Ticket hijo**: un issue enlazado al mapa como sub-issue de GitHub (ver
  **Convertir un issue en sub-issue de otro**). Donde no haya sub-issues, añade el
  hijo a una task list del cuerpo del mapa y pon `Part of #<map>` al principio del
  cuerpo del hijo. Etiquetas: `wayfinder:<type>` (`research`/`prototype`/`grilling`/`task`).
  Una vez reclamado, el ticket se asigna a quien lo lleva.
- **Bloqueos**: las **dependencias nativas** de GitHub, la representación canónica
  y visible en la UI. Añade una arista con
  `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`,
  donde `<blocker-db-id>` es el **database id** numérico del bloqueador
  (`gh api repos/<owner>/<repo>/issues/<n> --jq .id`, _no_ el `#number` ni el
  `node_id`). GitHub expone `issue_dependencies_summary.blocked_by` (solo
  bloqueadores abiertos, la puerta viva). Donde no haya dependencias, cae a una
  línea `Blocked by: #<n>, #<n>` al principio del cuerpo del hijo. Un ticket está
  desbloqueado cuando todos sus bloqueadores están cerrados.
- **Consulta de frontera**: lista los hijos abiertos del mapa
  (`gh issue list --state open`, acotado a los sub-issues / task list del mapa),
  descarta los que tengan un bloqueador abierto
  (`issue_dependencies_summary.blocked_by > 0`, o un issue abierto en la línea
  `Blocked by`) o un assignee; gana el primero en el orden del mapa.
- **Reclamar**: `gh issue edit <n> --add-assignee @me`, la primera escritura de la sesión.
- **Resolver**: `gh issue comment <n> --body "<respuesta>"`, luego `gh issue close <n>`,
  y luego añade un puntero de contexto (gist + enlace) a Decisions-so-far del mapa.

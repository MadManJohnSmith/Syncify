# Syncify Documentation (English)

[Español](README.md)

This folder holds the project's functional specifications and architecture
design decisions. The index states which document is the current reference
for what, and which files are historical evidence only, so nobody mistakes a
dated snapshot for current state. Where a document has not been translated,
the table below says so instead of leaving an English reader to discover it
mid-page.

**Working in this repo?** `AGENTS.md` at the repository root is the entry map
for agents: surfaces, deterministic commands, the gates that must not be
relaxed, and the landmines. Start there.

## How this documentation is organised

| Layer | Where | What it is |
|---|---|---|
| Entry map | `AGENTS.md`, `GLOSSARY.md` (repo root) | Where to start and domain vocabulary |
| Durable specs | `features/`, `adr/`, `APP_FLOW.en.md`, `design-system.md` | What the system is today |
| Agent configuration | `agents/` | Tracker, triage labels and consumption rules for this documentation |
| Living registers | this folder: the debt register, `COLA_*.md` queues, plans | Where the work and the debt stand |
| Historical | `historico/` | Dated snapshots: evidence, **not** current state |
| Audit trail | `auditoria/` | Per-track logs and the `matriz-ajustes.md` ledger |

## Language status

| Document | Language |
|---|---|
| `README.en.md` (this file) | English |
| `APP_FLOW.en.md` | English |
| `features/` design specs (8 files) | Spanish only |
| `adr/0001-identidad-de-tracks.md` | Spanish only |
| `PLAN_UNIFICACION_IMPORTACION.md` | Spanish only |
| `MATRIZ_PARIDAD_IMPORTACION.md` | Spanish only |
| `Deuda_Tecnica_y_UX.md` | Spanish only |
| `LYRICS_PROVIDER_MATRIX.md` | Spanish only |
| `diagrams/*.en.html` | English (`*.html` are the Spanish builds) |

Every Spanish-only document listed above still carries the guarantee the CI
gate `scripts/tests/test_docs_consistency.py` enforces: each file path it
cites resolves in the tree, so a path in them is a path you can open.

## Feature specs — `features/`

Functional specifications for the main subsystems. Each one opens with a
review stamp (`**Estado de revisión:**`) naming the date it was revalidated
against the code.

- `features/auth-flow.md`: OAuth, PKCE and browser session-capture flows,
  multi-account reconciliation.
- `features/authentication.md`: the debug-only Deezer login diagnostic probe
  (cookie jar, no values) from AUTH-WV-01.
- `features/download-pipeline.md`: orchestrator architecture, the queue and
  the native per-provider delivery pipelines.
- `features/track-browser.md`: the track-list machinery — the virtual-window
  kernel and the library composable (ADR-0025).
- `features/library-import.md`: import algorithms, deduplication and canonical
  identity.
- `features/metadata-enrichment.md`: enrichment from MusicBrainz, Last.fm and
  AcoustID, and the background worker.
- `features/playlist-system.md`: playlist synchronisation and persistence.
- `features/settings-system.md`: hierarchical configuration and its SQLite
  persistence.
- `features/PLAYER_PHASE1_MPRIS.md`: the built-in player — MediaSession
  integration, the native engine (`crates/syncify-player`) and the native MPRIS
  service (`src-tauri/src/services/mpris.rs`). An end-to-end D-Bus check in a
  real graphical session has **never** been run in this repository.

## Architecture decisions and flow

- `adr/`: architecture and scope decisions, numbered by date and written in
  Spanish. They are the only decision record; the old `DECISION_*.md` files are
  three-line pointers kept so dated citations still resolve.
  - `0001-identidad-de-tracks.md`: the canonical key
    `track_sources(service_id, service_track_id)` and the identity columns.
  - `0002-alcance-beta.md`: what stays out of the beta and under which condition
    it returns; the playback-lane amendment.
  - `0003-seleccion-de-edicion.md`: the beta decision against wrong-edition
    downloads; identity contracts ED-01…ED-04 and acceptance tests.
  - `0004-encolado-no-bloqueante.md`: the download queue must not block the UI and must scale to tens of thousands of tracks (proposed).
  - `0005-enrutado-de-descargas-por-cuenta.md`: a download is routed to an entitled account, reusing evidence from earlier deliveries (proposed).
  - `0006-contrato-de-sincronizacion.md`: the sync contract — configurable mode
  - `0007-cuentas-multiples-y-sesiones.md`: multiple accounts per service with no
  - `0008-modelo-de-biblioteca.md`: scope, tabs, detail panel and bulk selection over
    the aggregated library (proposed).
    active account, aggregated library, per-account webview profile and lifecycle (proposed).
    defaulting to import-only, quarantine, triggers, traversal and coverage (proposed).
- `APP_FLOW.en.md`: an end-to-end walkthrough of the current flow, from
  connecting an account through library import to a verified download on disk.
- `design-system.md`: the 10 themes (4 standard + 6 service), CSS tokens and
  the anti-FOUC rules (Spanish).

## Living matrices and plans

- `MATRIZ_PARIDAD_IMPORTACION.md`: living compatibility matrix, one column per
  service. This is the reference for what each provider can actually deliver —
  read it before trusting any blanket import claim (Spanish).
- `LYRICS_PROVIDER_MATRIX.md`: the catalogue of 12 user-configurable lyrics
  sources and their priority layer (renamed from `LYRICS_16_PROVIDER_MATRIX.md`;
  the fixed 16-strategy cascade it described no longer exists).
- `PLAN_UNIFICACION_IMPORTACION.md`: master plan for import parity across the
  six streaming services, in execution with per-phase status (Spanish).
- `COLA_FLUJO_DESCARGAS_PRE_BETA.md`: living queue for download-flow timing
  and fallback work (DLX-01…06, DFL-01: nothing applied yet).
- `COLA_RENDIMIENTO_FRONTEND_2026-10-13.md`: living queue for UI performance
  (FE-PERF-01…04; FE-PERF-02 applied in code).
- `COLA_WEBKIT_COMPOSICION_ACELERADA.md`: open WebKitGTK composition
  hypothesis with its compatibility invariants.
- `HANDOFF.md`: live handoff dated 2026-11-05; per-flow state and next step (downloads).
- `PLAN_MEJORA_POST_BETA.md`: the post-beta waiting list (not a work queue).
  Note: its IDs `C-01…C-06` and `D-01/D-02` are **its own** namespace and do
  not match the debt register's.

## Current state — what is done, and what is not

`Deuda_Tecnica_y_UX.md` is the living register of known debt: open debt
(D-01…D-08), accepted risks, the conditioned scope (what this register does
**not** declare pending) and the closed table (C-01…C-22). Spanish only.

The handoff closures are dated evidence, not current state: after the
**2026-10-11** closure, new queues of manually-found issues opened and the
tree keeps moving. The short version, for anyone reading the public pages:
Rust was green at the 2026-10-11 handoff. The recorded 2026-10-12 reruns
passed the UI suite (993 tests in 121 files; unversioned local record
`auditoria/handoff-vitest-rerun.txt`) and the Python suite (279 tests), with
frontend build and type checks reported OK. These are historical results, not
a claim about this revision. A real graphical session, live network, and
Windows/MSVC target have not been verified; consult the handoff and current
test runs before claiming release readiness.

Internal (Spanish-only) audit trail, if you are looking for why a change was
made or what is still owed:

- `auditoria/matriz-ajustes.md`: the R1 ledger that governs any new adjustment
  ("no adjustment without a prior row"). It is the canonical state for almost
  every queue and bitácora item.
- `auditoria/triaje.md` and `auditoria/cierre-handoff-2026-10-11.md`: the
  triage of orphan findings and the verifiable handoff closure, with its
  explicit list of what it did **not** verify.
- `auditoria/*-bitacora.md`: the per-track logs behind the individual fixes.
- `auditoria/CONSOLIDATED_PLAN.md` and
  `auditoria/AUDITORIA-PILARES-2026-10-18.md`: the OAuth+performance plan
  (essentially executed; errata note at its top) and the pillars audit.

## Historical — `historico/`

Dated records kept as evidence. None of them is current state; each carries a
"Estado actual" banner stating what was resolved afterwards and what is still
open only in that file:

- `PLAN-Correccion-168.md`: the consumed v2 plan of the 168; v1 retractions
  and ID→lane traceability.
- `PENDIENTES.md`: the v0.4.1 backlog closure act (no open work).
- `INFORME_HALLAZGOS_OMITIDOS.md`: what was deliberately set aside, and why.
- `Errores-Pruebas-2026-10-08.md`, `-11.md`, `-13.md` and `-13-flujo-limpio.md`:
  the manual-testing registers and their queues (ID spaces G-, L-, Q-, DSC-,
  DL-SVC-, MF-…).
- `perf-probe-2026-10-12.md` and `perf-run-2026-10-12.md`: backend performance
  measurements (the ISRC expression index is still not built).
- `RELEASE_NOTES_v0.6.0.md`, `RELEASE_NOTES_v0.6.1.md` and
  `RELEASE_NOTES_v0.6.2.md`: notes for the tagged releases `v0.6.0`, `v0.6.1`
  and `v0.6.2`.

## Identifier namespaces

IDs repeat across documents with different meanings. Always cite the ID
**together with its file**:

| Prefix | Defining source | Collision to watch |
|---|---|---|
| `D-01…D-08`, `C-xx`, `A-01` | `Deuda_Tecnica_y_UX.md` | `C-12/13/14` appear twice in the closed table; no `D-05` exists |
| `C-01…C-06`, `D-01/D-02` | `PLAN_MEJORA_POST_BETA.md` | **Collides** with the debt register: different meanings |
| `Q-01…Q-17` | `historico/Errores-Pruebas-2026-10-11.md` | `Q-01` also exists in `auditoria/triaje.md` with another meaning |
| `G-`, `L-`, `DSC-`, `FS-`, `AQ-`, `PR-`, `MET-`, `LY-`, `LOG-`, `M-`, `P-`, `PL-`, `BP-`, `DUP-`, `SYM-`, `BPM-` | `historico/Errores-Pruebas-2026-10-08.md` + `auditoria/triaje.md` + `auditoria/matriz-ajustes.md` | `L-`/`D-` reappear in the 10-13 register |
| `DLX-`, `DFL-` | `COLA_FLUJO_DESCARGAS_PRE_BETA.md` | — |
| `FE-PERF-` | `COLA_RENDIMIENTO_FRONTEND_2026-10-13.md` | — |
| `DL-SVC-`, `DL-DISK-`, `DL-UX-`, `TRAY-OPEN-`, `PLAY-TRACE-` | `historico/Errores-Pruebas-2026-10-13.md` | `D-16/D-17/D-18` collide with the debt register |
| `MF-*` | `historico/Errores-Pruebas-2026-10-13-flujo-limpio.md` | — |
| `ED-01…ED-04` | `adr/0003-seleccion-de-edicion.md` | — |
| `A1…A5`, `B1…B5` | `auditoria/CONSOLIDATED_PLAN.md` (historical) | Visually similar to the register's `A-01` |
| `SYNC-AUD-0xx` | header of `scripts/tests/test_docs_consistency.py` | No separate report enumerates them |

## Diagrams and assets

- `diagrams/`: explorable HTML diagrams. Each has an English build
  (`*.en.html`) and a Spanish build (`*.html`):
  [architecture](diagrams/architecture.en.html) ·
  [download journey](diagrams/download-journey.en.html) ·
  [library care](diagrams/library-care.en.html) ·
  [user journey](diagrams/user-journey.en.html).
- `screenshots/`, `design-mockups/`, `logo-candidatos/`: front-page and design
  assets, not documentation.

## Directory READMEs

- `crates/README.md` (at `crates/`): what each workspace crate does.
- `scripts/maintenance/README.md`: script maintenance tooling.
- `src-tauri/binaries/README.md`: bundled external binaries.
- `ui/public/fonts/README.md`: locally served fonts (no network).

Back to the front pages: [`../README.md`](../README.md) (English) and
[`../README.es.md`](../README.es.md) (Spanish).

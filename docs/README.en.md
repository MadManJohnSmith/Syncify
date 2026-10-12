# Syncify Documentation (English)

[Español](README.md)

This folder holds the project's functional specifications and architecture
design decisions. This index is the English entry point: it says what each
document covers and, where a document has not been translated yet, says so
instead of leaving an English reader to discover it mid-page.

## Language status

| Document | Language |
|---|---|
| `README.en.md` (this file) | English |
| `features/` design specs (7 files) | Spanish only |
| `DECISION_IDENTIDAD_TRACKS.md` | Spanish only |
| `PLAN_UNIFICACION_IMPORTACION.md` | Spanish only |
| `MATRIZ_PARIDAD_IMPORTACION.md` | Spanish only |
| `Deuda_Tecnica_y_UX.md` | Spanish only |
| `INFORME_HALLAZGOS_OMITIDOS.md` | Spanish only |
| `LYRICS_16_PROVIDER_MATRIX.md` | English |
| `diagrams/*.en.html` | English (`*.html` are the Spanish builds) |

Every Spanish-only document listed above still carries the guarantee the CI
gate `scripts/tests/test_docs_consistency.py` enforces: each file path it
cites resolves in the tree, so a path in them is a path you can open.

## Feature specs — `features/`

Functional specifications for the main subsystems. Each one opens with a
review stamp naming the date it was revalidated against the code.

- `features/auth-flow.md`: OAuth, PKCE and browser session-capture flows.
- `features/download-pipeline.md`: orchestrator architecture and the native
  per-provider delivery pipelines.
- `features/library-import.md`: import algorithms, deduplication and canonical
  identity.
- `features/metadata-enrichment.md`: enrichment from MusicBrainz, Last.fm and
  AcoustID.
- `features/playlist-system.md`: playlist synchronisation and persistence.
- `features/settings-system.md`: hierarchical configuration and its SQLite
  persistence.
- `features/PLAYER_PHASE1_MPRIS.md`: the built-in player's phase 1 — the
  MediaSession integration and the MPRIS spike. Its own stamp says the MPRIS
  half is **not** verified in a graphical session, and that is still the
  standing state: nothing in this repository has run a real D-Bus check of
  it yet.

## Current state — what is done, and what is not

The **2026-10-11** handoff closure is documented; a new queue of issues found during manual app testing opened afterward. This working tree is still in progress.
Two Spanish documents record it, and they are the authoritative answer to
"where does this repo stand":

- `HANDOFF-2026-10-11.md`: what is done and verified, what is still open, and
  which product decisions are blocking.
- `auditoria/cierre-handoff-2026-10-11.md`: the closing run's evidence and the
  explicit list of things it did **not** verify.

The short version, for anyone reading the public pages: Rust was green on
2026-10-11; in the 2026-10-12 rerun done ahead of the release, the UI suite
(993 tests across 121 files) and the Python suite (279 tests) both passed, and
the frontend build and type checks are OK. The real graphical session, the live
network and the Windows/MSVC target have not been exercised, which is the honest
reason the public front pages do not call the app release-ready.

Internal (Spanish-only) audit trail, if you are looking for why a change was
made or what is still owed:

- `auditoria/triaje.md` and `auditoria/matriz-ajustes.md`: the audit triage and
  the change register that governs new adjustments.
- `auditoria/*-bitacora.md`: the per-track logs behind the individual fixes.
- `PLAN-Correccion-168.md`, `Errores-Pruebas-2026-10-08.md` and
  `Errores-Pruebas-2026-10-11.md`: the correction plan and the recorded
  test-failure polls. The 2026-10-11 one is the new queue (status bar,
  accounts, download feedback, migrate, metadata issues, lyrics and the freeze
  on playback); Q-04, Q-06, Q-07 and Q-12 are marked **done** with automated
  tests, and the rest remain pending triage or real-session verification.

Back to the front pages: [`../README.md`](../README.md) (English) and
[`../README.es.md`](../README.es.md) (Spanish).

## Architecture and process

- `DECISION_IDENTIDAD_TRACKS.md`: architecture decision record (ADR) for the
  canonical key `track_sources(service_id, service_track_id)`.
- `PLAN_UNIFICACION_IMPORTACION.md`: master plan for import parity across the
  six streaming services.
- `MATRIZ_PARIDAD_IMPORTACION.md`: living compatibility matrix, one column per
  service. This is the reference for what each provider can actually deliver —
  read it before trusting any blanket import claim.
- `Deuda_Tecnica_y_UX.md`: the tracked register of known technical debt,
  including the accepted risks.
- `INFORME_HALLAZGOS_OMITIDOS.md`: report on findings that were set aside.

## Reference

- `LYRICS_16_PROVIDER_MATRIX.md`: the lyrics resolution cascade, 16 strategies
  across 10 providers, audited against the Rust engine.
- `diagrams/`: explorable HTML diagrams. Each has an English build
  (`*.en.html`) and a Spanish build (`*.html`):
  [architecture](diagrams/architecture.en.html) ·
  [download journey](diagrams/download-journey.en.html) ·
  [library care](diagrams/library-care.en.html) ·
  [user journey](diagrams/user-journey.en.html).

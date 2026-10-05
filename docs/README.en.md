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
| `features/` design specs (6 files) | Spanish only |
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

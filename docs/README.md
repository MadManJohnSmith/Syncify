# Documentación del Proyecto Syncify

[English](README.en.md)

Esta carpeta centraliza las especificaciones funcionales y decisiones de diseño de arquitectura del proyecto.

## Estructura de Documentación

### Especificaciones de Módulos y Arquitectura Vigente
- **`features/`**: Especificaciones funcionales de los subsistemas principales:
  - `auth-flow.md`: Flujos de autenticación OAuth, PKCE y scraping de sesión.
  - `download-pipeline.md`: Arquitectura del orquestador y los motores nativos por proveedor (FLAC directo en Qobuz, flujos segmentados en Tidal).
  - `library-import.md`: Algoritmos de importación, deduplicación e identidad canónica.
  - `metadata-enrichment.md`: Enriquecimiento con MusicBrainz, Last.fm y AcoustID.
  - `playlist-system.md`: Sincronización y persistencia de listas de reproducción.
  - `settings-system.md`: Configuración jerárquica y persistencia en SQLite.
- **`DECISION_IDENTIDAD_TRACKS.md`**: Registro de decisión arquitectónica (ADR) sobre la clave canónica `track_sources(service_id, service_track_id)`.
- **`PLAN_UNIFICACION_IMPORTACION.md`**: Plan maestro de paridad de importación entre los 6 servicios de streaming.
- **`MATRIZ_PARIDAD_IMPORTACION.md`**: Matriz viva de compatibilidad por servicio.
- **`LYRICS_16_PROVIDER_MATRIX.md`**: Matriz técnica de cascada de resolución de letras (16 estrategias).
- **`Deuda_Tecnica_y_UX.md`**: registro vivo de la deuda técnica conocida y de los riesgos aceptados.
- **`INFORME_HALLAZGOS_OMITIDOS.md`**: informe de los hallazgos que quedaron fuera del cuerpo de la auditoría.
- **`diagrams/`**: diagramas HTML explorables, cada uno con su versión inglesa (`*.en.html`) y española (`*.html`).

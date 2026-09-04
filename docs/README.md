# Documentación del Proyecto Syncify

Esta carpeta centraliza las especificaciones funcionales, decisiones de diseño de arquitectura y el histórico de auditorías y sprints del proyecto.

## Estructura de Documentación

### 1. Especificaciones de Módulos y Arquitectura Vigente
- **`features/`**: Especificaciones funcionales de los subsistemas principales:
  - `auth-flow.md`: Flujos de autenticación OAuth, PKCE y scraping de sesión.
  - `download-pipeline.md`: Arquitectura del orquestador y motores nativos FLAC/DASH.
  - `library-import.md`: Algoritmos de importación, deduplicación e identidad canónica.
  - `metadata-enrichment.md`: Enriquecimiento con MusicBrainz, Last.fm y AcoustID.
  - `playlist-system.md`: Sincronización y persistencia de listas de reproducción.
  - `settings-system.md`: Configuración jerárquica y persistencia en SQLite.
- **`DECISION_IDENTIDAD_TRACKS.md`**: Registro de decisión arquitectónica (ADR) sobre la clave canónica `track_sources(service_id, service_track_id)`.
- **`PLAN_UNIFICACION_IMPORTACION.md`**: Plan maestro de paridad de importación entre los 6 servicios de streaming.
- **`MATRIZ_PARIDAD_IMPORTACION.md`**: Matriz viva de compatibilidad por servicio.
- **`LYRICS_16_PROVIDER_MATRIX.md`**: Matriz técnica de cascada de resolución de letras (16 estrategias).

### 2. Archivos Históricos de Auditorías y Sprints Pasados

- **`sprints_archive/`**: Checklists matutinos, minutas de relevo de turnos (S195, S197) y checklists de auditoría manual/automática anteriores.

---


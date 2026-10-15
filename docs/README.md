# Documentación del Proyecto Syncify

[English](README.en.md)

Esta carpeta centraliza las especificaciones funcionales y decisiones de diseño de arquitectura del proyecto. Es también la puerta de entrada al estado real del repo: si quieres saber qué está hecho y qué no, empieza por «Estado actual».

## Estructura de Documentación
### Especificaciones de Módulos y Arquitectura Vigente
- **`features/`**: Especificaciones funcionales de los subsistemas principales (7 ficheros):
  - `auth-flow.md`: Flujos de autenticación OAuth, PKCE y scraping de sesión.
  - `download-pipeline.md`: Arquitectura del orquestador y los motores nativos por proveedor (FLAC directo en Qobuz, flujos segmentados en Tidal).
  - `library-import.md`: Algoritmos de importación, deduplicación e identidad canónica.
  - `metadata-enrichment.md`: Enriquecimiento con MusicBrainz, Last.fm y AcoustID.
  - `playlist-system.md`: Sincronización y persistencia de listas de reproducción.
  - `settings-system.md`: Configuración jerárquica y persistencia en SQLite.
  - `PLAYER_PHASE1_MPRIS.md`: fase 1 del reproductor integrado — integración con MediaSession y spike de MPRIS. Su propio sello advierte de que la parte MPRIS **no** está verificada en sesión gráfica, y sigue siendo así: en este repo nunca se ha ejecutado una comprobación real de D-Bus.
- **`DECISION_IDENTIDAD_TRACKS.md`**: Registro de decisión arquitectónica (ADR) sobre la clave canónica `track_sources(service_id, service_track_id)`.
- **`DECISION_SELECCION_EDICION.md`**: Decisión beta y contexto verificable para evitar descargas/importaciones de una edición incorrecta; contratos de identidad, bloques ED-01…ED-04 y pruebas de aceptación.
- **`PLAN_UNIFICACION_IMPORTACION.md`**: Plan maestro de paridad de importación entre los 6 servicios de streaming.
- **`MATRIZ_PARIDAD_IMPORTACION.md`**: Matriz viva de compatibilidad por servicio.
- **`LYRICS_16_PROVIDER_MATRIX.md`**: Matriz técnica de cascada de resolución de letras (16 estrategias).
- **`Deuda_Tecnica_y_UX.md`**: registro vivo de la deuda técnica conocida y de los riesgos aceptados.
- **`INFORME_HALLAZGOS_OMITIDOS.md`**: informe de los hallazgos que quedaron fuera del cuerpo de la auditoría.
- **`diagrams/`**: diagramas HTML explorables, cada uno con su versión inglesa (`*.en.html`) y española (`*.html`).

### Estado actual — qué está hecho y qué no

El cierre del relevo del **2026-10-11** está documentado; después se abrió una nueva cola de incidencias detectadas en pruebas manuales. Este árbol de trabajo sigue en desarrollo. Dos documentos en español son la referencia autoritativa sobre «dónde está este repo»:

- `HANDOFF-2026-10-11.md`: lo que está hecho y verificado, lo que sigue abierto y qué decisiones de producto bloquean.
- `auditoria/cierre-handoff-2026-10-11.md`: la evidencia de la corrida de cierre y la lista explícita de lo que **no** verificó.

En corto, para quien lea las páginas públicas: Rust estaba en verde el 2026-10-11; en la repetición del 2026-10-12 previa al lanzamiento pasaron la suite UI (993 pruebas en 121 archivos) y la suite Python (279 pruebas), y el build y los tipos del frontend quedaron OK. La sesión gráfica real, la red en vivo y el target Windows/MSVC no se han ejercitado, así que las portadas públicas no declaran la app lista para publicar.

- **`auditoria/`**: rastro de auditoría interno. Incluye `triaje.md` y `matriz-ajustes.md` (el registro de cambios que gobierna cualquier ajuste nuevo), las `*-bitacora.md` por carril, los registros de ejecución del relevo (`handoff-*.txt`, entre ellos la repetición del 2026-10-12: `handoff-vitest-rerun.txt` y `handoff-python-tests-rerun.txt`) y el cierre del relevo.
- **`PLAN-Correccion-168.md`**, **`Errores-Pruebas-2026-10-08.md`** y **`Errores-Pruebas-2026-10-11.md`**: el plan de corrección y los registros de fallos y pruebas anotadas. El del 11 es una cola nueva (status-bar, cuentas, descargas, migración, metadatos, letras y reproducción): Q-04, Q-06, Q-07 y Q-12 están marcadas **HECHO** con pruebas automatizadas, mientras las demás siguen pendientes de triaje o verificación real.

Vuelta a las portadas públicas: [`../README.md`](../README.md) (inglés) y
[`../README.es.md`](../README.es.md) (español).

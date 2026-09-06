# ADVERTENCIA: NO EJECUTAR - SCRIPTS OBSOLETOS ARCHIVADOS PARA AUDITORÍA HISTÓRICA

Este directorio contiene scripts heredados de reparación ad-hoc y manipulación directa de base de datos (`syncify.db`) que han sido desmantelados y purgados del árbol productivo como parte de la remediación **[TASK-126]**.

## ⚠️ Peligro de Corrupción de Integridad

Los scripts en este directorio:
1. Manipulaban directamente tablas del sistema como `_sqlx_migrations` (falsificando hashes SHA-384 para evadir el mecanismo de checksum canónico de `sqlx`).
2. Ejecutaban sentencias DDL (`ALTER TABLE`) y DML destructivas o mutaciones masivas fuera del ciclo formal de migraciones versionadas y del pipeline de Rust (`syncify-core-domain`, worker y transacciones controladas).
3. Dependían de rutas absolutas locales (`C:\Users\tardis\...`) no portables ni deterministas.

## 🚫 Política Estricta

**ESTÁ ESTRICTAMENTE PROHIBIDO EJECUTAR CUALQUIERA DE ESTOS SCRIPTS EN AMBIENTES DE DESARROLLO, STAGING O PRODUCCIÓN.**

Cualquier cambio de esquema debe implementarse exclusivamente a través de migraciones canónicas de `sqlx` en `src-tauri/migrations/`. Cualquier saneamiento o backfill de datos debe ocurrir dentro de las transacciones protegidas del runtime de Rust con sus respectivos tests de invariantes y prevención de recurrencia.

## Contenido Archivado

- `fix_migration_table.py`: Script que falsificaba entradas e inyectaba checksums artificiales en `_sqlx_migrations`.
- `apply_s81_migrations.py`: Script con DDLs fuera de ciclo y rutas absolutas Windows.
- `phase1_sql_repair.py` / `phase1_sql_repair.sql`: Reparación masiva ad-hoc de metadatos fuera del pipeline.
- `dedup_playlists.sql`: Sentencias SQL directas de deduplicación sin control de transacciones en runtime.
- `recalculate_audio_quality.py` / `recalculate_audio_quality.sql`: Recálculo ad-hoc de calidades en lugar del pipeline de análisis de streams.
- `repopulate_downloads_lyrics.sql`: Inserciones SQL directas fuera del worker de descargas.
- `backfill_featured_artists.py`: Backfill directo en SQLite eludiendo el grafo de artistas.
- `reset_phantom_enrichment.py` / `reset_phantom_enrichment.sql`: Resets directos de estado de enriquecimiento sin control de estado de máquina.
- `cache_mb_artists.py`: Fabricación ad-hoc de identificadores y caché fuera de los servicios de metadatos.
- `tag_probe.sh`, `tag_probe_phase2.sh`, `tag_probe_cleanup.sh`: Scripts shell de exploración y alteración de etiquetas.

-- Migration 0104: tamaño estimado del job de descarga (D02, spec #111)
-- Aditiva. La spec del modelo de job incluye `estimated_size_mb` y la 0102
-- no la creó; el panel de D08 la mostrará y la estimación de D02 la persiste.

ALTER TABLE download_jobs ADD COLUMN estimated_size_mb REAL;

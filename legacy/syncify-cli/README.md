# Syncify CLI & Experimental Prototyping Module (`legacy/syncify-cli`)

> **Estado**: Módulo histórico de prototipado y versión CLI en resguardo dentro del repositorio principal.

---

## 📌 Propósito y Alcance

Este subdirectorio contiene la versión **CLI recortada**, scripts de desarrollo y módulos experimentales desarrollados durante fases tempranas de Syncify. Se conserva directamente dentro del repositorio principal Git en `legacy/syncify-cli/` para preservar el progreso del proyecto, facilitar futuras integraciones incrementales y evitar la pérdida de código en respaldos externos.

---

## 📂 Contenido del Módulo

### 1. Binarios CLI (`src/bin/`)
- `syncify_cli.rs`: Interfaz de línea de comandos principal.
- `qobuz_url_downloader.rs` & `real_qobuz_downloader.rs`: Herramientas de descarga directa por URL.
- `qobuz_test.rs`, `batch_test.rs`, `generate_all_test_tracks.rs`, `verify_s113_tags.rs`: Scripts de prueba y verificación masiva.

### 2. Módulos de Descarga y Metadata (`src/download/` y `src/metadata/`)
- `tag_writer.rs`: Escritor avanzado de metadatos FLAC/VorbisComment via Lofty.
- `artist_info.rs`, `layout.rs`, `staging.rs`, `rescue.rs`, `playlist_resolver.rs`: Infraestructura de descarga y organización de archivos.
- `bandcamp.rs`, `soulseek.rs`: Prototipos iniciales de nuevos conectores.

### 3. Servicios de Enriquecimiento y Migraciones (`src/services/` y `migrations/`)
- `enrichment.rs`: Motor de resolución y enriquecimiento de metadatos (MusicBrainz/Last.fm/Spotify).
- `discogs.rs`: Conector experimental para Discogs.
- `0046_add_enrichment_source_types.sql`: Migración de base de datos para soporte de fuentes de enriquecimiento.
- `scripts/essentia_bridge.py`: Script de integración Python con la biblioteca de análisis de audio Essentia.

---

## ⚙️ Independencia del Workspace

Este paquete se encuentra **independiente del workspace de la aplicación gráfica desktop (`src-tauri`)** para garantizar que los prototipos no interfieran ni afecten la compilación principal del cliente gráfico.

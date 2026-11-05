# Crates del workspace

Los crates son la capa de dominio y de piezas pesadas que el núcleo
(`src-tauri`) y los puentes consumen. Cada uno se describe con la primera
línea de su propio `lib.rs`: si esa descripción y el código dejan de
coincidir, es señal de que el crate se movió y la doc no.

| Crate | Qué hace |
|---|---|
| `syncify-core-domain` | Dominio puro sin I/O: política de calidad (piso 320 kbps en `quality.rs`), modelos de error (`ErrorTaxonomy` en `errors.rs`), esquema de manifest, eventos de progreso tipados, reglas de preservación de portadas, validadores de bytes y utilidades de proceso (`CREATE_NO_WINDOW` en `process.rs`). |
| `syncify-metadata-domain` | Contrato de dominio y motor determinista de precedencia para el enriquecimiento de metadatos: scoring, conflicto de fuentes, género, idioma y país (`scoring.rs`, `genre.rs`, `language.rs`, `country.rs`). |
| `syncify-lyrics-domain` | Contrato de dominio de letras y motor determinista puro: cabeceras LRC, marcadores de borrado y manejo de karaoke (`header.rs`). |
| `syncify-flac-writer` | Escritura de tags VorbisComment en FLAC agnóstica de fuente, con preservación no destructiva de `METADATA_BLOCK_PICTURE` y del WebP animado; incluye la sanitización con recuperación de portadas (`sanitize_flac_pictures_with_recovery`, `salvage_cover_to_jpeg`). |
| `syncify-fmp4-flac` | Conversión de FLAC en ISOBMFF fragmentado (el DASH de Tidal) a FLAC nativo. |
| `syncify-tidal-downloader` | Descargador segmentado del DASH de Tidal; usa `syncify-fmp4-flac` para remontar el contenedor. |
| `syncify-player` | Motor de reproducción local nativo (rodio + symphonia); sustituye a la ruta retirada `<audio>` + `syncify-media://` (C-19 en `docs/Deuda_Tecnica_y_UX.md`). |

Los crates no dependen de Tauri ni de la BD: esa es la frontera. Si un cambio
necesita tocar `src-tauri` y un crate a la vez, el tipo compartido pertenece al
crate de dominio correspondiente, no al núcleo.

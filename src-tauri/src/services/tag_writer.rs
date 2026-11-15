//! FLAC Metadata DTO & Tag Writer for `src-tauri` using `metaflac`.
//!
//! Re-exported from shared workspace crate `syncify-flac-writer` (Corte 1B).
//! SB03 (#198) añade la política de etiquetado: los flags `write_*` de
//! `metadata_preferences` filtran el DTO antes de escribir.

use crate::models::MetadataPreferences;

pub use syncify_flac_writer::*;
pub use syncify_flac_writer::{FlacMetadata, TagWriteOptions};

/// Carga la política de etiquetado desde `metadata_preferences` (fila 1).
/// Si la fila no existe usa los defaults de fábrica, de modo que la política
/// nunca bloquea una descarga.
pub async fn load_tag_policy(db: &sqlx::SqlitePool) -> MetadataPreferences {
    sqlx::query_as::<_, MetadataPreferences>("SELECT * FROM metadata_preferences WHERE id = 1")
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// SB03 (#198): aplica los flags `write_*` al DTO. Un campo a `None` no se
/// escribe; los flags de provenancia limpian su valor aunque el flujo lo haya
/// poblado. Con los defaults de fábrica (migración 0105) el resultado es
/// idéntico al histórico.
pub fn apply_tag_policy(meta: &mut FlacMetadata, prefs: &MetadataPreferences) {
    if !prefs.write_releasetype {
        meta.release_type = None;
    }
    if !prefs.write_label {
        meta.label = None;
    }
    if !prefs.write_work_composer {
        meta.work = None;
        meta.composer = None;
    }
    if !prefs.write_musicbrainz_ids {
        meta.musicbrainz_track_id = None;
        meta.musicbrainz_artist_id = None;
        meta.musicbrainz_album_id = None;
        meta.musicbrainz_albumartist_id = None;
        meta.musicbrainz_release_group_id = None;
        meta.musicbrainz_work_id = None;
    }
    if !prefs.write_lyrics_tags {
        meta.lyrics_lrc = None;
    }
    if !prefs.write_download_source {
        meta.audio_source = None;
        meta.lyrics_source = None;
        meta.cover_source = None;
    }
    if !prefs.write_download_date {
        meta.download_date = None;
    }
    if !prefs.write_only_available_on {
        meta.only_available_on = None;
    }
    if !prefs.write_not_available_streaming {
        meta.not_available_streaming = None;
    }
    if !prefs.write_quality_score {
        meta.quality_score = None;
    }
}

/// Opciones del escritor derivadas de la política.
pub fn tag_write_options(prefs: &MetadataPreferences) -> TagWriteOptions {
    TagWriteOptions {
        preserve_custom_tags: prefs.preserve_custom_tags,
    }
}

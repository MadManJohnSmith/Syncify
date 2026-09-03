// S191 — Track tag editor IPC.
//
// Read: raw snapshot of every Vorbis facet present in the audio file, so the
// UI can show all container-written facets (S179 matrix mapping).
// Write: delegates to the existing roundtrip-verified writer
// (`apply_and_verify_flac_tags` from `syncify-flac-writer`, re-exported by
// `services::tag_writer`) and returns its `TagVerification` report.
//
// File resolution mirrors `embed_lyrics`: `downloads.file_path` keyed by the
// unique `downloads.track_id`.

use std::collections::BTreeMap;

/// Raw facet snapshot of an audio file's tags.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrackTagsSnapshot {
    pub track_id: i64,
    pub file_path: String,
    pub file_format: String,
    /// Every Vorbis comment facet currently in the file, sorted by key.
    pub all_tags: BTreeMap<String, Vec<String>>,
    pub has_cover: bool,
    pub cover_mime: Option<String>,
}

async fn resolve_track_audio_path(
    state: &State<'_, crate::AppState>,
    track_id: i64,
) -> Result<(String, String), String> {
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT file_path, file_format FROM downloads WHERE track_id = ? LIMIT 1",
    )
    .bind(track_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    match row {
        Some((path, format)) => Ok((path, format.unwrap_or_else(|| "FLAC".to_string()))),
        None => Err(format!(
            "El track {} no tiene archivo local descargado",
            track_id
        )),
    }
}

#[tauri::command]
pub async fn read_track_tags(
    state: State<'_, crate::AppState>,
    track_id: i64,
) -> Result<TrackTagsSnapshot, String> {
    tracing::info!("read_track_tags: track_id={}", track_id);
    let (file_path, file_format) = resolve_track_audio_path(&state, track_id).await?;

    // FLAC keeps the metaflac path (full Vorbis snapshot + picture info).
    // S200: every other container (M4A/MP3/…) now falls back to ffprobe so the
    // owner can SEE all his tags — the previous hard error hid them entirely.
    // Editing stays FLAC-only (writer boundary unchanged, honest in the UI).
    if !file_path.to_lowercase().ends_with(".flac") {
        return read_tags_via_ffprobe(track_id, file_path, file_format).await;
    }

    // metaflac is blocking file IO; keep the async runtime free.
    let snapshot = tauri::async_runtime::spawn_blocking(move || -> Result<TrackTagsSnapshot, String> {
        let tag = metaflac::Tag::read_from_path(&file_path)
            .map_err(|e| format!("No se pudo leer el archivo FLAC: {}", e))?;

        let mut all_tags = BTreeMap::new();
        if let Some(comments) = tag.vorbis_comments() {
            for (key, values) in comments.comments.iter() {
                all_tags.insert(key.to_uppercase(), values.clone());
            }
        }

        let (has_cover, cover_mime) = tag
            .pictures()
            .next()
            .map(|p| (true, Some(p.mime_type.clone())))
            .unwrap_or((false, None));

        Ok(TrackTagsSnapshot {
            track_id,
            file_path,
            file_format,
            all_tags,
            has_cover,
            cover_mime,
        })
    })
    .await
    .map_err(|e| format!("join error: {}", e))??;

    Ok(snapshot)
}

/// S200 — ffprobe fallback for non-FLAC containers (M4A/MP3/WAV/…).
/// Runs `ffprobe -print_format json -show_format` and maps `format.tags`
/// into the same uppercase-key snapshot the FLAC path produces. Cover-art
/// detection is not available through this path (has_cover=false, honest);
/// the dependency manager guarantees ffmpeg/ffprobe exists (tempo analyzer
/// already shells out to it).
async fn read_tags_via_ffprobe(
    track_id: i64,
    file_path: String,
    file_format: String,
) -> Result<TrackTagsSnapshot, String> {
    let output = crate::cmd_utils::create_tokio_command("ffprobe")
        .args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            &file_path,
        ])
        .output()
        .await
        .map_err(|e| format!("No se pudo ejecutar ffprobe: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe no pudo leer el archivo {}: {}",
            file_path,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let parsed: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("ffprobe JSON inválido: {}", e))?;

    let mut all_tags = BTreeMap::new();
    if let Some(tags) = parsed.pointer("/format/tags").and_then(|t| t.as_object()) {
        for (key, value) in tags {
            let rendered = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            all_tags.insert(key.to_uppercase(), vec![rendered]);
        }
    }

    Ok(TrackTagsSnapshot {
        track_id,
        file_path,
        file_format,
        all_tags,
        has_cover: false,
        cover_mime: None,
    })
}

/// Editable facet payload for the S191 editor. Technical/replay-gain facets
/// stay read-only through `read_track_tags`'s raw view; this payload covers
/// what a human curates. Missing fields are left untouched semantics-free:
/// every field maps onto the existing writer contract (None = skip).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TagEditPayload {
    pub title: String,
    pub artist: String,
    pub album: String,
    #[serde(default)] pub album_artist: Option<String>,
    #[serde(default)] pub composer: Option<String>,
    #[serde(default)] pub genre: Option<String>,
    #[serde(default)] pub style: Option<String>,
    #[serde(default)] pub mood: Option<String>,
    #[serde(default)] pub grouping: Option<String>,
    #[serde(default)] pub language: Option<String>,
    #[serde(default)] pub copyright: Option<String>,
    #[serde(default)] pub label: Option<String>,
    #[serde(default)] pub catalog_number: Option<String>,
    #[serde(default)] pub isrc: Option<String>,
    #[serde(default)] pub release_year: Option<String>,
    #[serde(default)] pub comment: Option<String>,
    #[serde(default)] pub track_number: Option<u32>,
    #[serde(default)] pub track_total: Option<u32>,
    #[serde(default)] pub disc_number: Option<u32>,
    #[serde(default)] pub disc_total: Option<u32>,
    #[serde(default)] pub bpm: Option<u32>,
    #[serde(default)] pub initial_key: Option<String>,
}

impl From<TagEditPayload> for syncify_flac_writer::FlacMetadata {
    fn from(p: TagEditPayload) -> Self {
        syncify_flac_writer::FlacMetadata {
            title: p.title,
            artist: p.artist,
            album: p.album,
            album_artist: p.album_artist,
            composer: p.composer,
            genre: p.genre,
            style: p.style,
            mood: p.mood,
            grouping: p.grouping,
            language: p.language,
            copyright: p.copyright,
            label: p.label,
            catalog_number: p.catalog_number,
            isrc: p.isrc,
            release_year: p.release_year,
            comment: p.comment,
            bpm: p.bpm,
            initial_key: p.initial_key,
            track_number: p.track_number.unwrap_or(0),
            track_total: p.track_total.unwrap_or(0),
            disc_number: p.disc_number.unwrap_or(0),
            disc_total: p.disc_total.unwrap_or(0),
            ..Default::default()
        }
    }
}

/// Write edited facets through the roundtrip-verified writer and return the
/// verification report (`tags_match` == true means the file was re-read and
/// every written facet matched expectations).
#[tauri::command]
pub async fn write_track_tags(
    state: State<'_, crate::AppState>,
    track_id: i64,
    metadata: TagEditPayload,
) -> Result<syncify_flac_writer::TagVerification, String> {
    tracing::info!("write_track_tags: track_id={}", track_id);
    let (file_path, _format) = resolve_track_audio_path(&state, track_id).await?;

    let flac_metadata: syncify_flac_writer::FlacMetadata = metadata.into();
    tauri::async_runtime::spawn_blocking(move || {
        syncify_flac_writer::apply_and_verify_flac_tags(
            std::path::Path::new(&file_path),
            &flac_metadata,
        )
    })
    .await
    .map_err(|e| format!("join error: {}", e))?
}

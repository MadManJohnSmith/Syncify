#[allow(unused_imports)]
use super::*;

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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT file_path, file_format FROM downloads WHERE track_id = ? LIMIT 1")
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
    let snapshot =
        tauri::async_runtime::spawn_blocking(move || -> Result<TrackTagsSnapshot, String> {
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
            "-v",
            "quiet",
            "-print_format",
            "json",
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

    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("ffprobe JSON inválido: {}", e))?;

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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TagEditPayload {
    pub title: String,
    pub artist: String,
    pub album: String,
    #[serde(default)]
    pub album_artist: Option<String>,
    #[serde(default)]
    pub composer: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub mood: Option<String>,
    #[serde(default)]
    pub grouping: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub copyright: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub catalog_number: Option<String>,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub release_year: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub track_number: Option<u32>,
    #[serde(default)]
    pub track_total: Option<u32>,
    #[serde(default)]
    pub disc_number: Option<u32>,
    #[serde(default)]
    pub disc_total: Option<u32>,
    #[serde(default)]
    pub bpm: Option<u32>,
    #[serde(default)]
    pub initial_key: Option<String>,
    #[serde(default)]
    pub artists: Option<Vec<String>>,
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
            artists: p.artists,
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
    metadata: Option<TagEditPayload>,
    tags: Option<TagEditPayload>,
) -> Result<syncify_flac_writer::TagVerification, String> {
    tracing::info!("write_track_tags: track_id={}", track_id);
    let payload = tags
        .or(metadata)
        .ok_or_else(|| "Missing required tags or metadata payload".to_string())?;
    let (file_path, file_format) = resolve_track_audio_path(&state, track_id).await?;

    let flac_metadata: syncify_flac_writer::FlacMetadata = payload.into();
    let tagged_path = file_path.clone();
    let verification = tauri::async_runtime::spawn_blocking(move || {
        syncify_flac_writer::apply_and_verify_flac_tags(
            std::path::Path::new(&tagged_path),
            &flac_metadata,
        )
    })
    .await
    .map_err(|e| format!("join error: {}", e))??;

    // D-03: a manual tag write is also a FLAC write, so the embedded cover art is
    // sanitized (and recovered where possible) right after it lands on disk. Non-fatal:
    // the tag verification result is what this command returns, and a cover loss is
    // reported through repair_history instead of failing the edit.
    if file_format.eq_ignore_ascii_case("flac") {
        let cover_ctx = crate::services::flac_cover_sanitizer::FlacCoverSanitizeContext {
            provenance: "commands.write_track_tags".to_string(),
            download_id: None,
            track_id: Some(track_id),
        };
        if let Err(e) = crate::services::flac_cover_sanitizer::sanitize_and_audit_flac_cover_art(
            Some(&state.db),
            std::path::Path::new(&file_path),
            &cover_ctx,
        )
        .await
        {
            tracing::warn!(error = %e, path = %file_path, "[Tags] FLAC cover sanitization failed (non-fatal)");
        }
    }

    Ok(verification)
}

// ---------------------------------------------------------------------------
// S202 / TASK-7.1 CR-5 — country & region tag repair (dry-run + apply)
// ---------------------------------------------------------------------------
//
// The plan comes from `syncify_metadata_domain::plan_country_repair`, the same
// domain helper the tag writers use to decide what `RELEASECOUNTRY` /
// `RELEASEREGION` must contain. Applying the plan therefore reproduces exactly
// what the next write would have produced, instead of a second opinion.

/// Vorbis tags that carry release country/region and must stay consistent.
const COUNTRY_TAGS: [&str; 3] = ["RELEASECOUNTRY", "COUNTRY", "RELEASEREGION"];

/// Result of a country/region repair (or of its dry run).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CountryTagRepairReport {
    pub track_id: i64,
    pub file_path: String,
    /// `false` for the dry run.
    pub applied: bool,
    pub needs_repair: bool,
    pub plan: syncify_metadata_domain::country::TagRepairPlan,
    /// Tag values read back from the file after the repair (empty when dry run).
    pub applied_tags: BTreeMap<String, Vec<String>>,
    /// Values present on disk at planning time.
    pub current_tags: BTreeMap<String, Vec<String>>,
}

fn first_tag(tags: &BTreeMap<String, Vec<String>>, key: &str) -> Option<String> {
    tags.get(key).and_then(|v| v.first()).map(|s| s.to_string())
}

/// Reads the country/region tags of a FLAC file.
fn read_country_tags(path: &std::path::Path) -> Result<BTreeMap<String, Vec<String>>, String> {
    let tag = metaflac::Tag::read_from_path(path)
        .map_err(|e| format!("No se pudo leer el archivo FLAC: {}", e))?;

    let mut country_tags = BTreeMap::new();
    if let Some(comments) = tag.vorbis_comments() {
        for key in COUNTRY_TAGS {
            if let Some(values) = comments.comments.get(key) {
                if !values.is_empty() {
                    country_tags.insert(key.to_string(), values.clone());
                }
            }
        }
    }
    Ok(country_tags)
}

/// Writes only the country/region tags of a FLAC file, leaving every other tag,
/// picture and metadata block untouched.
fn write_country_tags(
    path: &std::path::Path,
    plan: &syncify_metadata_domain::country::TagRepairPlan,
) -> Result<(), String> {
    let mut tag = metaflac::Tag::read_from_path(path)
        .map_err(|e| format!("No se pudo leer el archivo FLAC: {}", e))?;

    for key in COUNTRY_TAGS {
        let value = match key {
            "RELEASECOUNTRY" | "COUNTRY" => plan.target_country.clone(),
            _ => plan.target_region.clone(),
        };
        match value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            Some(v) => tag.set_vorbis(key, vec![v.to_string()]),
            None => tag.remove_vorbis(key),
        }
    }

    tag.write_to_path(path)
        .map_err(|e| format!("No se pudo escribir el archivo FLAC: {}", e))
}

/// Shared body of the dry-run and apply variants, on a resolved FLAC path.
///
/// Blocking (metaflac file IO); the commands run it inside `spawn_blocking`.
pub fn run_country_tag_repair_on_path(
    track_id: i64,
    file_path: &str,
    apply: bool,
) -> Result<CountryTagRepairReport, String> {
    let path = std::path::Path::new(file_path);
    let current_tags = read_country_tags(path)?;
    let plan = syncify_metadata_domain::plan_country_repair(
        first_tag(&current_tags, "RELEASECOUNTRY").as_deref(),
        first_tag(&current_tags, "RELEASEREGION").as_deref(),
    );

    if !apply {
        return Ok(CountryTagRepairReport {
            track_id,
            file_path: file_path.to_string(),
            applied: false,
            needs_repair: plan.needs_repair,
            plan,
            applied_tags: BTreeMap::new(),
            current_tags,
        });
    }

    if !plan.needs_repair {
        return Ok(CountryTagRepairReport {
            track_id,
            file_path: file_path.to_string(),
            applied: false,
            needs_repair: false,
            plan,
            applied_tags: current_tags.clone(),
            current_tags,
        });
    }

    write_country_tags(path, &plan)?;

    // Re-read from disk: the report states what the file actually contains.
    let applied_tags = read_country_tags(path)?;
    for key in COUNTRY_TAGS {
        let expected = match key {
            "RELEASECOUNTRY" | "COUNTRY" => plan.target_country.as_deref(),
            _ => plan.target_region.as_deref(),
        };
        match (expected, first_tag(&applied_tags, key)) {
            (None, None) => {}
            (Some(_), None) => {
                return Err(format!(
                    "La reparación no pudo escribir {} en {}",
                    key, file_path
                ))
            }
            (None, Some(_)) => {
                return Err(format!(
                    "La reparación no pudo eliminar {} de {}",
                    key, file_path
                ))
            }
            (Some(expected), Some(actual)) if expected.trim() != actual.trim() => {
                return Err(format!(
                    "La reparación de {} no coincide: se esperaba {:?} y se leyó {:?}",
                    key, expected, actual
                ))
            }
            _ => {}
        }
    }

    Ok(CountryTagRepairReport {
        track_id,
        file_path: file_path.to_string(),
        applied: true,
        needs_repair: true,
        plan,
        applied_tags,
        current_tags,
    })
}

/// Resolves the track's audio file and runs the repair off the async runtime.
async fn run_country_tag_repair(
    state: &State<'_, crate::AppState>,
    track_id: i64,
    apply: bool,
) -> Result<CountryTagRepairReport, String> {
    let (file_path, file_format) = resolve_track_audio_path(state, track_id).await?;
    if !file_path.to_lowercase().ends_with(".flac") {
        return Err(format!(
            "La reparación de país/región solo admite FLAC (archivo: {})",
            file_format
        ));
    }

    let owned = file_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        run_country_tag_repair_on_path(track_id, &owned, apply)
    })
    .await
    .map_err(|e| format!("join error: {}", e))?
}

/// S202 / TASK-7.1 CR-5 — dry run: report what the country/region repair would do.
#[tauri::command]
pub async fn plan_country_tag_repair(
    state: State<'_, crate::AppState>,
    track_id: i64,
) -> Result<CountryTagRepairReport, String> {
    tracing::info!("plan_country_tag_repair: track_id={}", track_id);
    run_country_tag_repair(&state, track_id, false).await
}

/// S202 / TASK-7.1 CR-5 — apply the country/region repair to the file on disk.
#[tauri::command]
pub async fn apply_country_tag_repair(
    state: State<'_, crate::AppState>,
    track_id: i64,
) -> Result<CountryTagRepairReport, String> {
    tracing::info!("apply_country_tag_repair: track_id={}", track_id);
    run_country_tag_repair(&state, track_id, true).await
}

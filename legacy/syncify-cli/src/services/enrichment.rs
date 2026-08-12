//! Metadata Enrichment Precedence Engine (Phase 2)
//!
//! Enforces field precedence table across Discogs, MusicBrainz, Last.fm, and Essentia
//! respecting field-level `source_type` ('manual' | 'enrichment') traceability flags.

#![allow(dead_code)]

use crate::services::discogs::{DiscogsClient, DiscogsReleaseDetails};
use crate::services::lastfm::LastFmClient;
use crate::services::musicbrainz::MusicBrainzClient;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Whitelist of mood tags from Last.fm
const MOOD_WHITELIST: &[&str] = &[
    "chill", "melancholy", "happy", "sad", "energetic", "relaxed",
    "dark", "romantic", "angry", "epic", "dreamy", "fun", "uplifting",
    "peaceful", "aggressive", "bittersweet", "intense", "nostalgic"
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictInfo {
    pub alternate_source: String,
    pub alternate_value: String,
    pub alternate_confidence: f64,
    pub conflict_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FieldResolution {
    Resolved {
        value: String,
        source: String,
        confidence: f64,
        resolved_at: String,
        conflict: Option<ConflictInfo>,
    },
    NotFound {
        source: String,
        checked_at: String,
    },
    NotSupported {
        reason: String,
    },
    SourceUnavailable {
        source: String,
        error: String,
    },
    Failed {
        source: String,
        error: String,
        failed_at: String,
    },
    NotRequested,
}

impl Default for FieldResolution {
    fn default() -> Self {
        FieldResolution::NotRequested
    }
}

impl FieldResolution {
    pub fn value(&self) -> Option<&str> {
        match self {
            FieldResolution::Resolved { value, .. } => Some(value.as_str()),
            _ => None,
        }
    }

    pub fn source(&self) -> Option<&str> {
        match self {
            FieldResolution::Resolved { source, .. } => Some(source.as_str()),
            FieldResolution::NotFound { source, .. } => Some(source.as_str()),
            FieldResolution::SourceUnavailable { source, .. } => Some(source.as_str()),
            FieldResolution::Failed { source, .. } => Some(source.as_str()),
            _ => None,
        }
    }

    pub fn confidence(&self) -> f64 {
        match self {
            FieldResolution::Resolved { confidence, .. } => *confidence,
            _ => 0.0,
        }
    }

    pub fn is_resolved(&self) -> bool {
        matches!(self, FieldResolution::Resolved { .. })
    }

    /// Try to merge a candidate value into FieldResolution while respecting manual source overrides,
    /// higher confidence precedence, avoiding empty/placeholder overwrites, and logging conflicts.
    pub fn merge_candidate(
        &mut self,
        new_val: Option<String>,
        source: &str,
        confidence: f64,
        now_ts: &str,
    ) {
        let clean_val = match new_val {
            Some(ref s) if !s.trim().is_empty() && s.trim() != "Unknown" && s.trim() != "???" => s.trim().to_string(),
            _ => return,
        };

        match self {
            FieldResolution::Resolved {
                ref mut value,
                source: ref mut curr_src,
                confidence: ref mut curr_conf,
                ref mut resolved_at,
                ref mut conflict,
            } => {
                if curr_src == "manual" {
                    return; // Preserve manual source unconditionally
                }

                if value == &clean_val {
                    if confidence > *curr_conf {
                        *curr_src = source.to_string();
                        *curr_conf = confidence;
                        *resolved_at = now_ts.to_string();
                    }
                    return;
                }

                // Conflict between valid sources
                if confidence > *curr_conf {
                    *conflict = Some(ConflictInfo {
                        alternate_source: curr_src.clone(),
                        alternate_value: value.clone(),
                        alternate_confidence: *curr_conf,
                        conflict_reason: format!("Replaced by higher-confidence candidate from {}", source),
                    });
                    *value = clean_val;
                    *curr_src = source.to_string();
                    *curr_conf = confidence;
                    *resolved_at = now_ts.to_string();
                } else {
                    if conflict.is_none() {
                        *conflict = Some(ConflictInfo {
                            alternate_source: source.to_string(),
                            alternate_value: clean_val,
                            alternate_confidence: confidence,
                            conflict_reason: format!("Lower confidence candidate rejected in favor of {}", curr_src),
                        });
                    }
                }
            }
            _ => {
                *self = FieldResolution::Resolved {
                    value: clean_val,
                    source: source.to_string(),
                    confidence,
                    resolved_at: now_ts.to_string(),
                    conflict: None,
                };
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct EnrichedMetadata {
    pub genre: Option<String>,
    pub style: Option<String>,
    pub mood: Option<String>,
    pub release_type: Option<String>,
    pub release_status: Option<String>,
    pub language: Option<String>,
    pub release_country: Option<String>,
    pub label: Option<String>,
    pub barcode: Option<String>,
    pub catalog_number: Option<String>,
    pub original_date: Option<String>,
    pub bpm: Option<f64>,
    pub key: Option<String>,
    pub energy: Option<f64>,
    pub danceability: Option<f64>,
    pub loudness: Option<f64>,

    // Explicit field resolutions (provenance & traceability)
    pub genre_res: FieldResolution,
    pub style_res: FieldResolution,
    pub mood_res: FieldResolution,
    pub release_type_res: FieldResolution,
    pub release_status_res: FieldResolution,
    pub language_res: FieldResolution,
    pub release_country_res: FieldResolution,
    pub label_res: FieldResolution,
    pub barcode_res: FieldResolution,
    pub catalog_number_res: FieldResolution,
    pub original_date_res: FieldResolution,
    pub bpm_res: FieldResolution,
    pub key_res: FieldResolution,
    pub isrc_res: FieldResolution,
    pub musicbrainz_recording_id_res: FieldResolution,
    pub musicbrainz_release_id_res: FieldResolution,
    pub musicbrainz_release_group_id_res: FieldResolution,
    pub musicbrainz_artist_id_res: FieldResolution,
    pub discogs_release_id_res: FieldResolution,
    pub title_res: FieldResolution,
    pub artist_res: FieldResolution,
    pub album_artist_res: FieldResolution,
    pub album_res: FieldResolution,
    pub track_number_res: FieldResolution,
    pub track_total_res: FieldResolution,
    pub disc_number_res: FieldResolution,
    pub disc_total_res: FieldResolution,

    pub enriched_at: String,
}

impl EnrichedMetadata {
    /// Synchronize resolution fields into legacy Option<String> fields for backwards compatibility
    pub fn sync_legacy_fields(&mut self) {
        if self.genre.is_none() {
            self.genre = self.genre_res.value().map(|s| s.to_string());
        }
        if self.style.is_none() {
            self.style = self.style_res.value().map(|s| s.to_string());
        }
        if self.mood.is_none() {
            self.mood = self.mood_res.value().map(|s| s.to_string());
        }
        if self.release_type.is_none() {
            self.release_type = self.release_type_res.value().map(|s| s.to_string());
        }
        if self.release_status.is_none() {
            self.release_status = self.release_status_res.value().map(|s| s.to_string());
        }
        if self.language.is_none() {
            self.language = self.language_res.value().map(|s| s.to_string());
        }
        if self.release_country.is_none() {
            self.release_country = self.release_country_res.value().map(|s| s.to_string());
        }
        if self.label.is_none() {
            self.label = self.label_res.value().map(|s| s.to_string());
        }
        if self.barcode.is_none() {
            self.barcode = self.barcode_res.value().map(|s| s.to_string());
        }
        if self.catalog_number.is_none() {
            self.catalog_number = self.catalog_number_res.value().map(|s| s.to_string());
        }
        if self.original_date.is_none() {
            self.original_date = self.original_date_res.value().map(|s| s.to_string());
        }
        if self.bpm.is_none() {
            self.bpm = self.bpm_res.value().and_then(|s| s.parse::<f64>().ok());
        }
        if self.key.is_none() {
            self.key = self.key_res.value().map(|s| s.to_string());
        }
    }
}

pub struct EnrichmentEngine {
    discogs: DiscogsClient,
    musicbrainz: MusicBrainzClient,
    lastfm: Option<LastFmClient>,
    discogs_cache: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, Option<DiscogsReleaseDetails>>>>,
}

impl EnrichmentEngine {
    pub fn new() -> Self {
        Self {
            discogs: DiscogsClient::new(),
            musicbrainz: MusicBrainzClient::new(),
            lastfm: LastFmClient::from_env().ok(),
            discogs_cache: std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Resolve enriched metadata for a track following exact field precedence rules.
    ///
    /// DISC-PRE-01: Discogs Precedence Rule for Genre:
    /// Discogs genre tag is prioritized ONLY when release `community.have >= 5`.
    /// The Discogs connector is experimental and evaluated per field.
    /// If `community.have < 5`, Discogs genre is skipped and MusicBrainz / Last.fm is used.
    pub async fn resolve_track_metadata(
        &self,
        artist: &str,
        album: &str,
        title: &str,
        audio_file_path: Option<&str>,
    ) -> EnrichedMetadata {
        let mut meta = EnrichedMetadata::default();
        let now_ts = chrono_now_iso();
        meta.enriched_at = now_ts.clone();

        meta.title_res.merge_candidate(Some(title.to_string()), "input", 1.0, &now_ts);
        meta.artist_res.merge_candidate(Some(artist.to_string()), "input", 1.0, &now_ts);
        meta.album_res.merge_candidate(Some(album.to_string()), "input", 1.0, &now_ts);

        let cache_key = format!("{}|{}", artist.to_lowercase(), album.to_lowercase());

        // 1. Discogs Release Lookup (Cached at Album Level)
        let discogs_release = {
            let mut cache = self.discogs_cache.lock().await;
            if let Some(cached) = cache.get(&cache_key) {
                cached.clone()
            } else {
                let format_type = if album.to_lowercase().contains("single") {
                    Some("Single")
                } else if album.to_lowercase().contains("ep") {
                    Some("EP")
                } else {
                    None
                };

                let res_opt = match self.discogs.search_release_with_format(artist, album, format_type).await {
                    Ok(Some(res)) => self.discogs.get_release(res.id).await.ok(),
                    _ => None,
                };
                cache.insert(cache_key, res_opt.clone());
                res_opt
            }
        };

        if let Some(ref d_rel) = discogs_release {
            meta.discogs_release_id_res.merge_candidate(Some(d_rel.id.to_string()), "discogs", 0.90, &now_ts);
            if let Some(ref country) = d_rel.country {
                meta.release_country_res.merge_candidate(Some(normalize_country_code(country)), "discogs", 0.70, &now_ts);
            }
            if let Some(first_label) = d_rel.labels.first() {
                meta.label_res.merge_candidate(Some(first_label.to_string()), "discogs", 0.70, &now_ts);
            }
            if let Some(first_style) = d_rel.styles.first() {
                meta.style_res.merge_candidate(Some(first_style.to_string()), "discogs", 0.85, &now_ts);
            }
        } else {
            meta.discogs_release_id_res = FieldResolution::NotFound {
                source: "discogs".to_string(),
                checked_at: now_ts.clone(),
            };
        }

        // 2. MusicBrainz Recording Lookup
        let norm_album = normalize_title(album);
        let mb_recording = match self.musicbrainz.search_recordings(title, artist, Some(album), 10).await {
            Ok(recs) => {
                let recs_vec: Vec<_> = recs.into_iter().collect();
                recs_vec.iter().cloned().find(|r| {
                    if let Some(ref rels) = r.releases {
                        rels.iter().any(|rel| {
                            let t = normalize_title(&rel.title);
                            t == norm_album || t.starts_with(&norm_album) || norm_album.starts_with(&t)
                        })
                    } else {
                        false
                    }
                })
                .or_else(|| recs_vec.into_iter().find(|r| r.releases.as_ref().map(|rels| !rels.is_empty()).unwrap_or(false)))
            }
            _ => None,
        };

        if let Some(ref mb_rec) = mb_recording {
            meta.musicbrainz_recording_id_res.merge_candidate(Some(mb_rec.id.clone()), "musicbrainz", 0.95, &now_ts);
        } else {
            meta.musicbrainz_recording_id_res = FieldResolution::NotFound {
                source: "musicbrainz".to_string(),
                checked_at: now_ts.clone(),
            };
        }

        // -------------------------------------------------------------
        // FIELD PRECEDENCE 1: GENRE
        // Discogs API if community.have >= 5 (confidence 0.85), else MusicBrainz (confidence 0.80)
        // -------------------------------------------------------------
        if let Some(ref d_rel) = discogs_release {
            if d_rel.community_have >= 5 && !d_rel.genres.is_empty() {
                if let Some(ref g) = d_rel.genres.first() {
                    meta.genre_res.merge_candidate(Some(g.to_string()), "discogs", 0.85, &now_ts);
                }
            }
        }

        if !meta.genre_res.is_resolved() {
            if let Some(ref mb_rec) = mb_recording {
                if let Ok(detail) = self.musicbrainz.get_recording_details(&mb_rec.id).await {
                    if let Some(genres) = detail.genres {
                        if let Some(first_g) = genres.first() {
                            meta.genre_res.merge_candidate(Some(first_g.name.clone()), "musicbrainz", 0.80, &now_ts);
                        }
                    }
                }
            }
        }

        // -------------------------------------------------------------
        // FIELD PRECEDENCE 2.5: MOOD (Last.fm top tags → MOOD_WHITELIST)
        // -------------------------------------------------------------
        if let Some(ref lastfm) = self.lastfm {
            if let Ok(tags) = lastfm.get_track_tags(artist, title).await {
                if !tags.is_empty() {
                    for tag in tags.iter().take(10) {
                        let tag_lower = tag.name.to_lowercase();
                        if MOOD_WHITELIST.contains(&tag_lower.as_str()) {
                            meta.mood_res.merge_candidate(Some(tag_lower), "lastfm", 0.75, &now_ts);
                            break;
                        }
                    }
                }
            }
        }

        // -------------------------------------------------------------
        // FIELD PRECEDENCE 3 & 7: ESSENTIA LOCAL AUDIO ANALYSIS (BPM, KEY, STYLE FALLBACK)
        // -------------------------------------------------------------
        if let Some(path) = audio_file_path {
            if let Ok(essentia_json) = self.run_essentia_bridge(path).await {
                if let Some(bpm_val) = essentia_json["bpm"].as_f64() {
                    meta.bpm_res.merge_candidate(Some(format!("{:.2}", bpm_val)), "essentia", 0.90, &now_ts);
                }
                if let Some(key_val) = essentia_json["key"].as_str() {
                    meta.key_res.merge_candidate(Some(key_val.to_string()), "essentia", 0.90, &now_ts);
                }
                meta.energy = essentia_json["energy"].as_f64();
                meta.danceability = essentia_json["danceability"].as_f64();
                meta.loudness = essentia_json["loudness"].as_f64();

                if !meta.mood_res.is_resolved() {
                    if let Some(m_val) = essentia_json["mood"].as_str() {
                        meta.mood_res.merge_candidate(Some(m_val.to_string()), "essentia", 0.65, &now_ts);
                    }
                }

                if !meta.style_res.is_resolved() {
                    if let Some(styles_arr) = essentia_json["styles"].as_array() {
                        if let Some(top_style) = styles_arr.first() {
                            let prob = top_style["probability"].as_f64().unwrap_or(0.0);
                            if prob >= 0.4 {
                                if let Some(s_val) = top_style["style"].as_str() {
                                    meta.style_res.merge_candidate(Some(s_val.to_string()), "essentia", 0.60, &now_ts);
                                }
                            }
                        }
                    }
                }
            }
        }

        // -------------------------------------------------------------
        // FIELD PRECEDENCE 4 & 5: RELEASETYPE / RELEASESTATUS / LANGUAGE / RELEASECOUNTRY (MusicBrainz)
        // -------------------------------------------------------------
        if artist.eq_ignore_ascii_case("various artists") {
            meta.release_type_res.merge_candidate(Some("Compilation".to_string()), "rule_engine", 1.0, &now_ts);
        }

        if let Some(ref mb_rec) = mb_recording {
            if let Some(ref rels) = mb_rec.releases {
                let is_album_match = |r_title: &str| {
                    let t = normalize_title(r_title);
                    t == norm_album || t.starts_with(&norm_album) || norm_album.starts_with(&t)
                };

                let selected_rel = rels.iter()
                    .find(|r| is_album_match(&r.title))
                    .or_else(|| rels.first());

                if let Some(first_rel) = selected_rel {
                    meta.musicbrainz_release_id_res.merge_candidate(Some(first_rel.id.clone()), "musicbrainz", 0.95, &now_ts);

                    if let Some(ref c) = first_rel.country {
                        meta.release_country_res.merge_candidate(Some(normalize_country_code(c)), "musicbrainz", 0.85, &now_ts);
                    }
                    if let Some(ref st) = first_rel.status {
                        meta.release_status_res.merge_candidate(Some(st.clone()), "musicbrainz", 0.85, &now_ts);
                    }
                    if let Some(ref d) = first_rel.date {
                        meta.original_date_res.merge_candidate(Some(d.clone()), "musicbrainz", 0.85, &now_ts);
                    }
                    if let Some(ref b) = first_rel.barcode {
                        meta.barcode_res.merge_candidate(Some(b.clone()), "musicbrainz", 0.85, &now_ts);
                    }
                    if let Some(ref txt) = first_rel.text_representation {
                        if let Some(ref l) = txt.language {
                            meta.language_res.merge_candidate(Some(normalize_language_code(l)), "musicbrainz", 0.85, &now_ts);
                        }
                    }
                    if meta.language_res.value().is_none() {
                        if let Ok(Some(full_rel)) = self.musicbrainz.lookup_release(&first_rel.id).await {
                            if let Some(ref txt) = full_rel.text_representation {
                                if let Some(ref l) = txt.language {
                                    meta.language_res.merge_candidate(Some(normalize_language_code(l)), "musicbrainz", 0.90, &now_ts);
                                }
                            }
                        }
                    }
                    if let Some(ref l_info_vec) = first_rel.label_info {
                        if let Some(first_l) = l_info_vec.first() {
                            if let Some(ref l_obj) = first_l.label {
                                meta.label_res.merge_candidate(Some(l_obj.name.clone()), "musicbrainz", 0.85, &now_ts);
                            }
                            if let Some(ref cat) = first_l.catalog_number {
                                meta.catalog_number_res.merge_candidate(Some(cat.clone()), "musicbrainz", 0.85, &now_ts);
                            }
                        }
                    }

                    if !meta.release_type_res.is_resolved() {
                        if let Some(ref rg) = first_rel.release_group {
                            meta.musicbrainz_release_group_id_res.merge_candidate(Some(rg.id.clone()), "musicbrainz", 0.95, &now_ts);

                            if let Some(ref pt) = rg.primary_type {
                                if pt.eq_ignore_ascii_case("compilation") {
                                    meta.release_type_res.merge_candidate(Some("Compilation".to_string()), "musicbrainz", 0.90, &now_ts);
                                } else {
                                    meta.release_type_res.merge_candidate(Some(pt.clone()), "musicbrainz", 0.90, &now_ts);
                                }
                            }
                        }
                    }
                }
            }
        }

        meta.sync_legacy_fields();
        meta
    }

    /// Helper to invoke local Python Essentia bridge script safely
    async fn run_essentia_bridge(&self, audio_path: &str) -> Result<serde_json::Value, String> {
        let output = if cfg!(target_os = "windows") {
            let wsl_audio_path = convert_to_wsl_path(audio_path);
            let wsl_script_path = resolve_essentia_script_path();

            tokio::process::Command::new("wsl")
                .arg("python3")
                .arg(&wsl_script_path)
                .arg(&wsl_audio_path)
                .output()
                .await
        } else {
            let script_path = if std::path::Path::new("legacy/syncify-cli/scripts/essentia_bridge.py").exists() {
                "legacy/syncify-cli/scripts/essentia_bridge.py"
            } else {
                "scripts/essentia_bridge.py"
            };
            tokio::process::Command::new("python3")
                .arg(script_path)
                .arg(audio_path)
                .output()
                .await
        }
        .map_err(|e| format!("Failed to run essentia_bridge.py: {}", e))?;

        if !output.status.success() {
            return Err("essentia_bridge.py returned non-zero exit code".to_string());
        }

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let json_part = stdout_str
            .lines()
            .rev()
            .find(|line| line.trim().starts_with('{') && line.trim().ends_with('}'))
            .or_else(|| stdout_str.find('{').map(|start| &stdout_str[start..]))
            .unwrap_or(&stdout_str);

        let parsed: serde_json::Value = serde_json::from_str(json_part)
            .map_err(|e| format!("Invalid JSON output from essentia_bridge: {} (raw: {})", e, stdout_str))?;

        if parsed["success"].as_bool().unwrap_or(false) {
            Ok(parsed)
        } else {
            Err(parsed["error"].as_str().unwrap_or("Unknown Essentia error").to_string())
        }
    }

    /// Persist enriched metadata into tracks table respecting source_type ('manual' skip)
    pub async fn apply_to_track(&self, db: &SqlitePool, track_id: i64, meta: &EnrichedMetadata) -> Result<(), String> {
        // Read source_type flags
        let row: Option<(String, String, String, String, String, String)> = sqlx::query_as(
            "SELECT genre_source_type, style_source_type, mood_source_type, bpm_source_type, key_source_type, label_source_type FROM tracks WHERE id = ?"
        )
        .bind(track_id)
        .fetch_optional(db)
        .await
        .map_err(|e| format!("Failed to read source_type flags: {}", e))?;

        let (genre_st, style_st, mood_st, bpm_st, key_st, _label_st) = row.unwrap_or((
            "enrichment".to_string(),
            "enrichment".to_string(),
            "enrichment".to_string(),
            "enrichment".to_string(),
            "enrichment".to_string(),
            "enrichment".to_string(),
        ));

        if genre_st == "enrichment" {
            if let Some(ref genre) = meta.genre {
                if !genre.trim().is_empty() {
                    let _ = sqlx::query("UPDATE tracks SET genre = ? WHERE id = ?")
                        .bind(genre)
                        .bind(track_id)
                        .execute(db)
                        .await;
                }
            }
        }

        if style_st == "enrichment" {
            if let Some(ref style) = meta.style {
                if !style.trim().is_empty() {
                    let _ = sqlx::query("UPDATE tracks SET style = ? WHERE id = ?")
                        .bind(style)
                        .bind(track_id)
                        .execute(db)
                        .await;
                }
            }
        }

        if mood_st == "enrichment" {
            if let Some(ref mood) = meta.mood {
                if !mood.trim().is_empty() {
                    let _ = sqlx::query("UPDATE tracks SET mood = ? WHERE id = ?")
                        .bind(mood)
                        .bind(track_id)
                        .execute(db)
                        .await;
                }
            }
        }

        if bpm_st == "enrichment" {
            if let Some(bpm) = meta.bpm {
                let _ = sqlx::query("UPDATE tracks SET bpm = ? WHERE id = ?")
                    .bind(bpm)
                    .bind(track_id)
                    .execute(db)
                    .await;
            }
        }

        if key_st == "enrichment" {
            if let Some(ref key) = meta.key {
                if !key.trim().is_empty() {
                    let _ = sqlx::query("UPDATE tracks SET initial_key = ? WHERE id = ?")
                        .bind(key)
                        .bind(track_id)
                        .execute(db)
                        .await;
                }
            }
        }

        Ok(())
    }
}

fn chrono_now_iso() -> String {
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    format!("{}", duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enriched_metadata_defaults() {
        let meta = EnrichedMetadata::default();
        assert!(meta.genre.is_none());
        assert!(meta.style.is_none());
        assert!(meta.mood.is_none());
        assert!(meta.bpm.is_none());
        assert_eq!(meta.genre_res, FieldResolution::NotRequested);
    }

    #[test]
    fn test_field_resolution_explicit_states() {
        let res_resolved = FieldResolution::Resolved {
            value: "Rock".to_string(),
            source: "discogs".to_string(),
            confidence: 0.85,
            resolved_at: "1000".to_string(),
            conflict: None,
        };
        assert!(res_resolved.is_resolved());
        assert_eq!(res_resolved.value(), Some("Rock"));
        assert_eq!(res_resolved.source(), Some("discogs"));
        assert_eq!(res_resolved.confidence(), 0.85);

        let res_not_found = FieldResolution::NotFound {
            source: "discogs".to_string(),
            checked_at: "1000".to_string(),
        };
        assert!(!res_not_found.is_resolved());

        let res_not_supported = FieldResolution::NotSupported {
            reason: "Provider doesn't index this field".to_string(),
        };
        assert!(!res_not_supported.is_resolved());

        let res_unavailable = FieldResolution::SourceUnavailable {
            source: "lastfm".to_string(),
            error: "HTTP 503".to_string(),
        };
        assert!(!res_unavailable.is_resolved());

        let res_failed = FieldResolution::Failed {
            source: "essentia".to_string(),
            error: "Non-zero exit code".to_string(),
            failed_at: "1000".to_string(),
        };
        assert!(!res_failed.is_resolved());
    }

    #[test]
    fn test_manual_override_preservation() {
        let mut res = FieldResolution::Resolved {
            value: "My Custom Genre".to_string(),
            source: "manual".to_string(),
            confidence: 1.0,
            resolved_at: "1000".to_string(),
            conflict: None,
        };

        // Attempt to merge automated candidate from Discogs
        res.merge_candidate(Some("Pop".to_string()), "discogs", 0.95, "2000");

        // Must remain 'My Custom Genre'
        assert_eq!(res.value(), Some("My Custom Genre"));
        assert_eq!(res.source(), Some("manual"));
    }

    #[test]
    fn test_conflict_recording() {
        let mut res = FieldResolution::Resolved {
            value: "Pop".to_string(),
            source: "musicbrainz".to_string(),
            confidence: 0.80,
            resolved_at: "1000".to_string(),
            conflict: None,
        };

        // Merge higher-confidence candidate from Discogs
        res.merge_candidate(Some("Rock".to_string()), "discogs", 0.90, "2000");

        assert_eq!(res.value(), Some("Rock"));
        assert_eq!(res.source(), Some("discogs"));
        assert_eq!(res.confidence(), 0.90);

        if let FieldResolution::Resolved { conflict: Some(ref c), .. } = res {
            assert_eq!(c.alternate_source, "musicbrainz");
            assert_eq!(c.alternate_value, "Pop");
        } else {
            panic!("Expected ConflictInfo to be recorded");
        }
    }

    #[test]
    fn test_empty_candidate_rejection() {
        let mut res = FieldResolution::Resolved {
            value: "Initial Genre".to_string(),
            source: "discogs".to_string(),
            confidence: 0.85,
            resolved_at: "1000".to_string(),
            conflict: None,
        };

        res.merge_candidate(Some("".to_string()), "lastfm", 0.99, "2000");
        assert_eq!(res.value(), Some("Initial Genre"));

        res.merge_candidate(Some("Unknown".to_string()), "lastfm", 0.99, "2000");
        assert_eq!(res.value(), Some("Initial Genre"));

        res.merge_candidate(None, "lastfm", 0.99, "2000");
        assert_eq!(res.value(), Some("Initial Genre"));
    }

    #[test]
    fn test_mood_whitelist() {
        assert!(MOOD_WHITELIST.contains(&"chill"));
        assert!(MOOD_WHITELIST.contains(&"melancholy"));
        assert!(!MOOD_WHITELIST.contains(&"invalid_mood_tag"));
    }

    #[tokio::test]
    async fn test_apply_to_track_skips_manual_source_type() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create memory DB");

        sqlx::query(
            "CREATE TABLE tracks (
                id INTEGER PRIMARY KEY,
                genre TEXT,
                style TEXT,
                genre_source_type TEXT DEFAULT 'manual',
                style_source_type TEXT DEFAULT 'enrichment',
                mood_source_type TEXT DEFAULT 'enrichment',
                bpm_source_type TEXT DEFAULT 'enrichment',
                key_source_type TEXT DEFAULT 'enrichment',
                label_source_type TEXT DEFAULT 'enrichment'
            )"
        )
        .execute(&pool)
        .await
        .expect("Failed to create table");

        sqlx::query("INSERT INTO tracks (id, genre, style, genre_source_type, style_source_type) VALUES (1, 'User Genre', 'Old Style', 'manual', 'enrichment')")
            .execute(&pool)
            .await
            .expect("Failed to insert track");

        let engine = EnrichmentEngine::new();
        let meta = EnrichedMetadata {
            genre: Some("Enriched Auto Genre".to_string()),
            style: Some("Enriched Auto Style".to_string()),
            ..Default::default()
        };

        let res = engine.apply_to_track(&pool, 1, &meta).await;
        assert!(res.is_ok(), "apply_to_track failed: {:?}", res);

        let row: (String, String) = sqlx::query_as("SELECT genre, style FROM tracks WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch track");

        assert_eq!(row.0, "User Genre", "Manual genre was incorrectly overwritten by enrichment engine");
        assert_eq!(row.1, "Enriched Auto Style", "Enrichment style was not updated");
    }
}

/// Convert Windows path (e.g. C:\foo\bar) to WSL path (/mnt/c/foo/bar)
fn convert_to_wsl_path(path: &str) -> String {
    let canonical = std::path::Path::new(path);
    let absolute = std::fs::canonicalize(canonical).unwrap_or_else(|_| std::path::PathBuf::from(path));
    let mut path_str = absolute.to_string_lossy().replace('\\', "/");

    if path_str.starts_with("//?/") {
        path_str = path_str[4..].to_string();
    } else if path_str.starts_with("\\\\?\\") {
        path_str = path_str[4..].to_string();
    }

    if path_str.len() >= 2 && path_str.chars().nth(1) == Some(':') {
        let drive = path_str.chars().next().unwrap().to_ascii_lowercase();
        format!("/mnt/{}{}", drive, &path_str[2..])
    } else {
        path_str
    }
}

fn resolve_essentia_script_path() -> String {
    let candidates = [
        "legacy/syncify-cli/scripts/essentia_bridge.py",
        "scripts/essentia_bridge.py",
        "../scripts/essentia_bridge.py",
        "../../scripts/essentia_bridge.py",
    ];
    for candidate in &candidates {
        if std::path::Path::new(candidate).exists() {
            return convert_to_wsl_path(candidate);
        }
    }
    convert_to_wsl_path("legacy/syncify-cli/scripts/essentia_bridge.py")
}

fn normalize_title(input: &str) -> String {
    input.to_lowercase()
        .replace('’', "'")
        .replace('‘', "'")
        .replace('“', "\"")
        .replace('”', "\"")
}

fn normalize_country_code(code: &str) -> String {
    match code.to_uppercase().as_str() {
        "XW" => "Worldwide".to_string(),
        "US" => "United States".to_string(),
        "GB" | "UK" => "United Kingdom".to_string(),
        "JP" => "Japan".to_string(),
        "DE" => "Germany".to_string(),
        "FR" => "France".to_string(),
        "CA" => "Canada".to_string(),
        "AU" => "Australia".to_string(),
        other => other.to_string(),
    }
}

pub fn normalize_language_code(code: &str) -> String {
    let trimmed = code.trim();
    match trimmed.to_lowercase().as_str() {
        "eng" | "en" => "English".to_string(),
        "spa" | "es" => "Spanish".to_string(),
        "fra" | "fre" | "fr" => "French".to_string(),
        "deu" | "ger" | "de" => "German".to_string(),
        "ita" | "it" => "Italian".to_string(),
        "jpn" | "ja" => "Japanese".to_string(),
        "kor" | "ko" => "Korean".to_string(),
        "zho" | "chi" | "zh" => "Chinese".to_string(),
        "por" | "pt" => "Portuguese".to_string(),
        "rus" | "ru" => "Russian".to_string(),
        "nld" | "dut" | "nl" => "Dutch".to_string(),
        "swe" | "sv" => "Swedish".to_string(),
        "nor" | "no" => "Norwegian".to_string(),
        "dan" | "da" => "Danish".to_string(),
        "fin" | "fi" => "Finnish".to_string(),
        "pol" | "pl" => "Polish".to_string(),
        "ces" | "cze" | "cs" => "Czech".to_string(),
        "ell" | "gre" | "el" => "Greek".to_string(),
        "tur" | "tr" => "Turkish".to_string(),
        "ara" | "ar" => "Arabic".to_string(),
        "heb" | "he" => "Hebrew".to_string(),
        "hin" | "hi" => "Hindi".to_string(),
        "zxx" => "Instrumental".to_string(),
        "mul" => "Multiple Languages".to_string(),
        "und" => "Undetermined".to_string(),
        _ => trimmed.to_string(),
    }
}

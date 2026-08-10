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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

    /// Resolve enriched metadata for a track following the exact Phase 2 precedence rules
    pub async fn resolve_track_metadata(
        &self,
        artist: &str,
        album: &str,
        title: &str,
        audio_file_path: Option<&str>,
    ) -> EnrichedMetadata {
        let mut meta = EnrichedMetadata::default();
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

        // -------------------------------------------------------------
        // FIELD PRECEDENCE 1: GENRE
        // Discogs API if community.have >= 5, else MusicBrainz genre tags
        // -------------------------------------------------------------
        if let Some(ref d_rel) = discogs_release {
            if d_rel.community_have >= 5 && !d_rel.genres.is_empty() {
                meta.genre = d_rel.genres.first().cloned();
            }
        }
        if meta.genre.is_none() {
            if let Some(ref mb_rec) = mb_recording {
                if let Ok(detail) = self.musicbrainz.get_recording_details(&mb_rec.id).await {
                    if let Some(genres) = detail.genres {
                        if let Some(first_g) = genres.first() {
                            meta.genre = Some(first_g.name.clone());
                        }
                    }
                }
            }
        }

        // -------------------------------------------------------------
        // FIELD PRECEDENCE 2: STYLE
        // Discogs API if release match; else Essentia top-1 style if prob >= 0.4
        // -------------------------------------------------------------
        if let Some(ref d_rel) = discogs_release {
            if !d_rel.styles.is_empty() {
                meta.style = d_rel.styles.first().cloned();
            }
        }

        // -------------------------------------------------------------
        // FIELD PRECEDENCE 2.5: MOOD (Last.fm top tags → MOOD_WHITELIST)
        // -------------------------------------------------------------
        if let Some(ref lastfm) = self.lastfm {
            if let Ok(tags) = lastfm.get_track_tags(artist, title).await {
                if !tags.is_empty() {
                    meta.mood = LastFmClient::extract_mood(&tags, MOOD_WHITELIST);
                    tracing::debug!("Last.fm mood for {}/{}: {:?}", artist, title, meta.mood);
                }
            }
        }

        // -------------------------------------------------------------
        // FIELD PRECEDENCE 3 & 7: ESSENTIA LOCAL AUDIO ANALYSIS (BPM, KEY, STYLE FALLBACK)
        // -------------------------------------------------------------
        if let Some(path) = audio_file_path {
            if let Ok(essentia_json) = self.run_essentia_bridge(path).await {
                meta.bpm = essentia_json["bpm"].as_f64();
                meta.key = essentia_json["key"].as_str().map(|s| s.to_string());
                meta.energy = essentia_json["energy"].as_f64();
                meta.danceability = essentia_json["danceability"].as_f64();
                meta.loudness = essentia_json["loudness"].as_f64();

                if meta.mood.is_none() {
                    meta.mood = essentia_json["mood"].as_str().map(|s| s.to_string());
                }

                if meta.style.is_none() {
                    if let Some(styles_arr) = essentia_json["styles"].as_array() {
                        if let Some(top_style) = styles_arr.first() {
                            let prob = top_style["probability"].as_f64().unwrap_or(0.0);
                            if prob >= 0.4 {
                                meta.style = top_style["style"].as_str().map(|s| s.to_string());
                            }
                        }
                    }
                }
            }
        }

        // -------------------------------------------------------------
        // FIELD PRECEDENCE 4 & 5: RELEASETYPE / RELEASESTATUS / LANGUAGE / RELEASECOUNTRY (MusicBrainz, fallback Discogs)
        // -------------------------------------------------------------
        if artist.eq_ignore_ascii_case("various artists") {
            meta.release_type = Some("Compilation".to_string());
        }
        if let Some(ref mb_rec) = mb_recording {
            if let Some(ref rels) = mb_rec.releases {
                let is_album_match = |r_title: &str| {
                    let t = normalize_title(r_title);
                    t == norm_album || t.starts_with(&norm_album) || norm_album.starts_with(&t)
                };

                // Priority chain: Official + matching album + (XW/US/GB) > Official + matching album > matching album > Official > first
                let selected_rel = rels.iter()
                    .find(|r| r.status.as_deref() == Some("Official") && is_album_match(&r.title) && r.country.as_deref().map(|c| c == "XW" || c == "US" || c == "GB").unwrap_or(false))
                    .or_else(|| rels.iter().find(|r| r.status.as_deref() == Some("Official") && is_album_match(&r.title)))
                    .or_else(|| rels.iter().find(|r| is_album_match(&r.title)))
                    .or_else(|| rels.iter().find(|r| r.status.as_deref() == Some("Official")))
                    .or_else(|| rels.first());
                if let Some(first_rel) = selected_rel {
                    if meta.release_type.is_none() {
                        if let Some(ref rg) = first_rel.release_group {
                            if rg.primary_type.as_deref().map(|s| s.eq_ignore_ascii_case("compilation")).unwrap_or(false) {
                                meta.release_type = Some("Compilation".to_string());
                            } else if let Some(ref pt) = rg.primary_type {
                                meta.release_type = Some(pt.clone());
                            } else if let Some(ref sec_types) = rg.secondary_types {
                                if sec_types.iter().any(|s| s.eq_ignore_ascii_case("compilation")) {
                                    meta.release_type = Some("Compilation".to_string());
                                } else if let Some(first_sec) = sec_types.first() {
                                    meta.release_type = Some(first_sec.clone());
                                }
                            }
                        }
                    }
                    meta.release_status = first_rel.status.clone();

                    // Direct release lookup to get language + country + status
                    if let Ok(full_rel) = self.musicbrainz.get_release_details(&first_rel.id).await {
                        meta.language = full_rel.text_representation.and_then(|tr| tr.language).map(|l| normalize_language_code(&l));
                        if meta.release_country.is_none() {
                            meta.release_country = full_rel.country.map(|c| normalize_country_code(&c));
                        }
                        if meta.release_status.is_none() {
                            meta.release_status = full_rel.status;
                        }
                        // Sprint 113: Extract barcode, catalog_number, original_date from MB Release
                        if meta.barcode.is_none() {
                            meta.barcode = full_rel.barcode.clone();
                        }
                        if meta.catalog_number.is_none() {
                            if let Some(ref label_info) = full_rel.label_info {
                                meta.catalog_number = label_info.iter()
                                    .find_map(|li| li.catalog_number.clone());
                            }
                        }
                        if meta.original_date.is_none() {
                            if let Some(ref rg) = full_rel.release_group {
                                meta.original_date = rg.first_release_date.clone();
                            }
                        }
                    }

                    // Fallback to other releases of the same recording if language, country, or status is missing
                    if meta.language.is_none() || meta.release_country.is_none() || meta.release_status.is_none() {
                        for other_rel in rels.iter() {
                            if other_rel.id == first_rel.id {
                                continue;
                            }
                            if meta.release_status.is_none() && other_rel.status.is_some() {
                                meta.release_status = other_rel.status.clone();
                            }
                            if let Ok(full_rel) = self.musicbrainz.get_release_details(&other_rel.id).await {
                                if meta.language.is_none() {
                                    meta.language = full_rel.text_representation.and_then(|tr| tr.language).map(|l| normalize_language_code(&l));
                                }
                                if meta.release_country.is_none() {
                                    meta.release_country = full_rel.country.map(|c| normalize_country_code(&c));
                                }
                                if meta.release_status.is_none() {
                                    meta.release_status = full_rel.status;
                                }
                                if meta.language.is_some() && meta.release_country.is_some() && meta.release_status.is_some() {
                                    break;
                                }
                            }
                        }
                    }
                    if meta.release_status.is_none() {
                        meta.release_status = Some("Official".to_string());
                    }
                    if meta.label.is_none() {
                        if let Some(ref label_info) = first_rel.label_info {
                            if let Some(first_label) = label_info.first() {
                                if let Some(ref l) = first_label.label {
                                    meta.label = Some(l.name.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        if meta.release_country.is_none() {
            if let Some(ref d_rel) = discogs_release {
                meta.release_country = d_rel.country.clone();
            }
        }

        // -------------------------------------------------------------
        // FIELD PRECEDENCE 6: LABEL (MusicBrainz, fallback Discogs)
        // -------------------------------------------------------------
        if meta.label.is_none() {
            if let Some(ref d_rel) = discogs_release {
                if !d_rel.labels.is_empty() {
                    meta.label = d_rel.labels.first().cloned();
                }
            }
        }

        // Sprint 113: Discogs barcode fallback deferred — DiscogsReleaseDetails
        // doesn't parse `identifiers` yet. Add in Sprint 114 when struct is extended.

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
            tokio::process::Command::new("python3")
                .arg("scripts/essentia_bridge.py")
                .arg(audio_path)
                .output()
                .await
        }
        .map_err(|e| format!("Failed to run essentia_bridge.py: {}", e))?;

        if !output.status.success() {
            return Err("essentia_bridge.py returned non-zero exit code".to_string());
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("Invalid JSON output from essentia_bridge: {}", e))?;

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

        // Insert track with manual genre 'User Genre' and style 'Old Style'
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

        // Apply enriched metadata
        let res = engine.apply_to_track(&pool, 1, &meta).await;
        assert!(res.is_ok(), "apply_to_track failed: {:?}", res);

        // Fetch track back
        let row: (String, String) = sqlx::query_as("SELECT genre, style FROM tracks WHERE id = 1")
            .fetch_one(&pool)
            .await
            .expect("Failed to fetch track");

        // Assert: genre MUST remain 'User Genre' because genre_source_type == 'manual'
        assert_eq!(row.0, "User Genre", "Manual genre was incorrectly overwritten by enrichment engine");

        // Assert: style SHOULD update to 'Enriched Auto Style' because style_source_type == 'enrichment'
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
        "scripts/essentia_bridge.py",
        "../scripts/essentia_bridge.py",
        "../../scripts/essentia_bridge.py",
    ];
    for candidate in &candidates {
        if std::path::Path::new(candidate).exists() {
            return convert_to_wsl_path(candidate);
        }
    }
    convert_to_wsl_path("scripts/essentia_bridge.py")
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

fn normalize_language_code(code: &str) -> String {
    match code.to_lowercase().as_str() {
        "eng" => "English".to_string(),
        "jpn" => "Japanese".to_string(),
        "spa" => "Spanish".to_string(),
        "fra" | "fre" => "French".to_string(),
        "deu" | "ger" => "German".to_string(),
        "mul" => "Multiple Languages".to_string(),
        "zxx" => "Instrumental".to_string(),
        other => other.to_string(),
    }
}

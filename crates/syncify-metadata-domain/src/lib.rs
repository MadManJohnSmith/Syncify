//! Domain contract & deterministic Precedence Engine for Syncify Metadata Enrichment.
//!
//! Shared across backend (`src-tauri`) and CLI (`legacy/syncify-cli`) to guarantee
//! strict parity, identical precedence rules, conflict recording, and safe entity handling.

pub mod fixtures;

use serde::{Deserialize, Serialize};

/// Source priority tiers (Rank 4 is highest)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SourcePriority {
    /// Inferred or heuristic fallback (e.g. filename parser, rule engine)
    Inferred = 1,
    /// MusicBrainz open database
    MusicBrainz = 2,
    /// Streaming service origin metadata (Spotify, Qobuz, Tidal)
    StreamingService = 3,
    /// User manual input or explicit override
    Manual = 4,
}

impl SourcePriority {
    pub fn from_source_name(source: &str) -> Self {
        match source.to_lowercase().as_str() {
            "manual" | "user" | "user_override" => SourcePriority::Manual,
            "spotify" | "qobuz" | "tidal" | "deezer" | "apple_music" | "stream" | "origin" | "input" => {
                SourcePriority::StreamingService
            }
            "musicbrainz" | "mb" => SourcePriority::MusicBrainz,
            _ => SourcePriority::Inferred,
        }
    }
}

/// Explicit resolution states for enrichment fields
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

/// Information registered when two valid enrichment sources conflict
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConflictInfo {
    pub alternate_source: String,
    pub alternate_value: String,
    pub alternate_confidence: f64,
    pub conflict_reason: String,
}

/// Field-specific validator avoiding naive global blacklists while rejecting invalid placeholders
pub struct FieldValidator;

impl FieldValidator {
    /// Validate track/album title: whitespace-only and synthetic '???' rejected.
    /// Genuine titles like 'Untitled' or 'Unknown' (if literal track name) are allowed unless pure placeholder.
    pub fn is_valid_title(val: &str) -> bool {
        let t = val.trim();
        !t.is_empty() && t != "???" && t != "null" && t != "None"
    }

    /// Validate artist name: whitespace-only rejected.
    /// 'Various Artists' and 'Various' are strictly VALID for compilation albums.
    pub fn is_valid_artist(val: &str) -> bool {
        let t = val.trim();
        !t.is_empty() && t != "???" && t != "null" && t != "None"
    }

    /// Validate year / date: '0000', '0', empty rejected.
    pub fn is_valid_year(val: &str) -> bool {
        let t = val.trim();
        if t.is_empty() || t == "0000" || t == "0" || t == "null" || t == "None" {
            return false;
        }
        t.chars().any(|c| c.is_ascii_digit())
    }

    /// Validate identifier (ISRC, UPC, MBID): empty, '0', 'null', 'None' rejected.
    pub fn is_valid_identifier(val: &str) -> bool {
        let t = val.trim();
        !t.is_empty() && t != "0" && t != "0000" && t != "null" && t != "None" && t != "???" && t != "N/A"
    }

    /// Validate label / organization: whitespace-only, generic 'N/A' rejected.
    pub fn is_valid_label(val: &str) -> bool {
        let t = val.trim();
        !t.is_empty() && !t.eq_ignore_ascii_case("n/a") && t != "null" && t != "None" && t != "???"
    }

    /// Validate genre / style / mood
    pub fn is_valid_genre(val: &str) -> bool {
        let t = val.trim();
        !t.is_empty()
            && !t.eq_ignore_ascii_case("unknown")
            && !t.eq_ignore_ascii_case("n/a")
            && t != "null"
            && t != "None"
            && t != "???"
    }

    /// Validate language code (ISO 639-1 / 639-2)
    pub fn is_valid_language(val: &str) -> bool {
        let t = val.trim();
        !t.is_empty() && (t.len() == 2 || t.len() == 3) && t.chars().all(|c| c.is_ascii_alphabetic())
    }

    /// Validate ISO 3166-1 country code
    pub fn is_valid_country(val: &str) -> bool {
        let t = val.trim();
        !t.is_empty() && (t.len() == 2 || t.len() == 3) && t.chars().all(|c| c.is_ascii_alphabetic())
    }

    /// Validate BPM
    pub fn is_valid_bpm(val: u32) -> bool {
        val > 0 && val < 500
    }

    /// Validate musical key
    pub fn is_valid_key(val: &str) -> bool {
        let t = val.trim();
        !t.is_empty() && !t.eq_ignore_ascii_case("unknown") && t != "null" && t != "None" && t != "???"
    }

    /// Validate AcoustID ID (UUID / hex format)
    pub fn is_valid_acoustid(val: &str) -> bool {
        let t = val.trim();
        !t.is_empty()
            && t != "0"
            && t != "null"
            && t != "None"
            && t != "???"
            && t != "N/A"
            && t.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
    }

    /// Validate ReplayGain / EBU R128 gain string
    pub fn is_valid_gain(val: &str) -> bool {
        let t = val.trim();
        !t.is_empty() && !t.eq_ignore_ascii_case("unknown") && t != "null" && t != "None" && t != "???"
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

    /// Merge candidate applying the strict Precedence Policy:
    /// 1. Manual source is immutable.
    /// 2. Higher SourcePriority wins.
    /// 3. If SourcePriority is equal, higher confidence wins.
    /// 4. If values conflict, log ConflictInfo without concealing it.
    pub fn merge_candidate(
        &mut self,
        new_val: Option<String>,
        source: &str,
        confidence: f64,
        now_ts: &str,
    ) {
        let clean_val = match new_val {
            Some(ref s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return,
        };

        let new_prio = SourcePriority::from_source_name(source);

        match self {
            FieldResolution::Resolved {
                ref mut value,
                source: ref mut curr_src,
                confidence: ref mut curr_conf,
                ref mut resolved_at,
                ref mut conflict,
            } => {
                let curr_prio = SourcePriority::from_source_name(curr_src);

                // Manual source is unconditionally preserved
                if curr_prio == SourcePriority::Manual {
                    return;
                }

                // If identical value, retain and update confidence if higher
                if value == &clean_val {
                    if new_prio > curr_prio || (new_prio == curr_prio && confidence > *curr_conf) {
                        *curr_src = source.to_string();
                        *curr_conf = confidence;
                        *resolved_at = now_ts.to_string();
                    }
                    return;
                }

                // Conflicting values: compare priority first, then confidence
                if new_prio > curr_prio || (new_prio == curr_prio && confidence > *curr_conf) {
                    *conflict = Some(ConflictInfo {
                        alternate_source: curr_src.clone(),
                        alternate_value: value.clone(),
                        alternate_confidence: *curr_conf,
                        conflict_reason: format!(
                            "Replaced by higher-priority candidate from {} (prio: {:?}, conf: {:.2})",
                            source, new_prio, confidence
                        ),
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
                            conflict_reason: format!(
                                "Lower priority candidate from {} rejected in favor of {}",
                                source, curr_src
                            ),
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

/// Enriched Metadata DTO for all metadata domains with provenance & precedence tracking
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct EnrichedMetadata {
    // 1. Title, Artist & Core Structure
    pub title: FieldResolution,
    pub artist: FieldResolution,
    pub album: FieldResolution,
    pub album_artist: FieldResolution,
    pub composer: FieldResolution,
    pub performers: FieldResolution,
    pub work: FieldResolution,
    pub track_number: FieldResolution,
    pub track_total: FieldResolution,
    pub disc_number: FieldResolution,
    pub disc_total: FieldResolution,
    pub disc_subtitle: FieldResolution,

    // 2. Release & Editorial Details
    pub release_year: FieldResolution,
    pub release_date: FieldResolution,
    pub original_date: FieldResolution,
    pub label: FieldResolution,
    pub catalog_number: FieldResolution,
    pub copyright: FieldResolution,
    pub release_type: FieldResolution,
    pub release_status: FieldResolution,
    pub release_country: FieldResolution,
    pub language: FieldResolution,

    // 3. Acoustic & Musical Properties
    pub genre: FieldResolution,
    pub style: FieldResolution,
    pub mood: FieldResolution,
    pub explicit: FieldResolution,
    pub bpm: FieldResolution,
    pub initial_key: FieldResolution,
    pub energy: FieldResolution,
    pub danceability: FieldResolution,
    pub loudness: FieldResolution,
    pub replaygain_track_gain: FieldResolution,
    pub replaygain_track_peak: FieldResolution,
    pub replaygain_album_gain: FieldResolution,
    pub replaygain_album_peak: FieldResolution,
    pub r128_track_gain: FieldResolution,
    pub comment: FieldResolution,

    // 4. Industry Identifiers & Provenance
    pub isrc: FieldResolution,
    pub barcode: FieldResolution,
    pub acoustid_id: FieldResolution,
    pub acoustid_fingerprint: FieldResolution,
    pub lyrics_source: FieldResolution,
    pub cover_source: FieldResolution,
    pub audio_source: FieldResolution,

    // 5. MusicBrainz MBIDs
    pub musicbrainz_recording_id: FieldResolution,
    pub musicbrainz_release_id: FieldResolution,
    pub musicbrainz_release_group_id: FieldResolution,
    pub musicbrainz_artist_id: FieldResolution,
    pub musicbrainz_albumartist_id: FieldResolution,
    pub musicbrainz_work_id: FieldResolution,

    pub enriched_at: String,
}

/// Audio analysis metrics extracted from physical audio stream / staging file
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AudioAnalysisMetrics {
    pub bpm: Option<u32>,
    pub initial_key: Option<String>,
    pub energy: Option<f64>,
    pub danceability: Option<f64>,
    pub loudness: Option<f64>,
    pub replaygain_track_gain: Option<String>,
    pub replaygain_track_peak: Option<String>,
    pub replaygain_album_gain: Option<String>,
    pub replaygain_album_peak: Option<String>,
    pub r128_track_gain: Option<String>,
    pub acoustid_id: Option<String>,
    pub acoustid_fingerprint: Option<String>,
    pub duration_sec: Option<f64>,
}

impl EnrichedMetadata {
    /// Apply audio analysis metrics (ReplayGain, Acoustic Features, AcoustID Fingerprint)
    /// following strict precedence rules (Inferred source).
    pub fn apply_audio_analysis(
        &mut self,
        analysis: &AudioAnalysisMetrics,
        source: &str,
        now_ts: &str,
    ) {
        if let Some(bpm_val) = analysis.bpm {
            if FieldValidator::is_valid_bpm(bpm_val) {
                self.bpm.merge_candidate(Some(bpm_val.to_string()), source, 0.85, now_ts);
            }
        }
        if let Some(ref key_val) = analysis.initial_key {
            if FieldValidator::is_valid_key(key_val) {
                self.initial_key.merge_candidate(Some(key_val.clone()), source, 0.85, now_ts);
            }
        }
        if let Some(en) = analysis.energy {
            self.energy.merge_candidate(Some(format!("{:.2}", en)), source, 0.85, now_ts);
        }
        if let Some(da) = analysis.danceability {
            self.danceability.merge_candidate(Some(format!("{:.2}", da)), source, 0.85, now_ts);
        }
        if let Some(lo) = analysis.loudness {
            self.loudness.merge_candidate(Some(format!("{:.1}", lo)), source, 0.85, now_ts);
        }
        if let Some(ref rtg) = analysis.replaygain_track_gain {
            if FieldValidator::is_valid_gain(rtg) {
                self.replaygain_track_gain.merge_candidate(Some(rtg.clone()), source, 0.85, now_ts);
            }
        }
        if let Some(ref rtp) = analysis.replaygain_track_peak {
            if FieldValidator::is_valid_gain(rtp) {
                self.replaygain_track_peak.merge_candidate(Some(rtp.clone()), source, 0.85, now_ts);
            }
        }
        if let Some(ref rag) = analysis.replaygain_album_gain {
            if FieldValidator::is_valid_gain(rag) {
                self.replaygain_album_gain.merge_candidate(Some(rag.clone()), source, 0.85, now_ts);
            }
        }
        if let Some(ref rap) = analysis.replaygain_album_peak {
            if FieldValidator::is_valid_gain(rap) {
                self.replaygain_album_peak.merge_candidate(Some(rap.clone()), source, 0.85, now_ts);
            }
        }
        if let Some(ref r128) = analysis.r128_track_gain {
            if FieldValidator::is_valid_gain(r128) {
                self.r128_track_gain.merge_candidate(Some(r128.clone()), source, 0.85, now_ts);
            }
        }
        if let Some(ref aid) = analysis.acoustid_id {
            if FieldValidator::is_valid_acoustid(aid) {
                self.acoustid_id.merge_candidate(Some(aid.clone()), source, 0.90, now_ts);
            }
        }
        if let Some(ref fp) = analysis.acoustid_fingerprint {
            if !fp.trim().is_empty() {
                self.acoustid_fingerprint.merge_candidate(Some(fp.clone()), source, 0.90, now_ts);
            }
        }
    }
}

pub fn normalize_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn chrono_now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    format!("{}.{:03}Z", dur.as_secs(), dur.subsec_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;

    #[test]
    fn test_manual_source_is_immutable_against_higher_confidence() {
        let mut field = FieldResolution::default();
        let now = chrono_now_iso();
        field.merge_candidate(Some("Manual Title".to_string()), "manual", 1.0, &now);

        // Attempt overwrite with MusicBrainz higher confidence
        field.merge_candidate(Some("MB Title".to_string()), "musicbrainz", 0.99, &now);

        assert_eq!(field.value(), Some("Manual Title"));
        assert_eq!(field.source(), Some("manual"));
        assert_eq!(field.confidence(), 1.0);
    }

    #[test]
    fn test_source_priority_streaming_over_musicbrainz() {
        let mut field = FieldResolution::default();
        let now = chrono_now_iso();
        // MusicBrainz candidate first
        field.merge_candidate(Some("MB Album".to_string()), "musicbrainz", 0.95, &now);
        assert_eq!(field.value(), Some("MB Album"));

        // Streaming candidate (higher priority) arrives
        field.merge_candidate(Some("Official Stream Album".to_string()), "qobuz", 0.90, &now);
        assert_eq!(field.value(), Some("Official Stream Album"));
        assert_eq!(field.source(), Some("qobuz"));

        if let FieldResolution::Resolved { ref conflict, .. } = field {
            assert!(conflict.is_some());
            let c = conflict.as_ref().unwrap();
            assert_eq!(c.alternate_source, "musicbrainz");
            assert_eq!(c.alternate_value, "MB Album");
        } else {
            panic!("Expected Resolved variant");
        }
    }

    #[test]
    fn test_placeholder_validation_rules() {
        assert!(!FieldValidator::is_valid_year("0000"));
        assert!(!FieldValidator::is_valid_year("0"));
        assert!(!FieldValidator::is_valid_year(""));
        assert!(FieldValidator::is_valid_year("1977"));

        assert!(!FieldValidator::is_valid_identifier(""));
        assert!(!FieldValidator::is_valid_identifier("null"));
        assert!(!FieldValidator::is_valid_identifier("None"));
        assert!(FieldValidator::is_valid_identifier("USRC12345678"));
        assert!(FieldValidator::is_valid_identifier("0035629007421"));

        // 'Various Artists' is VALID for compilations
        assert!(FieldValidator::is_valid_artist("Various Artists"));
        assert!(FieldValidator::is_valid_artist("Various"));
        assert!(!FieldValidator::is_valid_artist("???"));
        assert!(!FieldValidator::is_valid_artist("   "));
    }

    #[test]
    fn test_musicbrainz_exact_match_fixture_parsing() {
        let json_val: serde_json::Value = serde_json::from_str(FIXTURE_MB_EXACT_RECORDING_JSON).unwrap();
        assert_eq!(json_val["id"].as_str(), Some("b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d"));
        assert_eq!(json_val["title"].as_str(), Some("Heroes"));
        let releases = json_val["releases"].as_array().unwrap();
        assert_eq!(releases.len(), 1);
        assert_eq!(releases[0]["date"].as_str(), Some("1977-10-14"));
        assert_eq!(releases[0]["label-info"][0]["label"]["name"].as_str(), Some("RCA Victor"));
    }

    #[test]
    fn test_musicbrainz_alternative_release_selection() {
        let json_val: serde_json::Value = serde_json::from_str(FIXTURE_MB_ALTERNATIVE_RELEASE_JSON).unwrap();
        let releases = json_val["releases"].as_array().unwrap();

        let norm_album = normalize_title("Heroes");
        let matched_rel = releases.iter().find(|r| {
            let t = normalize_title(r["title"].as_str().unwrap_or(""));
            t == norm_album
        });

        assert!(matched_rel.is_some());
        let rel = matched_rel.unwrap();
        assert_eq!(rel["id"].as_str(), Some("673752e3-2e06-4447-aa72-a080ef8a1768"));
        assert_eq!(rel["date"].as_str(), Some("1977-10-14"));
    }

    #[test]
    fn test_full_source_hierarchy_precedence() {
        let mut field = FieldResolution::default();
        let now = chrono_now_iso();

        // 1. Inferred arrives first
        field.merge_candidate(Some("Inferred Genre".to_string()), "inferred", 0.50, &now);
        assert_eq!(field.value(), Some("Inferred Genre"));

        // 2. MusicBrainz beats Inferred
        field.merge_candidate(Some("MB Art Rock".to_string()), "musicbrainz", 0.80, &now);
        assert_eq!(field.value(), Some("MB Art Rock"));
        assert_eq!(field.source(), Some("musicbrainz"));

        // 3. StreamingService beats MusicBrainz
        field.merge_candidate(Some("Glam Rock".to_string()), "qobuz", 0.90, &now);
        assert_eq!(field.value(), Some("Glam Rock"));
        assert_eq!(field.source(), Some("qobuz"));

        // 4. Manual override beats StreamingService
        field.merge_candidate(Some("Experimental Rock".to_string()), "manual", 1.0, &now);
        assert_eq!(field.value(), Some("Experimental Rock"));
        assert_eq!(field.source(), Some("manual"));

        // 5. Subsequent candidates cannot overwrite Manual
        field.merge_candidate(Some("New Streaming Genre".to_string()), "qobuz", 1.0, &now);
        assert_eq!(field.value(), Some("Experimental Rock"));
        assert_eq!(field.source(), Some("manual"));
    }

    #[test]
    fn test_genre_bpm_key_and_iso_validators() {
        assert!(FieldValidator::is_valid_genre("Art Rock"));
        assert!(FieldValidator::is_valid_genre("Berlin Trilogy"));
        assert!(!FieldValidator::is_valid_genre("Unknown"));
        assert!(!FieldValidator::is_valid_genre("N/A"));
        assert!(!FieldValidator::is_valid_genre("null"));
        assert!(!FieldValidator::is_valid_genre(""));

        assert!(FieldValidator::is_valid_bpm(120));
        assert!(FieldValidator::is_valid_bpm(60));
        assert!(!FieldValidator::is_valid_bpm(0));
        assert!(!FieldValidator::is_valid_bpm(600));

        assert!(FieldValidator::is_valid_key("D"));
        assert!(FieldValidator::is_valid_key("C#m"));
        assert!(!FieldValidator::is_valid_key("Unknown"));
        assert!(!FieldValidator::is_valid_key(""));

        assert!(FieldValidator::is_valid_language("eng"));
        assert!(FieldValidator::is_valid_language("pl"));
        assert!(!FieldValidator::is_valid_language("english"));
        assert!(!FieldValidator::is_valid_language(""));

        assert!(FieldValidator::is_valid_country("GB"));
        assert!(FieldValidator::is_valid_country("USA"));
        assert!(!FieldValidator::is_valid_country("Great Britain"));
        assert!(!FieldValidator::is_valid_country(""));
    }

    #[test]
    fn test_unresolved_fields_remain_not_requested_or_not_found_without_invented_placeholders() {
        let meta = EnrichedMetadata::default();
        assert_eq!(meta.title.value(), None);
        assert_eq!(meta.artist.value(), None);
        assert_eq!(meta.genre.value(), None);
        assert_eq!(meta.bpm.value(), None);
        assert_eq!(meta.isrc.value(), None);
        assert_eq!(meta.barcode.value(), None);
        assert_eq!(meta.musicbrainz_recording_id.value(), None);
    }
}

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
        !t.is_empty() && t != "N/A" && t != "null" && t != "None" && t != "???"
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

/// Enriched Metadata DTO for the first group of metadata fields
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct EnrichedMetadata {
    // 1. Title & Structure
    pub title: FieldResolution,
    pub artist: FieldResolution,
    pub album: FieldResolution,
    pub album_artist: FieldResolution,
    pub track_number: FieldResolution,
    pub track_total: FieldResolution,
    pub disc_number: FieldResolution,
    pub disc_total: FieldResolution,

    // 2. Release & Editorial Details
    pub release_year: FieldResolution,
    pub original_date: FieldResolution,
    pub label: FieldResolution,
    pub catalog_number: FieldResolution,

    // 3. Industry Identifiers
    pub isrc: FieldResolution,
    pub barcode: FieldResolution,

    // 4. MusicBrainz MBIDs
    pub musicbrainz_recording_id: FieldResolution,
    pub musicbrainz_release_id: FieldResolution,
    pub musicbrainz_release_group_id: FieldResolution,
    pub musicbrainz_artist_id: FieldResolution,

    pub enriched_at: String,
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
}

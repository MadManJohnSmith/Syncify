//! Version Derivation & Disambiguation Engine
//! Provides confidence-based policy (High, Medium, Low) for deriving display_title and file_disambiguator.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionConfidence {
    /// Low confidence: Unstructured text, raw filenames, duration heuristics.
    /// NEVER automatically mutates disk or catalog; diagnostic/suggestion only.
    Low = 1,
    /// Medium confidence: Structured performer/remixer credits + distinct track position/ISRC in album.
    Medium = 2,
    /// High confidence: Provider explicit version field, explicit version title suffix, or MusicBrainz disambiguation.
    High = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionDerivationInput {
    pub title: String,
    pub provider_version: Option<String>,
    pub musicbrainz_disambiguation: Option<String>,
    pub performer_or_remixer_credit: Option<String>,
    pub comment_text: Option<String>,
    pub track_number: Option<i32>,
    pub is_duplicate_title_in_album: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedVersionInfo {
    pub source_title: String,
    pub display_title: Option<String>,
    pub file_disambiguator: Option<String>,
    pub confidence: VersionConfidence,
    pub reason: String,
}

impl DerivedVersionInfo {
    /// Rule: Only High and Medium confidence can produce display_title or file_disambiguator.
    pub fn can_apply_to_catalog_and_disk(&self) -> bool {
        self.confidence >= VersionConfidence::Medium && self.file_disambiguator.is_some()
    }
}

/// Derive version information and confidence from multiple metadata signals
pub fn derive_track_version(input: &VersionDerivationInput) -> DerivedVersionInfo {
    let raw_title = input.title.trim();
    let source_title = raw_title.to_string();

    // 1. Check for High Confidence: Explicit provider version field (e.g. Qobuz/Tidal version property)
    if let Some(ref ver) = input.provider_version {
        let v_clean = ver.trim();
        if !v_clean.is_empty() && !v_clean.eq_ignore_ascii_case("album version") {
            let disambiguator = clean_disambiguator(v_clean);
            let display_title = format_display_title(raw_title, &disambiguator);
            return DerivedVersionInfo {
                source_title,
                display_title: Some(display_title),
                file_disambiguator: Some(disambiguator),
                confidence: VersionConfidence::High,
                reason: format!("Explicit provider version field: '{}'", v_clean),
            };
        }
    }

    // 2. Check for High Confidence: MusicBrainz recording / release disambiguation
    if let Some(ref mb_dis) = input.musicbrainz_disambiguation {
        let mb_clean = mb_dis.trim();
        if !mb_clean.is_empty() {
            let disambiguator = clean_disambiguator(mb_clean);
            let display_title = format_display_title(raw_title, &disambiguator);
            return DerivedVersionInfo {
                source_title,
                display_title: Some(display_title),
                file_disambiguator: Some(disambiguator),
                confidence: VersionConfidence::High,
                reason: format!("MusicBrainz disambiguation provenance: '{}'", mb_clean),
            };
        }
    }

    // 3. Check for High Confidence: Explicit title version indicators (e.g. "(... Remix)", "[... Mix]")
    if let Some(extracted) = extract_version_from_title(raw_title) {
        let base_title = extract_base_title(raw_title);
        let disambiguator = clean_disambiguator(&extracted);
        let display_title = format_display_title(&base_title, &disambiguator);
        return DerivedVersionInfo {
            source_title,
            display_title: Some(display_title),
            file_disambiguator: Some(disambiguator),
            confidence: VersionConfidence::High,
            reason: format!("Explicit title version marker: '{}'", extracted),
        };
    }

    // 4. Check for Medium Confidence: Structured performer/remixer credit on duplicate album track
    if let Some(ref credit) = input.performer_or_remixer_credit {
        let cr = credit.trim();
        if !cr.is_empty() {
            if let Some(remix_name) = extract_remixer_from_credit(cr) {
                let disambiguator = format!("{} Remix", remix_name);
                let display_title = format_display_title(raw_title, &disambiguator);
                return DerivedVersionInfo {
                    source_title,
                    display_title: Some(display_title),
                    file_disambiguator: Some(disambiguator),
                    confidence: VersionConfidence::Medium,
                    reason: format!("Structured performer/remixer credit: '{}'", cr),
                };
            }
        }
    }

    // 5. Check for Low Confidence: Unstructured comment text or notes
    if let Some(ref comment) = input.comment_text {
        let cm = comment.trim();
        if cm.to_ascii_lowercase().contains("remix") || cm.to_ascii_lowercase().contains("live") || cm.to_ascii_lowercase().contains("version") {
            return DerivedVersionInfo {
                source_title,
                display_title: None,
                file_disambiguator: None,
                confidence: VersionConfidence::Low,
                reason: format!("Low confidence heuristic from free-form comment text: '{}'", cm),
            };
        }
    }

    // No version derivation detected
    DerivedVersionInfo {
        source_title,
        display_title: None,
        file_disambiguator: None,
        confidence: VersionConfidence::Low,
        reason: "No version signals detected".to_string(),
    }
}

fn clean_disambiguator(raw: &str) -> String {
    let mut s = raw.trim();
    if s.starts_with('(') && s.ends_with(')') && s.len() >= 2 {
        s = &s[1..s.len() - 1];
    } else if s.starts_with('[') && s.ends_with(']') && s.len() >= 2 {
        s = &s[1..s.len() - 1];
    }
    s.trim().to_string()
}

fn format_display_title(base_title: &str, disambiguator: &str) -> String {
    let clean_base = base_title.trim();
    let lower_base = clean_base.to_ascii_lowercase();
    let lower_dis = disambiguator.to_ascii_lowercase();

    if lower_base.contains(&lower_dis) {
        clean_base.to_string()
    } else {
        format!("{} ({})", clean_base, disambiguator)
    }
}

fn extract_version_from_title(title: &str) -> Option<String> {
    let keywords = [
        "remix", "mix", "edit", "live", "remaster", "remastered", "version",
        "acoustic", "deluxe", "extended", "instrumental", "re-recorded", "club mix", "radio edit"
    ];

    // Check parentheses (...)
    if let Some(start) = title.rfind('(') {
        if let Some(end) = title[start..].find(')') {
            let inner = &title[start + 1..start + end];
            let lower = inner.to_ascii_lowercase();
            if keywords.iter().any(|k| lower.contains(k)) {
                return Some(inner.trim().to_string());
            }
        }
    }

    // Check brackets [...]
    if let Some(start) = title.rfind('[') {
        if let Some(end) = title[start..].find(']') {
            let inner = &title[start + 1..start + end];
            let lower = inner.to_ascii_lowercase();
            if keywords.iter().any(|k| lower.contains(k)) {
                return Some(inner.trim().to_string());
            }
        }
    }

    // Check hyphen suffix: "Title - Radio Edit"
    if let Some(dash_pos) = title.rfind(" - ") {
        let suffix = &title[dash_pos + 3..];
        let lower = suffix.to_ascii_lowercase();
        if keywords.iter().any(|k| lower.contains(k)) {
            return Some(suffix.trim().to_string());
        }
    }

    None
}

fn extract_base_title(title: &str) -> String {
    if let Some(start) = title.rfind('(') {
        if let Some(end) = title[start..].find(')') {
            let before = title[..start].trim();
            let after = title[start + end + 1..].trim();
            return format!("{} {}", before, after).trim().to_string();
        }
    }
    if let Some(start) = title.rfind('[') {
        if let Some(end) = title[start..].find(']') {
            let before = title[..start].trim();
            let after = title[start + end + 1..].trim();
            return format!("{} {}", before, after).trim().to_string();
        }
    }
    if let Some(dash_pos) = title.rfind(" - ") {
        return title[..dash_pos].trim().to_string();
    }
    title.trim().to_string()
}

fn extract_remixer_from_credit(credit: &str) -> Option<String> {
    let lower = credit.to_ascii_lowercase();
    if let Some(pos) = lower.find("remixed by ") {
        return Some(credit[pos + 11..].trim().to_string());
    }
    if let Some(pos) = lower.find("remix by ") {
        return Some(credit[pos + 9..].trim().to_string());
    }
    if let Some(pos) = lower.find("remix: ") {
        return Some(credit[pos + 7..].trim().to_string());
    }
    if lower.ends_with(" remix") {
        return Some(credit[..credit.len() - 6].trim().to_string());
    }
    let parts: Vec<&str> = credit.split(&[':', '-', ','][..]).collect();
    if let Some(first) = parts.first() {
        let f = first.trim();
        if !f.is_empty() && f.len() < 30 {
            return Some(f.to_string());
        }
    }
    None
}

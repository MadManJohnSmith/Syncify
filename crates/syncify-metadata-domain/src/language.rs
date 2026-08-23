//! Canonical language normalization module.
//!
//! Provides deterministic mapping from ISO 639-1 (2-letter), ISO 639-2 (3-letter),
//! localized names (English, Spanish, French, German, Japanese, etc.)
//! to standard ISO 639 language codes.

use serde::{Deserialize, Serialize};

/// Language resolution result
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguageResolution {
    pub iso_639_2: String,
    pub iso_639_1: Option<String>,
    pub canonical_name: String,
}

/// Normalizes diacritics and whitespace for case-insensitive matching
fn sanitize_language_str(input: &str) -> String {
    let trimmed = input.trim();
    let lower = trimmed.to_lowercase();
    let mut normalized = String::with_capacity(lower.len());
    for c in lower.chars() {
        match c {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => normalized.push('a'),
            'é' | 'è' | 'ë' | 'ê' => normalized.push('e'),
            'í' | 'ì' | 'ï' | 'î' => normalized.push('i'),
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => normalized.push('o'),
            'ú' | 'ù' | 'ü' | 'û' => normalized.push('u'),
            'ñ' => normalized.push('n'),
            'ç' => normalized.push('c'),
            '.' | ',' | '-' | '_' | '/' | '[' | ']' | '(' | ')' => {}
            _ => normalized.push(c),
        }
    }
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Resolves an input language string into a normalized ISO 639 code (preferring 3-letter ISO 639-2).
pub fn resolve_language(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let sanitized = sanitize_language_str(trimmed);
    let lower = sanitized.as_str();

    match lower {
        "en" | "eng" | "english" | "ingles" => Some("eng".to_string()),
        "es" | "spa" | "spanish" | "espanol" | "castellano" => Some("spa".to_string()),
        "fr" | "fra" | "fre" | "french" | "francais" => Some("fra".to_string()),
        "de" | "deu" | "ger" | "german" | "deutsch" | "aleman" => Some("deu".to_string()),
        "ja" | "jpn" | "japanese" | "japones" | "nihongo" => Some("jpn".to_string()),
        "it" | "ita" | "italian" | "italiano" => Some("ita".to_string()),
        "pt" | "por" | "portuguese" | "portugues" => Some("por".to_string()),
        "ru" | "rus" | "russian" | "ruso" => Some("rus".to_string()),
        "zh" | "zho" | "chi" | "chinese" | "chino" | "mandarin" => Some("zho".to_string()),
        "ko" | "kor" | "korean" | "coreano" => Some("kor".to_string()),
        "nl" | "nld" | "dut" | "dutch" | "holandes" | "neerlandes" => Some("nld".to_string()),
        "pl" | "pol" | "polish" | "polaco" => Some("pol".to_string()),
        "sv" | "swe" | "swedish" | "sueco" => Some("swe".to_string()),
        "no" | "nor" | "norwegian" | "noruego" => Some("nor".to_string()),
        "da" | "dan" | "danish" | "danes" => Some("dan".to_string()),
        "fi" | "fin" | "finnish" | "finlandes" => Some("fin".to_string()),
        "el" | "ell" | "gre" | "greek" | "griego" => Some("ell".to_string()),
        "tr" | "tur" | "turkish" | "turco" => Some("tur".to_string()),
        "ar" | "ara" | "arabic" | "arabe" => Some("ara".to_string()),
        "hi" | "hin" | "hindi" => Some("hin".to_string()),
        "la" | "lat" | "latin" => Some("lat".to_string()),
        "he" | "heb" | "hebrew" | "hebreo" => Some("heb".to_string()),
        "cs" | "ces" | "cze" | "czech" | "checo" => Some("ces".to_string()),
        "hu" | "hun" | "hungarian" | "hungaro" => Some("hun".to_string()),
        "ro" | "ron" | "rum" | "romanian" | "rumano" => Some("ron".to_string()),
        "uk" | "ukr" | "ukrainian" | "ucraniano" => Some("ukr".to_string()),
        "vi" | "vie" | "vietnamese" | "vietnamita" => Some("vie".to_string()),
        "th" | "tha" | "thai" | "tailandes" => Some("tha".to_string()),
        "id" | "ind" | "indonesian" | "indonesio" => Some("ind".to_string()),
        "ms" | "msa" | "may" | "malay" | "malayo" => Some("msa".to_string()),
        "is" | "isl" | "ice" | "icelandic" | "islandes" => Some("isl".to_string()),
        "ga" | "gle" | "irish" | "irlandes" => Some("gle".to_string()),
        "ca" | "cat" | "catalan" => Some("cat".to_string()),
        "gl" | "glg" | "galician" | "gallego" => Some("glg".to_string()),
        "eu" | "eus" | "baq" | "basque" | "euskera" | "vasco" => Some("eus".to_string()),
        "instrumental" | "zxx" => Some("zxx".to_string()),
        _ => {
            // If it is already a 2 or 3 ASCII letter code, preserve it lowercased
            if (trimmed.len() == 2 || trimmed.len() == 3) && trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
                Some(trimmed.to_lowercase())
            } else {
                None
            }
        }
    }
}

/// Check if a language string is a valid ISO code or resolvable language name
pub fn is_valid_language(val: &str) -> bool {
    resolve_language(val).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_language_common_names() {
        assert_eq!(resolve_language("English"), Some("eng".to_string()));
        assert_eq!(resolve_language("english"), Some("eng".to_string()));
        assert_eq!(resolve_language("en"), Some("eng".to_string()));
        assert_eq!(resolve_language("eng"), Some("eng".to_string()));

        assert_eq!(resolve_language("Spanish"), Some("spa".to_string()));
        assert_eq!(resolve_language("Español"), Some("spa".to_string()));
        assert_eq!(resolve_language("es"), Some("spa".to_string()));
        assert_eq!(resolve_language("spa"), Some("spa".to_string()));

        assert_eq!(resolve_language("French"), Some("fra".to_string()));
        assert_eq!(resolve_language("français"), Some("fra".to_string()));
        assert_eq!(resolve_language("German"), Some("deu".to_string()));
        assert_eq!(resolve_language("Japanese"), Some("jpn".to_string()));
    }

    #[test]
    fn test_resolve_language_iso_codes() {
        assert_eq!(resolve_language("de"), Some("deu".to_string()));
        assert_eq!(resolve_language("ja"), Some("jpn".to_string()));
        assert_eq!(resolve_language("lat"), Some("lat".to_string()));
        assert_eq!(resolve_language("zxx"), Some("zxx".to_string()));
        assert_eq!(resolve_language("xyz"), Some("xyz".to_string())); // generic 3-letter code
    }

    #[test]
    fn test_resolve_invalid_language() {
        assert_eq!(resolve_language(""), None);
        assert_eq!(resolve_language("   "), None);
        assert_eq!(resolve_language("invalid_long_string_not_a_language"), None);
        assert_eq!(resolve_language("123"), None);
    }
}

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
        "no" | "nor" | "norwegian" | "noruego" | "nb" | "nob" | "nn" | "nno" => Some("nor".to_string()),
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
        "fa" | "fas" | "per" | "persian" | "farsi" => Some("fas".to_string()),
        "bg" | "bul" | "bulgarian" => Some("bul".to_string()),
        "sr" | "srp" | "serbian" => Some("srp".to_string()),
        "hr" | "hrv" | "croatian" => Some("hrv".to_string()),
        "sk" | "slk" | "slo" | "slovak" => Some("slk".to_string()),
        "sl" | "slv" | "slovenian" => Some("slv".to_string()),
        "lt" | "lit" | "lithuanian" => Some("lit".to_string()),
        "lv" | "lav" | "latvian" => Some("lav".to_string()),
        "et" | "est" | "estonian" => Some("est".to_string()),
        "af" | "afr" | "afrikaans" => Some("afr".to_string()),
        "sq" | "sqi" | "alb" | "albanian" => Some("sqi".to_string()),
        "hy" | "hye" | "arm" | "armenian" => Some("hye".to_string()),
        "az" | "aze" | "azerbaijani" => Some("aze".to_string()),
        "be" | "bel" | "belarusian" => Some("bel".to_string()),
        "bn" | "ben" | "bengali" => Some("ben".to_string()),
        "bs" | "bos" | "bosnian" => Some("bos".to_string()),
        "ka" | "kat" | "geo" | "georgian" => Some("kat".to_string()),
        "mk" | "mkd" | "mac" | "macedonian" => Some("mkd".to_string()),
        "sw" | "swa" | "swahili" => Some("swa".to_string()),
        "ta" | "tam" | "tamil" => Some("tam".to_string()),
        "te" | "tel" | "telugu" => Some("tel".to_string()),
        "ur" | "urd" | "urdu" => Some("urd".to_string()),
        "uz" | "uzb" | "uzbek" => Some("uzb".to_string()),
        "cy" | "cym" | "wel" | "welsh" => Some("cym".to_string()),
        "eo" | "epo" | "esperanto" => Some("epo".to_string()),
        "tl" | "tgl" | "tagalog" | "fil" | "filipino" => Some("tgl".to_string()),
        "am" | "amh" | "amharic" => Some("amh".to_string()),
        "km" | "khm" | "khmer" => Some("khm".to_string()),
        "lo" | "lao" => Some("lao".to_string()),
        "mn" | "mon" | "mongolian" => Some("mon".to_string()),
        "my" | "mya" | "bur" | "burmese" => Some("mya".to_string()),
        "ne" | "nep" | "nepali" => Some("nep".to_string()),
        "pa" | "pan" | "punjabi" => Some("pan".to_string()),
        "si" | "sin" | "sinhala" | "sinhalese" => Some("sin".to_string()),
        "so" | "som" | "somali" => Some("som".to_string()),
        "jv" | "jav" | "javanese" => Some("jav".to_string()),
        "su" | "sun" | "sundanese" => Some("sun".to_string()),
        "kn" | "kan" | "kannada" => Some("kan".to_string()),
        "ml" | "mal" | "malayalam" => Some("mal".to_string()),
        "mr" | "mar" | "marathi" => Some("mar".to_string()),
        "gu" | "guj" | "gujarati" => Some("guj".to_string()),
        "kk" | "kaz" | "kazakh" => Some("kaz".to_string()),
        "ky" | "kir" | "kyrgyz" => Some("kir".to_string()),
        "tg" | "tgk" | "tajik" => Some("tgk".to_string()),
        "tk" | "tuk" | "turkmen" => Some("tuk".to_string()),
        "ps" | "pus" | "pashto" => Some("pus".to_string()),
        "ku" | "kur" | "kurdish" => Some("kur".to_string()),
        "ht" | "hat" | "haitian" | "creole" => Some("hat".to_string()),
        "mg" | "mlg" | "malagasy" => Some("mlg".to_string()),
        "yo" | "yor" | "yoruba" => Some("yor".to_string()),
        "ig" | "ibo" | "igbo" => Some("ibo".to_string()),
        "ha" | "hau" | "hausa" => Some("hau".to_string()),
        "zu" | "zul" | "zulu" => Some("zul".to_string()),
        "xh" | "xho" | "xhosa" => Some("xho".to_string()),
        "sn" | "sna" | "shona" => Some("sna".to_string()),
        "st" | "sot" | "sotho" => Some("sot".to_string()),
        "tn" | "tsn" | "tswana" => Some("tsn".to_string()),
        "ts" | "tso" | "tsonga" => Some("tso".to_string()),
        "ss" | "ssw" | "swati" => Some("ssw".to_string()),
        "ve" | "ven" | "venda" => Some("ven".to_string()),
        "nr" | "nbl" => Some("nbl".to_string()),
        "nd" | "nde" => Some("nde".to_string()),
        "ny" | "nya" | "chichewa" | "chewa" => Some("nya".to_string()),
        "rw" | "kin" | "kinyarwanda" => Some("kin".to_string()),
        "rn" | "run" | "kirundi" => Some("run".to_string()),
        "sm" | "smo" | "samoan" => Some("smo".to_string()),
        "to" | "ton" | "tongan" => Some("ton".to_string()),
        "fj" | "fij" | "fijian" => Some("fij".to_string()),
        "mi" | "mri" | "mao" | "maori" => Some("mri".to_string()),
        "fo" | "fao" | "faroese" => Some("fao".to_string()),
        "gd" | "gla" | "gaelic" => Some("gla".to_string()),
        "gv" | "glv" | "manx" => Some("glv".to_string()),
        "kw" | "cor" | "cornish" => Some("cor".to_string()),
        "br" | "bre" | "breton" => Some("bre".to_string()),
        "co" | "cos" | "corsican" => Some("cos".to_string()),
        "sc" | "srd" | "sardinian" => Some("srd".to_string()),
        "mt" | "mlt" | "maltese" => Some("mlt".to_string()),
        "lb" | "ltz" | "luxembourgish" => Some("ltz".to_string()),
        "fy" | "fry" | "frisian" => Some("fry".to_string()),
        "yi" | "yid" | "yiddish" => Some("yid".to_string()),
        "sa" | "san" | "sanskrit" => Some("san".to_string()),
        "bo" | "bod" | "tib" | "tibetan" => Some("bod".to_string()),
        "dz" | "dzo" | "dzongkha" => Some("dzo".to_string()),
        "ug" | "uig" | "uyghur" => Some("uig".to_string()),
        "tt" | "tat" | "tatar" => Some("tat".to_string()),
        "ba" | "bak" | "bashkir" => Some("bak".to_string()),
        "cv" | "chv" | "chuvash" => Some("chv".to_string()),
        "kv" | "kom" | "komi" => Some("kom".to_string()),
        "os" | "oss" | "ossetian" => Some("oss".to_string()),
        "ab" | "abk" | "abkhazian" => Some("abk".to_string()),
        "av" | "ava" | "avaric" => Some("ava".to_string()),
        "ce" | "che" | "chechen" => Some("che".to_string()),
        "mo" | "mol" | "moldavian" => Some("ron".to_string()),
        "or" | "ori" | "oriya" => Some("ori".to_string()),
        "as" | "asm" | "assamese" => Some("asm".to_string()),
        "qu" | "que" | "quechua" => Some("que".to_string()),
        "ay" | "aym" | "aymara" => Some("aym".to_string()),
        "gn" | "grn" | "guarani" => Some("grn".to_string()),
        "iu" | "iku" | "inuktitut" => Some("iku".to_string()),
        "ik" | "ipk" | "inupiaq" => Some("ipk".to_string()),
        "kl" | "kal" | "kalaallisut" | "greenlandic" => Some("kal".to_string()),
        "haw" | "hawaiian" => Some("haw".to_string()),
        "instrumental" | "zxx" => Some("zxx".to_string()),
        _ => {
            // If it is already a 3 ASCII letter code, preserve it lowercased
            if trimmed.len() == 3 && trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
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
        assert_eq!(resolve_language("fr"), Some("fra".to_string()));
        assert_eq!(resolve_language("fra"), Some("fra".to_string()));
        assert_eq!(resolve_language("fre"), Some("fra".to_string()));

        assert_eq!(resolve_language("German"), Some("deu".to_string()));
        assert_eq!(resolve_language("de"), Some("deu".to_string()));
        assert_eq!(resolve_language("deu"), Some("deu".to_string()));
        assert_eq!(resolve_language("ger"), Some("deu".to_string()));

        assert_eq!(resolve_language("Japanese"), Some("jpn".to_string()));
        assert_eq!(resolve_language("ja"), Some("jpn".to_string()));
        assert_eq!(resolve_language("jpn"), Some("jpn".to_string()));
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
        assert_eq!(resolve_language("xx"), None); // Unknown 2-letter code rejected instead of leaking 2-letter
    }
}

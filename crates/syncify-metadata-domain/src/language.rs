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

/// Derives the dominant musical language (ISO 639-2 code) for a canonical country name or ISO alpha-2 code.
pub fn default_language_for_country(country: &str) -> Option<&'static str> {
    let lower = country.trim().to_lowercase();
    match lower.as_str() {
        "united states" | "us" | "usa" | "united kingdom" | "gb" | "uk" | "great britain"
        | "australia" | "au" | "new zealand" | "nz" | "canada" | "ca" | "ireland" | "ie" => Some("eng"),
        "spain" | "es" | "mexico" | "mx" | "argentina" | "ar" | "colombia" | "co"
        | "chile" | "cl" | "peru" | "pe" | "venezuela" | "ve" | "ecuador" | "ec"
        | "guatemala" | "gt" | "cuba" | "cu" | "bolivia" | "bo" | "dominican republic" | "do"
        | "honduras" | "hn" | "paraguay" | "py" | "el salvador" | "sv" | "nicaragua" | "ni"
        | "costa rica" | "cr" | "puerto rico" | "pr" | "panama" | "pa" | "uruguay" | "uy" => Some("spa"),
        "france" | "fr" | "belgium" | "be" | "monaco" | "mc" => Some("fra"),
        "germany" | "de" | "austria" | "at" | "switzerland" | "ch" => Some("deu"),
        "italy" | "it" => Some("ita"),
        "brazil" | "br" | "portugal" | "pt" => Some("por"),
        "japan" | "jp" => Some("jpn"),
        "south korea" | "korea" | "kr" => Some("kor"),
        "russia" | "ru" | "belarus" | "by" | "kazakhstan" | "kz" | "ukraine" | "ua" => Some("rus"),
        "china" | "cn" | "taiwan" | "tw" | "hong kong" | "hk" => Some("zho"),
        "netherlands" | "nl" => Some("nld"),
        "sweden" | "se" => Some("swe"),
        "norway" | "no" => Some("nor"),
        "denmark" | "dk" => Some("dan"),
        "finland" | "fi" => Some("fin"),
        "poland" | "pl" => Some("pol"),
        "turkey" | "tr" => Some("tur"),
        "india" | "in" => Some("hin"),
        "greece" | "gr" => Some("ell"),
        "czech republic" | "cz" => Some("ces"),
        "hungary" | "hu" => Some("hun"),
        "romania" | "ro" => Some("ron"),
        "israel" | "il" => Some("heb"),
        "thailand" | "th" => Some("tha"),
        "vietnam" | "vn" => Some("vie"),
        "indonesia" | "id" => Some("ind"),
        "iceland" | "is" => Some("isl"),
        "albania" | "al" => Some("sqi"),
        "bahrain" | "bh" | "saudi arabia" | "sa" | "egypt" | "eg" | "united arab emirates" | "ae" => Some("ara"),
        _ => None,
    }
}

/// Check if a language string is a valid ISO code or resolvable language name
pub fn is_valid_language(val: &str) -> bool {
    resolve_language(val).is_some()
}

/// ISO code -> canonical English display name table used on the tag wire format.
///
/// directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
/// Keys cover ISO 639-1 (2-letter) plus ISO 639-2/B and 639-2/T (3-letter) spellings for
/// every language produced by [`default_language_for_country`] (eng, spa, fra, deu, ita,
/// por, jpn, kor, rus, zho, nld, swe, nor, dan, fin, pol, tur, hin, ell, ces, hun, ron,
/// heb, tha, vie, ind, isl, sqi, ara) and for the usual catalog languages handled by
/// [`resolve_language`]. `zxx`/instrumental is deliberately absent: it has no
/// natural-language name, so it stays a code on the wire instead of inventing one.
const LANGUAGE_DISPLAY_NAMES: &[(&str, &str)] = &[
    // Core catalog + default_language_for_country outputs (with B/T aliases)
    ("en", "English"), ("eng", "English"),
    ("es", "Spanish"), ("spa", "Spanish"),
    ("fr", "French"), ("fra", "French"), ("fre", "French"),
    ("de", "German"), ("deu", "German"), ("ger", "German"),
    ("it", "Italian"), ("ita", "Italian"),
    ("pt", "Portuguese"), ("por", "Portuguese"),
    ("ja", "Japanese"), ("jpn", "Japanese"),
    ("ko", "Korean"), ("kor", "Korean"),
    ("ru", "Russian"), ("rus", "Russian"),
    ("zh", "Chinese"), ("zho", "Chinese"), ("chi", "Chinese"),
    ("nl", "Dutch"), ("nld", "Dutch"), ("dut", "Dutch"),
    ("sv", "Swedish"), ("swe", "Swedish"),
    ("no", "Norwegian"), ("nor", "Norwegian"),
    ("nb", "Norwegian"), ("nob", "Norwegian"),
    ("nn", "Norwegian"), ("nno", "Norwegian"),
    ("da", "Danish"), ("dan", "Danish"),
    ("fi", "Finnish"), ("fin", "Finnish"),
    ("pl", "Polish"), ("pol", "Polish"),
    ("cs", "Czech"), ("ces", "Czech"), ("cze", "Czech"),
    ("hu", "Hungarian"), ("hun", "Hungarian"),
    ("tr", "Turkish"), ("tur", "Turkish"),
    ("ar", "Arabic"), ("ara", "Arabic"),
    ("he", "Hebrew"), ("heb", "Hebrew"),
    ("hi", "Hindi"), ("hin", "Hindi"),
    ("th", "Thai"), ("tha", "Thai"),
    ("vi", "Vietnamese"), ("vie", "Vietnamese"),
    ("uk", "Ukrainian"), ("ukr", "Ukrainian"),
    ("el", "Greek"), ("ell", "Greek"), ("gre", "Greek"),
    ("ro", "Romanian"), ("ron", "Romanian"), ("rum", "Romanian"),
    ("ca", "Catalan"), ("cat", "Catalan"),
    ("gl", "Galician"), ("glg", "Galician"),
    ("eu", "Basque"), ("eus", "Basque"), ("baq", "Basque"),
    ("is", "Icelandic"), ("isl", "Icelandic"), ("ice", "Icelandic"),
    ("sr", "Serbian"), ("srp", "Serbian"),
    ("hr", "Croatian"), ("hrv", "Croatian"),
    ("bg", "Bulgarian"), ("bul", "Bulgarian"),
    ("id", "Indonesian"), ("ind", "Indonesian"),
    ("tl", "Tagalog"), ("tgl", "Tagalog"), ("fil", "Tagalog"),
    ("yo", "Yoruba"), ("yor", "Yoruba"),
    ("sq", "Albanian"), ("sqi", "Albanian"), ("alb", "Albanian"),
    // Rest of the resolve_language catalog
    ("la", "Latin"), ("lat", "Latin"),
    ("ms", "Malay"), ("msa", "Malay"), ("may", "Malay"),
    ("fa", "Persian"), ("fas", "Persian"), ("per", "Persian"),
    ("sk", "Slovak"), ("slk", "Slovak"), ("slo", "Slovak"),
    ("sl", "Slovenian"), ("slv", "Slovenian"),
    ("lt", "Lithuanian"), ("lit", "Lithuanian"),
    ("lv", "Latvian"), ("lav", "Latvian"),
    ("et", "Estonian"), ("est", "Estonian"),
    ("af", "Afrikaans"), ("afr", "Afrikaans"),
    ("hy", "Armenian"), ("hye", "Armenian"), ("arm", "Armenian"),
    ("az", "Azerbaijani"), ("aze", "Azerbaijani"),
    ("be", "Belarusian"), ("bel", "Belarusian"),
    ("bn", "Bengali"), ("ben", "Bengali"),
    ("bs", "Bosnian"), ("bos", "Bosnian"),
    ("ka", "Georgian"), ("kat", "Georgian"), ("geo", "Georgian"),
    ("mk", "Macedonian"), ("mkd", "Macedonian"), ("mac", "Macedonian"),
    ("sw", "Swahili"), ("swa", "Swahili"),
    ("ta", "Tamil"), ("tam", "Tamil"),
    ("te", "Telugu"), ("tel", "Telugu"),
    ("ur", "Urdu"), ("urd", "Urdu"),
    ("uz", "Uzbek"), ("uzb", "Uzbek"),
    ("cy", "Welsh"), ("cym", "Welsh"), ("wel", "Welsh"),
    ("eo", "Esperanto"), ("epo", "Esperanto"),
    ("am", "Amharic"), ("amh", "Amharic"),
    ("km", "Khmer"), ("khm", "Khmer"),
    ("lo", "Lao"), ("lao", "Lao"),
    ("mn", "Mongolian"), ("mon", "Mongolian"),
    ("my", "Burmese"), ("mya", "Burmese"), ("bur", "Burmese"),
    ("ne", "Nepali"), ("nep", "Nepali"),
    ("pa", "Punjabi"), ("pan", "Punjabi"),
    ("si", "Sinhala"), ("sin", "Sinhala"),
    ("so", "Somali"), ("som", "Somali"),
    ("jv", "Javanese"), ("jav", "Javanese"),
    ("su", "Sundanese"), ("sun", "Sundanese"),
    ("kn", "Kannada"), ("kan", "Kannada"),
    ("ml", "Malayalam"), ("mal", "Malayalam"),
    ("mr", "Marathi"), ("mar", "Marathi"),
    ("gu", "Gujarati"), ("guj", "Gujarati"),
    ("kk", "Kazakh"), ("kaz", "Kazakh"),
    ("ky", "Kyrgyz"), ("kir", "Kyrgyz"),
    ("tg", "Tajik"), ("tgk", "Tajik"),
    ("tk", "Turkmen"), ("tuk", "Turkmen"),
    ("ps", "Pashto"), ("pus", "Pashto"),
    ("ku", "Kurdish"), ("kur", "Kurdish"),
    ("ht", "Haitian Creole"), ("hat", "Haitian Creole"),
    ("mg", "Malagasy"), ("mlg", "Malagasy"),
    ("ig", "Igbo"), ("ibo", "Igbo"),
    ("ha", "Hausa"), ("hau", "Hausa"),
    ("zu", "Zulu"), ("zul", "Zulu"),
    ("xh", "Xhosa"), ("xho", "Xhosa"),
    ("sn", "Shona"), ("sna", "Shona"),
    ("st", "Sotho"), ("sot", "Sotho"),
    ("tn", "Tswana"), ("tsn", "Tswana"),
    ("ts", "Tsonga"), ("tso", "Tsonga"),
    ("ss", "Swati"), ("ssw", "Swati"),
    ("ve", "Venda"), ("ven", "Venda"),
    ("nr", "Southern Ndebele"), ("nbl", "Southern Ndebele"),
    ("nd", "Northern Ndebele"), ("nde", "Northern Ndebele"),
    ("ny", "Chichewa"), ("nya", "Chichewa"),
    ("rw", "Kinyarwanda"), ("kin", "Kinyarwanda"),
    ("rn", "Kirundi"), ("run", "Kirundi"),
    ("sm", "Samoan"), ("smo", "Samoan"),
    ("to", "Tongan"), ("ton", "Tongan"),
    ("fj", "Fijian"), ("fij", "Fijian"),
    ("mi", "Maori"), ("mri", "Maori"), ("mao", "Maori"),
    ("fo", "Faroese"), ("fao", "Faroese"),
    ("gd", "Scottish Gaelic"), ("gla", "Scottish Gaelic"),
    ("gv", "Manx"), ("glv", "Manx"),
    ("kw", "Cornish"), ("cor", "Cornish"),
    ("br", "Breton"), ("bre", "Breton"),
    ("co", "Corsican"), ("cos", "Corsican"),
    ("sc", "Sardinian"), ("srd", "Sardinian"),
    ("mt", "Maltese"), ("mlt", "Maltese"),
    ("lb", "Luxembourgish"), ("ltz", "Luxembourgish"),
    ("fy", "Frisian"), ("fry", "Frisian"),
    ("yi", "Yiddish"), ("yid", "Yiddish"),
    ("sa", "Sanskrit"), ("san", "Sanskrit"),
    ("bo", "Tibetan"), ("bod", "Tibetan"), ("tib", "Tibetan"),
    ("dz", "Dzongkha"), ("dzo", "Dzongkha"),
    ("ug", "Uyghur"), ("uig", "Uyghur"),
    ("tt", "Tatar"), ("tat", "Tatar"),
    ("ba", "Bashkir"), ("bak", "Bashkir"),
    ("cv", "Chuvash"), ("chv", "Chuvash"),
    ("kv", "Komi"), ("kom", "Komi"),
    ("os", "Ossetian"), ("oss", "Ossetian"),
    ("ab", "Abkhazian"), ("abk", "Abkhazian"),
    ("av", "Avaric"), ("ava", "Avaric"),
    ("ce", "Chechen"), ("che", "Chechen"),
    ("or", "Oriya"), ("ori", "Oriya"),
    ("as", "Assamese"), ("asm", "Assamese"),
    ("qu", "Quechua"), ("que", "Quechua"),
    ("ay", "Aymara"), ("aym", "Aymara"),
    ("gn", "Guarani"), ("grn", "Guarani"),
    ("iu", "Inuktitut"), ("iku", "Inuktitut"),
    ("ik", "Inupiaq"), ("ipk", "Inupiaq"),
    ("kl", "Kalaallisut"), ("kal", "Kalaallisut"),
    ("haw", "Hawaiian"),
];

/// Returns the canonical English display name for an ISO 639-1 / 639-2-B / 639-2-T code.
///
/// directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
/// Case-insensitive exact match on the CODE only. If the input is already a known name
/// ("English", "Deutsch") or something unknown, returns `None` so the caller keeps it
/// verbatim — never invent a name that is not in [`LANGUAGE_DISPLAY_NAMES`].
pub fn language_display_name(input: &str) -> Option<&'static str> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    LANGUAGE_DISPLAY_NAMES
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(trimmed))
        .map(|(_, name)| *name)
}

/// Wire-format value for LANGUAGE tags (FLAC VorbisComment `LANGUAGE`, MP4/M4A `©lng`).
///
/// directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
/// Codes become English display names ("eng" -> "English"). Inputs that are already
/// names/endonyms are canonicalized through [`resolve_language`] ("Deutsch" -> deu ->
/// "German") so one language always yields one wire form regardless of upstream spelling.
/// Unknown values pass through verbatim (never invented). This single helper backs both
/// container writers AND their verifiers.
pub fn wire_language_value(input: &str) -> String {
    if let Some(name) = language_display_name(input) {
        return name.to_string();
    }
    match resolve_language(input) {
        Some(code) => language_display_name(&code)
            .map(|name| name.to_string())
            .unwrap_or(code),
        None => input.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_language_for_country() {
        assert_eq!(default_language_for_country("United Kingdom"), Some("eng"));
        assert_eq!(default_language_for_country("US"), Some("eng"));
        assert_eq!(default_language_for_country("Spain"), Some("spa"));
        assert_eq!(default_language_for_country("Mexico"), Some("spa"));
        assert_eq!(default_language_for_country("Germany"), Some("deu"));
        assert_eq!(default_language_for_country("France"), Some("fra"));
        assert_eq!(default_language_for_country("Japan"), Some("jpn"));
        assert_eq!(default_language_for_country("UnknownLand"), None);
    }

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

    #[test]
    fn test_language_display_name_codes_and_aliases() {
        // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
        assert_eq!(language_display_name("eng"), Some("English"));
        assert_eq!(language_display_name("ENG"), Some("English"));
        assert_eq!(language_display_name(" en "), Some("English"));
        assert_eq!(language_display_name("spa"), Some("Spanish"));
        assert_eq!(language_display_name("fra"), Some("French"));
        assert_eq!(language_display_name("fre"), Some("French")); // 639-2/B alias
        assert_eq!(language_display_name("deu"), Some("German"));
        assert_eq!(language_display_name("ger"), Some("German")); // 639-2/B alias
        assert_eq!(language_display_name("ita"), Some("Italian"));
        assert_eq!(language_display_name("por"), Some("Portuguese"));
        assert_eq!(language_display_name("rus"), Some("Russian"));
        assert_eq!(language_display_name("nld"), Some("Dutch"));
        assert_eq!(language_display_name("dut"), Some("Dutch")); // 639-2/B alias
        assert_eq!(language_display_name("jpn"), Some("Japanese"));
        assert_eq!(language_display_name("kor"), Some("Korean"));
        assert_eq!(language_display_name("zho"), Some("Chinese"));
        assert_eq!(language_display_name("chi"), Some("Chinese")); // 639-2/B alias
        assert_eq!(language_display_name("swe"), Some("Swedish"));
        assert_eq!(language_display_name("nor"), Some("Norwegian"));
        assert_eq!(language_display_name("dan"), Some("Danish"));
        assert_eq!(language_display_name("fin"), Some("Finnish"));
        assert_eq!(language_display_name("pol"), Some("Polish"));
        assert_eq!(language_display_name("ces"), Some("Czech"));
        assert_eq!(language_display_name("cze"), Some("Czech"));
        assert_eq!(language_display_name("hun"), Some("Hungarian"));
        assert_eq!(language_display_name("tur"), Some("Turkish"));
        assert_eq!(language_display_name("ara"), Some("Arabic"));
        assert_eq!(language_display_name("heb"), Some("Hebrew"));
        assert_eq!(language_display_name("hin"), Some("Hindi"));
        assert_eq!(language_display_name("tha"), Some("Thai"));
        assert_eq!(language_display_name("vie"), Some("Vietnamese"));
        assert_eq!(language_display_name("ukr"), Some("Ukrainian"));
        assert_eq!(language_display_name("ell"), Some("Greek"));
        assert_eq!(language_display_name("gre"), Some("Greek"));
        assert_eq!(language_display_name("ron"), Some("Romanian"));
        assert_eq!(language_display_name("rum"), Some("Romanian"));
        assert_eq!(language_display_name("cat"), Some("Catalan"));
        assert_eq!(language_display_name("glg"), Some("Galician"));
        assert_eq!(language_display_name("eus"), Some("Basque"));
        assert_eq!(language_display_name("baq"), Some("Basque"));
        assert_eq!(language_display_name("isl"), Some("Icelandic"));
        assert_eq!(language_display_name("srp"), Some("Serbian"));
        assert_eq!(language_display_name("hrv"), Some("Croatian"));
        assert_eq!(language_display_name("bul"), Some("Bulgarian"));
        assert_eq!(language_display_name("ind"), Some("Indonesian"));
        assert_eq!(language_display_name("tgl"), Some("Tagalog"));
        assert_eq!(language_display_name("fil"), Some("Tagalog")); // fil/tgl family
        assert_eq!(language_display_name("yor"), Some("Yoruba"));
    }

    #[test]
    fn test_language_display_table_covers_default_language_for_country_outputs() {
        // Every language default_language_for_country can produce must have a display name.
        const COUNTRIES: &[&str] = &[
            "united states", "us", "usa", "united kingdom", "gb", "uk", "great britain",
            "australia", "au", "new zealand", "nz", "canada", "ca", "ireland", "ie",
            "spain", "es", "mexico", "mx", "argentina", "ar", "colombia", "co", "chile",
            "cl", "peru", "pe", "venezuela", "ve", "ecuador", "ec", "guatemala", "gt",
            "cuba", "cu", "bolivia", "bo", "dominican republic", "do", "honduras", "hn",
            "paraguay", "py", "el salvador", "sv", "nicaragua", "ni", "costa rica", "cr",
            "puerto rico", "pr", "panama", "pa", "uruguay", "uy",
            "france", "fr", "belgium", "be", "monaco", "mc",
            "germany", "de", "austria", "at", "switzerland", "ch", "italy", "it",
            "brazil", "br", "portugal", "pt", "japan", "jp",
            "south korea", "korea", "kr", "russia", "ru", "belarus", "by",
            "kazakhstan", "kz", "ukraine", "ua", "china", "cn", "taiwan", "tw",
            "hong kong", "hk", "netherlands", "nl", "sweden", "se", "norway", "no",
            "denmark", "dk", "finland", "fi", "poland", "pl", "turkey", "tr", "india",
            "in", "greece", "gr", "czech republic", "cz", "hungary", "hu", "romania",
            "ro", "israel", "il", "thailand", "th", "vietnam", "vn", "indonesia", "id",
            "iceland", "is", "albania", "al", "bahrain", "bh", "saudi arabia", "sa",
            "egypt", "eg", "united arab emirates", "ae",
        ];
        for country in COUNTRIES {
            let code = default_language_for_country(country).unwrap_or_else(|| {
                panic!("default_language_for_country('{}') must resolve", country)
            });
            assert!(
                language_display_name(code).is_some(),
                "display table must cover '{}' produced by default_language_for_country('{}')",
                code,
                country
            );
        }
    }

    #[test]
    fn test_language_display_name_known_names_and_unknown_return_none() {
        // Already-a-name and unknown inputs return None: caller keeps them verbatim.
        assert_eq!(language_display_name("English"), None);
        assert_eq!(language_display_name("spanish"), None);
        assert_eq!(language_display_name("Deutsch"), None);
        assert_eq!(language_display_name(""), None);
        assert_eq!(language_display_name("   "), None);
        assert_eq!(language_display_name("Klingonish"), None);
        assert_eq!(language_display_name("xy"), None);
        assert_eq!(language_display_name("123"), None);
    }

    #[test]
    fn test_wire_language_value_names_on_the_wire() {
        // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
        // Codes become names...
        assert_eq!(wire_language_value("eng"), "English");
        assert_eq!(wire_language_value("spa"), "Spanish");
        assert_eq!(wire_language_value("fra"), "French");
        assert_eq!(wire_language_value("jpn"), "Japanese");
        // ...names/endonyms canonicalize stably regardless of upstream spelling...
        assert_eq!(wire_language_value("English"), "English");
        assert_eq!(wire_language_value("english"), "English");
        assert_eq!(wire_language_value("Deutsch"), "German");
        assert_eq!(wire_language_value("Español"), "Spanish");
        assert_eq!(wire_language_value("ingles"), "English");
        // ...unknown values stay verbatim (never invented)
        assert_eq!(wire_language_value("xyz"), "xyz");
        assert_eq!(wire_language_value("zxx"), "zxx");
        assert_eq!(wire_language_value("instrumental"), "zxx"); // no invented name for zxx
        assert_eq!(wire_language_value("Klingonish"), "Klingonish");
    }
}

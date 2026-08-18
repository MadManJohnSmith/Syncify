//! Canonical country & region normalization module.
//!
//! Provides deterministic mapping from ISO alpha-2, ISO alpha-3, localized names
//! (English, Spanish, diacritics), legacy aliases (e.g. UK -> GB) to ISO 3166-1 alpha-2.
//! Preserves regional and supranational entities (Europe, Worldwide) without false country conversion.

use serde::{Deserialize, Serialize};

/// Resolution classification for country and region metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CountryResolution {
    /// Canonical sovereign country with ISO 3166-1 alpha-2 code
    Country {
        iso_alpha2: String,
        canonical_name: String,
    },
    /// Supranational / regional entity (MusicBrainz XE/XW or named regions)
    Region {
        region_code: Option<String>,
        region_name: String,
    },
    /// Unknown or unresolved input
    Unknown(String),
}

impl CountryResolution {
    pub fn is_country(&self) -> bool {
        matches!(self, CountryResolution::Country { .. })
    }

    pub fn is_region(&self) -> bool {
        matches!(self, CountryResolution::Region { .. })
    }

    pub fn country_code(&self) -> Option<&str> {
        match self {
            CountryResolution::Country { iso_alpha2, .. } => Some(iso_alpha2.as_str()),
            _ => None,
        }
    }

    pub fn region_name(&self) -> Option<&str> {
        match self {
            CountryResolution::Region { region_name, .. } => Some(region_name.as_str()),
            _ => None,
        }
    }

    pub fn region_code(&self) -> Option<&str> {
        match self {
            CountryResolution::Region { region_code: Some(code), .. } => Some(code.as_str()),
            _ => None,
        }
    }
}

/// Normalizes diacritics and punctuation for case-insensitive matching
fn sanitize_country_str(input: &str) -> String {
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

/// Resolves an input string into a structured CountryResolution
pub fn resolve_country(input: &str) -> CountryResolution {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return CountryResolution::Unknown(String::new());
    }

    let upper = trimmed.to_uppercase();
    let sanitized = sanitize_country_str(trimmed);

    // 1. Direct ISO 3166-1 Alpha-2 / Alpha-3 / Legacy Table Matching
    match upper.as_str() {
        // --- Legacy CLI Exact Matches ---
        "AF" | "AFG" => return country("AF", "Afghanistan"),
        "AT" | "AUT" => return country("AT", "Austria"),
        "ES" | "ESP" => return country("ES", "Spain"),
        "MX" | "MEX" => return country("MX", "Mexico"),
        "NL" | "NLD" => return country("NL", "Netherlands"),
        "PL" | "POL" => return country("PL", "Poland"),
        "US" | "USA" => return country("US", "United States"),
        "GB" | "GBR" | "UK" => return country("GB", "United Kingdom"),
        "JP" | "JPN" => return country("JP", "Japan"),
        "DE" | "DEU" => return country("DE", "Germany"),
        "FR" | "FRA" => return country("FR", "France"),
        "CA" | "CAN" => return country("CA", "Canada"),
        "AU" | "AUS" => return country("AU", "Australia"),
        "IT" | "ITA" => return country("IT", "Italy"),
        "BR" | "BRA" => return country("BR", "Brazil"),
        "AR" | "ARG" => return country("AR", "Argentina"),
        "CL" | "CHL" => return country("CL", "Chile"),
        "CO" | "COL" => return country("CO", "Colombia"),
        "PE" | "PER" => return country("PE", "Peru"),
        "SE" | "SWE" => return country("SE", "Sweden"),
        "NO" | "NOR" => return country("NO", "Norway"),
        "DK" | "DNK" => return country("DK", "Denmark"),
        "FI" | "FIN" => return country("FI", "Finland"),
        "BE" | "BEL" => return country("BE", "Belgium"),
        "CH" | "CHE" => return country("CH", "Switzerland"),
        "PT" | "PRT" => return country("PT", "Portugal"),
        "IE" | "IRL" => return country("IE", "Ireland"),
        "NZ" | "NZL" => return country("NZ", "New Zealand"),
        "RU" | "RUS" => return country("RU", "Russia"),
        "CN" | "CHN" => return country("CN", "China"),
        "KR" | "KOR" => return country("KR", "South Korea"),
        "IN" | "IND" => return country("IN", "India"),
        "ZA" | "ZAF" => return country("ZA", "South Africa"),
        "GR" | "GRC" => return country("GR", "Greece"),
        "TR" | "TUR" => return country("TR", "Turkey"),
        "CZ" | "CZE" => return country("CZ", "Czech Republic"),
        "HU" | "HUN" => return country("HU", "Hungary"),
        "RO" | "ROU" => return country("RO", "Romania"),
        "BG" | "BGR" => return country("BG", "Bulgaria"),
        "UA" | "UKR" => return country("UA", "Ukraine"),
        "IL" | "ISR" => return country("IL", "Israel"),
        "EG" | "EGY" => return country("EG", "Egypt"),
        "SA" | "SAU" => return country("SA", "Saudi Arabia"),
        "AE" | "ARE" => return country("AE", "United Arab Emirates"),
        "SG" | "SGP" => return country("SG", "Singapore"),
        "HK" | "HKG" => return country("HK", "Hong Kong"),
        "TW" | "TWN" => return country("TW", "Taiwan"),
        "TH" | "THA" => return country("TH", "Thailand"),
        "ID" | "IDN" => return country("ID", "Indonesia"),
        "MY" | "MYS" => return country("MY", "Malaysia"),
        "PH" | "PHL" => return country("PH", "Philippines"),
        "VN" | "VNM" => return country("VN", "Vietnam"),
        "IS" | "ISL" => return country("IS", "Iceland"),
        "LU" | "LUX" => return country("LU", "Luxembourg"),
        "CU" | "CUB" => return country("CU", "Cuba"),
        "DO" | "DOM" => return country("DO", "Dominican Republic"),
        "PR" | "PRI" => return country("PR", "Puerto Rico"),
        "UY" | "URY" => return country("UY", "Uruguay"),
        "VE" | "VEN" => return country("VE", "Venezuela"),
        "EC" | "ECU" => return country("EC", "Ecuador"),
        "BO" | "BOL" => return country("BO", "Bolivia"),
        "PY" | "PRY" => return country("PY", "Paraguay"),
        "CR" | "CRI" => return country("CR", "Costa Rica"),
        "PA" | "PAN" => return country("PA", "Panama"),
        "GT" | "GTM" => return country("GT", "Guatemala"),

        // --- Explicit Regional / Supranational Entities ---
        "XE" => return region(Some("XE"), "Europe"),
        "XW" => return region(Some("XW"), "Worldwide"),
        "[WORLDWIDE]" | "WORLDWIDE" => return region(Some("XW"), "Worldwide"),
        "EUROPE" => return region(Some("XE"), "Europe"),
        _ => {}
    }

    // 2. Localized Name / Alias Match (English, Spanish, Diacritics Normalized)
    match sanitized.as_str() {
        // Spain / España
        "spain" | "espana" => country("ES", "Spain"),

        // United States / EE.UU. / Estados Unidos
        "united states"
        | "united states of america"
        | "estados unidos"
        | "estados unidos de america"
        | "eeuu"
        | "ee uu" => country("US", "United States"),

        // United Kingdom / Great Britain / UK / Reino Unido
        "united kingdom"
        | "great britain"
        | "reino unido"
        | "gran bretana"
        | "england"
        | "scotland"
        | "wales"
        | "britain" => country("GB", "United Kingdom"),

        // Germany / Alemania / Deutschland
        "germany" | "alemania" | "deutschland" => country("DE", "Germany"),

        // France / Francia
        "france" | "francia" => country("FR", "France"),

        // Japan / Japón
        "japan" | "japon" | "nippon" | "nihon" => country("JP", "Japan"),

        // Canada / Canadá
        "canada" => country("CA", "Canada"),

        // Australia
        "australia" => country("AU", "Australia"),

        // Mexico / México
        "mexico" => country("MX", "Mexico"),

        // Netherlands / Países Bajos / Holanda
        "netherlands" | "paises bajos" | "holanda" | "the netherlands" => {
            country("NL", "Netherlands")
        }

        // Poland / Polonia
        "poland" | "polonia" | "polska" => country("PL", "Poland"),

        // Austria
        "austria" | "osterreich" => country("AT", "Austria"),

        // Afghanistan / Afganistán
        "afghanistan" | "afganistan" => country("AF", "Afghanistan"),

        // Italy / Italia
        "italy" | "italia" => country("IT", "Italy"),

        // Brazil / Brasil
        "brazil" | "brasil" => country("BR", "Brazil"),

        // Argentina
        "argentina" => country("AR", "Argentina"),

        // Chile
        "chile" => country("CL", "Chile"),

        // Colombia
        "colombia" => country("CO", "Colombia"),

        // Peru / Perú
        "peru" => country("PE", "Peru"),

        // Sweden / Suecia
        "sweden" | "suecia" | "sverige" => country("SE", "Sweden"),

        // Norway / Noruega
        "norway" | "noruega" | "norge" => country("NO", "Norway"),

        // Denmark / Dinamarca
        "denmark" | "dinamarca" | "danmark" => country("DK", "Denmark"),

        // Finland / Finlandia
        "finland" | "finlandia" | "suomi" => country("FI", "Finland"),

        // Belgium / Bélgica
        "belgium" | "belgica" | "belgique" => country("BE", "Belgium"),

        // Switzerland / Suiza
        "switzerland" | "suiza" | "schweiz" | "suisse" => country("CH", "Switzerland"),

        // Portugal
        "portugal" => country("PT", "Portugal"),

        // Ireland / Irlanda
        "ireland" | "irlanda" | "eire" => country("IE", "Ireland"),

        // New Zealand / Nueva Zelanda
        "new zealand" | "nueva zelanda" => country("NZ", "New Zealand"),

        // Russia / Rusia
        "russia" | "rusia" | "russian federation" => country("RU", "Russia"),

        // China
        "china" | "peoples republic of china" => country("CN", "China"),

        // South Korea / Corea del Sur
        "south korea" | "corea del sur" | "korea republic of" | "republic of korea" => {
            country("KR", "South Korea")
        }

        // India
        "india" => country("IN", "India"),

        // South Africa / Sudáfrica
        "south africa" | "sudafrica" => country("ZA", "South Africa"),

        // Greece / Grecia
        "greece" | "grecia" => country("GR", "Greece"),

        // Turkey / Turquía
        "turkey" | "turquia" | "turkiye" => country("TR", "Turkey"),

        // Czech Republic / República Checa
        "czech republic" | "czechia" | "republica checa" => country("CZ", "Czech Republic"),

        // Hungary / Hungría
        "hungary" | "hungria" => country("HU", "Hungary"),

        // Romania / Rumania
        "romania" | "rumania" => country("RO", "Romania"),

        // Bulgaria
        "bulgaria" => country("BG", "Bulgaria"),

        // Ukraine / Ucrania
        "ukraine" | "ucrania" => country("UA", "Ukraine"),

        // Israel
        "israel" => country("IL", "Israel"),

        // Egypt / Egipto
        "egypt" | "egipto" => country("EG", "Egypt"),

        // Saudi Arabia / Arabia Saudita
        "saudi arabia" | "arabia saudita" => country("SA", "Saudi Arabia"),

        // United Arab Emirates / Emiratos Árabes Unidos
        "united arab emirates" | "emiratos arabes unidos" | "uae" => {
            country("AE", "United Arab Emirates")
        }

        // Singapore / Singapur
        "singapore" | "singapur" => country("SG", "Singapore"),

        // Hong Kong
        "hong kong" => country("HK", "Hong Kong"),

        // Taiwan / Taiwán
        "taiwan" => country("TW", "Taiwan"),

        // Thailand / Tailandia
        "thailand" | "tailandia" => country("TH", "Thailand"),

        // Indonesia
        "indonesia" => country("ID", "Indonesia"),

        // Malaysia / Malasia
        "malaysia" | "malasia" => country("MY", "Malaysia"),

        // Philippines / Filipinas
        "philippines" | "filipinas" => country("PH", "Philippines"),

        // Vietnam
        "vietnam" => country("VN", "Vietnam"),

        // Iceland / Islandia
        "iceland" | "islandia" => country("IS", "Iceland"),

        // Luxembourg / Luxemburgo
        "luxembourg" | "luxemburgo" => country("LU", "Luxembourg"),

        // Cuba
        "cuba" => country("CU", "Cuba"),

        // Dominican Republic / República Dominicana
        "dominican republic" | "republica dominicana" => country("DO", "Dominican Republic"),

        // Puerto Rico
        "puerto rico" => country("PR", "Puerto Rico"),

        // Uruguay
        "uruguay" => country("UY", "Uruguay"),

        // Venezuela
        "venezuela" => country("VE", "Venezuela"),

        // Ecuador
        "ecuador" => country("EC", "Ecuador"),

        // Bolivia
        "bolivia" => country("BO", "Bolivia"),

        // Paraguay
        "paraguay" => country("PY", "Paraguay"),

        // Costa Rica
        "costa rica" => country("CR", "Costa Rica"),

        // Panama / Panamá
        "panama" => country("PA", "Panama"),

        // Guatemala
        "guatemala" => country("GT", "Guatemala"),

        // Regional / Supranational
        "europe" | "europa" => region(Some("XE"), "Europe"),
        "worldwide" | "global" | "international" | "mundial" => region(Some("XW"), "Worldwide"),

        // Unresolved / Passthrough if valid 2-letter alphabetic code
        _ => {
            if trimmed.len() == 2 && trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
                // Return uppercase ISO alpha-2 candidate
                CountryResolution::Country {
                    iso_alpha2: upper,
                    canonical_name: trimmed.to_string(),
                }
            } else {
                CountryResolution::Unknown(trimmed.to_string())
            }
        }
    }
}

/// Helper constructor for Country variant
fn country(iso: &str, name: &str) -> CountryResolution {
    CountryResolution::Country {
        iso_alpha2: iso.to_uppercase(),
        canonical_name: name.to_string(),
    }
}

/// Helper constructor for Region variant
fn region(code: Option<&str>, name: &str) -> CountryResolution {
    CountryResolution::Region {
        region_code: code.map(|s| s.to_uppercase()),
        region_name: name.to_string(),
    }
}

/// Normalizes any recognized country input to ISO 3166-1 alpha-2 uppercase.
/// Returns `None` if the input is a region or unrecognized.
pub fn normalize_country_code(input: &str) -> Option<String> {
    match resolve_country(input) {
        CountryResolution::Country { iso_alpha2, .. } => Some(iso_alpha2),
        _ => None,
    }
}

/// Normalizes any recognized regional or supranational entity to its canonical name (e.g. "Europe", "Worldwide").
/// Returns `None` if the input is a sovereign country or unrecognized.
pub fn normalize_region_name(input: &str) -> Option<String> {
    match resolve_country(input) {
        CountryResolution::Region { region_name, .. } => Some(region_name),
        _ => None,
    }
}

/// Normalizes any recognized regional entity preserving its regional code (e.g. "XE", "XW") or name.
/// Returns `None` if the input is a sovereign country or unrecognized.
pub fn normalize_region_code_or_name(input: &str) -> Option<String> {
    match resolve_country(input) {
        CountryResolution::Region { region_code: Some(code), .. } => Some(code),
        CountryResolution::Region { region_name, .. } => Some(region_name),
        _ => None,
    }
}

/// Normalizes country or region to canonical output string.
/// For countries, returns ISO 3166-1 alpha-2 uppercase (e.g. "ES", "GB", "US").
/// For regions, returns the region code or name (e.g. "XE", "XW").
pub fn normalize_country_or_region(input: &str) -> Option<String> {
    match resolve_country(input) {
        CountryResolution::Country { iso_alpha2, .. } => Some(iso_alpha2),
        CountryResolution::Region {
            region_code: Some(code),
            ..
        } => Some(code),
        CountryResolution::Region { region_name, .. } => Some(region_name),
        CountryResolution::Unknown(_) => None,
    }
}

/// Returns the canonical display name in English if recognized
pub fn normalize_country_name(input: &str) -> Option<String> {
    match resolve_country(input) {
        CountryResolution::Country { canonical_name, .. } => Some(canonical_name),
        CountryResolution::Region { region_name, .. } => Some(region_name),
        CountryResolution::Unknown(_) => None,
    }
}

/// Resolves input into separate (Country, Region) tuple
pub fn resolve_country_and_region(input: &str) -> (Option<String>, Option<String>) {
    match resolve_country(input) {
        CountryResolution::Country { iso_alpha2, .. } => (Some(iso_alpha2), None),
        CountryResolution::Region { region_code, region_name } => {
            (None, Some(region_code.unwrap_or(region_name)))
        }
        CountryResolution::Unknown(_) => (None, None),
    }
}

/// Tag repair plan for FLAC metadata country/region tags
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagRepairPlan {
    pub original_country: Option<String>,
    pub original_region: Option<String>,
    pub target_country: Option<String>,
    pub target_region: Option<String>,
    pub needs_repair: bool,
    pub reason: Option<String>,
}

/// Computes tag repair plan for country/region fields without modifying anything (dry-run pure computation).
pub fn plan_country_repair(
    current_country: Option<&str>,
    current_region: Option<&str>,
) -> TagRepairPlan {
    let mut plan = TagRepairPlan {
        original_country: current_country.map(|s| s.to_string()),
        original_region: current_region.map(|s| s.to_string()),
        target_country: current_country.map(|s| s.to_string()),
        target_region: current_region.map(|s| s.to_string()),
        needs_repair: false,
        reason: None,
    };

    if let Some(c_str) = current_country {
        let trimmed = c_str.trim();
        if !trimmed.is_empty() {
            match resolve_country(trimmed) {
                CountryResolution::Country { iso_alpha2, .. } => {
                    if trimmed != iso_alpha2 {
                        plan.target_country = Some(iso_alpha2);
                        plan.needs_repair = true;
                        plan.reason = Some("Normalized to standard ISO 3166-1 alpha-2 uppercase".to_string());
                    }
                }
                CountryResolution::Region { region_name, region_code } => {
                    // Moving from country tag to region tag
                    plan.target_country = None;
                    let target_reg = region_code.unwrap_or(region_name);
                    if plan.target_region.is_none() {
                        plan.target_region = Some(target_reg);
                    }
                    plan.needs_repair = true;
                    plan.reason = Some(format!("Moved non-country regional entity '{}' to RELEASEREGION", trimmed));
                }
                CountryResolution::Unknown(_) => {
                    // Unknown value in country field: remove invalid country
                    plan.target_country = None;
                    plan.needs_repair = true;
                    plan.reason = Some(format!("Removed invalid country value '{}'", trimmed));
                }
            }
        }
    }

    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iso_alpha2_exact_matches() {
        assert_eq!(normalize_country_code("ES").as_deref(), Some("ES"));
        assert_eq!(normalize_country_code("es").as_deref(), Some("ES"));
        assert_eq!(normalize_country_code("GB").as_deref(), Some("GB"));
        assert_eq!(normalize_country_code("US").as_deref(), Some("US"));
        assert_eq!(normalize_country_code("FR").as_deref(), Some("FR"));
        assert_eq!(normalize_country_code("DE").as_deref(), Some("DE"));
        assert_eq!(normalize_country_code("MX").as_deref(), Some("MX"));
        assert_eq!(normalize_country_code("NL").as_deref(), Some("NL"));
        assert_eq!(normalize_country_code("PL").as_deref(), Some("PL"));
        assert_eq!(normalize_country_code("AT").as_deref(), Some("AT"));
        assert_eq!(normalize_country_code("AF").as_deref(), Some("AF"));
    }

    #[test]
    fn test_iso_alpha3_matches() {
        assert_eq!(normalize_country_code("ESP").as_deref(), Some("ES"));
        assert_eq!(normalize_country_code("esp").as_deref(), Some("ES"));
        assert_eq!(normalize_country_code("GBR").as_deref(), Some("GB"));
        assert_eq!(normalize_country_code("USA").as_deref(), Some("US"));
        assert_eq!(normalize_country_code("DEU").as_deref(), Some("DE"));
        assert_eq!(normalize_country_code("FRA").as_deref(), Some("FR"));
        assert_eq!(normalize_country_code("JPN").as_deref(), Some("JP"));
        assert_eq!(normalize_country_code("MEX").as_deref(), Some("MX"));
        assert_eq!(normalize_country_code("NLD").as_deref(), Some("NL"));
        assert_eq!(normalize_country_code("POL").as_deref(), Some("PL"));
        assert_eq!(normalize_country_code("AUT").as_deref(), Some("AT"));
        assert_eq!(normalize_country_code("AFG").as_deref(), Some("AF"));
    }

    #[test]
    fn test_localized_names_english_and_spanish() {
        assert_eq!(normalize_country_code("Spain").as_deref(), Some("ES"));
        assert_eq!(normalize_country_code("España").as_deref(), Some("ES"));
        assert_eq!(normalize_country_code("Espana").as_deref(), Some("ES"));

        assert_eq!(normalize_country_code("United States").as_deref(), Some("US"));
        assert_eq!(normalize_country_code("Estados Unidos").as_deref(), Some("US"));
        assert_eq!(normalize_country_code("EE.UU.").as_deref(), Some("US"));
        assert_eq!(normalize_country_code("EEUU").as_deref(), Some("US"));

        assert_eq!(normalize_country_code("United Kingdom").as_deref(), Some("GB"));
        assert_eq!(normalize_country_code("Reino Unido").as_deref(), Some("GB"));
        assert_eq!(normalize_country_code("Great Britain").as_deref(), Some("GB"));
        assert_eq!(normalize_country_code("Gran Bretaña").as_deref(), Some("GB"));
        assert_eq!(normalize_country_code("UK").as_deref(), Some("GB"));
        assert_eq!(normalize_country_code("uk").as_deref(), Some("GB"));

        assert_eq!(normalize_country_code("Germany").as_deref(), Some("DE"));
        assert_eq!(normalize_country_code("Alemania").as_deref(), Some("DE"));
        assert_eq!(normalize_country_code("Deutschland").as_deref(), Some("DE"));

        assert_eq!(normalize_country_code("France").as_deref(), Some("FR"));
        assert_eq!(normalize_country_code("Francia").as_deref(), Some("FR"));

        assert_eq!(normalize_country_code("Japan").as_deref(), Some("JP"));
        assert_eq!(normalize_country_code("Japón").as_deref(), Some("JP"));
        assert_eq!(normalize_country_code("Japon").as_deref(), Some("JP"));

        assert_eq!(normalize_country_code("Canada").as_deref(), Some("CA"));
        assert_eq!(normalize_country_code("Canadá").as_deref(), Some("CA"));

        assert_eq!(normalize_country_code("Mexico").as_deref(), Some("MX"));
        assert_eq!(normalize_country_code("México").as_deref(), Some("MX"));

        assert_eq!(normalize_country_code("Netherlands").as_deref(), Some("NL"));
        assert_eq!(normalize_country_code("Países Bajos").as_deref(), Some("NL"));
        assert_eq!(normalize_country_code("Holanda").as_deref(), Some("NL"));

        assert_eq!(normalize_country_code("Poland").as_deref(), Some("PL"));
        assert_eq!(normalize_country_code("Polonia").as_deref(), Some("PL"));

        assert_eq!(normalize_country_code("Austria").as_deref(), Some("AT"));
        assert_eq!(normalize_country_code("Afghanistan").as_deref(), Some("AF"));
        assert_eq!(normalize_country_code("Afganistán").as_deref(), Some("AF"));
    }

    #[test]
    fn test_regional_and_supranational_entities_not_converted_to_country() {
        assert_eq!(normalize_country_code("Europe"), None);
        assert_eq!(normalize_country_code("XE"), None);
        assert_eq!(normalize_country_code("Worldwide"), None);
        assert_eq!(normalize_country_code("XW"), None);
        assert_eq!(normalize_country_code("[Worldwide]"), None);

        // Structured resolution preserves regional entity
        assert_eq!(
            resolve_country("Europe"),
            CountryResolution::Region {
                region_code: Some("XE".to_string()),
                region_name: "Europe".to_string(),
            }
        );
        assert_eq!(
            resolve_country("Worldwide"),
            CountryResolution::Region {
                region_code: Some("XW".to_string()),
                region_name: "Worldwide".to_string(),
            }
        );
        assert_eq!(
            resolve_country("[Worldwide]"),
            CountryResolution::Region {
                region_code: Some("XW".to_string()),
                region_name: "Worldwide".to_string(),
            }
        );

        assert_eq!(normalize_region_name("Europe").as_deref(), Some("Europe"));
        assert_eq!(normalize_region_name("XE").as_deref(), Some("Europe"));
        assert_eq!(normalize_region_code_or_name("XE").as_deref(), Some("XE"));
        assert_eq!(normalize_region_name("Worldwide").as_deref(), Some("Worldwide"));
        assert_eq!(normalize_region_code_or_name("XW").as_deref(), Some("XW"));

        let (c, r) = resolve_country_and_region("XE");
        assert_eq!(c, None);
        assert_eq!(r, Some("XE".to_string()));

        let (c, r) = resolve_country_and_region("Spain");
        assert_eq!(c, Some("ES".to_string()));
        assert_eq!(r, None);
    }

    #[test]
    fn test_unknown_inputs_not_invented() {
        assert_eq!(normalize_country_code(""), None);
        assert_eq!(normalize_country_code("   "), None);
        assert_eq!(normalize_country_code("UnknownCountry123"), None);
        assert_eq!(
            resolve_country("UnknownCountry123"),
            CountryResolution::Unknown("UnknownCountry123".to_string())
        );
        let (c, r) = resolve_country_and_region("UnknownCountry123");
        assert_eq!(c, None);
        assert_eq!(r, None);
    }

    #[test]
    fn test_tag_repair_plan() {
        // XE in country -> moved to region, country cleared
        let plan_xe = plan_country_repair(Some("XE"), None);
        assert!(plan_xe.needs_repair);
        assert_eq!(plan_xe.target_country, None);
        assert_eq!(plan_xe.target_region, Some("XE".to_string()));

        // XW in country -> moved to region, country cleared
        let plan_xw = plan_country_repair(Some("XW"), None);
        assert!(plan_xw.needs_repair);
        assert_eq!(plan_xw.target_country, None);
        assert_eq!(plan_xw.target_region, Some("XW".to_string()));

        // Europe in country -> moved to region
        let plan_europe = plan_country_repair(Some("Europe"), None);
        assert!(plan_europe.needs_repair);
        assert_eq!(plan_europe.target_country, None);
        assert_eq!(plan_europe.target_region, Some("XE".to_string()));

        // Spain in country -> normalized to ES
        let plan_spain = plan_country_repair(Some("Spain"), None);
        assert!(plan_spain.needs_repair);
        assert_eq!(plan_spain.target_country, Some("ES".to_string()));

        // US in country -> already canonical, no repair
        let plan_us = plan_country_repair(Some("US"), None);
        assert!(!plan_us.needs_repair);
        assert_eq!(plan_us.target_country, Some("US".to_string()));

        // Unknown in country -> removed
        let plan_unknown = plan_country_repair(Some("NonExistentCountry99"), None);
        assert!(plan_unknown.needs_repair);
        assert_eq!(plan_unknown.target_country, None);
    }
}

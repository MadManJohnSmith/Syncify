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
            CountryResolution::Region {
                region_code: Some(code),
                ..
            } => Some(code.as_str()),
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
        "AF" | "AFG" => return country("AF", "Afghanistan"),
        "AX" | "ALA" => return country("AX", "Åland Islands"),
        "AL" | "ALB" => return country("AL", "Albania"),
        "DZ" | "DZA" => return country("DZ", "Algeria"),
        "AS" | "ASM" => return country("AS", "American Samoa"),
        "AD" | "AND" => return country("AD", "Andorra"),
        "AO" | "AGO" => return country("AO", "Angola"),
        "AI" | "AIA" => return country("AI", "Anguilla"),
        "AQ" | "ATA" => return country("AQ", "Antarctica"),
        "AG" | "ATG" => return country("AG", "Antigua and Barbuda"),
        "AR" | "ARG" => return country("AR", "Argentina"),
        "AM" | "ARM" => return country("AM", "Armenia"),
        "AW" | "ABW" => return country("AW", "Aruba"),
        "AU" | "AUS" => return country("AU", "Australia"),
        "AT" | "AUT" => return country("AT", "Austria"),
        "AZ" | "AZE" => return country("AZ", "Azerbaijan"),
        "BS" | "BHS" => return country("BS", "Bahamas"),
        "BH" | "BHR" => return country("BH", "Bahrain"),
        "BD" | "BGD" => return country("BD", "Bangladesh"),
        "BB" | "BRB" => return country("BB", "Barbados"),
        "BY" | "BLR" => return country("BY", "Belarus"),
        "BE" | "BEL" => return country("BE", "Belgium"),
        "BZ" | "BLZ" => return country("BZ", "Belize"),
        "BJ" | "BEN" => return country("BJ", "Benin"),
        "BM" | "BMU" => return country("BM", "Bermuda"),
        "BT" | "BTN" => return country("BT", "Bhutan"),
        "BO" | "BOL" => return country("BO", "Bolivia"),
        "BQ" | "BES" => return country("BQ", "Bonaire, Sint Eustatius and Saba"),
        "BA" | "BIH" => return country("BA", "Bosnia and Herzegovina"),
        "BW" | "BWA" => return country("BW", "Botswana"),
        "BV" | "BVT" => return country("BV", "Bouvet Island"),
        "BR" | "BRA" => return country("BR", "Brazil"),
        "IO" | "IOT" => return country("IO", "British Indian Ocean Territory"),
        "BN" | "BRN" => return country("BN", "Brunei"),
        "BG" | "BGR" => return country("BG", "Bulgaria"),
        "BF" | "BFA" => return country("BF", "Burkina Faso"),
        "BI" | "BDI" => return country("BI", "Burundi"),
        "CV" | "CPV" => return country("CV", "Cabo Verde"),
        "KH" | "KHM" => return country("KH", "Cambodia"),
        "CM" | "CMR" => return country("CM", "Cameroon"),
        "CA" | "CAN" => return country("CA", "Canada"),
        "KY" | "CYM" => return country("KY", "Cayman Islands"),
        "CF" | "CAF" => return country("CF", "Central African Republic"),
        "TD" | "TCD" => return country("TD", "Chad"),
        "CL" | "CHL" => return country("CL", "Chile"),
        "CN" | "CHN" => return country("CN", "China"),
        "CX" | "CXR" => return country("CX", "Christmas Island"),
        "CC" | "CCK" => return country("CC", "Cocos (Keeling) Islands"),
        "CO" | "COL" => return country("CO", "Colombia"),
        "KM" | "COM" => return country("KM", "Comoros"),
        "CG" | "COG" => return country("CG", "Republic of the Congo"),
        "CD" | "COD" => return country("CD", "Democratic Republic of the Congo"),
        "CK" | "COK" => return country("CK", "Cook Islands"),
        "CR" | "CRI" => return country("CR", "Costa Rica"),
        "CI" | "CIV" => return country("CI", "Côte d'Ivoire"),
        "HR" | "HRV" => return country("HR", "Croatia"),
        "CU" | "CUB" => return country("CU", "Cuba"),
        "CW" | "CUW" => return country("CW", "Curaçao"),
        "CY" | "CYP" => return country("CY", "Cyprus"),
        "CZ" | "CZE" => return country("CZ", "Czech Republic"),
        "DK" | "DNK" => return country("DK", "Denmark"),
        "DJ" | "DJI" => return country("DJ", "Djibouti"),
        "DM" | "DMA" => return country("DM", "Dominica"),
        "DO" | "DOM" => return country("DO", "Dominican Republic"),
        "EC" | "ECU" => return country("EC", "Ecuador"),
        "EG" | "EGY" => return country("EG", "Egypt"),
        "SV" | "SLV" => return country("SV", "El Salvador"),
        "GQ" | "GNQ" => return country("GQ", "Equatorial Guinea"),
        "ER" | "ERI" => return country("ER", "Eritrea"),
        "EE" | "EST" => return country("EE", "Estonia"),
        "SZ" | "SWZ" => return country("SZ", "Eswatini"),
        "ET" | "ETH" => return country("ET", "Ethiopia"),
        "FK" | "FLK" => return country("FK", "Falkland Islands"),
        "FO" | "FRO" => return country("FO", "Faroe Islands"),
        "FJ" | "FJI" => return country("FJ", "Fiji"),
        "FI" | "FIN" => return country("FI", "Finland"),
        "FR" | "FRA" => return country("FR", "France"),
        "GF" | "GUF" => return country("GF", "French Guiana"),
        "PF" | "PYF" => return country("PF", "French Polynesia"),
        "TF" | "ATF" => return country("TF", "French Southern Territories"),
        "GA" | "GAB" => return country("GA", "Gabon"),
        "GM" | "GMB" => return country("GM", "Gambia"),
        "GE" | "GEO" => return country("GE", "Georgia"),
        "DE" | "DEU" => return country("DE", "Germany"),
        "GH" | "GHA" => return country("GH", "Ghana"),
        "GI" | "GIB" => return country("GI", "Gibraltar"),
        "GR" | "GRC" => return country("GR", "Greece"),
        "GL" | "GRL" => return country("GL", "Greenland"),
        "GD" | "GRD" => return country("GD", "Grenada"),
        "GP" | "GLP" => return country("GP", "Guadeloupe"),
        "GU" | "GUM" => return country("GU", "Guam"),
        "GT" | "GTM" => return country("GT", "Guatemala"),
        "GG" | "GGY" => return country("GG", "Guernsey"),
        "GN" | "GIN" => return country("GN", "Guinea"),
        "GW" | "GNB" => return country("GW", "Guinea-Bissau"),
        "GY" | "GUY" => return country("GY", "Guyana"),
        "HT" | "HTI" => return country("HT", "Haiti"),
        "HM" | "HMD" => return country("HM", "Heard Island and McDonald Islands"),
        "VA" | "VAT" => return country("VA", "Holy See"),
        "HN" | "HND" => return country("HN", "Honduras"),
        "HK" | "HKG" => return country("HK", "Hong Kong"),
        "HU" | "HUN" => return country("HU", "Hungary"),
        "IS" | "ISL" => return country("IS", "Iceland"),
        "IN" | "IND" => return country("IN", "India"),
        "ID" | "IDN" => return country("ID", "Indonesia"),
        "IR" | "IRN" => return country("IR", "Iran"),
        "IQ" | "IRQ" => return country("IQ", "Iraq"),
        "IE" | "IRL" => return country("IE", "Ireland"),
        "IM" | "IMN" => return country("IM", "Isle of Man"),
        "IL" | "ISR" => return country("IL", "Israel"),
        "IT" | "ITA" => return country("IT", "Italy"),
        "JM" | "JAM" => return country("JM", "Jamaica"),
        "JP" | "JPN" => return country("JP", "Japan"),
        "JE" | "JEY" => return country("JE", "Jersey"),
        "JO" | "JOR" => return country("JO", "Jordan"),
        "KZ" | "KAZ" => return country("KZ", "Kazakhstan"),
        "KE" | "KEN" => return country("KE", "Kenya"),
        "KI" | "KIR" => return country("KI", "Kiribati"),
        "KP" | "PRK" => return country("KP", "North Korea"),
        "KR" | "KOR" => return country("KR", "South Korea"),
        "KW" | "KWT" => return country("KW", "Kuwait"),
        "KG" | "KGZ" => return country("KG", "Kyrgyzstan"),
        "LA" | "LAO" => return country("LA", "Laos"),
        "LV" | "LVA" => return country("LV", "Latvia"),
        "LB" | "LBN" => return country("LB", "Lebanon"),
        "LS" | "LSO" => return country("LS", "Lesotho"),
        "LR" | "LBR" => return country("LR", "Liberia"),
        "LY" | "LBY" => return country("LY", "Libya"),
        "LI" | "LIE" => return country("LI", "Liechtenstein"),
        "LT" | "LTU" => return country("LT", "Lithuania"),
        "LU" | "LUX" => return country("LU", "Luxembourg"),
        "MO" | "MAC" => return country("MO", "Macao"),
        "MG" | "MDG" => return country("MG", "Madagascar"),
        "MW" | "MWI" => return country("MW", "Malawi"),
        "MY" | "MYS" => return country("MY", "Malaysia"),
        "MV" | "MDV" => return country("MV", "Maldives"),
        "ML" | "MLI" => return country("ML", "Mali"),
        "MT" | "MLT" => return country("MT", "Malta"),
        "MH" | "MHL" => return country("MH", "Marshall Islands"),
        "MQ" | "MTQ" => return country("MQ", "Martinique"),
        "MR" | "MRT" => return country("MR", "Mauritania"),
        "MU" | "MUS" => return country("MU", "Mauritius"),
        "YT" | "MYT" => return country("YT", "Mayotte"),
        "MX" | "MEX" => return country("MX", "Mexico"),
        "FM" | "FSM" => return country("FM", "Micronesia"),
        "MD" | "MDA" => return country("MD", "Moldova"),
        "MC" | "MCO" => return country("MC", "Monaco"),
        "MN" | "MNG" => return country("MN", "Mongolia"),
        "ME" | "MNE" => return country("ME", "Montenegro"),
        "MS" | "MSR" => return country("MS", "Montserrat"),
        "MA" | "MAR" => return country("MA", "Morocco"),
        "MZ" | "MOZ" => return country("MZ", "Mozambique"),
        "MM" | "MMR" => return country("MM", "Myanmar"),
        "NA" | "NAM" => return country("NA", "Namibia"),
        "NR" | "NRU" => return country("NR", "Nauru"),
        "NP" | "NPL" => return country("NP", "Nepal"),
        "NL" | "NLD" => return country("NL", "Netherlands, Kingdom of the"),
        "NC" | "NCL" => return country("NC", "New Caledonia"),
        "NZ" | "NZL" => return country("NZ", "New Zealand"),
        "NI" | "NIC" => return country("NI", "Nicaragua"),
        "NE" | "NER" => return country("NE", "Niger"),
        "NG" | "NGA" => return country("NG", "Nigeria"),
        "NU" | "NIU" => return country("NU", "Niue"),
        "NF" | "NFK" => return country("NF", "Norfolk Island"),
        "MK" | "MKD" => return country("MK", "North Macedonia"),
        "MP" | "MNP" => return country("MP", "Northern Mariana Islands"),
        "NO" | "NOR" => return country("NO", "Norway"),
        "OM" | "OMN" => return country("OM", "Oman"),
        "PK" | "PAK" => return country("PK", "Pakistan"),
        "PW" | "PLW" => return country("PW", "Palau"),
        "PS" | "PSE" => return country("PS", "Palestine"),
        "PA" | "PAN" => return country("PA", "Panama"),
        "PG" | "PNG" => return country("PG", "Papua New Guinea"),
        "PY" | "PRY" => return country("PY", "Paraguay"),
        "PE" | "PER" => return country("PE", "Peru"),
        "PH" | "PHL" => return country("PH", "Philippines"),
        "PN" | "PCN" => return country("PN", "Pitcairn"),
        "PL" | "POL" => return country("PL", "Poland"),
        "PT" | "PRT" => return country("PT", "Portugal"),
        "PR" | "PRI" => return country("PR", "Puerto Rico"),
        "QA" | "QAT" => return country("QA", "Qatar"),
        "RE" | "REU" => return country("RE", "Réunion"),
        "RO" | "ROU" => return country("RO", "Romania"),
        "RU" | "RUS" => return country("RU", "Russia"),
        "RW" | "RWA" => return country("RW", "Rwanda"),
        "BL" | "BLM" => return country("BL", "Saint Barthélemy"),
        "SH" | "SHN" => return country("SH", "Saint Helena"),
        "KN" | "KNA" => return country("KN", "Saint Kitts and Nevis"),
        "LC" | "LCA" => return country("LC", "Saint Lucia"),
        "MF" | "MAF" => return country("MF", "Saint Martin"),
        "PM" | "SPM" => return country("PM", "Saint Pierre and Miquelon"),
        "VC" | "VCT" => return country("VC", "Saint Vincent and the Grenadines"),
        "WS" | "WSM" => return country("WS", "Samoa"),
        "SM" | "SMR" => return country("SM", "San Marino"),
        "ST" | "STP" => return country("ST", "São Tomé and Príncipe"),
        "SA" | "SAU" => return country("SA", "Saudi Arabia"),
        "SN" | "SEN" => return country("SN", "Senegal"),
        "RS" | "SRB" => return country("RS", "Serbia"),
        "SC" | "SYC" => return country("SC", "Seychelles"),
        "SL" | "SLE" => return country("SL", "Sierra Leone"),
        "SG" | "SGP" => return country("SG", "Singapore"),
        "SX" | "SXM" => return country("SX", "Sint Maarten (Dutch part)"),
        "SK" | "SVK" => return country("SK", "Slovakia"),
        "SI" | "SVN" => return country("SI", "Slovenia"),
        "SB" | "SLB" => return country("SB", "Solomon Islands"),
        "SO" | "SOM" => return country("SO", "Somalia"),
        "ZA" | "ZAF" => return country("ZA", "South Africa"),
        "GS" | "SGS" => return country("GS", "South Georgia and the South Sandwich Islands"),
        "SS" | "SSD" => return country("SS", "South Sudan"),
        "ES" | "ESP" => return country("ES", "Spain"),
        "LK" | "LKA" => return country("LK", "Sri Lanka"),
        "SD" | "SDN" => return country("SD", "Sudan"),
        "SR" | "SUR" => return country("SR", "Suriname"),
        "SJ" | "SJM" => return country("SJ", "Svalbard and Jan Mayen"),
        "SE" | "SWE" => return country("SE", "Sweden"),
        "CH" | "CHE" => return country("CH", "Switzerland"),
        "SY" | "SYR" => return country("SY", "Syria"),
        "TW" | "TWN" => return country("TW", "Taiwan"),
        "TJ" | "TJK" => return country("TJ", "Tajikistan"),
        "TZ" | "TZA" => return country("TZ", "Tanzania"),
        "TH" | "THA" => return country("TH", "Thailand"),
        "TL" | "TLS" => return country("TL", "Timor-Leste"),
        "TG" | "TGO" => return country("TG", "Togo"),
        "TK" | "TKL" => return country("TK", "Tokelau"),
        "TO" | "TON" => return country("TO", "Tonga"),
        "TT" | "TTO" => return country("TT", "Trinidad and Tobago"),
        "TN" | "TUN" => return country("TN", "Tunisia"),
        "TR" | "TUR" => return country("TR", "Türkiye"),
        "TM" | "TKM" => return country("TM", "Turkmenistan"),
        "TC" | "TCA" => return country("TC", "Turks and Caicos Islands"),
        "TV" | "TUV" => return country("TV", "Tuvalu"),
        "UG" | "UGA" => return country("UG", "Uganda"),
        "UA" | "UKR" => return country("UA", "Ukraine"),
        "AE" | "ARE" => return country("AE", "United Arab Emirates"),
        "GB" | "GBR" | "UK" => return country("GB", "United Kingdom"),
        "US" | "USA" => return country("US", "United States"),
        "UM" | "UMI" => return country("UM", "United States Minor Outlying Islands"),
        "UY" | "URY" => return country("UY", "Uruguay"),
        "UZ" | "UZB" => return country("UZ", "Uzbekistan"),
        "VU" | "VUT" => return country("VU", "Vanuatu"),
        "VE" | "VEN" => return country("VE", "Venezuela"),
        "VN" | "VNM" => return country("VN", "Vietnam"),
        "VG" | "VGB" => return country("VG", "British Virgin Islands"),
        "VI" | "VIR" => return country("VI", "U.S. Virgin Islands"),
        "WF" | "WLF" => return country("WF", "Wallis and Futuna"),
        "EH" | "ESH" => return country("EH", "Western Sahara"),
        "YE" | "YEM" => return country("YE", "Yemen"),
        "ZM" | "ZMB" => return country("ZM", "Zambia"),
        "ZW" | "ZWE" => return country("ZW", "Zimbabwe"),

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
        "united kingdom" | "great britain" | "reino unido" | "gran bretana" | "england"
        | "scotland" | "wales" | "britain" => country("GB", "United Kingdom"),

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

        // Unresolved / Unknown country or code
        _ => {
            tracing::debug!(target: "country_resolution", "Unrecognized country input: '{}'", trimmed);
            CountryResolution::Unknown(trimmed.to_string())
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

/// Wire-format value for the `COUNTRY` / `RELEASECOUNTRY` tags.
///
/// directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
/// Sovereign countries emit their canonical English name ("US"/"USA"/"United States" ->
/// "United States"); regions keep their canonical region name; unrecognized input is
/// echoed back verbatim (never invented). This single helper backs BOTH the tag writers
/// and their verifiers (FLAC and MP4/M4A) so a file can never be written in one format
/// and audited against another.
pub fn wire_country_value(input: &str) -> String {
    match resolve_country(input) {
        CountryResolution::Country { canonical_name, .. } => canonical_name,
        CountryResolution::Region { region_name, .. } => region_name,
        CountryResolution::Unknown(_) => input.to_string(),
    }
}

/// Wire-format value for the `RELEASEREGION` companion tag when the input resolves to a
/// regional/supranational entity (region code preferred, e.g. "XE"/"XW", else its name).
/// Returns `None` for sovereign countries and unrecognized values.
pub fn wire_region_value(input: &str) -> Option<String> {
    match resolve_country(input) {
        CountryResolution::Region {
            region_code,
            region_name,
        } => Some(region_code.unwrap_or(region_name)),
        _ => None,
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
///
/// The plan is the exact inverse of what the tag writers put on disk: it targets
/// `wire_country_value` / `wire_region_value`, the SAME helpers `apply_flac_tags`
/// and the verifiers use. A repair therefore never produces a file the app would
/// flag as mismatched, and values the contract echoes verbatim (unrecognized
/// input) are left alone rather than deleted.
///
/// Typical cases:
/// - `RELEASECOUNTRY = "US"` (pre-2026-08-24 alpha-2 contract) → `"United States"`.
/// - `RELEASECOUNTRY = "Europe"` with no `RELEASEREGION` → adds `RELEASEREGION = "XE"`
///   (regions are written to BOTH tags by the writer, so neither one is dropped).
/// - `RELEASECOUNTRY = "United States"` → nothing to repair.
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

    let raw_country = match current_country {
        Some(c) => c,
        None => return plan,
    };
    let trimmed = raw_country.trim();

    if trimmed.is_empty() {
        // The writers never emit an empty RELEASECOUNTRY, so an empty one is drift.
        plan.target_country = None;
        plan.needs_repair = true;
        plan.reason = Some("Removed empty country value".to_string());
        return plan;
    }

    match resolve_country(trimmed) {
        CountryResolution::Country { canonical_name, .. } => {
            if trimmed != canonical_name {
                plan.target_country = Some(canonical_name.clone());
                plan.needs_repair = true;
                plan.reason = Some(format!(
                    "Normalized country '{}' to canonical name '{}'",
                    trimmed, canonical_name
                ));
            }
        }
        CountryResolution::Region {
            region_name,
            region_code,
        } => {
            // Regions are written to RELEASECOUNTRY *and* RELEASEREGION, so the
            // repair completes the pair instead of moving the value across tags.
            plan.target_country = Some(region_name.clone());
            let wire_region = region_code.unwrap_or_else(|| region_name.clone());
            let region_already_correct = current_region
                .map(|r| r.trim().eq_ignore_ascii_case(wire_region.trim()))
                .unwrap_or(false);
            if !region_already_correct {
                plan.target_region = Some(wire_region.clone());
                plan.needs_repair = true;
            }
            if trimmed != region_name {
                plan.needs_repair = true;
            }
            if plan.reason.is_none() {
                plan.reason = Some(format!(
                    "Normalized regional entity '{}' to '{}' + RELEASEREGION '{}'",
                    trimmed, region_name, wire_region
                ));
            }
        }
        CountryResolution::Unknown(_) => {
            // Contrato del propietario 2026-08-24: unrecognized input is echoed
            // back verbatim and never invented, so there is nothing to repair.
            plan.target_country = Some(raw_country.to_string());
        }
    }

    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iso_alpha2_exact_matches() {
        assert_eq!(resolve_country("ES").country_code(), Some("ES"));
        assert_eq!(resolve_country("es").country_code(), Some("ES"));
        assert_eq!(resolve_country("GB").country_code(), Some("GB"));
        assert_eq!(resolve_country("US").country_code(), Some("US"));
        assert_eq!(resolve_country("FR").country_code(), Some("FR"));
        assert_eq!(resolve_country("DE").country_code(), Some("DE"));
        assert_eq!(resolve_country("MX").country_code(), Some("MX"));
        assert_eq!(resolve_country("NL").country_code(), Some("NL"));
        assert_eq!(resolve_country("PL").country_code(), Some("PL"));
        assert_eq!(resolve_country("AT").country_code(), Some("AT"));
        assert_eq!(resolve_country("AF").country_code(), Some("AF"));
    }

    #[test]
    fn test_iso_alpha3_matches() {
        assert_eq!(resolve_country("ESP").country_code(), Some("ES"));
        assert_eq!(resolve_country("esp").country_code(), Some("ES"));
        assert_eq!(resolve_country("GBR").country_code(), Some("GB"));
        assert_eq!(resolve_country("USA").country_code(), Some("US"));
        assert_eq!(resolve_country("DEU").country_code(), Some("DE"));
        assert_eq!(resolve_country("FRA").country_code(), Some("FR"));
        assert_eq!(resolve_country("JPN").country_code(), Some("JP"));
        assert_eq!(resolve_country("MEX").country_code(), Some("MX"));
        assert_eq!(resolve_country("NLD").country_code(), Some("NL"));
        assert_eq!(resolve_country("POL").country_code(), Some("PL"));
        assert_eq!(resolve_country("AUT").country_code(), Some("AT"));
        assert_eq!(resolve_country("AFG").country_code(), Some("AF"));
    }

    #[test]
    fn test_localized_names_english_and_spanish() {
        assert_eq!(resolve_country("Spain").country_code(), Some("ES"));
        assert_eq!(resolve_country("España").country_code(), Some("ES"));
        assert_eq!(resolve_country("Espana").country_code(), Some("ES"));

        assert_eq!(
            resolve_country("United States").country_code(),
            Some("US")
        );
        assert_eq!(
            resolve_country("Estados Unidos").country_code(),
            Some("US")
        );
        assert_eq!(resolve_country("EE.UU.").country_code(), Some("US"));
        assert_eq!(resolve_country("EEUU").country_code(), Some("US"));

        assert_eq!(
            resolve_country("United Kingdom").country_code(),
            Some("GB")
        );
        assert_eq!(resolve_country("Reino Unido").country_code(), Some("GB"));
        assert_eq!(
            resolve_country("Great Britain").country_code(),
            Some("GB")
        );
        assert_eq!(
            resolve_country("Gran Bretaña").country_code(),
            Some("GB")
        );
        assert_eq!(resolve_country("UK").country_code(), Some("GB"));
        assert_eq!(resolve_country("uk").country_code(), Some("GB"));

        assert_eq!(resolve_country("Germany").country_code(), Some("DE"));
        assert_eq!(resolve_country("Alemania").country_code(), Some("DE"));
        assert_eq!(resolve_country("Deutschland").country_code(), Some("DE"));

        assert_eq!(resolve_country("France").country_code(), Some("FR"));
        assert_eq!(resolve_country("Francia").country_code(), Some("FR"));

        assert_eq!(resolve_country("Japan").country_code(), Some("JP"));
        assert_eq!(resolve_country("Japón").country_code(), Some("JP"));
        assert_eq!(resolve_country("Japon").country_code(), Some("JP"));

        assert_eq!(resolve_country("Canada").country_code(), Some("CA"));
        assert_eq!(resolve_country("Canadá").country_code(), Some("CA"));

        assert_eq!(resolve_country("Mexico").country_code(), Some("MX"));
        assert_eq!(resolve_country("México").country_code(), Some("MX"));

        assert_eq!(resolve_country("Netherlands").country_code(), Some("NL"));
        assert_eq!(
            resolve_country("Países Bajos").country_code(),
            Some("NL")
        );
        assert_eq!(resolve_country("Holanda").country_code(), Some("NL"));

        assert_eq!(resolve_country("Poland").country_code(), Some("PL"));
        assert_eq!(resolve_country("Polonia").country_code(), Some("PL"));

        assert_eq!(resolve_country("Austria").country_code(), Some("AT"));
        assert_eq!(resolve_country("Afghanistan").country_code(), Some("AF"));
        assert_eq!(resolve_country("Afganistán").country_code(), Some("AF"));
    }

    #[test]
    fn test_regional_and_supranational_entities_not_converted_to_country() {
        assert_eq!(resolve_country("Europe").country_code(), None);
        assert_eq!(resolve_country("XE").country_code(), None);
        assert_eq!(resolve_country("Worldwide").country_code(), None);
        assert_eq!(resolve_country("XW").country_code(), None);
        assert_eq!(resolve_country("[Worldwide]").country_code(), None);

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

        assert_eq!(resolve_country("Europe").region_name(), Some("Europe"));
        assert_eq!(resolve_country("XE").region_name(), Some("Europe"));
        assert_eq!(resolve_country("XE").region_code(), Some("XE"));
        assert_eq!(
            resolve_country("Worldwide").region_name(),
            Some("Worldwide")
        );
        assert_eq!(resolve_country("XW").region_code(), Some("XW"));

        // Country input resolves to a code and no region; region input the reverse.
        assert_eq!(resolve_country("XE").country_code(), None);
        assert_eq!(resolve_country("Spain").country_code(), Some("ES"));
        assert_eq!(resolve_country("Spain").region_code(), None);
    }

    #[test]
    fn test_unknown_inputs_not_invented() {
        assert_eq!(resolve_country("").country_code(), None);
        assert_eq!(resolve_country("   ").country_code(), None);
        assert_eq!(resolve_country("UnknownCountry123").country_code(), None);
        assert_eq!(
            resolve_country("UnknownCountry123"),
            CountryResolution::Unknown("UnknownCountry123".to_string())
        );
        assert_eq!(resolve_country("UnknownCountry123").region_name(), None);
    }

    #[test]
    fn test_wire_country_value_emits_canonical_names() {
        // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
        assert_eq!(wire_country_value("US"), "United States");
        assert_eq!(wire_country_value("USA"), "United States");
        assert_eq!(wire_country_value("us"), "United States");
        assert_eq!(wire_country_value("GB"), "United Kingdom");
        assert_eq!(wire_country_value("UK"), "United Kingdom");
        assert_eq!(wire_country_value("DE"), "Germany");
        assert_eq!(wire_country_value("DEU"), "Germany");
        assert_eq!(wire_country_value("AL"), "Albania");
        assert_eq!(wire_country_value("MX"), "Mexico");
        assert_eq!(wire_country_value("Germany"), "Germany");
        assert_eq!(wire_country_value("Alemania"), "Germany");

        // Canonical name input must come out identical (idempotent on the wire)
        assert_eq!(wire_country_value("United States"), "United States");
        assert_eq!(wire_country_value("United Kingdom"), "United Kingdom");
        assert_eq!(wire_country_value(" United States "), "United States");
    }

    #[test]
    fn test_wire_country_value_regions_and_unknown_verbatim() {
        // Regions keep their canonical name on COUNTRY/RELEASECOUNTRY...
        assert_eq!(wire_country_value("Europe"), "Europe");
        assert_eq!(wire_country_value("Worldwide"), "Worldwide");
        assert_eq!(wire_country_value("[Worldwide]"), "Worldwide");
        assert_eq!(wire_country_value("XE"), "Europe");
        assert_eq!(wire_country_value("XW"), "Worldwide");
        // ...unrecoverable input stays verbatim (never invented)
        assert_eq!(wire_country_value("UnknownCountry123"), "UnknownCountry123");
        assert_eq!(wire_country_value(""), "");

        // RELEASEREGION keeps code-preferred semantics for regions only
        assert_eq!(wire_region_value("XE"), Some("XE".to_string()));
        assert_eq!(wire_region_value("Europe"), Some("XE".to_string()));
        assert_eq!(wire_region_value("[Worldwide]"), Some("XW".to_string()));
        assert_eq!(wire_region_value("US"), None);
        assert_eq!(wire_region_value("United States"), None);
        assert_eq!(wire_region_value("UnknownCountry123"), None);
    }

    #[test]
    fn test_tag_repair_plan_targets_the_wire_contract() {
        // The repair must land exactly where the tag writers land: every target
        // equals wire_country_value / wire_region_value of the original value.
        for raw in [
            "US",
            "USA",
            "us",
            "GB",
            "DE",
            "Deutschland",
            "Spain",
            "México",
            "XE",
            "Europe",
            "XW",
            "Worldwide",
            "United States",
            "UnknownCountry123",
        ] {
            let plan = plan_country_repair(Some(raw), None);
            assert_eq!(
                plan.target_country.as_deref(),
                Some(wire_country_value(raw).as_str()),
                "target country for {:?} must match the wire value",
                raw
            );
        }
    }

    #[test]
    fn test_tag_repair_plan_upgrades_legacy_alpha2_country() {
        // Pre-2026-08-24 files carry ISO alpha-2; the wire contract is the name.
        let plan_us = plan_country_repair(Some("US"), None);
        assert!(plan_us.needs_repair);
        assert_eq!(plan_us.original_country.as_deref(), Some("US"));
        assert_eq!(plan_us.target_country.as_deref(), Some("United States"));
        assert!(plan_us.reason.is_some());

        let plan_spain = plan_country_repair(Some("ES"), None);
        assert!(plan_spain.needs_repair);
        assert_eq!(plan_spain.target_country.as_deref(), Some("Spain"));

        // Already canonical on the wire -> nothing to do.
        let plan_canonical = plan_country_repair(Some("United States"), None);
        assert!(!plan_canonical.needs_repair);
        assert_eq!(
            plan_canonical.target_country.as_deref(),
            Some("United States")
        );
        assert!(plan_canonical.reason.is_none());
    }

    #[test]
    fn test_tag_repair_plan_completes_the_region_pair() {
        // A region in the country slot lives in BOTH tags on the wire, so the
        // repair adds RELEASEREGION instead of moving the value across tags.
        let plan_europe = plan_country_repair(Some("Europe"), None);
        assert!(plan_europe.needs_repair);
        assert_eq!(plan_europe.target_country.as_deref(), Some("Europe"));
        assert_eq!(plan_europe.target_region.as_deref(), Some("XE"));

        let plan_xw = plan_country_repair(Some("XW"), None);
        assert_eq!(plan_xw.target_country.as_deref(), Some("Worldwide"));
        assert_eq!(plan_xw.target_region.as_deref(), Some("XW"));

        // Already complete pair -> no repair.
        let plan_complete = plan_country_repair(Some("Europe"), Some("XE"));
        assert!(!plan_complete.needs_repair);
        assert_eq!(plan_complete.target_country.as_deref(), Some("Europe"));
        assert_eq!(plan_complete.target_region.as_deref(), Some("XE"));

        // An existing RELEASEREGION that differs is still repaired.
        let plan_wrong_region = plan_country_repair(Some("Europe"), Some("Worldwide"));
        assert!(plan_wrong_region.needs_repair);
        assert_eq!(plan_wrong_region.target_region.as_deref(), Some("XE"));

        // A sovereign country never carries RELEASEREGION from this repair.
        let plan_country = plan_country_repair(Some("US"), Some("XE"));
        assert!(plan_country.needs_repair);
        assert_eq!(
            plan_country.target_country.as_deref(),
            Some("United States")
        );
        assert_eq!(plan_country.target_region.as_deref(), Some("XE"));
    }

    #[test]
    fn test_tag_repair_plan_leaves_unrecognized_and_empty_input_alone() {
        // The contract echoes unrecognized input verbatim: deleting it would make
        // the next write put the value back.
        let plan_unknown = plan_country_repair(Some("NonExistentCountry99"), None);
        assert!(!plan_unknown.needs_repair);
        assert_eq!(
            plan_unknown.target_country.as_deref(),
            Some("NonExistentCountry99")
        );

        // An empty country tag carries no value at all -> removed.
        let plan_empty = plan_country_repair(Some("   "), None);
        assert!(plan_empty.needs_repair);
        assert_eq!(plan_empty.target_country, None);

        // No country tag -> untouched plan.
        let plan_absent = plan_country_repair(None, Some("XE"));
        assert!(!plan_absent.needs_repair);
        assert_eq!(plan_absent.target_country, None);
        assert_eq!(plan_absent.target_region.as_deref(), Some("XE"));
    }
}

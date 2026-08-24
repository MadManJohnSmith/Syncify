//! Genre validation, sanitation, junk rejection, and multi-value fusion.

/// Context containing track attributes used to reject genres matching entity names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenreContext<'a> {
    pub title: Option<&'a str>,
    pub artist: Option<&'a str>,
    pub album: Option<&'a str>,
    pub label: Option<&'a str>,
}

impl<'a> GenreContext<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_title(mut self, title: Option<&'a str>) -> Self {
        self.title = title;
        self
    }

    pub fn with_artist(mut self, artist: Option<&'a str>) -> Self {
        self.artist = artist;
        self
    }

    pub fn with_album(mut self, album: Option<&'a str>) -> Self {
        self.album = album;
        self
    }

    pub fn with_label(mut self, label: Option<&'a str>) -> Self {
        self.label = label;
        self
    }
}

/// Substrings that immediately disqualify a string from being a valid musical genre.
const JUNK_SUBSTRINGS: &[&str] = &[
    "feat.",
    "remaster",
    "version",
    "live",
    "deluxe",
    "edition",
];

/// Blacklist of exact mood descriptors that are not musical genres.
const MOOD_BLACKLIST: &[&str] = &[
    "emotional",
    "energetic",
    "extremely bored",
    "fun",
    "groovy",
    "happy",
    "haunting",
    "hedonistic",
    "melancholy",
    "mellow",
    "raw",
    "rebellious",
    "reflective",
    "relaxed",
    "romantic",
    "self-hatred",
    "smooth",
    "soothing",
    "sweet",
    "upbeat",
];

/// Blacklist of track/release metadata artifacts and user tags.
const METADATA_BLACKLIST: &[&str] = &[
    "hidden track",
    "interview",
    "meme",
    "misc",
    "non-music",
    "part ii",
    "recordings with subtle differences",
    "remark",
    "sillyname",
    "sitarsploitation",
    "test",
    "title track",
    "varios",
    "well-known",
];

/// Blacklist of usage contexts, playlist types, charts, and eras.
const CONTEXT_BLACKLIST: &[&str] = &[
    "exercise",
    "kuschelrock",
    "late 60's early 70's",
    "offizielle charts",
    "oldies",
    "party",
    "series de televisión",
    "series de television",
    "top 40",
    "video game",
];

/// Blacklist of isolated languages or continents (not composite styles).
const ISOLATED_TERMS_BLACKLIST: &[&str] = &[
    "english",
    "spanish",
    "áfrica",
    "africa",
];

/// Checks whether a genre string is a corrupt scraper concatenation (e.g. `Dance_electronic`, `Indieindie`, `Rerip Grunge`).
pub fn is_corrupt_concatenation(lower: &str) -> bool {
    // 1. Contains underscore
    if lower.contains('_') {
        return true;
    }
    // 2. Starts with "rerip "
    if lower.starts_with("rerip ") {
        return true;
    }
    // 3. Immediate word duplication like "indieindie"
    if lower.len() >= 6 && lower.len() % 2 == 0 {
        let half = lower.len() / 2;
        if lower[..half] == lower[half..] {
            return true;
        }
    }
    false
}

/// Validates whether a candidate string is a genuine genre.
pub fn is_valid_genre(val: &str) -> bool {
    is_valid_genre_with_context(val, None)
}

/// Validates whether a candidate string is a genuine genre within the context of a track.
///
/// Rules:
/// - Rejects empty strings, placeholders ("unknown", "n/a", "null", "none", "???", "-").
/// - Rejects corrupt scraper concatenations (contains `_`, duplicate word `indieindie`, `rerip ` prefix).
/// - Rejects mood descriptors (exact match, case-insensitive).
/// - Rejects track metadata artifacts (exact match, case-insensitive).
/// - Rejects usage contexts and charts (exact match, case-insensitive).
/// - Rejects isolated language/continent terms (exact match, case-insensitive).
/// - Rejects values containing junk substrings ("feat.", "remaster", "version", "live", "deluxe", "edition").
/// - Rejects values that match the track's title, artist, album, or record label (case-insensitive).
pub fn is_valid_genre_with_context(val: &str, context: Option<&GenreContext>) -> bool {
    let trimmed = val.trim();
    if trimmed.is_empty()
        || trimmed.chars().count() < 2
        || trimmed.eq_ignore_ascii_case("unknown")
        || trimmed.eq_ignore_ascii_case("n/a")
        || trimmed.eq_ignore_ascii_case("null")
        || trimmed.eq_ignore_ascii_case("none")
        || trimmed == "???"
        || trimmed == "--"
        || trimmed == "-"
    {
        return false;
    }

    let lower = trimmed.to_lowercase();

    // Check corrupt scraper concatenations
    if is_corrupt_concatenation(&lower) {
        tracing::debug!(target: "genre_validation", "Rejected corrupt concatenation genre: '{}'", val);
        return false;
    }

    // Check exact mood blacklist
    if MOOD_BLACKLIST.contains(&lower.as_str()) {
        tracing::debug!(target: "genre_validation", "Rejected mood descriptor as genre: '{}'", val);
        return false;
    }

    // Check exact metadata artifact blacklist
    if METADATA_BLACKLIST.contains(&lower.as_str()) {
        tracing::debug!(target: "genre_validation", "Rejected metadata artifact as genre: '{}'", val);
        return false;
    }

    // Check exact context / chart blacklist
    if CONTEXT_BLACKLIST.contains(&lower.as_str()) {
        tracing::debug!(target: "genre_validation", "Rejected context/chart term as genre: '{}'", val);
        return false;
    }

    // Check isolated language / continent blacklist
    if ISOLATED_TERMS_BLACKLIST.contains(&lower.as_str()) {
        tracing::debug!(target: "genre_validation", "Rejected isolated term as genre: '{}'", val);
        return false;
    }

    // Check junk substrings
    for &junk in JUNK_SUBSTRINGS {
        if lower.contains(junk) {
            tracing::debug!(target: "genre_validation", "Rejected genre containing junk substring '{}': '{}'", junk, val);
            return false;
        }
    }

    // Check context matches (title, artist, album, label)
    if let Some(ctx) = context {
        if let Some(t) = ctx.title {
            let t_clean = t.trim();
            if !t_clean.is_empty() && lower == t_clean.to_lowercase() {
                return false;
            }
        }
        if let Some(a) = ctx.artist {
            let a_clean = a.trim();
            if !a_clean.is_empty() && lower == a_clean.to_lowercase() {
                return false;
            }
        }
        if let Some(alb) = ctx.album {
            let alb_clean = alb.trim();
            if !alb_clean.is_empty() && lower == alb_clean.to_lowercase() {
                return false;
            }
        }
        if let Some(lbl) = ctx.label {
            let lbl_clean = lbl.trim();
            if !lbl_clean.is_empty() && lower == lbl_clean.to_lowercase() {
                return false;
            }
        }
    }

    true
}

/// Normalizes capitalization of a genre token while preserving Title Case,
/// multi-lingual characters, hyphenated subgenres, and standard acronyms.
pub fn normalize_genre_token(token: &str) -> String {
    let t = token.trim();
    if t.is_empty() {
        return String::new();
    }

    let lower = t.to_lowercase();
    match lower.as_str() {
        "r&b" => return "R&B".to_string(),
        "edm" => return "EDM".to_string(),
        "idm" => return "IDM".to_string(),
        "ost" => return "OST".to_string(),
        "dj" => return "DJ".to_string(),
        "bpm" => return "BPM".to_string(),
        "uk garage" => return "UK Garage".to_string(),
        "k-pop" => return "K-Pop".to_string(),
        "j-pop" => return "J-Pop".to_string(),
        _ => {}
    }

    // If all-uppercase (length > 2) or all-lowercase, normalize each word / hyphenated part
    let is_all_upper = t.len() > 2 && t.chars().all(|c| !c.is_alphabetic() || c.is_uppercase());
    let is_all_lower = t.chars().all(|c| !c.is_alphabetic() || c.is_lowercase());

    if is_all_upper || is_all_lower {
        let words: Vec<String> = t
            .split_whitespace()
            .map(|word| {
                if word.contains('-') {
                    let subparts: Vec<String> = word
                        .split('-')
                        .map(|sub| {
                            let s = sub.to_lowercase();
                            let mut chars = s.chars();
                            match chars.next() {
                                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                                None => String::new(),
                            }
                        })
                        .collect();
                    subparts.join("-")
                } else {
                    let s = word.to_lowercase();
                    let mut chars = s.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                        None => String::new(),
                    }
                }
            })
            .collect();
        words.join(" ")
    } else {
        // Mixed casing present (e.g. "Variété française", "Música Latina", "Synth-pop")
        // Ensure first character is capitalized if alphabetic
        let mut chars = t.chars();
        match chars.next() {
            Some(first) if first.is_lowercase() => {
                first.to_uppercase().collect::<String>() + chars.as_str()
            }
            _ => t.to_string(),
        }
    }
}

/// Fuses multiple genre inputs across providers with context-aware junk rejection,
/// splitting across delimiters (';' and '/'), collapsing whitespace and duplicate separators,
/// case-insensitive deduplication, and capitalization normalization.
pub fn fuse_genres_with_context(
    genre_inputs: &[&str],
    context: Option<&GenreContext>,
) -> Vec<String> {
    let mut unique_genres: Vec<String> = Vec::new();

    for input in genre_inputs {
        let trimmed_input = input.trim();
        if trimmed_input.is_empty() {
            continue;
        }

        // Split on both ';' and '/'
        let tokens = trimmed_input.split(|c| c == ';' || c == '/');
        for raw in tokens {
            let t = raw.trim();
            if is_valid_genre_with_context(t, context) {
                // Check if already present (Unicode case-insensitive)
                let t_lower = t.to_lowercase();
                if !unique_genres.iter().any(|g| g.to_lowercase() == t_lower) {
                    let cleaned = normalize_genre_token(t);
                    if !cleaned.is_empty() {
                        // Also check normalized form against collected genres
                        let cleaned_lower = cleaned.to_lowercase();
                        if !unique_genres.iter().any(|g| g.to_lowercase() == cleaned_lower) {
                            unique_genres.push(cleaned);
                        }
                    }
                }
            }
        }
    }

    unique_genres
}

/// Backwards-compatible `fuse_genres` without context.
pub fn fuse_genres(genre_inputs: &[&str]) -> Vec<String> {
    fuse_genres_with_context(genre_inputs, None)
}

/// Formats fused genres as a standard semicolon-separated string (e.g. `"Rock; Pop; Disco"`).
/// Returns `None` if all inputs are invalid, empty, or junk.
pub fn format_fused_genres_with_context(
    genre_inputs: &[&str],
    context: Option<&GenreContext>,
) -> Option<String> {
    let fused = fuse_genres_with_context(genre_inputs, context);
    if fused.is_empty() {
        None
    } else {
        Some(fused.join("; "))
    }
}

/// Backwards-compatible `format_fused_genres` without context.
pub fn format_fused_genres(genre_inputs: &[&str]) -> Option<String> {
    format_fused_genres_with_context(genre_inputs, None)
}

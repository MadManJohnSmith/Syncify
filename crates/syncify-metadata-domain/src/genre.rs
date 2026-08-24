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

/// Canonical variant matrix derived from the owner's physical audit of 531 genre terms.
///
/// Owner canonical rule: between variants of the same genre family, the spelling with the
/// highest frequency in the audit wins; on a tie the hyphen-free form wins.
///
/// Row-by-row decision table (audit frequencies in parentheses):
///   R B(5) / R&B / Rnb(4)                          -> "R&B"            (5 beats 4; '&' label is the registered winner)
///   Early R&B / Rhythm And Blues(3) / Rhythm & Blues(1) -> INTACT      (distinct R&B facets; only PURE R&B spellings fuse)
///   Soul And R B(1) / Soul And R&b(2)              -> "Soul And R&B"   (2 beats 1)
///   Adult Contemporary R&B                         -> INTACT           (compound, single variant)
///   Rock And Roll(9) / Rock & Roll(8) / Rock Roll(2) -> "Rock And Roll" (9 beats 8 beats 2)
///   Hip-Hop(20) / Hip Hop(21)                      -> "Hip Hop"        (21 beats 20; Orchestrator S184 arbitration)
///   Synth-Pop(20) / Synthpop(8) / Synth Pop(2)     -> "Synth-Pop"      (20 beats 8 beats 2)
///   Dance Pop(14) / Dance-Pop(6)                   -> "Dance Pop"      (14 beats 6)
///   Alternative-Pop                                -> "Alternative Pop" (owner rule: hyphen-free preferred for single variants)
///   Electro-Pop                                    -> "Electropop"     (owner audit: portmanteau is the attested winner)
///   Doo-Wop(2) / Doo Wop(3)                        -> "Doo Wop"        (3 beats 2)
///   Nu-Metal(2) / Nu Metal(5)                      -> "Nu Metal"       (5 beats 2)
///   Folk-Rock(2) / Folk Rock(12)                   -> "Folk Rock"      (12 beats 2)
///   Blues-Rock(1) / Blues Rock(11)                 -> "Blues Rock"     (11 beats 1)
///   Country-Rock(1) / Country Rock(6)              -> "Country Rock"   (6 beats 1)
///   Jazz-Rock(2) = Jazz Rock(2)                    -> "Jazz Rock"      (TIE -> hyphen-free)
///   Rap-Metal(3) / Rap Metal(2)                    -> "Rap-Metal"      (3 beats 2; hyphenated winner KEPT)
///   Rap-Rock(2) = Rap Rock(2)                      -> "Rap Rock"       (TIE -> hyphen-free)
///   Trip-Hop(1) / Trip Hop(10)                     -> "Trip Hop"       (10 beats 1)
///   Post-Bop = Post Bop                            -> "Post Bop"       (TIE -> hyphen-free)
///   Punk-Pop(2) / Pop-Punk(1) / Pop Punk(5)        -> "Pop Punk"       (family winner 5 beats 2 beats 1)
///   Emo-Pop                                        -> INTACT           (single variant, no fusion)
///   Dark Wave(3) / Darkwave(2)                     -> "Dark Wave"      (3 beats 2)
///   Neo Glam = Neo-Glam                            -> "Neo Glam"       (TIE -> hyphen-free)
///   Hairmetal(1) / Hair Metal(2)                   -> "Hair Metal"     (2 beats 1)
///   Psychadelic(2) / Psychedelic                   -> "Psychedelic"    (typo correction, never emit "Psychadelic")
///   2 Tone(1) = Two Tone(1)                        -> "2 Tone"         (TIE -> numeric form per audit label)
///   World-Fusion                                   -> INTACT           (single variant, no fusion)
///   Jazz Vocal / Jazz Vocals / Vocal Jazz          -> INTACT           (NO fusion: distinct facets per source, documented)
///
/// ARBITRATED ROW (Orchestrator S184): Hip-Hop(20) -> "Hip Hop"(21) initially conflicted
/// with the protected pin `genre_case_dedupe_test::... == ["Hip-Hop"]` and was suspended
/// per the sprint protocol. The Orchestrator resolved the conflict by hierarchical rule:
/// the 5 protected suites guard the owner's ANTI-JUNK semantics (what counts as junk),
/// they do not freeze historical casing/hyphen; the owner's S184 directive explicitly
/// fuses variants of the same genre and its own audit gives Hip Hop(21) > Hip-Hop(20).
/// The row is ACTIVE and the protected expectation was updated with citation.
///
/// Matching is EXACT over a normalized key (lowercase, diacritic-free, '&'/'-'/'_' folded
/// to spaces, standalone 'and' dropped, whitespace collapsed) — NEVER substring — so
/// compounds like "Party Rap", "Oldies Rock", "English Folk" or "Spanish Pop" can never
/// collide with a row key.
const CANONICAL_GENRE_VARIANTS: &[(&[&str], &str)] = &[
    (&["R B", "R&B", "Rnb"], "R&B"),
    (&["Soul And R B", "Soul And R&b"], "Soul And R&B"),
    (&["Rock And Roll", "Rock & Roll", "Rock Roll"], "Rock And Roll"),
    (&["Hip-Hop", "Hip Hop"], "Hip Hop"),
    (&["Synth-Pop", "Synthpop", "Synth Pop"], "Synth-Pop"),
    (&["Dance Pop", "Dance-Pop"], "Dance Pop"),
    (&["Alternative-Pop", "Alternative Pop"], "Alternative Pop"),
    (&["Electro-Pop", "Electropop"], "Electropop"),
    (&["Doo-Wop", "Doo Wop"], "Doo Wop"),
    (&["Nu-Metal", "Nu Metal"], "Nu Metal"),
    (&["Folk-Rock", "Folk Rock"], "Folk Rock"),
    (&["Blues-Rock", "Blues Rock"], "Blues Rock"),
    (&["Country-Rock", "Country Rock"], "Country Rock"),
    (&["Jazz-Rock", "Jazz Rock"], "Jazz Rock"),
    (&["Rap-Metal", "Rap Metal"], "Rap-Metal"),
    (&["Rap-Rock", "Rap Rock"], "Rap Rock"),
    (&["Trip-Hop", "Trip Hop"], "Trip Hop"),
    (&["Post-Bop", "Post Bop"], "Post Bop"),
    (&["Punk-Pop", "Pop-Punk", "Pop Punk"], "Pop Punk"),
    (&["Dark Wave", "Darkwave"], "Dark Wave"),
    (&["Neo Glam", "Neo-Glam"], "Neo Glam"),
    (&["Hairmetal", "Hair Metal"], "Hair Metal"),
    (&["Psychadelic", "Psychedelic"], "Psychedelic"),
    (&["2 Tone", "Two Tone"], "2 Tone"),
];

/// Normalized exact-match key for a genre variant: lowercase, diacritic-free,
/// '&'-/'-'/_'-folded to spaces, standalone 'and' dropped, whitespace collapsed.
/// Used ONLY for table lookup — output values always come from CANONICAL_GENRE_VARIANTS.
fn genre_variant_key(term: &str) -> String {
    let lower = term.trim().to_lowercase();
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
            '&' | '-' | '_' => normalized.push(' '),
            _ => normalized.push(c),
        }
    }
    normalized
        .split_whitespace()
        .filter(|word| *word != "and")
        .collect::<Vec<_>>()
        .join(" ")
}

/// Canonicalizes a genre term against [`CANONICAL_GENRE_VARIANTS`] (exact key match).
///
/// Applied inside the fusion pipeline AFTER junk validation and BEFORE case-insensitive
/// dedupe. Terms with no matrix hit pass through unchanged (trimmed) — the function never
/// invents genres and never matches by substring.
pub fn canonicalize_genre(term: &str) -> String {
    let trimmed = term.trim();
    let key = genre_variant_key(trimmed);
    if key.is_empty() {
        return trimmed.to_string();
    }
    for (variants, canonical) in CANONICAL_GENRE_VARIANTS {
        for variant in *variants {
            if genre_variant_key(variant) == key {
                return (*canonical).to_string();
            }
        }
    }
    trimmed.to_string()
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
    fuse_genres_with_context_and_delimiters(genre_inputs, context, true)
}

/// Same fusion pipeline as [`fuse_genres_with_context`] but splitting ONLY on ';'.
///
/// Used for secondary descriptor fields (STYLE / MOOD / TAGS) where a '/' is part of a
/// composite descriptor (e.g. "Glam Rock / Berlin Trilogy") and must be preserved verbatim,
/// while GENRE follows the S174 contract of splitting both ';' and '/' into discrete blocks.
pub fn fuse_genres_semicolon_only_with_context(
    genre_inputs: &[&str],
    context: Option<&GenreContext>,
) -> Vec<String> {
    fuse_genres_with_context_and_delimiters(genre_inputs, context, false)
}

pub fn fuse_genres_with_context_and_delimiters(
    genre_inputs: &[&str],
    context: Option<&GenreContext>,
    split_slash: bool,
) -> Vec<String> {
    let mut unique_genres: Vec<String> = Vec::new();

    for input in genre_inputs {
        let trimmed_input = input.trim();
        if trimmed_input.is_empty() {
            continue;
        }

        // Split on ';' (and on '/' only when the caller requests multi-genre semantics)
        let tokens: Vec<&str> = if split_slash {
            trimmed_input.split(|c| c == ';' || c == '/').collect()
        } else {
            trimmed_input.split(';').collect()
        };
        for raw in tokens {
            let t = raw.trim();
            if is_valid_genre_with_context(t, context) {
                // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
                // Canonical variant matrix applied AFTER validation and BEFORE dedupe:
                // "r&b" and "R B" collapse into the audited winner "R&B", etc. Exact key
                // matching only; unmatched terms pass through untouched.
                let canonical = canonicalize_genre(t);
                let t_lower = canonical.to_lowercase();
                if !unique_genres.iter().any(|g| g.to_lowercase() == t_lower) {
                    let cleaned = normalize_genre_token(&canonical);
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

/// Backwards-compatible semicolon-only `fuse_genres` without context.
pub fn fuse_genres_semicolon_only(genre_inputs: &[&str]) -> Vec<String> {
    fuse_genres_semicolon_only_with_context(genre_inputs, None)
}

/// Splits secondary facet values (STYLE / MOOD / TAGS) on ';' only, preserving
/// slash-joined composite descriptors ("Glam Rock / Berlin Trilogy") as single values.
/// Applies the same validation, dedup and capitalization pipeline as [`fuse_genres`].
pub fn split_facet_values(genre_inputs: &[&str]) -> Vec<String> {
    fuse_genres_semicolon_only_with_context(genre_inputs, None)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonicalize_genre_matrix_rows() {
        // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
        assert_eq!(canonicalize_genre("R B"), "R&B");
        assert_eq!(canonicalize_genre("r&b"), "R&B");
        assert_eq!(canonicalize_genre("Rnb"), "R&B");
        assert_eq!(canonicalize_genre("Soul And R B"), "Soul And R&B");
        assert_eq!(canonicalize_genre("Soul And R&b"), "Soul And R&B");
        assert_eq!(canonicalize_genre("Rock & Roll"), "Rock And Roll");
        assert_eq!(canonicalize_genre("Rock Roll"), "Rock And Roll");
        assert_eq!(canonicalize_genre("Rock And Roll"), "Rock And Roll");
        assert_eq!(canonicalize_genre("Synthpop"), "Synth-Pop");
        assert_eq!(canonicalize_genre("Synth Pop"), "Synth-Pop");
        assert_eq!(canonicalize_genre("synth-pop"), "Synth-Pop");
        assert_eq!(canonicalize_genre("Dance-Pop"), "Dance Pop");
        assert_eq!(canonicalize_genre("Alternative-Pop"), "Alternative Pop");
        assert_eq!(canonicalize_genre("Electro-Pop"), "Electropop");
        assert_eq!(canonicalize_genre("Electropop"), "Electropop");
        assert_eq!(canonicalize_genre("Doo-Wop"), "Doo Wop");
        assert_eq!(canonicalize_genre("Nu-Metal"), "Nu Metal");
        assert_eq!(canonicalize_genre("Folk-Rock"), "Folk Rock");
        assert_eq!(canonicalize_genre("Blues-Rock"), "Blues Rock");
        assert_eq!(canonicalize_genre("Country-Rock"), "Country Rock");
        assert_eq!(canonicalize_genre("Jazz-Rock"), "Jazz Rock"); // TIE -> hyphen-free
        assert_eq!(canonicalize_genre("Rap Metal"), "Rap-Metal"); // winner keeps hyphen
        assert_eq!(canonicalize_genre("Rap-Rock"), "Rap Rock");   // TIE -> hyphen-free
        assert_eq!(canonicalize_genre("Trip-Hop"), "Trip Hop");
        assert_eq!(canonicalize_genre("Post-Bop"), "Post Bop");   // TIE -> hyphen-free
        assert_eq!(canonicalize_genre("Punk-Pop"), "Pop Punk");
        assert_eq!(canonicalize_genre("Pop-Punk"), "Pop Punk");
        assert_eq!(canonicalize_genre("Darkwave"), "Dark Wave");
        assert_eq!(canonicalize_genre("Neo-Glam"), "Neo Glam");   // TIE -> hyphen-free
        assert_eq!(canonicalize_genre("Hairmetal"), "Hair Metal");
        assert_eq!(canonicalize_genre("Psychadelic"), "Psychedelic"); // typo correction
        assert_eq!(canonicalize_genre("Two Tone"), "2 Tone");     // TIE -> audited label

        // ARBITRATED ROW (Orchestrator S184): Hip-Hop(20)/Hip Hop(21) fuse to the audit
        // winner "Hip Hop"; every spelling and casing collapses to it.
        assert_eq!(canonicalize_genre("Hip-Hop"), "Hip Hop");
        assert_eq!(canonicalize_genre("hip-hop"), "Hip Hop");
        assert_eq!(canonicalize_genre("HIP-HOP"), "Hip Hop");
        assert_eq!(canonicalize_genre("Hip Hop"), "Hip Hop");
    }

    #[test]
    fn test_canonicalize_genre_intact_single_variant_facets() {
        // Single-variant and facet-distinct terms must never fuse or mutate
        assert_eq!(canonicalize_genre("Early R&B"), "Early R&B");
        assert_eq!(canonicalize_genre("Rhythm And Blues"), "Rhythm And Blues");
        assert_eq!(canonicalize_genre("Rhythm & Blues"), "Rhythm & Blues");
        assert_eq!(canonicalize_genre("Adult Contemporary R&B"), "Adult Contemporary R&B");
        assert_eq!(canonicalize_genre("Emo-Pop"), "Emo-Pop");
        assert_eq!(canonicalize_genre("World-Fusion"), "World-Fusion");

        // Jazz vocal facets stay separate (owner decision, documented in matrix)
        assert_eq!(canonicalize_genre("Jazz Vocal"), "Jazz Vocal");
        assert_eq!(canonicalize_genre("Jazz Vocals"), "Jazz Vocals");
        assert_eq!(canonicalize_genre("Vocal Jazz"), "Vocal Jazz");
    }

    #[test]
    fn test_canonicalize_genre_no_false_positives_exact_match_only() {
        // Compounds sharing words with matrix rows must pass through untouched
        assert_eq!(canonicalize_genre("Party Rap"), "Party Rap");
        assert_eq!(canonicalize_genre("Oldies Rock"), "Oldies Rock");
        assert_eq!(canonicalize_genre("English Folk"), "English Folk");
        assert_eq!(canonicalize_genre("Spanish Pop"), "Spanish Pop");
        assert_eq!(canonicalize_genre("Post-Punk"), "Post-Punk");
        assert_eq!(canonicalize_genre("Indie Rock"), "Indie Rock");
        assert_eq!(canonicalize_genre("K-Pop"), "K-Pop");
        assert_eq!(canonicalize_genre("Synthpop Legends"), "Synthpop Legends");
        assert_eq!(canonicalize_genre("Progressive / Ambient"), "Progressive / Ambient");
        assert_eq!(canonicalize_genre("Glam Rock / Berlin Trilogy"), "Glam Rock / Berlin Trilogy");
    }

    #[test]
    fn test_fuse_genres_applies_matrix_after_validation_before_dedupe() {
        // Owner integration example from S184
        let fused = fuse_genres(&["r&b; R B; Funk"]);
        assert_eq!(fused, vec!["R&B".to_string(), "Funk".to_string()]);

        // Variants arriving from different providers collapse into one winner
        let fused_multi = fuse_genres(&["Synthpop", "Synth Pop", "Rock"]);
        assert_eq!(fused_multi, vec!["Synth-Pop".to_string(), "Rock".to_string()]);

        // Junk still wins over canonicalization: validation happens first
        assert_eq!(fuse_genres(&["Synthpop_soft Rock_pop"]), Vec::<String>::new());
    }
}

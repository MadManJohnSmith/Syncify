//! Genre Canonicalization Matrix Test (S184)
//!
//! Validates `syncify_metadata_domain::canonicalize_genre` and its integration point
//! inside `fuse_genres` (applied AFTER junk validation and BEFORE the case-insensitive
//! dedupe) against the owner's physical audit of 531 genre terms.
//!
//! Owner canonical rule: between variants of the same genre family, the spelling with the
//! highest audit frequency wins; on a tie the hyphen-free form wins.
//!
//! directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
//!
//! ARBITRATED ROW (Orchestrator S184): Hip-Hop(20) -> "Hip Hop"(21) initially conflicted
//! with the protected pin `genre_case_dedupe_test::test_casing_normalization_and_acronym_
//! preservation` and was suspended per the sprint protocol. The Orchestrator resolved the
//! conflict: the protected suites guard anti-junk semantics, not historical casing, and
//! the owner audit gives Hip Hop(21) > Hip-Hop(20), so the row is ACTIVE and the pinned
//! expectation was updated with citation.

use syncify_metadata_domain::{canonicalize_genre, format_fused_genres, fuse_genres};

/// Every CANONICAL_GENRE_VARIANTS row: (variant input, expected canonical output).
/// Frequencies in parentheses come from the owner's physical audit of 531 terms.
const MATRIX_ROWS: &[(&str, &str)] = &[
    // R B(5) / R&B / Rnb(4): 5 beats 4 -> '&' form wins
    ("R B", "R&B"),
    ("r&b", "R&B"),
    ("Rnb", "R&B"),
    // Soul And R B(1) / Soul And R&b(2): 2 beats 1
    ("Soul And R B", "Soul And R&B"),
    ("Soul And R&b", "Soul And R&B"),
    // Rock And Roll(9) / Rock & Roll(8) / Rock Roll(2): 9 beats 8 beats 2
    ("Rock And Roll", "Rock And Roll"),
    ("Rock & Roll", "Rock And Roll"),
    ("Rock Roll", "Rock And Roll"),
    // Hip-Hop(20) / Hip Hop(21): 21 beats 20 — Orchestrator S184 arbitration
    ("Hip-Hop", "Hip Hop"),
    ("hip-hop", "Hip Hop"),
    ("Hip Hop", "Hip Hop"),
    // Synth-Pop(20) / Synthpop(8) / Synth Pop(2): 20 beats 8 beats 2
    ("Synth-Pop", "Synth-Pop"),
    ("Synthpop", "Synth-Pop"),
    ("Synth Pop", "Synth-Pop"),
    // Dance Pop(14) / Dance-Pop(6): 14 beats 6
    ("Dance Pop", "Dance Pop"),
    ("Dance-Pop", "Dance Pop"),
    // Alternative-Pop -> Alternative Pop (owner rule: hyphen-free preferred)
    ("Alternative-Pop", "Alternative Pop"),
    // Electro-Pop -> Electropop (owner audit: portmanteau attested winner)
    ("Electro-Pop", "Electropop"),
    // Doo-Wop(2) / Doo Wop(3): 3 beats 2
    ("Doo-Wop", "Doo Wop"),
    // Nu-Metal(2) / Nu Metal(5): 5 beats 2
    ("Nu-Metal", "Nu Metal"),
    // Folk-Rock(2) / Folk Rock(12): 12 beats 2
    ("Folk-Rock", "Folk Rock"),
    // Blues-Rock(1) / Blues Rock(11): 11 beats 1
    ("Blues-Rock", "Blues Rock"),
    // Country-Rock(1) / Country Rock(6): 6 beats 1
    ("Country-Rock", "Country Rock"),
    // Rap-Metal(3) / Rap Metal(2): 3 beats 2 — hyphenated winner KEPT
    ("Rap-Metal", "Rap-Metal"),
    ("Rap Metal", "Rap-Metal"),
    // Trip-Hop(1) / Trip Hop(10): 10 beats 1
    ("Trip-Hop", "Trip Hop"),
    // Punk-Pop(2) / Pop-Punk(1) / Pop Punk(5): owner-audited family, 5 beats 2 beats 1
    ("Punk-Pop", "Pop Punk"),
    ("Pop-Punk", "Pop Punk"),
    ("Pop Punk", "Pop Punk"),
    // Dark Wave(3) / Darkwave(2): 3 beats 2
    ("Dark Wave", "Dark Wave"),
    ("Darkwave", "Dark Wave"),
    // Hairmetal(1) / Hair Metal(2): 2 beats 1
    ("Hairmetal", "Hair Metal"),
    // Psychadelic(2) -> Psychedelic: typo correction, never emit the misspelling
    ("Psychadelic", "Psychedelic"),
];

#[test]
fn test_matrix_every_variant_resolves_to_canonical() {
    for (input, expected) in MATRIX_ROWS {
        assert_eq!(
            canonicalize_genre(input),
            *expected,
            "matrix row '{input}' must canonicalize to '{expected}'"
        );
    }
}

#[test]
fn test_matrix_canonical_output_is_idempotent() {
    for (_, canonical) in MATRIX_ROWS {
        assert_eq!(
            canonicalize_genre(canonical),
            *canonical,
            "canonical form '{canonical}' must map to itself"
        );
    }
}

#[test]
fn test_matrix_case_insensitive_matching() {
    // Matching is exact over a normalized lowercase key: any casing of a variant resolves.
    assert_eq!(canonicalize_genre("SYNTHPOP"), "Synth-Pop");
    assert_eq!(canonicalize_genre("synth-pop"), "Synth-Pop");
    assert_eq!(canonicalize_genre("sYnTh PoP"), "Synth-Pop");
    assert_eq!(canonicalize_genre("RNB"), "R&B");
    assert_eq!(canonicalize_genre("r b"), "R&B");
    assert_eq!(canonicalize_genre("nu-metal"), "Nu Metal");
    assert_eq!(canonicalize_genre("TRIP-HOP"), "Trip Hop");
    assert_eq!(canonicalize_genre("darkWAVE"), "Dark Wave");
}

#[test]
fn test_tie_break_hyphen_free_form_wins() {
    // Owner rule: empate -> forma sin guión.
    assert_eq!(canonicalize_genre("Jazz-Rock"), "Jazz Rock"); // Jazz-Rock(2) = Jazz Rock(2)
    assert_eq!(canonicalize_genre("Jazz Rock"), "Jazz Rock");
    assert_eq!(canonicalize_genre("Rap-Rock"), "Rap Rock");   // Rap-Rock(2) = Rap Rock(2)
    assert_eq!(canonicalize_genre("Rap Rock"), "Rap Rock");
    assert_eq!(canonicalize_genre("Post-Bop"), "Post Bop");   // Post-Bop = Post Bop
    assert_eq!(canonicalize_genre("Post Bop"), "Post Bop");
    assert_eq!(canonicalize_genre("Neo-Glam"), "Neo Glam");   // Neo-Glam = Neo Glam
    assert_eq!(canonicalize_genre("Neo Glam"), "Neo Glam");
    // 2 Tone(1) = Two Tone(1): tie resolved to the numeric audited label
    assert_eq!(canonicalize_genre("Two Tone"), "2 Tone");
    assert_eq!(canonicalize_genre("2 Tone"), "2 Tone");
}

#[test]
fn test_single_variant_facets_stay_intact() {
    // Single-variant rows and facet-distinct terms are NEVER mutated or fused.
    assert_eq!(canonicalize_genre("Emo-Pop"), "Emo-Pop");           // única variante, intacta
    assert_eq!(canonicalize_genre("World-Fusion"), "World-Fusion"); // única variante, intacta
    assert_eq!(canonicalize_genre("Adult Contemporary R&B"), "Adult Contemporary R&B"); // compuesto intacto
    assert_eq!(canonicalize_genre("Early R&B"), "Early R&B");       // faceta R&B distinta
    assert_eq!(canonicalize_genre("Rhythm And Blues"), "Rhythm And Blues"); // faceta distinta
    assert_eq!(canonicalize_genre("Rhythm & Blues"), "Rhythm & Blues");     // faceta distinta

    // Jazz vocal facets: NO fusionar (facetas distintas según fuente)
    assert_eq!(canonicalize_genre("Jazz Vocal"), "Jazz Vocal");
    assert_eq!(canonicalize_genre("Jazz Vocals"), "Jazz Vocals");
    assert_eq!(canonicalize_genre("Vocal Jazz"), "Vocal Jazz");
}

#[test]
fn test_no_false_positives_exact_match_only() {
    // Compounds sharing words with matrix rows must pass through untouched:
    // matching is EXACT over the whole normalized key, never substring.
    assert_eq!(canonicalize_genre("Party Rap"), "Party Rap");
    assert_eq!(canonicalize_genre("Oldies Rock"), "Oldies Rock");
    assert_eq!(canonicalize_genre("English Folk"), "English Folk");
    assert_eq!(canonicalize_genre("Spanish Pop"), "Spanish Pop");
    assert_eq!(canonicalize_genre("Synthpop Legends"), "Synthpop Legends");
    assert_eq!(canonicalize_genre("Nu-Metal Core"), "Nu-Metal Core");
    assert_eq!(canonicalize_genre("Post-Punk"), "Post-Punk");
    assert_eq!(canonicalize_genre("Indie Rock"), "Indie Rock");
    assert_eq!(canonicalize_genre("K-Pop"), "K-Pop");
    assert_eq!(canonicalize_genre("Korea Rock"), "Korea Rock");
    assert_eq!(canonicalize_genre("Dance Pop Remixes"), "Dance Pop Remixes");
    assert_eq!(canonicalize_genre(""), "");
    // Whitespace-only input trims to empty (never invents a genre from blanks)
    assert_eq!(canonicalize_genre("   "), "");
}

#[test]
fn test_fuse_genres_owner_integration_example() {
    // Owner example from S184: variants collapse into the audited winner.
    let fused = fuse_genres(&["r&b; R B; Funk"]);
    assert_eq!(fused, vec!["R&B".to_string(), "Funk".to_string()]);
    assert_eq!(format_fused_genres(&["r&b; R B; Funk"]).as_deref(), Some("R&B; Funk"));
}

#[test]
fn test_fuse_genres_dedupe_runs_after_canonicalization() {
    // Variants arriving from different providers collapse into ONE winner entry...
    let fused = fuse_genres(&["Synthpop", "Synth Pop", "Rock"]);
    assert_eq!(fused, vec!["Synth-Pop".to_string(), "Rock".to_string()]);

    // ...regardless of arrival order or casing.
    let fused_reordered = fuse_genres(&["dance-pop", "Dance Pop"]);
    assert_eq!(fused_reordered, vec!["Dance Pop".to_string()]);

    let fused_mixed = fuse_genres(&["Folk Rock", "folk-rock", "FOLK-ROCK"]);
    assert_eq!(fused_mixed, vec!["Folk Rock".to_string()]);
}

#[test]
fn test_fuse_genres_validation_precedes_canonicalization() {
    // Junk validation happens BEFORE the matrix: corrupted concatenations never reach it.
    assert_eq!(fuse_genres(&["Synthpop_soft Rock_pop"]), Vec::<String>::new());
    assert_eq!(fuse_genres(&["rerip Synth-Pop"]), Vec::<String>::new());
    assert_eq!(fuse_genres(&["Psychadelic"]), vec!["Psychedelic".to_string()]);
}

#[test]
fn test_fuse_genres_unmatched_terms_pass_through_untouched() {
    // Terms outside the matrix keep their normalized token form (no invention);
    // "Hip-Hop" IS in the matrix (Orchestrator S184 arbitration) and fuses to "Hip Hop".
    let fused = fuse_genres(&["Hip-Hop", "Party Rap", "Vocal Jazz"]);
    assert_eq!(
        fused,
        vec!["Hip Hop".to_string(), "Party Rap".to_string(), "Vocal Jazz".to_string()]
    );
}

//! S177 Genre Sanitation - Case Deduplication & Delimiter Collapsing Test Suite
//!
//! Validates:
//! 1. Delimiter splitting and collapsing: ';', '/', multiple ';;', '//', ' ; / '.
//! 2. Case-insensitive deduplication: ["Rock", "rock", "ROCK", "Rock;"] -> ["Rock"].
//! 3. Preservation of clean capitalization / Title Case.
//! 4. Preservation of multi-lingual genres without language filtering (e.g. French, Spanish, German, Japanese).
//! 5. Proper formatting as standard semicolon-separated string.

use syncify_metadata_domain::{format_fused_genres, fuse_genres};

#[test]
fn test_case_insensitive_deduplication_variations() {
    let inputs = ["Rock", "rock", "ROCK", "Rock;", "  rock  ", "RoCk"];
    let fused = fuse_genres(&inputs);
    assert_eq!(fused, vec!["Rock".to_string()]);

    let formatted = format_fused_genres(&inputs).unwrap();
    assert_eq!(formatted, "Rock");
}

#[test]
fn test_delimiter_splitting_and_collapsing() {
    let inputs = [
        "Rock;;;Pop // / Disco / ; ",
        "  Indie Rock / Alternative Rock ; ; ",
        "Electronic/Ambient;;Downtempo",
    ];

    let fused = fuse_genres(&inputs);
    assert_eq!(
        fused,
        vec![
            "Rock".to_string(),
            "Pop".to_string(),
            "Disco".to_string(),
            "Indie Rock".to_string(),
            "Alternative Rock".to_string(),
            "Electronic".to_string(),
            "Ambient".to_string(),
            "Downtempo".to_string(),
        ]
    );

    let formatted = format_fused_genres(&inputs).unwrap();
    assert_eq!(
        formatted,
        "Rock; Pop; Disco; Indie Rock; Alternative Rock; Electronic; Ambient; Downtempo"
    );
}

#[test]
fn test_multilingual_genre_preservation_with_dedup() {
    let inputs = [
        "Variété française",
        "variété française",
        "Música Latina",
        "música latina",
        "Chanson française",
        "Neue Deutsche Härte",
        "neue deutsche härte",
        "J-Pop / K-Pop",
        "j-pop",
    ];

    let fused = fuse_genres(&inputs);
    assert_eq!(
        fused,
        vec![
            "Variété française".to_string(),
            "Música Latina".to_string(),
            "Chanson française".to_string(),
            "Neue Deutsche Härte".to_string(),
            "J-Pop".to_string(),
            "K-Pop".to_string(),
        ]
    );

    let formatted = format_fused_genres(&inputs).unwrap();
    assert_eq!(
        formatted,
        "Variété française; Música Latina; Chanson française; Neue Deutsche Härte; J-Pop; K-Pop"
    );
}

#[test]
fn test_casing_normalization_and_acronym_preservation() {
    let inputs = [
        "r&b",
        "edm",
        "synth-pop",
        "post-punk",
        "hip-hop",
    ];

    let fused = fuse_genres(&inputs);
    // Should normalize single/all-lowercase genres cleanly while keeping valid structure.
    // "Hip Hop" (antes "Hip-Hop"): directiva propietario S184: matriz de variantes con
    // regla de frecuencia sobre auditoría 531 términos (Hip Hop 21 > Hip-Hop 20); el
    // propósito del suite —dedupe de caja— permanece intacto.
    assert_eq!(
        fused,
        vec![
            "R&B".to_string(),
            "EDM".to_string(),
            "Synth-Pop".to_string(),
            "Post-Punk".to_string(),
            "Hip Hop".to_string(),
        ]
    );
}

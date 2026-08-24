//! S177 Genre Hardening - Mood Descriptors Rejection Test Suite
//!
//! Validates that all 20 non-musical mood descriptors identified in the 500-track audit
//! are rejected case-insensitively while preserving genuine musical genres.

use syncify_metadata_domain::{fuse_genres, is_valid_genre};

#[test]
fn test_all_20_mood_descriptors_rejected() {
    let moods = [
        "Emotional",
        "emotional",
        "EMOTIONAL",
        "Energetic",
        "energetic",
        "Extremely Bored",
        "extremely bored",
        "Fun",
        "fun",
        "Groovy",
        "groovy",
        "Happy",
        "happy",
        "Haunting",
        "haunting",
        "Hedonistic",
        "hedonistic",
        "Melancholy",
        "melancholy",
        "Mellow",
        "mellow",
        "Raw",
        "raw",
        "Rebellious",
        "rebellious",
        "Reflective",
        "reflective",
        "Relaxed",
        "relaxed",
        "Romantic",
        "romantic",
        "Self-Hatred",
        "self-hatred",
        "Smooth",
        "smooth",
        "Soothing",
        "soothing",
        "Sweet",
        "sweet",
        "Upbeat",
        "upbeat",
    ];

    for mood in moods {
        assert!(
            !is_valid_genre(mood),
            "Expected mood '{}' to be rejected by is_valid_genre",
            mood
        );
    }
}

#[test]
fn test_fuse_genres_strips_moods_and_retains_valid_genres() {
    let input = [
        "Rock; Energetic; Melancholy; Post-Punk",
        "Happy; Indie Pop; Upbeat; Dream Pop",
        "Smooth; Jazz; Mellow; Soul",
    ];

    let fused = fuse_genres(&input);
    assert_eq!(
        fused,
        vec![
            "Rock".to_string(),
            "Post-Punk".to_string(),
            "Indie Pop".to_string(),
            "Dream Pop".to_string(),
            "Jazz".to_string(),
            "Soul".to_string(),
        ]
    );
}

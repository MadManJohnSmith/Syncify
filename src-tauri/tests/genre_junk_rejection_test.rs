//! S177 Genre Sanitation - Junk Rejection Test Suite
//!
//! Validates:
//! 1. Rejection of genres matching track title (case-insensitive, trimmed).
//! 2. Rejection of genres matching track artist (case-insensitive, trimmed).
//! 3. Rejection of genres matching track album (case-insensitive, trimmed).
//! 4. Rejection of genres matching record label (case-insensitive, trimmed).
//! 5. Rejection of junk substrings: "feat.", "remaster", "version", "live", "deluxe", "edition".
//! 6. Rejection of invalid placeholders: "unknown", "n/a", "null", "none", "???", "", "-".
//! 7. Safe degradation to empty / None with warning when all candidate genres are junk.
//! 8. Preservation of valid genres when mixed with junk candidates.

use syncify_metadata_domain::{
    format_fused_genres_with_context, fuse_genres_with_context, FieldValidator, GenreContext,
};

#[test]
fn test_rejection_of_entity_matches_title_artist_album_label() {
    let ctx = GenreContext::new()
        .with_title(Some("Dear Rosemary"))
        .with_artist(Some("Foo Fighters"))
        .with_album(Some("Wasting Light"))
        .with_label(Some("RCA Records"));

    // Exact matches
    assert!(!FieldValidator::is_valid_genre_with_context("Dear Rosemary", Some(&ctx)));
    assert!(!FieldValidator::is_valid_genre_with_context("dear rosemary", Some(&ctx)));
    assert!(!FieldValidator::is_valid_genre_with_context("  Foo Fighters  ", Some(&ctx)));
    assert!(!FieldValidator::is_valid_genre_with_context("foo fighters", Some(&ctx)));
    assert!(!FieldValidator::is_valid_genre_with_context("Wasting Light", Some(&ctx)));
    assert!(!FieldValidator::is_valid_genre_with_context("wasting light", Some(&ctx)));
    assert!(!FieldValidator::is_valid_genre_with_context("RCA Records", Some(&ctx)));
    assert!(!FieldValidator::is_valid_genre_with_context("rca records", Some(&ctx)));

    // Unrelated valid genres must be accepted
    assert!(FieldValidator::is_valid_genre_with_context("Post-Grunge", Some(&ctx)));
    assert!(FieldValidator::is_valid_genre_with_context("Alternative Rock", Some(&ctx)));
    assert!(FieldValidator::is_valid_genre_with_context("Hard Rock", Some(&ctx)));
}

#[test]
fn test_rejection_of_junk_substring_patterns() {
    let junk_samples = [
        "2011 Remaster",
        "Remastered 2020",
        "remaster",
        "feat. MC Flipside",
        "Feat. Drake",
        "(feat. Kendrick Lamar)",
        "Album Version",
        "Extended Version",
        "Original Version",
        "Live at Wembley",
        "Recorded Live",
        "Live 1994",
        "Deluxe Edition",
        "Deluxe",
        "Special Edition",
        "Expanded Edition",
        "20th Anniversary Edition",
    ];

    for junk in &junk_samples {
        assert!(
            !FieldValidator::is_valid_genre(junk),
            "Expected '{}' to be rejected as junk genre",
            junk
        );
        assert!(
            !FieldValidator::is_valid_genre_with_context(junk, None),
            "Expected '{}' to be rejected as junk genre with context",
            junk
        );
    }
}

#[test]
fn test_rejection_of_placeholders() {
    let placeholders = [
        "",
        "   ",
        "unknown",
        "Unknown",
        "UNKNOWN",
        "n/a",
        "N/A",
        "null",
        "Null",
        "NULL",
        "None",
        "none",
        "NONE",
        "???",
        "-",
    ];

    for ph in &placeholders {
        assert!(
            !FieldValidator::is_valid_genre(ph),
            "Expected placeholder '{}' to be rejected",
            ph
        );
    }
}

#[test]
fn test_fuse_genres_filtering_junk_mixed_with_valid() {
    let ctx = GenreContext::new()
        .with_title(Some("Hi Friend!"))
        .with_artist(Some("deadmau5"))
        .with_album(Some("Random Album Title"))
        .with_label(Some("Ultra Records"));

    let raw_inputs = [
        "deadmau5",
        "Electro House; Hi Friend!",
        "2011 Remaster",
        "Progressive House / Ultra Records",
        "feat. MC Flipside",
        "Random Album Title",
        "Live at BBC",
        "Deluxe Edition",
        "unknown",
    ];

    let fused = fuse_genres_with_context(&raw_inputs, Some(&ctx));
    assert_eq!(
        fused,
        vec![
            "Electro House".to_string(),
            "Progressive House".to_string(),
        ]
    );

    let formatted = format_fused_genres_with_context(&raw_inputs, Some(&ctx));
    assert_eq!(
        formatted,
        Some("Electro House; Progressive House".to_string())
    );
}

#[test]
fn test_safe_degradation_to_empty_when_all_genres_are_junk() {
    let ctx = GenreContext::new()
        .with_title(Some("Song Title"))
        .with_artist(Some("Artist Name"))
        .with_album(Some("Album Name"))
        .with_label(Some("Label Name"));

    let all_junk_inputs = [
        "Song Title",
        "Artist Name",
        "Album Name",
        "Label Name",
        "2021 Remaster / Deluxe Edition",
        "Live in Tokyo; feat. Someone",
        "Unknown",
        "N/A",
    ];

    let fused = fuse_genres_with_context(&all_junk_inputs, Some(&ctx));
    assert!(
        fused.is_empty(),
        "Expected all junk genres to degrade to empty list, got: {:?}",
        fused
    );

    let formatted = format_fused_genres_with_context(&all_junk_inputs, Some(&ctx));
    assert_eq!(
        formatted, None,
        "Expected all junk genres to format as None (graceful degradation)"
    );
}

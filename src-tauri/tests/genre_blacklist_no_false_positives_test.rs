//! S177 Genre Hardening - No False Positives Test Suite
//!
//! Validates that exact blacklist matching does NOT reject legitimate composite genres
//! containing similar words (e.g. "Party Rap", "Oldies Rock", "English Folk").

use syncify_metadata_domain::{fuse_genres, is_valid_genre};

#[test]
fn test_compound_genres_not_falsely_rejected() {
    let valid_compound_genres = [
        "Party Rap",
        "Party Rock",
        "Oldies Rock",
        "Oldies Pop",
        "Video Game Music",
        "Video Game Soundtrack",
        "English Folk",
        "English Pop",
        "Spanish Pop",
        "Spanish Rock",
        "Latin Rock",
        "Indie Rock",
        "Shoegaze",
        "Dream Pop",
        "Gothic Rock",
        "Post-Punk Revival",
        "Chanson Française",
        "Pop Rock",
        "Classic Rock",
        "Hard Rock",
        "Alternative Rock",
        "Electronic Rock",
    ];

    for genre in valid_compound_genres {
        assert!(
            is_valid_genre(genre),
            "False positive detected: legitimate genre '{}' was rejected",
            genre
        );
    }
}

#[test]
fn test_fuse_genres_preserves_compound_genres() {
    let inputs = [
        "Party Rap; Oldies Rock; English Folk; Spanish Pop; Video Game Music",
    ];

    let fused = fuse_genres(&inputs);
    assert_eq!(
        fused,
        vec![
            "Party Rap".to_string(),
            "Oldies Rock".to_string(),
            "English Folk".to_string(),
            "Spanish Pop".to_string(),
            "Video Game Music".to_string(),
        ]
    );
}

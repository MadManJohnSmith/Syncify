//! S177 Genre Hardening - Metadata Artifacts, Contexts, and Isolated Terms Rejection Test Suite
//!
//! Validates:
//! 1. Rejection of 14 metadata artifacts and user tags.
//! 2. Rejection of 10 usage contexts, playlists, charts, and media types.
//! 3. Rejection of 3 isolated language/continent terms.

use syncify_metadata_domain::{fuse_genres, is_valid_genre};

#[test]
fn test_metadata_artifacts_rejected() {
    let artifacts = [
        "Hidden Track",
        "hidden track",
        "Interview",
        "interview",
        "Meme",
        "Misc",
        "Non-Music",
        "Part Ii",
        "Recordings With Subtle Differences",
        "Remark",
        "Sillyname",
        "Sitarsploitation",
        "Test",
        "Title Track",
        "Varios",
        "Well-Known",
    ];

    for artifact in artifacts {
        assert!(
            !is_valid_genre(artifact),
            "Expected metadata artifact '{}' to be rejected",
            artifact
        );
    }
}

#[test]
fn test_usage_contexts_and_charts_rejected() {
    let contexts = [
        "Exercise",
        "exercise",
        "Kuschelrock",
        "Late 60's Early 70's",
        "late 60's early 70's",
        "Offizielle Charts",
        "Oldies",
        "oldies",
        "Party",
        "party",
        "Series de televisión",
        "series de television",
        "Top 40",
        "top 40",
        "Video Game",
        "video game",
    ];

    for ctx in contexts {
        assert!(
            !is_valid_genre(ctx),
            "Expected usage context '{}' to be rejected",
            ctx
        );
    }
}

#[test]
fn test_isolated_terms_rejected() {
    let isolated = ["English", "english", "Spanish", "spanish", "África", "africa"];

    for item in isolated {
        assert!(
            !is_valid_genre(item),
            "Expected isolated term '{}' to be rejected",
            item
        );
    }
}

#[test]
fn test_fuse_genres_purges_junk_and_keeps_legitimate_genres() {
    let input = [
        "Party; Rock; Video Game; Electronic; Top 40",
        "Hidden Track; English; Folk Rock; Spanish; Oldies",
    ];

    let fused = fuse_genres(&input);
    assert_eq!(
        fused,
        vec![
            "Rock".to_string(),
            "Electronic".to_string(),
            "Folk Rock".to_string(),
        ]
    );
}

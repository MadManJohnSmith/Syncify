//! S177 Genre Hardening - Corrupt Scraper Concatenation Rejection Test Suite
//!
//! Validates rejection of underscore-joined composite tags, duplicate word concatenation,
//! and 'Rerip ' prefixes originating from external scrapers (Last.fm, Discogs).

use syncify_metadata_domain::{fuse_genres, is_valid_genre};

#[test]
fn test_all_16_corrupt_concatenations_rejected() {
    let corrupt_tags = [
        "Dance_electronic",
        "Electronic_synthpop",
        "Funk Soul_funk_soul",
        "Grunge_alternative Rock_alternative",
        "House_electronic_ambient",
        "Pop_electronic_synthpop",
        "Rock_hard Rock_emo",
        "Rock_pop",
        "Synthpop_soft Rock_pop Rock",
        "Techno_rock_pop Rock",
        "Techno_stage Screen_soundtrack_techno_stage Screen",
        "Uk_synthpop_synthpop",
        "Indieindie",
        "indieindie",
        "Rerip Grunge",
        "rerip grunge",
        "Rerip Pop Rock",
        "Rerip Soundtrack",
    ];

    for tag in corrupt_tags {
        assert!(
            !is_valid_genre(tag),
            "Expected corrupt concatenation '{}' to be rejected",
            tag
        );
    }
}

#[test]
fn test_fuse_genres_filters_out_corrupt_concatenations() {
    let inputs = [
        "Rock; Dance_electronic; Synth-Pop; Electronic_synthpop",
        "Indieindie; Indie Rock; Rerip Grunge; Grunge",
    ];

    let fused = fuse_genres(&inputs);
    assert_eq!(
        fused,
        vec![
            "Rock".to_string(),
            "Synth-Pop".to_string(),
            "Indie Rock".to_string(),
            "Grunge".to_string(),
        ]
    );
}

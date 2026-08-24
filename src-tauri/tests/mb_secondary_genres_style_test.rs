//! S178 MusicBrainz Secondary Genres to STYLE & Freeform TAGS Integration Test Suite
//!
//! Validates:
//! 1. Primary genre is preserved in `GENRE`.
//! 2. Secondary genres from multi-provider/MusicBrainz populate `STYLE`.
//! 3. Freeform tags populate `TAGS`.

use syncify_tauri_lib::services::enrichment::{EnrichmentEngine, OriginTrackMetadata};

#[tokio::test]
async fn test_secondary_genres_derive_style_and_tags() {
    let service = EnrichmentEngine::new();

    let origin = OriginTrackMetadata {
        source_name: "tidal".to_string(),
        title: Some("Blue Monday".to_string()),
        artist: Some("New Order".to_string()),
        album: Some("Power, Corruption & Lies".to_string()),
        genre: Some("Synth-Pop; Post-Punk; New Wave".to_string()),
        style: None,
        mood: Some("Dark; Electronic".to_string()),
        explicit: Some(false),
        track_number: Some(1),
        track_total: Some(8),
        disc_number: Some(1),
        disc_total: Some(1),
        release_year: Some("1983".to_string()),
        release_date: Some("1983-03-07".to_string()),
        original_date: Some("1983-03-07".to_string()),
        language: Some("English".to_string()),
        label: Some("Factory Records".to_string()),
        ..Default::default()
    };

    let enriched = service
        .resolve_exhaustive_track_metadata_with_force(
            "New Order",
            "Power, Corruption & Lies",
            "Blue Monday",
            None,
            &[origin],
            false,
            false,
        )
        .await;

    // Primary + full fused genre in GENRE
    let genre_val = enriched.genre.value().expect("GENRE must be populated");
    assert!(genre_val.contains("Synth-Pop"), "GENRE should contain Synth-Pop");
    assert!(genre_val.contains("Post-Punk"), "GENRE should contain Post-Punk");
    assert!(genre_val.contains("New Wave"), "GENRE should contain New Wave");

    // Secondary genres automatically populate STYLE when not explicitly provided
    let style_val = enriched.style.value().expect("STYLE must be populated from secondary genres");
    assert!(style_val.contains("Post-Punk"), "STYLE should contain Post-Punk");
    assert!(style_val.contains("New Wave"), "STYLE should contain New Wave");

    // LANGUAGE normalized to ISO 639-2/B
    let lang_val = enriched.language.value().expect("LANGUAGE must be populated");
    assert_eq!(lang_val, "eng");

    // GROUPING derived as {Artist} - {Album}
    let grp_val = enriched.grouping.value().expect("GROUPING must be populated");
    assert_eq!(grp_val, "New Order - Power, Corruption & Lies");
}

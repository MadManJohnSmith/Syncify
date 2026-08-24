//! S176M Exhaustive Enrichment - No Early Exit & Multi-Provider Completion Test Suite
//!
//! Tests:
//! 1. The engine does NOT early exit on the first provider result; queries and aggregates ALL configured providers.
//! 2. Missing fields in Provider A are fulfilled by Provider B, C, or MusicBrainz.
//! 3. Adherence to strict precedence hierarchy: Manual > StreamingMetadata (Qobuz/Tidal) > MusicBrainz > SpotifyMetadata > LocalAudioAnalysis.
//! 4. Lower priority candidates cannot overwrite higher priority candidates without force == true.
//! 5. BPM is only populated when delivered by a genuine source/analysis; never fabricated.
//! 6. Label and Genre variants are merged across providers.

use syncify_tauri_lib::services::enrichment::{EnrichmentEngine, OriginTrackMetadata};

#[tokio::test]
async fn test_enrichment_no_early_exit_aggregates_all_providers() {
    let engine = EnrichmentEngine::new();

    // Provider 1: Qobuz has core track info but lacks Language, Composer, Performers, BPM, Barcode
    let qobuz_source = OriginTrackMetadata {
        title: Some("Space Oddity".to_string()),
        artist: Some("David Bowie".to_string()),
        album: Some("Space Oddity".to_string()),
        label: Some("Parlophone".to_string()),
        source_name: "qobuz".to_string(),
        ..Default::default()
    };

    // Provider 2: Tidal delivers Language, Composer, Performers, BPM
    let tidal_source = OriginTrackMetadata {
        title: Some("Space Oddity".to_string()),
        artist: Some("David Bowie".to_string()),
        album: Some("Space Oddity".to_string()),
        composer: Some("David Bowie".to_string()),
        performers: Some("David Bowie, Rick Wakeman".to_string()),
        language: Some("English".to_string()),
        bpm: Some(136),
        label: Some("Jones/Tintoretto Entertainment Co., LLC".to_string()),
        source_name: "tidal".to_string(),
        ..Default::default()
    };

    // Provider 3: Spotify delivers Barcode, Release Year, Explicit flag
    let spotify_source = OriginTrackMetadata {
        title: Some("Space Oddity".to_string()),
        artist: Some("David Bowie".to_string()),
        album: Some("Space Oddity".to_string()),
        barcode: Some("0825646284093".to_string()),
        release_year: Some("1969".to_string()),
        explicit: Some(false),
        source_name: "spotify".to_string(),
        ..Default::default()
    };

    let enriched = engine.resolve_exhaustive_track_metadata(
        "David Bowie",
        "Space Oddity",
        "Space Oddity",
        None,
        &[qobuz_source, tidal_source, spotify_source],
        false,
    ).await;

    // Verify fields from Provider 1 (Qobuz)
    assert_eq!(enriched.title.value(), Some("Space Oddity"));
    assert_eq!(enriched.artist.value(), Some("David Bowie"));

    // Verify fields filled in by Provider 2 (Tidal) without early exit on Provider 1
    assert_eq!(enriched.composer.value(), Some("David Bowie"));
    assert_eq!(enriched.performers.value(), Some("David Bowie, Rick Wakeman"));
    assert_eq!(enriched.language.value(), Some("eng"));
    assert_eq!(enriched.bpm.value(), Some("136"));

    // Verify fields filled in by Provider 3 (Spotify) without early exit on Provider 1 or 2
    assert_eq!(enriched.barcode.value(), Some("0825646284093"));
    assert_eq!(enriched.release_year.value(), Some("1969"));
    assert_eq!(enriched.explicit.value(), Some("0"));

    // Verify label variants fused across providers
    let label_val = enriched.label.value().unwrap();
    assert!(label_val.contains("Parlophone"));
    assert!(label_val.contains("Jones/Tintoretto Entertainment Co., LLC"));
}

#[tokio::test]
async fn test_bpm_never_invented_when_no_source_provides_it() {
    let engine = EnrichmentEngine::new();

    let qobuz_source = OriginTrackMetadata {
        title: Some("Sound and Vision".to_string()),
        artist: Some("David Bowie".to_string()),
        album: Some("Low".to_string()),
        bpm: None,
        source_name: "qobuz".to_string(),
        ..Default::default()
    };

    let spotify_source = OriginTrackMetadata {
        title: Some("Sound and Vision".to_string()),
        artist: Some("David Bowie".to_string()),
        album: Some("Low".to_string()),
        bpm: None,
        source_name: "spotify".to_string(),
        ..Default::default()
    };

    let enriched = engine.resolve_exhaustive_track_metadata(
        "David Bowie",
        "Low",
        "Sound and Vision",
        None,
        &[qobuz_source, spotify_source],
        false,
    ).await;

    assert_eq!(enriched.bpm.value(), None);
}

#[tokio::test]
async fn test_precedence_hierarchy_and_force_override() {
    let engine = EnrichmentEngine::new();

    // Streaming provider (Qobuz) has higher priority (4) than Spotify (2)
    let qobuz_source = OriginTrackMetadata {
        title: Some("Heroes (Official Master)".to_string()),
        artist: Some("David Bowie".to_string()),
        album: Some("Heroes".to_string()),
        source_name: "qobuz".to_string(),
        ..Default::default()
    };

    let spotify_source = OriginTrackMetadata {
        title: Some("Heroes - 2017 Remaster".to_string()),
        artist: Some("David Bowie".to_string()),
        album: Some("Heroes".to_string()),
        source_name: "spotify".to_string(),
        ..Default::default()
    };

    // Standard run: Qobuz beats Spotify
    let standard = engine.resolve_exhaustive_track_metadata(
        "David Bowie",
        "Heroes",
        "Heroes",
        None,
        &[spotify_source.clone(), qobuz_source.clone()],
        false,
    ).await;

    assert_eq!(standard.title.value(), Some("Heroes (Official Master)"));

    // Forced run with Spotify coming last with force == true
    let forced = engine.resolve_exhaustive_track_metadata_with_force(
        "David Bowie",
        "Heroes",
        "Heroes",
        None,
        &[qobuz_source, spotify_source],
        false,
        true,
    ).await;

    assert_eq!(forced.title.value(), Some("Heroes - 2017 Remaster"));
}

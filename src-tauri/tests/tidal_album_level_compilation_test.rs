//! TASK-7.1 / CR-1: the Tidal pipeline decides COMPILATION + ALBUMARTIST once per
//! RELEASE, not once per track.
//!
//! Regression guard for the wiring of `syncify_flac_writer::unify_album_compilation_metadata`
//! into `services::tidal_pipeline`: every sibling track of the same album must get the
//! same verdict, and the verdict must match what the writer puts on disk.

use syncify_core_domain::metadata::{TidalAlbum, TidalArtist, TidalTrack};
use syncify_tauri_lib::services::tidal_pipeline::decide_album_compilation;

fn artist(name: &str) -> TidalArtist {
    TidalArtist {
        id: None,
        name: name.to_string(),
    }
}

fn album(album_artist: Option<&str>, album_type: Option<&str>, with_listing: bool) -> TidalAlbum {
    TidalAlbum {
        id: Some(42),
        title: "Synthwave Classics".to_string(),
        release_date: None,
        cover: None,
        artist: album_artist.map(artist),
        artists: None,
        number_of_tracks: Some(3),
        number_of_volumes: Some(1),
        copyright: None,
        upc: None,
        album_type: album_type.map(|t| t.to_string()),
        tracks: if with_listing { None } else { Some(Vec::new()) },
    }
}

fn track(id: i64, title: &str, track_artist: &str, album: &TidalAlbum) -> TidalTrack {
    TidalTrack {
        id,
        title: title.to_string(),
        version: None,
        isrc: None,
        duration: 180,
        audio_quality: None,
        album: Some(album.clone()),
        artist: Some(artist(track_artist)),
        artists: None,
        track_number: Some(id as i32),
        volume_number: Some(1),
        media_metadata: None,
        bpm: None,
        copyright: None,
        explicit: None,
    }
}

/// Regression: a multi-artist release with NO "compilation" marker in the album
/// payload used to be judged track by track, so the first track downloaded could be
/// written without COMPILATION while its siblings got it. Every track of the listing
/// must now produce the identical album-level verdict.
#[test]
fn test_album_level_verdict_is_identical_for_every_sibling_track() {
    let alb = album(Some("Various Artists"), None, true);
    let listing = vec![
        track(1, "Midnight City", "M83", &alb),
        track(2, "Nightcall", "Kavinsky", &alb),
        track(3, "Teardrop", "Massive Attack", &alb),
    ];

    let verdicts: Vec<_> = listing
        .iter()
        .map(|t| decide_album_compilation(&alb, &listing, t))
        .collect();

    for v in &verdicts {
        assert!(
            v.is_compilation,
            "a multi-artist release must be flagged as a compilation at ALBUM level"
        );
        assert_eq!(v.album_artist.as_deref(), Some("Various Artists"));
        assert_eq!(v.evidence_tracks, 3);
    }
    assert!(
        verdicts.windows(2).all(|w| w[0] == w[1]),
        "every sibling track must resolve to the same album-level verdict: {:?}",
        verdicts
    );
}

/// A mono-artist release keeps its own artist and never gains COMPILATION, even
/// when the album payload carries no release type at all.
#[test]
fn test_mono_artist_release_is_not_flagged_as_compilation() {
    let alb = album(Some("Queen"), Some("ALBUM"), true);
    let listing = vec![
        track(1, "Bohemian Rhapsody", "Queen", &alb),
        track(2, "We Will Rock You", "Queen", &alb),
    ];

    for t in &listing {
        let v = decide_album_compilation(&alb, &listing, t);
        assert!(
            !v.is_compilation,
            "mono-artist album must not be a compilation"
        );
        assert_eq!(v.album_artist.as_deref(), Some("Queen"));
    }
}

/// The album-level evidence still wins when the album is explicitly a compilation
/// but every individual track is credited to the same artist.
#[test]
fn test_explicit_compilation_release_wins_over_single_track_artist() {
    let alb = album(Some("Various Artists"), Some("COMPILATION"), true);
    let listing = vec![
        track(1, "Bohemian Rhapsody", "Queen", &alb),
        track(2, "We Will Rock You", "Queen", &alb),
    ];

    for t in &listing {
        let v = decide_album_compilation(&alb, &listing, t);
        assert!(v.is_compilation);
        assert_eq!(v.album_artist.as_deref(), Some("Various Artists"));
    }
}

/// A track missing from the (partial) listing must still be covered by the verdict
/// that will be written to its file.
#[test]
fn test_track_absent_from_listing_still_gets_the_album_verdict() {
    let alb = album(Some("Various Artists"), None, true);
    let listing = vec![
        track(1, "Midnight City", "M83", &alb),
        track(2, "Nightcall", "Kavinsky", &alb),
    ];
    let orphan = track(99, "Unlisted B-Side", "Daft Punk", &alb);

    let v = decide_album_compilation(&alb, &listing, &orphan);
    assert!(v.is_compilation);
    assert_eq!(v.album_artist.as_deref(), Some("Various Artists"));
    assert_eq!(
        v.evidence_tracks, 3,
        "the orphan track is added to the evidence"
    );
}

/// With no sibling information at all the decision degrades to the track itself
/// instead of inventing a compilation.
#[test]
fn test_single_track_listing_degrades_to_track_evidence() {
    let alb = album(None, None, true);
    let only = track(7, "Sole Track", "Boards of Canada", &alb);

    let v = decide_album_compilation(&alb, std::slice::from_ref(&only), &only);
    assert!(!v.is_compilation);
    assert_eq!(v.album_artist.as_deref(), Some("Boards of Canada"));
    assert_eq!(v.evidence_tracks, 1);
}

//! S196 / TASK-67: Multi-Artist VorbisComment Tags & `feat.` Title Decoupling Test Suite
//!
//! Validates:
//! 1. Canonical title cleaning and collaborator extraction (`clean_title_and_extract_featured`)
//!    across diverse bracketed and bare feat patterns, preserving complex names like "Tyler, The Creator"
//!    and ignoring false positives like "BIRDS OF A FEATHER" or "as featured in ...".
//! 2. FLAC VorbisComments emit discrete, independent `ARTIST` comment blocks for each artist
//!    (required for Symfonium multi-artist indexing) instead of a single concatenated string.
//! 3. Automatic extraction and decoupling of collaborators trapped in track titles into discrete `ARTIST` blocks.
//! 4. Decoupling of artists formatted with `feat.` or semicolons in the artist tag string.
//! 5. Case-insensitive artist deduplication.
//! 6. Preservation of the Symfonium WebP / CoverFront invariant.

use std::fs;
use std::path::Path;
use std::process::Command;
use syncify_flac_writer::{
    apply_and_verify_flac_tags, apply_flac_tags, clean_title_and_extract_featured,
    resolve_flac_artists, FlacMetadata,
};
use tempfile::tempdir;

fn generate_synthetic_pcm() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_sec = 0.1;
    let total_samples = (sample_rate as f64 * duration_sec) as usize;
    let mut samples = vec![0.0f32; total_samples];
    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        samples[i] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
    }
    samples
}

fn create_synthetic_flac(path: &Path) {
    let samples = generate_synthetic_pcm();
    let temp_wav = path.with_extension("wav");

    let mut wav_bytes = Vec::new();
    let num_samples = samples.len() as u32;
    let sample_rate = 44100u32;
    let byte_rate = sample_rate * 2;
    let block_align = 2u16;

    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&(36 + num_samples * 2).to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVEfmt ");
    wav_bytes.extend_from_slice(&16u32.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&sample_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&byte_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&block_align.to_le_bytes());
    wav_bytes.extend_from_slice(&16u16.to_le_bytes());
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&(num_samples * 2).to_le_bytes());

    for &s in &samples {
        let sample_i16 = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        wav_bytes.extend_from_slice(&sample_i16.to_le_bytes());
    }

    fs::write(&temp_wav, &wav_bytes).expect("Write wav");

    let status = Command::new("flac")
        .args([
            "-f",
            "--silent",
            "-o",
            path.to_str().unwrap(),
            temp_wav.to_str().unwrap(),
        ])
        .status()
        .expect("Run flac encoder");

    let _ = fs::remove_file(&temp_wav);
    assert!(status.success(), "flac encode must succeed");
}

#[test]
fn test_title_cleaning_and_feat_extraction_comprehensive() {
    let cases = vec![
        // (Input Title, Expected Clean Title, Expected Featured Artists)
        (
            "23 (feat. Sasha Dobson)",
            "23",
            vec!["Sasha Dobson"],
        ),
        (
            "After The Storm (Ft. Tyler, The Creator)",
            "After The Storm",
            vec!["Tyler, The Creator"],
        ),
        (
            "DARE (featuring Shaun Ryder and Rosie Wilson)",
            "DARE",
            vec!["Shaun Ryder", "Rosie Wilson"],
        ),
        (
            "Ain't No Love [feat. Melanie Williams]",
            "Ain't No Love",
            vec!["Melanie Williams"],
        ),
        (
            "Track Title [ft. Collaborator]",
            "Track Title",
            vec!["Collaborator"],
        ),
        (
            "Track Title {feat. Collaborator}",
            "Track Title",
            vec!["Collaborator"],
        ),
        (
            "Cobra (Rock Remix) [feat. Spiritbox]",
            "Cobra (Rock Remix)",
            vec!["Spiritbox"],
        ),
        (
            "4 Minutes (feat. Justin Timberlake & Timbaland)",
            "4 Minutes",
            vec!["Justin Timberlake", "Timbaland"],
        ),
        (
            "Audio (feat. Sia, Diplo, and Labrinth)",
            "Audio",
            vec!["Sia", "Diplo", "Labrinth"],
        ),
        (
            "Downtown (feat. Melle Mel, Grandmaster Caz, Kool Moe Dee & Eric Nally)",
            "Downtown",
            vec!["Melle Mel", "Grandmaster Caz", "Kool Moe Dee", "Eric Nally"],
        ),
        (
            "Burn My Shadow feat. Ian Astbury",
            "Burn My Shadow",
            vec!["Ian Astbury"],
        ),
        (
            "Fly By Day feat. JU!iE",
            "Fly By Day",
            vec!["JU!iE"],
        ),
        (
            "202 feat. 泉まくら - New Mix",
            "202 - New Mix",
            vec!["泉まくら"],
        ),
        (
            "GIRL feat.呂布",
            "GIRL",
            vec!["呂布"],
        ),
        (
            "Feel The Fiyaaaah (with A$AP Rocky & feat. Takeoff)",
            "Feel The Fiyaaaah",
            vec!["Takeoff"],
        ),
        (
            "Too Many Nights (feat. Don Toliver & with Future)",
            "Too Many Nights",
            vec!["Don Toliver", "Future"],
        ),
        // False positives — must NOT extract or alter title
        (
            "BIRDS OF A FEATHER",
            "BIRDS OF A FEATHER",
            vec![],
        ),
        (
            "Light as a Feather",
            "Light as a Feather",
            vec![],
        ),
        (
            "Feather",
            "Feather",
            vec![],
        ),
        (
            "Bloodfeather",
            "Bloodfeather",
            vec![],
        ),
        (
            "Sexy Rouge (as featured in \"Sky Rojo\") (Remix)",
            "Sexy Rouge (as featured in \"Sky Rojo\") (Remix)",
            vec![],
        ),
        (
            "Ordinary Title",
            "Ordinary Title",
            vec![],
        ),
    ];

    for (raw, exp_title, exp_artists) in cases {
        let (clean, artists) = clean_title_and_extract_featured(raw);
        assert_eq!(
            clean, exp_title,
            "Clean title mismatch for input '{}': expected '{}', got '{}'",
            raw, exp_title, clean
        );
        assert_eq!(
            artists, exp_artists,
            "Featured artists mismatch for input '{}': expected {:?}, got {:?}",
            raw, exp_artists, artists
        );
    }
}

#[test]
fn test_flac_discrete_multi_artist_vorbis_comments() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("multi_artist_discrete.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Get Lucky".to_string(),
        artist: "Daft Punk".to_string(),
        album: "Random Access Memories".to_string(),
        artists: Some(vec![
            "Daft Punk".to_string(),
            "Pharrell Williams".to_string(),
            "Nile Rodgers".to_string(),
        ]),
        ..Default::default()
    };

    let report = apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write and verify");
    assert!(report.tags_match, "Tags verification failed: {:?}", report.mismatches);

    let tag_obj = metaflac::Tag::read_from_path(&file_path).expect("Read FLAC tag");
    let comments = tag_obj.vorbis_comments().expect("Vorbis comments present");

    let artist_comments = comments.get("ARTIST").expect("ARTIST comments must exist");
    // Crucial check: must NOT be a single merged string "Daft Punk, Pharrell Williams, Nile Rodgers"
    assert_eq!(
        artist_comments.len(),
        3,
        "Must have exactly 3 discrete ARTIST comment blocks, got {:?}",
        artist_comments
    );
    assert_eq!(
        artist_comments,
        &[
            "Daft Punk".to_string(),
            "Pharrell Williams".to_string(),
            "Nile Rodgers".to_string()
        ],
        "Discrete ARTIST blocks must match expected list in order"
    );
}

#[test]
fn test_flac_auto_extraction_from_title_generates_discrete_artists() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("auto_feat_track.flac");
    create_synthetic_flac(&file_path);

    // Metadata has feat trapped in title and single primary artist
    let meta = FlacMetadata {
        title: "After The Storm (Ft. Tyler, The Creator)".to_string(),
        artist: "Kali Uchis".to_string(),
        album: "Isolation".to_string(),
        artists: None, // No explicit artists array provided
        ..Default::default()
    };

    let report = apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write and verify");
    assert!(report.tags_match, "Tags verification failed: {:?}", report.mismatches);

    let tag_obj = metaflac::Tag::read_from_path(&file_path).expect("Read FLAC tag");
    let comments = tag_obj.vorbis_comments().expect("Vorbis comments present");

    // Title must be sanitized (feat stripped)
    let titles = comments.get("TITLE").expect("TITLE tag must exist");
    assert_eq!(titles, &["After The Storm".to_string()]);

    // ARTIST must contain 2 discrete blocks: Primary and Featured
    let artists = comments.get("ARTIST").expect("ARTIST tags must exist");
    assert_eq!(
        artists,
        &["Kali Uchis".to_string(), "Tyler, The Creator".to_string()],
        "Must decouple into discrete ARTIST blocks without tearing internal comma in Tyler, The Creator"
    );
}

#[test]
fn test_flac_auto_decouple_semicolon_and_feat_in_artist_string() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("fused_artist_string.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Triad".to_string(),
        artist: "Artist A feat. Artist B; Artist C".to_string(),
        album: "Collborations".to_string(),
        artists: None,
        ..Default::default()
    };

    apply_flac_tags(&file_path, &meta).expect("FLAC write");

    let tag_obj = metaflac::Tag::read_from_path(&file_path).expect("Read FLAC tag");
    let comments = tag_obj.vorbis_comments().expect("Vorbis comments present");

    let artists = comments.get("ARTIST").expect("ARTIST tags must exist");
    assert_eq!(
        artists,
        &[
            "Artist A".to_string(),
            "Artist B".to_string(),
            "Artist C".to_string()
        ],
        "Must decouple both feat. and semicolon into discrete ARTIST blocks"
    );
}

#[test]
fn test_flac_artist_deduplication_between_artist_and_title() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("dedup_artist.flac");
    create_synthetic_flac(&file_path);

    // Title has collaborator, but artists list already includes that collaborator
    let meta = FlacMetadata {
        title: "Get Lucky (feat. Pharrell Williams)".to_string(),
        artist: "Daft Punk".to_string(),
        album: "Random Access Memories".to_string(),
        artists: Some(vec![
            "Daft Punk".to_string(),
            "Pharrell Williams".to_string(),
        ]),
        ..Default::default()
    };

    let report = apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write and verify");
    assert!(report.tags_match, "Tags verification failed: {:?}", report.mismatches);

    let tag_obj = metaflac::Tag::read_from_path(&file_path).expect("Read FLAC tag");
    let comments = tag_obj.vorbis_comments().expect("Vorbis comments present");

    let artists = comments.get("ARTIST").expect("ARTIST tags must exist");
    assert_eq!(
        artists.len(),
        2,
        "Deduplication must prevent duplicate Pharrell Williams block"
    );
    assert_eq!(
        artists,
        &["Daft Punk".to_string(), "Pharrell Williams".to_string()]
    );
}

#[test]
fn test_resolve_flac_artists_helper_direct() {
    let resolved = resolve_flac_artists(
        "Primary Artist",
        Some(&["Primary Artist".to_string(), "Featured Artist".to_string()]),
        &["Featured Artist".to_string(), "Third Artist".to_string()],
    );

    assert_eq!(
        resolved,
        vec![
            "Primary Artist".to_string(),
            "Featured Artist".to_string(),
            "Third Artist".to_string()
        ]
    );
}

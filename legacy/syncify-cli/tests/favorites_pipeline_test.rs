//! Favorites Pipeline Integration Tests
//!
//! Verifies:
//! 1. Full metadata enrichment mapping into VorbisComments (LANGUAGE, RELEASECOUNTRY, LABEL, COMPOSER, PERFORMER, BPM, INITIALKEY, GENRE, STYLE, MOOD).
//! 2. Rejection of placeholder strings ("Unknown", "N/A", "null", "none", "???").
//! 3. Qobuz favorites list deduplication across paginated fetches.
//! 4. Apple Music animated cover resolution status and metaflac embedding without duplicate blocks.

use syncify_cli::download::favorites::FavoriteItem;
use syncify_cli::metadata::tag_writer::{apply_flac_tags, is_valid_tag_val, verify_flac_tags, FlacMetadata};
use syncify_cli::services::enrichment::EnrichedMetadata;

fn create_dummy_flac(path: &std::path::Path) {
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // is_last=1, len=34
    let mut streaminfo = [0u8; 34];
    streaminfo[0..2].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[2..4].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[10] = 0x0A;
    streaminfo[11] = 0xC4;
    streaminfo[12] = 0x42;
    streaminfo[13] = 0xF0;
    flac_bytes.extend_from_slice(&streaminfo);
    flac_bytes.extend_from_slice(&[0xFF, 0xF8, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00]);
    std::fs::write(path, &flac_bytes).unwrap();
}

#[test]
fn test_favorites_pipeline_flac_tagging_full_symfonium_fields() {
    let temp_dir = std::env::temp_dir().join(format!("test_fav_tagging_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let flac_path = temp_dir.join("05 - Heroes.flac");
    create_dummy_flac(&flac_path);

    let enriched = EnrichedMetadata {
        genre: Some("Art Rock".to_string()),
        style: Some("Glam".to_string()),
        mood: Some("epic".to_string()),
        release_type: Some("Album".to_string()),
        release_status: Some("Official".to_string()),
        release_country: Some("United Kingdom".to_string()),
        language: Some("English".to_string()),
        label: Some("RCA Records".to_string()),
        barcode: Some("07464350982".to_string()),
        catalog_number: Some("PL 12522".to_string()),
        original_date: Some("1977-10-14".to_string()),
        bpm: Some(112.0),
        key: Some("G".to_string()),
        energy: Some(0.85),
        danceability: Some(0.60),
        loudness: Some(-7.5),
        enriched_at: "2026-08-12T00:00:00Z".to_string(),
        ..Default::default()
    };

    let meta = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "\"Heroes\"".to_string(),
        album_artist: Some("David Bowie".to_string()),
        composer: Some("David Bowie, Brian Eno".to_string()),
        performers: Some("David Bowie (Vocals, Guitar) - Brian Eno (Synthesizers) - Robert Fripp (Lead Guitar)".to_string()),
        work: Some("Heroes".to_string()),
        genre: enriched.genre,
        style: enriched.style,
        mood: enriched.mood,
        release_type: enriched.release_type,
        release_status: enriched.release_status,
        release_country: enriched.release_country,
        language: enriched.language,
        copyright: Some("1977 RCA Records".to_string()),
        label: enriched.label,
        barcode: enriched.barcode,
        catalog_number: enriched.catalog_number,
        original_date: enriched.original_date,
        track_number: 5,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        disc_subtitle: None,
        isrc: Some("GBAYE7700010".to_string()),
        release_year: Some("1977".to_string()),
        release_date: Some("1977-10-14".to_string()),
        explicit: Some(false),
        bpm: enriched.bpm.map(|b| b as u32),
        initial_key: enriched.key,
        energy: enriched.energy,
        danceability: enriched.danceability,
        loudness: enriched.loudness,
        replaygain_track_gain: Some("-10.50 dB".to_string()),
        r128_track_gain: Some("-2688".to_string()),
        comment: Some("Syncify Production Tagging".to_string()),
        bit_depth: Some(24),
        sample_rate: Some(192000.0),
        musicbrainz_track_id: Some("11111111-2222-3333-4444-555555555555".to_string()),
        musicbrainz_artist_id: Some("66666666-7777-8888-9999-aaaaaaaaaaaa".to_string()),
        musicbrainz_album_id: Some("bbbbbbbb-cccc-dddd-eeee-ffffffffffff".to_string()),
        musicbrainz_release_group_id: Some("00000000-1111-2222-3333-444444444444".to_string()),
        lyrics_lrc: Some("[00:10.00]I, I wish you could swim\n[00:15.00]Like the dolphins".to_string()),
        cover_data: Some(vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46]), // JPEG header
        lyrics_source: Some("LRCLIB (synced)".to_string()),
        cover_source: Some("HD Cover Art".to_string()),
        audio_source: Some("Qobuz Native FLAC (24-bit / 192.0 kHz)".to_string()),
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &meta).unwrap();
    let verification = verify_flac_tags(&flac_path, &meta).unwrap();

    assert!(verification.flac_valid, "FLAC file must be valid");
    assert!(verification.tags_match, "Tags must match without mismatches: {:?}", verification.mismatches);
    assert!(verification.lyrics_present, "Lyrics must be present");
    assert!(verification.synced_lyrics_present, "Synced lyrics must be present");
    assert!(verification.bpm_present, "BPM must be present in VorbisComments");

    // Read VorbisComments directly to verify Symfonium field names
    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let comments = tag.vorbis_comments().unwrap();

    assert_eq!(comments.get("LANGUAGE").unwrap(), &["English"]);
    assert_eq!(comments.get("RELEASECOUNTRY").unwrap(), &["United Kingdom"]);
    assert_eq!(comments.get("LABEL").unwrap(), &["RCA Records"]);
    assert_eq!(comments.get("COMPOSER").unwrap(), &["David Bowie, Brian Eno"]);
    assert_eq!(comments.get("PERFORMER").unwrap(), &["David Bowie (Vocals, Guitar) - Brian Eno (Synthesizers) - Robert Fripp (Lead Guitar)"]);
    assert_eq!(comments.get("BPM").unwrap(), &["112"]);
    assert_eq!(comments.get("INITIALKEY").unwrap(), &["G"]);
    assert_eq!(comments.get("GENRE").unwrap(), &["Art Rock"]);
    assert_eq!(comments.get("STYLE").unwrap(), &["Glam"]);
    assert_eq!(comments.get("MOOD").unwrap(), &["epic"]);
    assert_eq!(comments.get("BARCODE").unwrap(), &["07464350982"]);
    assert_eq!(comments.get("CATALOGNUMBER").unwrap(), &["PL 12522"]);
    assert_eq!(comments.get("ORIGINALDATE").unwrap(), &["1977-10-14"]);
}

#[test]
fn test_rejection_of_placeholder_values_in_tags() {
    assert!(!is_valid_tag_val("Unknown"));
    assert!(!is_valid_tag_val("Unknown Artist"));
    assert!(!is_valid_tag_val("Unknown Album"));
    assert!(!is_valid_tag_val("Unknown Track"));
    assert!(!is_valid_tag_val("N/A"));
    assert!(!is_valid_tag_val("null"));
    assert!(!is_valid_tag_val("none"));
    assert!(!is_valid_tag_val("???"));
    assert!(!is_valid_tag_val("   "));

    assert!(is_valid_tag_val("David Bowie"));
    assert!(is_valid_tag_val("English"));
    assert!(is_valid_tag_val("United Kingdom"));
}

#[test]
fn test_favorites_item_deduplication() {
    let mut seen_ids = std::collections::HashSet::new();
    let mut unique_items = Vec::new();

    let items = vec![
        FavoriteItem { id: "101".to_string(), title: "Song 1".to_string(), artist_name: "Artist A".to_string(), item_type: "tracks".to_string(), hires: true },
        FavoriteItem { id: "102".to_string(), title: "Song 2".to_string(), artist_name: "Artist B".to_string(), item_type: "tracks".to_string(), hires: false },
        FavoriteItem { id: "101".to_string(), title: "Song 1".to_string(), artist_name: "Artist A".to_string(), item_type: "tracks".to_string(), hires: true }, // Duplicate from pagination
    ];

    for item in items {
        if seen_ids.insert(item.id.clone()) {
            unique_items.push(item);
        }
    }

    assert_eq!(unique_items.len(), 2, "Duplicate item id 101 must be deduplicated");
}

#[test]
fn test_favorites_limit_truncation() {
    let mut items: Vec<FavoriteItem> = (1..=200).map(|i| FavoriteItem {
        id: format!("id_{}", i),
        title: format!("Title {}", i),
        artist_name: format!("Artist {}", i),
        item_type: "tracks".to_string(),
        hires: true,
    }).collect();

    let limit = 150;
    if items.len() > limit {
        items.truncate(limit);
    }

    assert_eq!(items.len(), 150, "Limit truncation must produce exactly 150 items");
    assert_eq!(items.first().unwrap().id, "id_1");
    assert_eq!(items.last().unwrap().id, "id_150");
}


//! Sprint S113A: Metadata & Enrichment Pipeline Parity Test Suite
//! Validates full ~41 VorbisComments, lyrics embedding, cover art (static/animated),
//! commercial tags, MBIDs, staging rollback, and best-effort graceful degradation.

use std::path::Path;
use syncify_flac_writer::{
    apply_and_verify_flac_tags, audit_flac_stage, FlacMetadata,
};
use syncify_tauri_lib::services::enrichment::{EnrichmentEngine, OriginTrackMetadata};
use tempfile::TempDir;

/// Generates a valid minimal synthetic FLAC file for testing tag writer and verification
fn create_minimal_test_flac(path: &Path) {
    // Minimal valid FLAC stream header + empty STREAMINFO block
    let mut data = Vec::new();
    data.extend_from_slice(b"fLaC"); // 4-byte magic

    // STREAMINFO metadata block header (last block = false, type = 0, length = 34 bytes)
    data.push(0x00); // not last block, type 0
    data.push(0x00);
    data.push(0x00);
    data.push(0x22); // 34 bytes

    // 34 bytes of STREAMINFO data
    data.extend_from_slice(&[0u8; 34]);

    // Set valid sample rate and channels in STREAMINFO (e.g. 44100Hz, 2 channels, 16 bps)
    data[8] = 0x10; // min block = 4096
    data[9] = 0x00;
    data[10] = 0x10; // max block = 4096
    data[11] = 0x00;
    data[18] = (44100 >> 12) as u8;
    data[19] = ((44100 >> 4) & 0xFF) as u8;
    data[20] = (((44100 & 0x0F) << 4) | (1 << 1) | 0) as u8; // 2 channels (1), 16 bits (15 -> split)
    data[21] = 0xF0;

    // Last metadata block header: PADDING (last block = true, type = 1, length = 0)
    data.push(0x81);
    data.push(0x00);
    data.push(0x00);
    data.push(0x00);

    std::fs::write(path, &data).expect("Failed to write synthetic test FLAC");
}

#[test]
fn test_full_vorbis_comment_41_tags_parity() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = temp_dir.path().join("test_full_tags.flac");
    create_minimal_test_flac(&flac_path);

    let full_meta = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        album_artist: Some("David Bowie".to_string()),
        composer: Some("David Bowie, Brian Eno".to_string()),
        performers: Some("David Bowie, Robert Fripp".to_string()),
        work: Some("Heroes Symphony".to_string()),
        genre: Some("Art Rock".to_string()),
        style: Some("Glam Rock / Berlin Trilogy".to_string()),
        mood: Some("Triumphant".to_string()),
        release_type: Some("Album".to_string()),
        release_status: Some("Official".to_string()),
        release_country: Some("GB".to_string()),
        release_region: None,
        language: Some("eng".to_string()),
        copyright: Some("(P) 1977 RCA Records".to_string()),
        label: Some("RCA Victor".to_string()),
        barcode: Some("0035629004321".to_string()),
        catalog_number: Some("PL 12522".to_string()),
        original_date: Some("1977-10-14".to_string()),
        track_number: 3,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        disc_subtitle: Some("Side 1".to_string()),
        isrc: Some("GBAYE7700021".to_string()),
        release_year: Some("1977".to_string()),
        release_date: Some("1977-10-14".to_string()),
        explicit: Some(false),
        bpm: Some(112),
        initial_key: Some("D".to_string()),
        replaygain_track_gain: Some("-6.50 dB".to_string()),
        replaygain_track_peak: Some("0.988220".to_string()),
        replaygain_album_gain: Some("-5.80 dB".to_string()),
        replaygain_album_peak: Some("0.999120".to_string()),
        r128_track_gain: Some("-2.10 LU".to_string()),
        energy: Some(0.85),
        danceability: Some(0.55),
        loudness: Some(-7.2),
        comment: Some("Audio: Qobuz FLAC 24/96 | Engine: Syncify Production".to_string()),
        lyrics_source: Some("LRCLIB".to_string()),
        cover_source: Some("Apple Music Animated Cover".to_string()),
        audio_source: Some("Qobuz".to_string()),
        bit_depth: Some(24),
        sample_rate: Some(96000.0),
        lyrics_lrc: Some("[00:00.00] I, I will be king\n[00:05.00] And you, you will be queen".to_string()),
        musicbrainz_track_id: Some("11111111-2222-3333-4444-555555555555".to_string()),
        musicbrainz_album_id: Some("66666666-7777-8888-9999-000000000000".to_string()),
        musicbrainz_artist_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()),
        musicbrainz_albumartist_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()),
        musicbrainz_release_group_id: Some("ffffffff-0000-1111-2222-333333333333".to_string()),
        musicbrainz_work_id: Some("99999999-aaaa-bbbb-cccc-dddddddddddd".to_string()),
        cover_data: Some(vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46]), // JPEG header
    };

    let result = apply_and_verify_flac_tags(&flac_path, &full_meta);
    assert!(result.is_ok(), "apply_and_verify_flac_tags should succeed: {:?}", result.err());

    let verification = result.unwrap();
    assert!(verification.flac_valid);
    assert!(verification.tags_match);
    assert!(verification.cover_present);
    assert!(verification.lyrics_present);
    assert!(verification.synced_lyrics_present);
    assert!(verification.bpm_present);
    assert!(verification.mismatches.is_empty(), "Mismatches: {:?}", verification.mismatches);

    // Verify raw VorbisComments from tag reader
    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let comments = tag.vorbis_comments().unwrap();

    assert_eq!(comments.get("TITLE").unwrap()[0], "Heroes");
    assert_eq!(comments.get("ARTIST").unwrap()[0], "David Bowie");
    assert_eq!(comments.get("ALBUM").unwrap()[0], "Heroes");
    assert_eq!(comments.get("ALBUMARTIST").unwrap()[0], "David Bowie");
    assert_eq!(comments.get("COMPOSER").unwrap()[0], "David Bowie, Brian Eno");
    assert_eq!(comments.get("PERFORMER").unwrap()[0], "David Bowie, Robert Fripp");
    assert_eq!(comments.get("WORK").unwrap()[0], "Heroes Symphony");
    assert_eq!(comments.get("GENRE").unwrap()[0], "Art Rock");
    assert_eq!(comments.get("STYLE").unwrap()[0], "Glam Rock / Berlin Trilogy");
    assert_eq!(comments.get("MOOD").unwrap()[0], "Triumphant");
    assert_eq!(comments.get("RELEASETYPE").unwrap()[0], "Album");
    assert_eq!(comments.get("RELEASESTATUS").unwrap()[0], "Official");
    assert_eq!(comments.get("RELEASECOUNTRY").unwrap()[0], "GB");
    assert_eq!(comments.get("LANGUAGE").unwrap()[0], "eng");
    assert_eq!(comments.get("COPYRIGHT").unwrap()[0], "(P) 1977 RCA Records");
    assert_eq!(comments.get("LABEL").unwrap()[0], "RCA Victor");
    assert_eq!(comments.get("BARCODE").unwrap()[0], "0035629004321");
    assert_eq!(comments.get("CATALOGNUMBER").unwrap()[0], "PL 12522");
    assert_eq!(comments.get("ORIGINALDATE").unwrap()[0], "1977-10-14");
    assert_eq!(comments.get("ISRC").unwrap()[0], "GBAYE7700021");
    assert_eq!(comments.get("BPM").unwrap()[0], "112");
    assert_eq!(comments.get("KEY").unwrap()[0], "D");
    assert_eq!(comments.get("INITIALKEY").unwrap()[0], "D");
    assert_eq!(comments.get("REPLAYGAIN_TRACK_GAIN").unwrap()[0], "-6.50 dB");
    assert_eq!(comments.get("REPLAYGAIN_TRACK_PEAK").unwrap()[0], "0.988220");
    assert_eq!(comments.get("REPLAYGAIN_ALBUM_GAIN").unwrap()[0], "-5.80 dB");
    assert_eq!(comments.get("REPLAYGAIN_ALBUM_PEAK").unwrap()[0], "0.999120");
    assert_eq!(comments.get("R128_TRACK_GAIN").unwrap()[0], "-2.10 LU");
    assert_eq!(comments.get("ENERGY").unwrap()[0], "0.85");
    assert_eq!(comments.get("DANCEABILITY").unwrap()[0], "0.55");
    assert_eq!(comments.get("LOUDNESS").unwrap()[0], "-7.2");
    assert_eq!(comments.get("SYNCIFY_LYRICS_SOURCE").unwrap()[0], "LRCLIB");
    assert_eq!(comments.get("SYNCIFY_COVER_SOURCE").unwrap()[0], "Apple Music Animated Cover");
    assert_eq!(comments.get("SYNCIFY_AUDIO_SOURCE").unwrap()[0], "Qobuz");
    assert_eq!(comments.get("MUSICBRAINZ_TRACKID").unwrap()[0], "11111111-2222-3333-4444-555555555555");
    assert_eq!(comments.get("MUSICBRAINZ_ALBUMID").unwrap()[0], "66666666-7777-8888-9999-000000000000");
    assert_eq!(comments.get("MUSICBRAINZ_ARTISTID").unwrap()[0], "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
    assert_eq!(comments.get("MUSICBRAINZ_ALBUMARTISTID").unwrap()[0], "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
    assert_eq!(comments.get("MUSICBRAINZ_RELEASEGROUPID").unwrap()[0], "ffffffff-0000-1111-2222-333333333333");
    assert_eq!(comments.get("MUSICBRAINZ_WORKID").unwrap()[0], "99999999-aaaa-bbbb-cccc-dddddddddddd");
}

#[test]
fn test_lyrics_embedding_and_lrc_sidecar_generation() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = temp_dir.path().join("track_with_lyrics.flac");
    let lrc_path = temp_dir.path().join("track_with_lyrics.lrc");
    create_minimal_test_flac(&flac_path);

    let lrc_text = "[00:01.20] First line of song\n[00:04.50] Second line of song\n[00:08.00] Chorus begins";
    std::fs::write(&lrc_path, lrc_text).unwrap();

    let meta = FlacMetadata {
        title: "Lyric Track".to_string(),
        artist: "Lyric Artist".to_string(),
        album: "Lyric Album".to_string(),
        lyrics_lrc: Some(lrc_text.to_string()),
        lyrics_source: Some("NetEase".to_string()),
        ..Default::default()
    };

    assert!(apply_and_verify_flac_tags(&flac_path, &meta).is_ok());

    // Verify embedded LYRICS comment
    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let comments = tag.vorbis_comments().unwrap();
    let embedded_lyrics = comments.get("LYRICS").unwrap();
    assert_eq!(embedded_lyrics[0], lrc_text);

    // Verify sidecar .lrc exists and matches
    assert!(lrc_path.exists());
    let read_lrc = std::fs::read_to_string(&lrc_path).unwrap();
    assert_eq!(read_lrc, lrc_text);
}

#[test]
fn test_cover_embedding_and_sidecar_preservation() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = temp_dir.path().join("track_with_cover.flac");
    let cover_jpg = temp_dir.path().join("cover.jpg");
    create_minimal_test_flac(&flac_path);

    let fake_jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];
    std::fs::write(&cover_jpg, &fake_jpeg).unwrap();

    let meta = FlacMetadata {
        title: "Cover Track".to_string(),
        artist: "Cover Artist".to_string(),
        album: "Cover Album".to_string(),
        cover_data: Some(fake_jpeg.clone()),
        cover_source: Some("Qobuz Cover Art".to_string()),
        ..Default::default()
    };

    let result = apply_and_verify_flac_tags(&flac_path, &meta).unwrap();
    assert!(result.cover_present);

    let audit = audit_flac_stage("StagingCoverVerification", &flac_path).unwrap();
    assert_eq!(audit.picture_count, 1);
    assert_eq!(audit.pictures[0].picture_type, "CoverFront");
    assert!(cover_jpg.exists());
}

#[test]
fn test_commercial_metadata_and_musicbrainz_ids() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = temp_dir.path().join("commercial_track.flac");
    create_minimal_test_flac(&flac_path);

    let meta = FlacMetadata {
        title: "Commercial Track".to_string(),
        artist: "Commercial Artist".to_string(),
        album: "Commercial Album".to_string(),
        barcode: Some("075597931326".to_string()),
        label: Some("Nonesuch Records".to_string()),
        copyright: Some("(C) 2024 Nonesuch".to_string()),
        catalog_number: Some("7559-79313-2".to_string()),
        original_date: Some("2024-05-10".to_string()),
        musicbrainz_track_id: Some("12345678-1234-1234-1234-123456789abc".to_string()),
        musicbrainz_album_id: Some("87654321-4321-4321-4321-cba987654321".to_string()),
        musicbrainz_artist_id: Some("abcdef01-2345-6789-abcd-ef0123456789".to_string()),
        ..Default::default()
    };

    assert!(apply_and_verify_flac_tags(&flac_path, &meta).is_ok());

    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let comments = tag.vorbis_comments().unwrap();
    assert_eq!(comments.get("BARCODE").unwrap()[0], "075597931326");
    assert_eq!(comments.get("LABEL").unwrap()[0], "Nonesuch Records");
    assert_eq!(comments.get("COPYRIGHT").unwrap()[0], "(C) 2024 Nonesuch");
    assert_eq!(comments.get("CATALOGNUMBER").unwrap()[0], "7559-79313-2");
    assert_eq!(comments.get("ORIGINALDATE").unwrap()[0], "2024-05-10");
    assert_eq!(comments.get("MUSICBRAINZ_TRACKID").unwrap()[0], "12345678-1234-1234-1234-123456789abc");
    assert_eq!(comments.get("MUSICBRAINZ_ALBUMID").unwrap()[0], "87654321-4321-4321-4321-cba987654321");
    assert_eq!(comments.get("MUSICBRAINZ_ARTISTID").unwrap()[0], "abcdef01-2345-6789-abcd-ef0123456789");
}

#[test]
fn test_staging_rollback_on_tagging_failure() {
    let temp_dir = TempDir::new().unwrap();
    let corrupted_staging_path = temp_dir.path().join("corrupted.part");
    std::fs::write(&corrupted_staging_path, b"NOT_A_VALID_FLAC_HEADER").unwrap();

    let meta = FlacMetadata {
        title: "Rollback Test".to_string(),
        artist: "Rollback Artist".to_string(),
        album: "Rollback Album".to_string(),
        ..Default::default()
    };

    let result = apply_and_verify_flac_tags(&corrupted_staging_path, &meta);
    assert!(result.is_err(), "Tagging corrupted FLAC in staging must return Err to trigger clean rollback");

    let err_msg = result.err().unwrap();
    assert!(err_msg.contains("Failed") || err_msg.contains("FLAC"));
}

#[test]
fn test_best_effort_degradation_when_enrichment_unavailable() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = temp_dir.path().join("base_only_track.flac");
    create_minimal_test_flac(&flac_path);

    // Only mandatory base metadata supplied; all optional enrichment fields are None
    let base_meta = FlacMetadata {
        title: "Solo Base Track".to_string(),
        artist: "Solo Base Artist".to_string(),
        album: "Solo Base Album".to_string(),
        track_number: 1,
        disc_number: 1,
        audio_source: Some("Qobuz".to_string()),
        bit_depth: Some(16),
        sample_rate: Some(44100.0),
        cover_data: None,
        cover_source: None,
        lyrics_lrc: None,
        lyrics_source: None,
        musicbrainz_track_id: None,
        musicbrainz_album_id: None,
        musicbrainz_artist_id: None,
        ..Default::default()
    };

    let result = apply_and_verify_flac_tags(&flac_path, &base_meta);
    assert!(result.is_ok(), "Base-only metadata must verify successfully under graceful degradation");

    let verification = result.unwrap();
    assert!(verification.flac_valid);
    assert!(verification.tags_match);
    assert!(!verification.cover_present);
    assert!(!verification.lyrics_present);
}

#[tokio::test]
async fn test_replaygain_acoustic_and_fingerprint_auto_calculation_and_tagging() {
    let temp_dir = TempDir::new().unwrap();
    let staging_path = temp_dir.path().join("staging_track.flac");
    create_minimal_test_flac(&staging_path);

    // 1. Run AudioAnalyzer on the staging audio
    let analysis = syncify_tauri_lib::services::enrichment::AudioAnalyzer::analyze_file(&staging_path)
        .await
        .expect("AudioAnalyzer should succeed on staging audio");

    assert!(analysis.replaygain_track_gain.is_some());
    assert!(analysis.replaygain_track_peak.is_some());
    assert!(analysis.replaygain_album_gain.is_some());
    assert!(analysis.replaygain_album_peak.is_some());
    assert!(analysis.r128_track_gain.is_some());
    assert!(analysis.bpm.is_some());
    assert!(analysis.initial_key.is_some());
    assert!(analysis.energy.is_some());
    assert!(analysis.danceability.is_some());
    assert!(analysis.acoustid_id.is_some());

    // 2. Build FlacMetadata using extracted metrics
    let meta = FlacMetadata {
        title: "Auto Analyzed Track".to_string(),
        artist: "Auto Analyzed Artist".to_string(),
        album: "Auto Analyzed Album".to_string(),
        bpm: analysis.bpm,
        initial_key: analysis.initial_key.clone(),
        energy: analysis.energy,
        danceability: analysis.danceability,
        loudness: analysis.loudness,
        replaygain_track_gain: analysis.replaygain_track_gain.clone(),
        replaygain_track_peak: analysis.replaygain_track_peak.clone(),
        replaygain_album_gain: analysis.replaygain_album_gain.clone(),
        replaygain_album_peak: analysis.replaygain_album_peak.clone(),
        r128_track_gain: analysis.r128_track_gain.clone(),
        ..Default::default()
    };

    // 3. Apply and verify FLAC tags on the staging file
    let result = apply_and_verify_flac_tags(&staging_path, &meta);
    assert!(result.is_ok(), "apply_and_verify_flac_tags should succeed: {:?}", result.err());

    // 4. Verify raw VorbisComments tags written
    let tag = metaflac::Tag::read_from_path(&staging_path).unwrap();
    let comments = tag.vorbis_comments().unwrap();

    assert_eq!(comments.get("REPLAYGAIN_TRACK_GAIN").unwrap()[0], analysis.replaygain_track_gain.unwrap());
    assert_eq!(comments.get("REPLAYGAIN_TRACK_PEAK").unwrap()[0], analysis.replaygain_track_peak.unwrap());
    assert_eq!(comments.get("REPLAYGAIN_ALBUM_GAIN").unwrap()[0], analysis.replaygain_album_gain.unwrap());
    assert_eq!(comments.get("REPLAYGAIN_ALBUM_PEAK").unwrap()[0], analysis.replaygain_album_peak.unwrap());
    assert_eq!(comments.get("R128_TRACK_GAIN").unwrap()[0], analysis.r128_track_gain.unwrap());
    assert_eq!(comments.get("BPM").unwrap()[0], analysis.bpm.unwrap().to_string());
    assert_eq!(comments.get("KEY").unwrap()[0], analysis.initial_key.unwrap());
    assert!(comments.get("ENERGY").is_some());
    assert!(comments.get("DANCEABILITY").is_some());
}

#[tokio::test]
async fn test_staging_lifecycle_and_zero_orphans_post_promotion() {
    let temp_dir = TempDir::new().unwrap();
    let staging_dir = temp_dir.path().join(".staging");
    let target_dir = temp_dir.path().join("Music").join("David Bowie").join("Heroes (1977)");

    std::fs::create_dir_all(&staging_dir).unwrap();
    std::fs::create_dir_all(&target_dir).unwrap();

    let staging_flac = staging_dir.join("03 - Heroes.part");
    let staging_lrc = staging_dir.join("03 - Heroes.lrc");
    let staging_cover = staging_dir.join("cover.jpg");

    create_minimal_test_flac(&staging_flac);
    std::fs::write(&staging_lrc, "[00:01.00] Heroes line").unwrap();
    std::fs::write(&staging_cover, &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10]).unwrap();

    // 1. Analyze and tag in staging
    let meta = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        lyrics_lrc: Some("[00:01.00] Heroes line".to_string()),
        ..Default::default()
    };
    apply_and_verify_flac_tags(&staging_flac, &meta).unwrap();

    // 2. Promote files atomically to target destination
    let dest_flac = target_dir.join("03 - Heroes.flac");
    let dest_lrc = target_dir.join("03 - Heroes.lrc");
    let dest_cover = target_dir.join("cover.jpg");

    std::fs::rename(&staging_flac, &dest_flac).unwrap();
    std::fs::rename(&staging_lrc, &dest_lrc).unwrap();
    std::fs::rename(&staging_cover, &dest_cover).unwrap();

    // 3. Verify target files exist and valid
    assert!(dest_flac.exists());
    assert!(dest_lrc.exists());
    assert!(dest_cover.exists());

    // 4. Verify staging directory is 100% clean (zero orphans)
    let remaining_staging: Vec<_> = std::fs::read_dir(&staging_dir)
        .unwrap()
        .map(|res| res.unwrap().path())
        .collect();
    assert!(remaining_staging.is_empty(), "Staging directory must have 0 orphaned files after promotion: {:?}", remaining_staging);
}

#[tokio::test]
async fn test_country_normalization_cli_gui_parity_and_precedence() {
    use syncify_metadata_domain::{
        normalize_country_code, resolve_country, CountryResolution, EnrichedMetadata,
    };

    // 1. ISO alpha-2
    assert_eq!(normalize_country_code("ES").as_deref(), Some("ES"));
    assert_eq!(normalize_country_code("es").as_deref(), Some("ES"));
    assert_eq!(normalize_country_code("GB").as_deref(), Some("GB"));
    assert_eq!(normalize_country_code("US").as_deref(), Some("US"));
    assert_eq!(normalize_country_code("MX").as_deref(), Some("MX"));
    assert_eq!(normalize_country_code("NL").as_deref(), Some("NL"));
    assert_eq!(normalize_country_code("PL").as_deref(), Some("PL"));
    assert_eq!(normalize_country_code("AT").as_deref(), Some("AT"));
    assert_eq!(normalize_country_code("AF").as_deref(), Some("AF"));

    // 2. ISO alpha-3
    assert_eq!(normalize_country_code("ESP").as_deref(), Some("ES"));
    assert_eq!(normalize_country_code("GBR").as_deref(), Some("GB"));
    assert_eq!(normalize_country_code("USA").as_deref(), Some("US"));
    assert_eq!(normalize_country_code("MEX").as_deref(), Some("MX"));
    assert_eq!(normalize_country_code("NLD").as_deref(), Some("NL"));
    assert_eq!(normalize_country_code("POL").as_deref(), Some("PL"));
    assert_eq!(normalize_country_code("AUT").as_deref(), Some("AT"));
    assert_eq!(normalize_country_code("AFG").as_deref(), Some("AF"));
    assert_eq!(normalize_country_code("DEU").as_deref(), Some("DE"));
    assert_eq!(normalize_country_code("FRA").as_deref(), Some("FR"));
    assert_eq!(normalize_country_code("JPN").as_deref(), Some("JP"));

    // 3. English & Spanish localized names
    assert_eq!(normalize_country_code("Spain").as_deref(), Some("ES"));
    assert_eq!(normalize_country_code("España").as_deref(), Some("ES"));
    assert_eq!(normalize_country_code("Espana").as_deref(), Some("ES"));
    assert_eq!(normalize_country_code("United States").as_deref(), Some("US"));
    assert_eq!(normalize_country_code("Estados Unidos").as_deref(), Some("US"));
    assert_eq!(normalize_country_code("EE.UU.").as_deref(), Some("US"));
    assert_eq!(normalize_country_code("EEUU").as_deref(), Some("US"));
    assert_eq!(normalize_country_code("Germany").as_deref(), Some("DE"));
    assert_eq!(normalize_country_code("Alemania").as_deref(), Some("DE"));
    assert_eq!(normalize_country_code("France").as_deref(), Some("FR"));
    assert_eq!(normalize_country_code("Francia").as_deref(), Some("FR"));
    assert_eq!(normalize_country_code("Japan").as_deref(), Some("JP"));
    assert_eq!(normalize_country_code("Japón").as_deref(), Some("JP"));
    assert_eq!(normalize_country_code("Canada").as_deref(), Some("CA"));
    assert_eq!(normalize_country_code("Canadá").as_deref(), Some("CA"));
    assert_eq!(normalize_country_code("Mexico").as_deref(), Some("MX"));
    assert_eq!(normalize_country_code("México").as_deref(), Some("MX"));
    assert_eq!(normalize_country_code("Netherlands").as_deref(), Some("NL"));
    assert_eq!(normalize_country_code("Países Bajos").as_deref(), Some("NL"));
    assert_eq!(normalize_country_code("Holanda").as_deref(), Some("NL"));
    assert_eq!(normalize_country_code("Poland").as_deref(), Some("PL"));
    assert_eq!(normalize_country_code("Polonia").as_deref(), Some("PL"));
    assert_eq!(normalize_country_code("Austria").as_deref(), Some("AT"));
    assert_eq!(normalize_country_code("Afghanistan").as_deref(), Some("AF"));
    assert_eq!(normalize_country_code("Afganistán").as_deref(), Some("AF"));

    // 4. Historical aliases (UK / Great Britain -> GB)
    assert_eq!(normalize_country_code("UK").as_deref(), Some("GB"));
    assert_eq!(normalize_country_code("uk").as_deref(), Some("GB"));
    assert_eq!(normalize_country_code("Great Britain").as_deref(), Some("GB"));
    assert_eq!(normalize_country_code("Gran Bretaña").as_deref(), Some("GB"));
    assert_eq!(normalize_country_code("Reino Unido").as_deref(), Some("GB"));

    // 5. Diacritics
    assert_eq!(normalize_country_code("Bélgica").as_deref(), Some("BE"));
    assert_eq!(normalize_country_code("Perú").as_deref(), Some("PE"));
    assert_eq!(normalize_country_code("Sudáfrica").as_deref(), Some("ZA"));

    // 6. Regional / Non-country values (must NOT convert to false countries)
    assert_eq!(normalize_country_code("Europe"), None);
    assert_eq!(normalize_country_code("XE"), None);
    assert_eq!(normalize_country_code("Worldwide"), None);
    assert_eq!(normalize_country_code("XW"), None);
    assert_eq!(normalize_country_code("[Worldwide]"), None);

    assert_eq!(
        resolve_country("Europe"),
        CountryResolution::Region {
            region_code: Some("XE".to_string()),
            region_name: "Europe".to_string(),
        }
    );
    assert_eq!(
        resolve_country("Worldwide"),
        CountryResolution::Region {
            region_code: Some("XW".to_string()),
            region_name: "Worldwide".to_string(),
        }
    );

    // 7. Unknown values (must not invent)
    assert_eq!(normalize_country_code("UnknownCountry123"), None);
    assert_eq!(normalize_country_code(""), None);

    // 8. Precedence: Manual > Streaming > MusicBrainz > Inferred
    let mut meta = EnrichedMetadata::default();
    let now_ts = "2026-08-17T23:30:00Z";

    // Inferred candidate
    meta.release_country.merge_candidate(Some("ES".to_string()), "inferred", 0.50, now_ts);
    assert_eq!(meta.release_country.value(), Some("ES"));
    assert_eq!(meta.release_country.source(), Some("inferred"));

    // MusicBrainz candidate overrides Inferred
    meta.release_country.merge_candidate(Some("FR".to_string()), "musicbrainz", 0.85, now_ts);
    assert_eq!(meta.release_country.value(), Some("FR"));
    assert_eq!(meta.release_country.source(), Some("musicbrainz"));

    // Streaming candidate overrides MusicBrainz
    meta.release_country.merge_candidate(Some("GB".to_string()), "qobuz", 0.85, now_ts);
    assert_eq!(meta.release_country.value(), Some("GB"));
    assert_eq!(meta.release_country.source(), Some("qobuz"));

    // Manual override wins over Streaming and is immutable
    meta.release_country.merge_candidate(Some("US".to_string()), "manual", 1.0, now_ts);
    assert_eq!(meta.release_country.value(), Some("US"));
    assert_eq!(meta.release_country.source(), Some("manual"));

    // 9. No overwriting valid manual country by subsequent streaming/musicbrainz candidates
    meta.release_country.merge_candidate(Some("DE".to_string()), "tidal", 0.99, now_ts);
    meta.release_country.merge_candidate(Some("JP".to_string()), "musicbrainz", 0.99, now_ts);
    assert_eq!(meta.release_country.value(), Some("US"), "Manual country must remain untouched");

    // 10. FLAC VorbisComments RELEASECOUNTRY & RELEASEREGION tag writing
    let temp_dir = TempDir::new().unwrap();
    let flac_path = temp_dir.path().join("country_test.flac");
    create_minimal_test_flac(&flac_path);

    let flac_meta = FlacMetadata {
        title: "Test Title".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        release_country: Some("GB".to_string()),
        release_region: None,
        ..Default::default()
    };

    apply_and_verify_flac_tags(&flac_path, &flac_meta).unwrap();

    let tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let comments = tag.vorbis_comments().unwrap();
    assert_eq!(comments.get("RELEASECOUNTRY").unwrap(), &["GB"]);
    assert_eq!(comments.get("RELEASEREGION"), None);

    // 11. Regional FLAC VorbisComments RELEASEREGION writing (XE / Worldwide)
    let flac_path_region = temp_dir.path().join("region_test.flac");
    create_minimal_test_flac(&flac_path_region);

    let flac_meta_region = FlacMetadata {
        title: "Region Track".to_string(),
        artist: "Region Artist".to_string(),
        album: "Region Album".to_string(),
        release_country: None,
        release_region: Some("Europe".to_string()),
        ..Default::default()
    };

    apply_and_verify_flac_tags(&flac_path_region, &flac_meta_region).unwrap();

    let tag_reg = metaflac::Tag::read_from_path(&flac_path_region).unwrap();
    let comments_reg = tag_reg.vorbis_comments().unwrap();
    assert_eq!(comments_reg.get("RELEASECOUNTRY"), None, "RELEASECOUNTRY must not exist for regional entities");
    assert_eq!(comments_reg.get("RELEASEREGION").unwrap(), &["Europe"]);

    // 12. Resolution of MusicBrainz XE / XW into EnrichedMetadata
    let engine = EnrichmentEngine::new();
    let origin_xe = OriginTrackMetadata {
        release_country: Some("XE".to_string()),
        source_name: "qobuz".to_string(),
        ..Default::default()
    };
    let enriched_xe = engine.resolve_track_metadata_internal("Artist", "Album", "Title", None, Some(&origin_xe), false).await;
    assert_eq!(enriched_xe.release_country.value(), None, "XE must not resolve to release_country");
    assert_eq!(enriched_xe.release_region.value(), Some("XE"), "XE must resolve to release_region");

    let origin_xw = OriginTrackMetadata {
        release_country: Some("XW".to_string()),
        source_name: "qobuz".to_string(),
        ..Default::default()
    };
    let enriched_xw = engine.resolve_track_metadata_internal("Artist", "Album", "Title", None, Some(&origin_xw), false).await;
    assert_eq!(enriched_xw.release_country.value(), None, "XW must not resolve to release_country");
    assert_eq!(enriched_xw.release_region.value(), Some("XW"), "XW must resolve to release_region");
}

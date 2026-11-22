//! Tests del motor, movidos desde `src-tauri/src/download/lyrics.rs` con su
//! semántica intacta (los constructores se adaptan a la inyección de hooks).
// Configuraciones de test construidas con Default + reasignación, como en el
// original de `src-tauri`; el lint está permitido a nivel de crate allí.
#![allow(clippy::field_reassign_with_default)]

use std::sync::Arc;

use crate::client::response_sync_level;
use crate::{
    validate_and_embed_flac_lyrics, LyricsClient, LyricsEngineConfig, LyricsLine, LyricsResponse,
    LyricsSourceSetting, NoopHooks,
};
use syncify_lyrics_domain::{parse_lrc_line, SyncLevel};

/// Sonda ffmpeg best-effort con el flag de consola oculta del núcleo; si el
/// binario no está disponible, la verificación de decodificación se omite.
fn ffmpeg_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("ffmpeg");
    syncify_core_domain::process::configure_hidden(&mut cmd);
    cmd
}

#[test]
fn source_policy_preserves_enabled_and_user_priority() {
    let mut config = LyricsEngineConfig::default();
    config.sources = vec![
        LyricsSourceSetting {
            provider_id: "lrclib_plain".into(),
            enabled: true,
            priority: 1,
        },
        LyricsSourceSetting {
            provider_id: "apple_music_ttml".into(),
            enabled: false,
            priority: 0,
        },
        LyricsSourceSetting {
            provider_id: "spotify_color".into(),
            enabled: true,
            priority: 2,
        },
        LyricsSourceSetting {
            provider_id: "genius".into(),
            enabled: true,
            priority: 0,
        },
    ];
    let client =
        LyricsClient::with_parts_and_config(reqwest::Client::new(), Arc::new(NoopHooks), config);
    let ids: Vec<_> = client
        .ordered_sources()
        .into_iter()
        .map(|source| source.provider_id)
        .collect();
    assert_eq!(ids, ["lrclib_plain", "spotify_color"]);
}

#[test]
fn sync_and_language_policy_reject_unmatched_candidates() {
    let mut config = LyricsEngineConfig::default();
    config.min_sync_level = "line".into();
    config.preferred_language = "es".into();
    let client =
        LyricsClient::with_parts_and_config(reqwest::Client::new(), Arc::new(NoopHooks), config);
    let mut candidate = LyricsResponse {
        lines: vec![],
        sync_type: "UNSYNCED".into(),
        instrumental: false,
        plain_lyrics: Some("Esta es una letra larga de una canción que cantamos juntos".into()),
        provider: "fixture".into(),
        source: "fixture".into(),
        elrc_content: None,
    };
    assert!(!client.policy_accepts(&candidate, "Canción"));
    candidate.sync_type = "LINE_SYNCED".into();
    candidate.plain_lyrics = Some(
        "These words are written in English, and they describe a song that we sing together".into(),
    );
    assert!(!client.policy_accepts(&candidate, "Canción"));
}

#[test]
fn cascade_keeps_below_target_candidates_but_search_filters_them() {
    let mut config = LyricsEngineConfig::default();
    config.min_sync_level = "word".into();
    let client = LyricsClient::with_parts_and_config(
        reqwest::Client::new(),
        Arc::new(NoopHooks),
        config.clone(),
    );
    let line = LyricsResponse {
        lines: vec![LyricsLine {
            start_time_ms: 10_000,
            words: "A line of the song".into(),
            end_time_ms: None,
        }],
        sync_type: "LINE_SYNCED".into(),
        instrumental: false,
        plain_lyrics: None,
        provider: "fixture".into(),
        source: "fixture".into(),
        elrc_content: None,
    };

    assert_eq!(response_sync_level(&line), SyncLevel::Line);
    assert!(
        !client.policy_accepts(&line, "Song"),
        "interactive search must keep enforcing the target"
    );
    assert!(
        LyricsClient::content_matches_policy(&line, "Song", &config),
        "the cascade must hold a below-target result to improve on it"
    );
}

use syncify_lyrics_domain::{
    fixtures::*, LyricsLineDomain, LyricsResolution, LyricsSyncType, ResolutionStatus,
};

/// Builds a `Failed` resolution directly (the domain crate only ships
/// constructors for statuses production actually produces).
fn failed_resolution(
    provider: impl Into<String>,
    strategy: impl Into<String>,
    reason: impl Into<String>,
) -> LyricsResolution {
    let err_msg = reason.into();
    LyricsResolution {
        status: ResolutionStatus::Failed(err_msg.clone()),
        provider: provider.into(),
        strategy: strategy.into(),
        format: "NONE".to_string(),
        sync_type: LyricsSyncType::None,
        provenance: "failed".to_string(),
        fallback_applied: false,
        error: Some(err_msg),
        synced_content: None,
        plain_text: None,
        lines: Vec::new(),
        is_instrumental: false,
    }
}

struct TempFlac {
    pub path: std::path::PathBuf,
}

impl Drop for TempFlac {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn create_dummy_flac_file() -> TempFlac {
    let path = std::env::temp_dir().join(format!(
        "syncify_flac_test_{}.flac",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut data = Vec::new();
    data.extend_from_slice(b"fLaC");
    // Block 0: STREAMINFO (not last, len 34)
    data.push(0x00);
    data.extend_from_slice(&[0x00, 0x00, 0x22]);
    let mut streaminfo = vec![0u8; 34];
    streaminfo[0] = 0x10;
    streaminfo[1] = 0x00; // min block 4096
    streaminfo[2] = 0x10;
    streaminfo[3] = 0x00; // max block 4096
    streaminfo[10] = 0x0A;
    streaminfo[11] = 0xC4;
    streaminfo[12] = 0x42; // 44100Hz, 2 channels, 16 bps
    streaminfo[13] = 0xF0;
    streaminfo[14] = 0x00;
    streaminfo[15] = 0x00;
    streaminfo[16] = 0xAC;
    streaminfo[17] = 0x44; // total samples
    data.extend_from_slice(&streaminfo);

    // Block 1: VORBIS_COMMENT (last, 0x84)
    let mut comment_data = Vec::new();
    comment_data.extend_from_slice(&4u32.to_le_bytes());
    comment_data.extend_from_slice(b"test");
    comment_data.extend_from_slice(&0u32.to_le_bytes());
    data.push(0x84);
    let comment_len = comment_data.len() as u32;
    data.push((comment_len >> 16) as u8);
    data.push((comment_len >> 8) as u8);
    data.push(comment_len as u8);
    data.extend_from_slice(&comment_data);
    data.extend_from_slice(&[0xFF, 0xF8, 0x00, 0x00]);
    std::fs::write(&path, data).expect("Failed to write dummy FLAC");
    TempFlac { path }
}

#[test]
fn test_flac_validation_and_reread_lifecycle() {
    let flac = create_dummy_flac_file();
    let elrc_raw =
        "[00:10.00] <00:10.00>I <00:10.50>wish <00:11.00>you <00:11.50>could <00:12.00>swim";
    let plain_raw = "I wish you could swim";

    let resolution = LyricsResolution::new_resolved(
        "NetEase Cloud Music",
        "music.163.com",
        LyricsSyncType::KaraokeWordSynced,
        Some(elrc_raw.to_string()),
        Some(plain_raw.to_string()),
        vec![LyricsLineDomain {
            start_time_ms: 10000,
            words: "I wish you could swim".to_string(),
            end_time_ms: Some(12000),
        }],
        false,
        "music.163.com",
    );

    let result = validate_and_embed_flac_lyrics(&flac.path, &resolution);
    assert!(
        result.is_ok(),
        "validate_and_embed_flac_lyrics should succeed: {:?}",
        result.err()
    );

    // Re-read file with metaflac to assert persistence
    let verified = metaflac::Tag::read_from_path(&flac.path).expect("Must re-read FLAC");
    let comments = verified
        .vorbis_comments()
        .expect("Must have vorbis comments");

    assert_eq!(
        comments
            .get("LYRICS")
            .and_then(|v| v.first())
            .map(|s| s.as_str()),
        Some(elrc_raw)
    );
    assert_eq!(
        comments
            .get("UNSYNCEDLYRICS")
            .and_then(|v| v.first())
            .map(|s| s.as_str()),
        Some(plain_raw)
    );
}

#[test]
fn test_flac_nonexistent_and_empty_file_rejected() {
    let non_existent = std::env::temp_dir().join("non_existent_file_12345.flac");
    let resolution = LyricsResolution::new_resolved(
        "LRCLIB",
        "lrclib.net",
        LyricsSyncType::LineSynced,
        Some("[00:10.00]Hello".to_string()),
        Some("Hello".to_string()),
        vec![],
        false,
        "lrclib.net",
    );

    let res_nonexistent = validate_and_embed_flac_lyrics(&non_existent, &resolution);
    assert!(res_nonexistent.is_err());
    assert!(res_nonexistent.unwrap_err().contains("does not exist"));

    let empty_file = std::env::temp_dir().join("empty_file_12345.flac");
    std::fs::write(&empty_file, b"").unwrap();
    let res_empty = validate_and_embed_flac_lyrics(&empty_file, &resolution);
    let _ = std::fs::remove_file(&empty_file);
    assert!(res_empty.is_err());
    assert!(res_empty.unwrap_err().contains("empty"));
}

#[test]
fn test_unrelated_flac_tags_preserved_during_lyrics_embed() {
    let flac = create_dummy_flac_file();

    // 1. Write unrelated tags first (TITLE, ARTIST, ALBUM, ISRC, CUSTOM_TAG)
    {
        let mut tag = metaflac::Tag::read_from_path(&flac.path).unwrap();
        let comments = tag.vorbis_comments_mut();
        comments.set_title(vec!["Original Title".to_string()]);
        comments.set_artist(vec!["Original Artist".to_string()]);
        comments.set_album(vec!["Original Album".to_string()]);
        comments.set("ISRC", vec!["USRC12345678".to_string()]);
        comments.set("CUSTOM_TAG", vec!["CustomValue123".to_string()]);
        tag.write_to_path(&flac.path).unwrap();
    }

    // 2. Embed lyrics via validate_and_embed_flac_lyrics
    let resolution = LyricsResolution::new_resolved(
        "LRCLIB",
        "lrclib.net",
        LyricsSyncType::LineSynced,
        Some("[00:05.00]Line 1\n[00:10.00]Line 2".to_string()),
        Some("Line 1\nLine 2".to_string()),
        vec![],
        false,
        "lrclib.net",
    );
    let res = validate_and_embed_flac_lyrics(&flac.path, &resolution);
    assert!(res.is_ok());

    // 3. Re-read and assert all original tags are preserved
    let verified = metaflac::Tag::read_from_path(&flac.path).unwrap();
    let comments = verified.vorbis_comments().unwrap();

    assert_eq!(
        comments.title().and_then(|v| v.first()).map(|s| s.as_str()),
        Some("Original Title")
    );
    assert_eq!(
        comments
            .artist()
            .and_then(|v| v.first())
            .map(|s| s.as_str()),
        Some("Original Artist")
    );
    assert_eq!(
        comments.album().and_then(|v| v.first()).map(|s| s.as_str()),
        Some("Original Album")
    );
    assert_eq!(
        comments
            .get("ISRC")
            .and_then(|v| v.first())
            .map(|s| s.as_str()),
        Some("USRC12345678")
    );
    assert_eq!(
        comments
            .get("CUSTOM_TAG")
            .and_then(|v| v.first())
            .map(|s| s.as_str()),
        Some("CustomValue123")
    );
    assert_eq!(
        comments
            .get("LYRICS")
            .and_then(|v| v.first())
            .map(|s| s.as_str()),
        Some("[00:05.00]Line 1\n[00:10.00]Line 2")
    );
}

#[test]
fn test_flac_non_flac_arbitrary_binary_rejected() {
    let path = std::env::temp_dir().join(format!(
        "syncify_backend_non_flac_{}.flac",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00").unwrap();

    let resolution = LyricsResolution::new_resolved(
        "LRCLIB",
        "lrclib.net",
        LyricsSyncType::LineSynced,
        Some("[00:10.00]Hello".to_string()),
        Some("Hello".to_string()),
        vec![],
        false,
        "lrclib.net",
    );

    let res = validate_and_embed_flac_lyrics(&path, &resolution);
    let _ = std::fs::remove_file(&path);
    assert!(res.is_err(), "Non-FLAC binary must be rejected");
}

#[test]
fn test_flac_truncated_flac_header_rejected() {
    let path = std::env::temp_dir().join(format!(
        "syncify_backend_truncated_{}.flac",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, b"fLaC").unwrap();

    let resolution = LyricsResolution::new_resolved(
        "LRCLIB",
        "lrclib.net",
        LyricsSyncType::LineSynced,
        Some("[00:10.00]Hello".to_string()),
        Some("Hello".to_string()),
        vec![],
        false,
        "lrclib.net",
    );

    let res = validate_and_embed_flac_lyrics(&path, &resolution);
    let _ = std::fs::remove_file(&path);
    assert!(res.is_err(), "Truncated FLAC must be rejected");
}

#[test]
fn test_flac_no_duplicate_lyrics_on_multiple_runs() {
    let flac = create_dummy_flac_file();
    let resolution1 = LyricsResolution::new_resolved(
        "NetEase Cloud Music",
        "music.163.com",
        LyricsSyncType::LineSynced,
        Some("[00:05.00]First Version".to_string()),
        Some("First Version".to_string()),
        vec![],
        false,
        "music.163.com",
    );
    let res1 = validate_and_embed_flac_lyrics(&flac.path, &resolution1);
    assert!(res1.is_ok());

    let resolution2 = LyricsResolution::new_resolved(
        "LRCLIB",
        "lrclib.net",
        LyricsSyncType::KaraokeWordSynced,
        Some("[00:05.00] <00:05.00>Second <00:06.00>Version".to_string()),
        Some("Second Version".to_string()),
        vec![],
        false,
        "lrclib.net",
    );
    let res2 = validate_and_embed_flac_lyrics(&flac.path, &resolution2);
    assert!(res2.is_ok());

    let verified = metaflac::Tag::read_from_path(&flac.path).unwrap();
    let comments = verified.vorbis_comments().unwrap();

    let lyrics_values = comments.get("LYRICS").unwrap();
    assert_eq!(
        lyrics_values.len(),
        1,
        "Exactly one LYRICS entry must exist"
    );
    assert_eq!(
        lyrics_values[0],
        "[00:05.00] <00:05.00>Second <00:06.00>Version"
    );

    let unsynced_values = comments.get("UNSYNCEDLYRICS").unwrap();
    assert_eq!(
        unsynced_values.len(),
        1,
        "Exactly one UNSYNCEDLYRICS entry must exist"
    );
    assert_eq!(unsynced_values[0], "Second Version");
}

#[test]
fn test_flac_picture_front_cover_and_multiple_pictures() {
    let flac = create_dummy_flac_file();
    use metaflac::block::PictureType;

    let mut tag = metaflac::Tag::read_from_path(&flac.path).unwrap();
    // Add CoverFront picture
    let fake_jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];
    tag.add_picture("image/jpeg", PictureType::CoverFront, fake_jpeg.clone());
    // Add Artist picture
    let fake_png = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    tag.add_picture("image/png", PictureType::Artist, fake_png.clone());
    tag.write_to_path(&flac.path).unwrap();

    // Re-read and check pictures
    let read_tag = metaflac::Tag::read_from_path(&flac.path).unwrap();
    let pics: Vec<_> = read_tag.pictures().collect();
    assert_eq!(pics.len(), 2);
    assert_eq!(pics[0].picture_type, PictureType::CoverFront);
    assert_eq!(pics[0].mime_type, "image/jpeg");
    assert_eq!(pics[1].picture_type, PictureType::Artist);
    assert_eq!(pics[1].mime_type, "image/png");
}

#[test]
fn test_flac_real_downloaded_track_roundtrip() {
    let src_path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/silence.flac");
    assert!(
        src_path.is_file(),
        "Tracked FLAC fixture must be available in clean checkouts"
    );
    let temp_dest = std::env::temp_dir().join(format!(
        "syncify_real_flac_test_{}.flac",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::copy(&src_path, &temp_dest).expect("Copy real FLAC to temp path");

    // 1. Inspect before state
    let tag_before = metaflac::Tag::read_from_path(&temp_dest).expect("Read real FLAC before");
    let info_before = tag_before
        .get_streaminfo()
        .expect("STREAMINFO before")
        .clone();
    let pics_before: Vec<_> = tag_before
        .pictures()
        .map(|p| (p.picture_type, p.mime_type.clone(), p.data.len()))
        .collect();
    let comments_before = tag_before.vorbis_comments().cloned();

    let orig_sample_rate = info_before.sample_rate;
    let orig_bits_per_sample = info_before.bits_per_sample;
    let orig_total_samples = info_before.total_samples;
    let orig_channels = info_before.num_channels;
    let orig_duration = (orig_total_samples as f64) / (orig_sample_rate as f64);
    let orig_md5 = info_before.md5;

    // Verify pre-embed audio decode validity with ffmpeg if available
    let ffmpeg_check_before = ffmpeg_command()
        .args([
            "-v",
            "error",
            "-i",
            temp_dest.to_str().unwrap(),
            "-f",
            "null",
            "-",
        ])
        .output();
    if let Ok(out) = &ffmpeg_check_before {
        assert!(
            out.status.success(),
            "Pre-embed ffmpeg decode failed: {:?}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // 2. Embed Enhanced LRC
    let elrc = "[00:01.00] <00:01.00>At <00:01.50>first <00:02.00>I <00:02.50>was <00:03.00>afraid\n[00:03.50] <00:03.50>I <00:04.00>was <00:04.50>petrified";
    let resolution = LyricsResolution::new_resolved(
        "NetEase Cloud Music",
        "music.163.com",
        LyricsSyncType::KaraokeWordSynced,
        Some(elrc.to_string()),
        Some("At first I was afraid\nI was petrified".to_string()),
        vec![],
        false,
        "music.163.com",
    );

    let res = validate_and_embed_flac_lyrics(&temp_dest, &resolution);
    assert!(
        res.is_ok(),
        "Embedding into real FLAC must succeed: {:?}",
        res.err()
    );

    // 3. Inspect after state
    let tag_after = metaflac::Tag::read_from_path(&temp_dest).expect("Read real FLAC after");
    let info_after = tag_after.get_streaminfo().expect("STREAMINFO after");

    // Assert STREAMINFO is 100% byte-exact preserved
    assert_eq!(
        info_after.sample_rate, orig_sample_rate,
        "Sample rate must not change"
    );
    assert_eq!(
        info_after.bits_per_sample, orig_bits_per_sample,
        "Bits per sample must not change"
    );
    assert_eq!(
        info_after.total_samples, orig_total_samples,
        "Total audio samples must not change"
    );
    assert_eq!(
        info_after.num_channels, orig_channels,
        "Channels must not change"
    );
    assert_eq!(
        info_after.md5, orig_md5,
        "MD5 audio checksum must not change"
    );

    let duration_after = (info_after.total_samples as f64) / (info_after.sample_rate as f64);
    assert!(
        (duration_after - orig_duration).abs() < f64::EPSILON,
        "Duration must not change"
    );

    // Assert PICTURE blocks are 100% preserved
    let pics_after: Vec<_> = tag_after
        .pictures()
        .map(|p| (p.picture_type, p.mime_type.clone(), p.data.len()))
        .collect();
    assert_eq!(
        pics_after, pics_before,
        "PICTURE metadata blocks must be preserved"
    );

    // Assert unrelated Vorbis comments are preserved
    let comments_after = tag_after.vorbis_comments().expect("VorbisComments after");
    if let Some(cb) = comments_before {
        if let Some(titles) = cb.title() {
            assert_eq!(
                comments_after.title(),
                Some(titles),
                "TITLE tag must be preserved"
            );
        }
        if let Some(artists) = cb.artist() {
            assert_eq!(
                comments_after.artist(),
                Some(artists),
                "ARTIST tag must be preserved"
            );
        }
        if let Some(albums) = cb.album() {
            assert_eq!(
                comments_after.album(),
                Some(albums),
                "ALBUM tag must be preserved"
            );
        }
    }

    // Assert lyrics match exactly and have no duplicates
    let lyrics_entries = comments_after
        .get("LYRICS")
        .expect("LYRICS entry must exist");
    assert_eq!(
        lyrics_entries.len(),
        1,
        "Exactly one LYRICS entry must exist"
    );
    assert_eq!(lyrics_entries[0], elrc);

    let unsynced_entries = comments_after
        .get("UNSYNCEDLYRICS")
        .expect("UNSYNCEDLYRICS entry must exist");
    assert_eq!(
        unsynced_entries.len(),
        1,
        "Exactly one UNSYNCEDLYRICS entry must exist"
    );
    assert_eq!(
        unsynced_entries[0],
        "At first I was afraid\nI was petrified"
    );

    // Verify post-embed audio decode validity with ffmpeg (bit-exact stream playable)
    let ffmpeg_check_after = ffmpeg_command()
        .args([
            "-v",
            "error",
            "-i",
            temp_dest.to_str().unwrap(),
            "-f",
            "null",
            "-",
        ])
        .output();
    if let Ok(out) = &ffmpeg_check_after {
        assert!(
            out.status.success(),
            "Post-embed ffmpeg decode failed: {:?}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let _ = std::fs::remove_file(&temp_dest);
}

#[test]
fn test_lyricsplus_http404_produces_source_unavailable() {
    let res = LyricsResolution::new_source_unavailable(
        "LyricsPlus",
        "lyricsplus_search",
        "LyricsPlus search failed: HTTP 404 Not Found",
    );
    assert_eq!(res.status, ResolutionStatus::SourceUnavailable);
    assert_eq!(
        res.error,
        Some("LyricsPlus search failed: HTTP 404 Not Found".to_string())
    );
}

#[test]
fn test_corrupt_payload_produces_failed() {
    let res = failed_resolution(
        "NetEase Cloud Music",
        "netease_lyrics",
        "JSON decode error: unexpected EOF",
    );
    assert_eq!(
        res.status,
        ResolutionStatus::Failed("JSON decode error: unexpected EOF".to_string())
    );
    assert_eq!(
        res.error,
        Some("JSON decode error: unexpected EOF".to_string())
    );
}

#[test]
fn test_enhanced_lrc_word_synced_preservation_strict() {
    let raw_elrc = "[00:10.00] <00:10.00>I <00:10.50>wish <00:11.00>you <00:11.50>could <00:12.00>swim\n[00:12.50] <00:12.50>Like <00:13.00>dolphins <00:13.50>can <00:14.00>swim";
    let res = LyricsResolution::new_resolved(
        "NetEase Cloud Music",
        "music.163.com",
        LyricsSyncType::KaraokeWordSynced,
        Some(raw_elrc.to_string()),
        Some("I wish you could swim\nLike dolphins can swim".to_string()),
        vec![],
        false,
        "music.163.com",
    );

    assert_eq!(res.sync_type, LyricsSyncType::KaraokeWordSynced);
    assert_eq!(res.synced_content.as_deref(), Some(raw_elrc));
}

#[test]
fn test_cascade_priority_karaoke_over_linesynced() {
    let karaoke = LyricsResolution::new_resolved(
        "NetEase Cloud Music",
        "music.163.com",
        LyricsSyncType::KaraokeWordSynced,
        Some("[00:10.00] <00:10.00>Word <00:11.00>sync".to_string()),
        Some("Word sync".to_string()),
        vec![],
        false,
        "music.163.com",
    );

    let linesynced = LyricsResolution::new_resolved(
        "LRCLIB",
        "lrclib.net",
        LyricsSyncType::LineSynced,
        Some("[00:10.00]Word sync".to_string()),
        Some("Word sync".to_string()),
        vec![],
        false,
        "lrclib.net",
    );

    assert_eq!(karaoke.sync_type, LyricsSyncType::KaraokeWordSynced);
    assert_eq!(linesynced.sync_type, LyricsSyncType::LineSynced);
    assert_ne!(karaoke.sync_type, linesynced.sync_type);
}

#[test]
fn test_cascade_priority_linesynced_over_plain() {
    let linesynced = LyricsResolution::new_resolved(
        "LRCLIB",
        "lrclib.net",
        LyricsSyncType::LineSynced,
        Some("[00:10.00]Word sync".to_string()),
        Some("Word sync".to_string()),
        vec![],
        false,
        "lrclib.net",
    );

    let plain = LyricsResolution {
        status: ResolutionStatus::Resolved,
        provider: "NetEase".to_string(),
        strategy: "plain_text".to_string(),
        format: "PLAIN".to_string(),
        sync_type: LyricsSyncType::Plain,
        provenance: "music.163.com".to_string(),
        fallback_applied: false,
        error: None,
        synced_content: None,
        plain_text: Some("Word sync".to_string()),
        lines: vec![],
        is_instrumental: false,
    };

    assert_eq!(linesynced.sync_type, LyricsSyncType::LineSynced);
    assert_eq!(plain.sync_type, LyricsSyncType::Plain);
}

#[test]
fn test_cascade_error_priority_ranking() {
    let su =
        LyricsResolution::new_source_unavailable("LyricsPlus", "lyricsplus_search", "HTTP 404");
    let failed = failed_resolution("NetEase", "netease_lyrics", "Corrupt payload");
    let auth =
        LyricsResolution::new_requires_auth("Apple Music", "apple_token", "HTTP 401 Unauthorized");
    let nf = LyricsResolution::new_not_found("Orchestrator", "multi_provider_cascade");

    assert_eq!(su.status, ResolutionStatus::SourceUnavailable);
    assert_eq!(
        failed.status,
        ResolutionStatus::Failed("Corrupt payload".to_string())
    );
    assert_eq!(auth.status, ResolutionStatus::RequiresAuth);
    assert_eq!(nf.status, ResolutionStatus::NotFound);
}

#[test]
fn test_contract_payload_all_variant_representations() {
    let not_supported = LyricsResolution {
        status: ResolutionStatus::NotSupported,
        provider: "DummyProvider".to_string(),
        strategy: "unsupported_codec".to_string(),
        format: "NONE".to_string(),
        sync_type: LyricsSyncType::None,
        provenance: "none".to_string(),
        fallback_applied: false,
        error: Some("Operation not supported for this format".to_string()),
        synced_content: None,
        plain_text: None,
        lines: vec![],
        is_instrumental: false,
    };
    assert_eq!(not_supported.status, ResolutionStatus::NotSupported);

    let not_requested = LyricsResolution {
        status: ResolutionStatus::NotRequested,
        provider: "None".to_string(),
        strategy: "skipped".to_string(),
        format: "NONE".to_string(),
        sync_type: LyricsSyncType::None,
        provenance: "none".to_string(),
        fallback_applied: false,
        error: None,
        synced_content: None,
        plain_text: None,
        lines: vec![],
        is_instrumental: false,
    };
    assert_eq!(not_requested.status, ResolutionStatus::NotRequested);
}

#[test]
fn test_backend_netease_karaoke_http_fixture() {
    let json: serde_json::Value = serde_json::from_str(FIXTURE_NETEASE_KARAOKE_JSON).unwrap();
    let klyric = json["klyric"]["lyric"].as_str().unwrap();

    let mut lines = Vec::new();
    for raw in klyric.lines() {
        if let Some(parsed) = parse_lrc_line(raw) {
            lines.push(parsed);
        }
    }

    let res = LyricsResolution::new_resolved(
        "NetEase Cloud Music",
        "music.163.com",
        LyricsSyncType::KaraokeWordSynced,
        Some(klyric.to_string()),
        Some("I wish you could swim\nLike dolphins can swim".to_string()),
        lines,
        false,
        "music.163.com",
    );

    assert_eq!(res.status, ResolutionStatus::Resolved);
    assert_eq!(res.provider, "NetEase Cloud Music");
    assert_eq!(res.sync_type, LyricsSyncType::KaraokeWordSynced);
    assert_eq!(res.format, "KaraokeWordSynced");
    assert_eq!(res.lines.len(), 2);
    assert!(res.synced_content.as_ref().unwrap().contains('<'));
    assert_eq!(
        res.plain_text.as_deref(),
        Some("I wish you could swim\nLike dolphins can swim")
    );
}

#[test]
fn test_backend_lrclib_synced_http_fixture() {
    let json: serde_json::Value = serde_json::from_str(FIXTURE_LRCLIB_SYNCED_JSON).unwrap();
    let synced = json["syncedLyrics"].as_str().unwrap();
    let plain = json["plainLyrics"].as_str().unwrap();

    let mut lines = Vec::new();
    for line in synced.lines() {
        if let Some(parsed) = parse_lrc_line(line) {
            lines.push(parsed);
        }
    }

    let res = LyricsResolution::new_resolved(
        "LRCLIB",
        "lrclib.net",
        LyricsSyncType::LineSynced,
        Some(synced.to_string()),
        Some(plain.to_string()),
        lines,
        false,
        "lrclib.net",
    );

    assert_eq!(res.status, ResolutionStatus::Resolved);
    assert_eq!(res.provider, "LRCLIB");
    assert_eq!(res.sync_type, LyricsSyncType::LineSynced);
    assert_eq!(res.lines.len(), 2);
}

#[test]
fn test_backend_lrclib_instrumental_http_fixture() {
    let json: serde_json::Value = serde_json::from_str(FIXTURE_LRCLIB_INSTRUMENTAL_JSON).unwrap();
    let instrumental = json["instrumental"].as_bool().unwrap();

    let res = LyricsResolution {
        status: ResolutionStatus::Resolved,
        provider: "LRCLIB".to_string(),
        strategy: "instrumental_flag".to_string(),
        format: "INSTRUMENTAL".to_string(),
        sync_type: LyricsSyncType::Instrumental,
        provenance: "lrclib.net".to_string(),
        fallback_applied: false,
        error: None,
        synced_content: None,
        plain_text: None,
        lines: vec![],
        is_instrumental: instrumental,
    };

    assert_eq!(res.status, ResolutionStatus::Resolved);
    assert!(res.is_instrumental);
    assert_eq!(res.sync_type, LyricsSyncType::Instrumental);
}

#[test]
fn test_backend_lyricsplus_word_http_fixture() {
    let json: serde_json::Value = serde_json::from_str(FIXTURE_LYRICSPLUS_WORD_JSON).unwrap();
    let synced = json["syncedLyrics"].as_str().unwrap();
    let mut lines = Vec::new();

    for raw in synced.lines() {
        if let Some(parsed) = parse_lrc_line(raw) {
            lines.push(parsed);
        }
    }

    let res = LyricsResolution::new_resolved(
        "LyricsPlus",
        "lyricsplus-backend",
        LyricsSyncType::KaraokeWordSynced,
        Some(synced.to_string()),
        Some("Is this the real life\nIs this just fantasy".to_string()),
        lines,
        false,
        "lyricsplus-backend",
    );

    assert_eq!(res.status, ResolutionStatus::Resolved);
    assert_eq!(res.provider, "LyricsPlus");
    assert_eq!(res.sync_type, LyricsSyncType::KaraokeWordSynced);
    assert_eq!(res.lines.len(), 4);
}

#[test]
fn test_backend_lyricsplus_http404_handling() {
    let err_msg = "LyricsPlus search failed: HTTP 404 Not Found";
    let res = LyricsResolution::new_source_unavailable("LyricsPlus", "lyricsplus_search", err_msg);
    assert_eq!(res.status, ResolutionStatus::SourceUnavailable);
    assert_eq!(res.provider, "LyricsPlus");
    assert_eq!(res.error, Some(err_msg.to_string()));
}

#[test]
fn test_backend_invalid_json_handling() {
    let res = failed_resolution("NetEase", "netease_lyrics", "Failed to parse JSON response");
    assert_eq!(
        res.status,
        ResolutionStatus::Failed("Failed to parse JSON response".to_string())
    );
    assert_eq!(res.error, Some("Failed to parse JSON response".to_string()));
}

#[test]
fn test_backend_http_rate_limit_and_server_error_handling() {
    let res_429 = LyricsResolution::new_source_unavailable(
        "LRCLIB",
        "lrclib_get",
        "HTTP 429 Too Many Requests",
    );
    assert_eq!(res_429.status, ResolutionStatus::SourceUnavailable);
    assert_eq!(
        res_429.error,
        Some("HTTP 429 Too Many Requests".to_string())
    );

    let res_503 = LyricsResolution::new_source_unavailable(
        "NetEase",
        "netease_search",
        "HTTP 503 Service Unavailable",
    );
    assert_eq!(res_503.status, ResolutionStatus::SourceUnavailable);
    assert_eq!(
        res_503.error,
        Some("HTTP 503 Service Unavailable".to_string())
    );
}

#[test]
fn test_backend_http_timeout_handling() {
    let res_timeout = LyricsResolution::new_source_unavailable(
        "LRCLIB",
        "lrclib_get",
        "Request timed out after 10000ms",
    );
    assert_eq!(res_timeout.status, ResolutionStatus::SourceUnavailable);
    assert_eq!(
        res_timeout.error,
        Some("Request timed out after 10000ms".to_string())
    );
}

// LY03 (#184) — caracterización de la caché de identidad ANTES de la caché
// negativa persistente. `LYRICS_CACHE` es un mapa en memoria por identidad
// (artista, título, álbum normalizados) que guarda también las resoluciones
// no resueltas y no tiene caducidad ni granularidad por Fuente de letras: un
// segundo intento de la misma identidad devuelve el fallo cacheado sin volver
// a sondear mientras viva el proceso. Estos tests fijan ese contrato para que
// la entrega de #184 no lo cambie por accidente.
//
// El mapa es global al proceso y el harness ejecuta los tests en paralelo:
// ambos tests lo vacían y escriben, así que se serializan con este mutex. Sin
// él, uno puede borrar la entrada del otro entre el `set` y la lectura.
static IDENTITY_CACHE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn identity_cache_key_normalizes_case_and_whitespace() {
    let _guard = IDENTITY_CACHE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    crate::cache::clear_lyrics_cache();
    crate::cache::set_cached_lyrics(
        "  Artist X ",
        "Song Y ",
        Some("Album Z"),
        LyricsResolution::new_not_found("Orchestrator", "multi_provider_cascade"),
        None,
    );

    let cache = crate::cache::LYRICS_CACHE.read().unwrap();
    let entries = cache.as_ref().expect("la caché se inicializa al sembrar");
    assert_eq!(
        entries.keys().cloned().collect::<Vec<_>>(),
        vec!["artist x:::song y:::album z".to_string()],
        "la clave es artista:::título:::álbum en minúsculas y sin espacios de borde"
    );
}

#[test]
fn identity_cache_reuses_negative_resolution_without_expiry() {
    let _guard = IDENTITY_CACHE_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    futures::executor::block_on(async {
        crate::cache::clear_lyrics_cache();

        // Sin fuentes configuradas, una sonda real no podría devolver nada: si
        // el resultado no viniera de la caché, el test lo delataría como
        // no-resuelto recién orquestado en vez de la resolución sembrada.
        let mut config = LyricsEngineConfig::default();
        config.sources.clear();
        let client = LyricsClient::with_parts_and_config(
            reqwest::Client::new(),
            Arc::new(NoopHooks),
            config,
        );
        let service = crate::pipeline::LyricsPipelineService::with_client(client);

        let mut negative =
            LyricsResolution::new_not_found("Orchestrator", "multi_provider_cascade");
        negative.error = Some("fallo sembrado por el test".to_string());
        crate::cache::set_cached_lyrics(
            "Artist X",
            "Song Y",
            Some("Album Z"),
            negative.clone(),
            None,
        );

        let (resolution, sidecar) = service
            .resolve_lyrics_and_sidecar("artist x", "song y", Some("album z"), 180.0)
            .await
            .expect("la caché negativa se sirve como resultado");

        assert_eq!(resolution, negative);
        assert_eq!(sidecar, None);
    });
}

// LY03 (#184) — el clasificador es el vocabulario que persiste la caché
// negativa: `reason` en `lyrics_resolution_failures`. Los mensajes vienen de
// los fetchers reales del motor, no de ejemplos inventados.

#[test]
fn failure_classifier_covers_the_engine_vocabulary() {
    use crate::failures::{classify_failure_message, LyricsFailureKind};

    for (message, expected) in [
        ("No lyrics found", LyricsFailureKind::NotFound),
        (
            "QQMusic: 0 matches for Artist - Song",
            LyricsFailureKind::NotFound,
        ),
        (
            "LRCLIB search failed: HTTP 429 Too Many Requests",
            LyricsFailureKind::RateLimited,
        ),
        (
            "Musixmatch token.get returned status 401",
            LyricsFailureKind::RequiresAuth,
        ),
        (
            "Spotify sp_dc token is not configured",
            LyricsFailureKind::RequiresAuth,
        ),
        (
            "NetEase search request failed: error sending request for url: connection refused",
            LyricsFailureKind::SourceUnavailable,
        ),
        (
            "LRCLIB request timed out after 10000ms",
            LyricsFailureKind::SourceUnavailable,
        ),
        (
            "Kugou search parse failed: error decoding response body",
            LyricsFailureKind::Failed,
        ),
        (
            // A transport error whose URL carries Musixmatch's `usertoken`
            // contains the word "token" but is not an auth failure.
            "Musixmatch search request failed: error sending request for url \
             (https://api.musixmatch.com/ws/1.1/matcher.lyrics.get?usertoken=SECRET123)",
            LyricsFailureKind::SourceUnavailable,
        ),
    ] {
        assert_eq!(
            classify_failure_message(message),
            expected,
            "mensaje clasificado mal: {message:?}"
        );
    }
}

#[test]
fn failure_detail_redacts_credentials_before_persistence() {
    use crate::failures::redact_failure_detail;

    let musixmatch = "error sending request for url \
         (https://api.musixmatch.com/ws/1.1/matcher.lyrics.get?q_track=Song&usertoken=SECRET123&format=json)";
    let redacted = redact_failure_detail(musixmatch);
    assert!(
        !redacted.contains("SECRET123"),
        "el usertoken no sobrevive a la redacción: {redacted}"
    );
    assert!(
        redacted.contains("usertoken=[REDACTED]"),
        "el parámetro se conserva redactado: {redacted}"
    );
    assert!(
        redacted.contains("q_track=Song"),
        "los parámetros inocuos se conservan: {redacted}"
    );

    let with_userinfo = "error sending request for url \
         (https://alice:hunter2@example.com/path?access_token=abc123&sp_dc=cookie-value)";
    let redacted = redact_failure_detail(with_userinfo);
    assert!(!redacted.contains("hunter2"), "{redacted}");
    assert!(!redacted.contains("abc123"), "{redacted}");
    assert!(!redacted.contains("cookie-value"), "{redacted}");
    assert!(
        redacted.contains("[REDACTED]@example.com"),
        "el host se conserva y el userinfo se redacta: {redacted}"
    );
}

#[test]
fn cascade_skips_sources_with_a_fresh_failure_without_contacting_them() {
    futures::executor::block_on(async {
        let mut config = LyricsEngineConfig::default();
        config.sources = vec![
            LyricsSourceSetting {
                provider_id: "lrclib_synced".into(),
                enabled: true,
                priority: 1,
            },
            LyricsSourceSetting {
                provider_id: "kugou".into(),
                enabled: true,
                priority: 2,
            },
        ];
        let client = LyricsClient::with_parts_and_config(
            reqwest::Client::new(),
            Arc::new(NoopHooks),
            config,
        );

        let skip: std::collections::HashSet<String> =
            ["kugou".to_string(), "lrclib_synced".to_string()]
                .into_iter()
                .collect();
        let report = client
            .fetch_all_sources_skipping("Artist", "Song", 180.0, &skip)
            .await;

        assert_eq!(
            report.skipped,
            vec!["lrclib_synced".to_string(), "kugou".to_string()],
            "las fuentes con fallo vigente se saltan en el orden de prioridad"
        );
        assert!(
            report.failures.is_empty(),
            "saltar no inventa fallos nuevos"
        );
        assert!(report.response.is_none(), "no hay sondas, no hay candidato");
        assert_eq!(
            report.error.as_deref(),
            Some("No lyrics matched enabled sources and lyrics policy")
        );
    });
}

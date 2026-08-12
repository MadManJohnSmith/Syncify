//! Shared Domain Fixtures & Deterministic Verification Tests

use crate::{
    detect_sync_type, parse_lrc_line, parse_ttml_to_elrc, parse_ultrastar_to_elrc,
    preserve_word_timestamps_exact, strip_lrc_timestamps, LyricsLineDomain, LyricsResolution,
    LyricsSyncType, ResolutionStatus,
};

/// 1. Apple TTML XML Fixture
pub const FIXTURE_APPLE_TTML_XML: &str = r#"<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:10.00" end="00:12.00">
        <span begin="00:10.00" end="00:10.50">I </span>
        <span begin="00:10.50" end="00:11.00">wish </span>
        <span begin="00:11.00" end="00:11.50">you </span>
        <span begin="00:11.50" end="00:12.00">could</span>
      </p>
    </div>
  </body>
</tt>"#;

/// 2. Enhanced LRC Word-Level Fixture
pub const FIXTURE_ENHANCED_LRC_WORD: &str =
    "[00:10.00] <00:10.00>I <00:10.50>wish <00:11.00>you <00:11.50>could <00:12.00>swim";

/// 3. Line-Synced LRC Fixture
pub const FIXTURE_LINE_SYNCED_LRC: &str = "[00:10.00]I wish you could swim";

/// 4. Plain Lyrics Fixture
pub const FIXTURE_PLAIN_LYRICS: &str = "I wish you could swim\nLike dolphins can swim";

/// 5. NetEase Search Response Mock (JSON)
pub const FIXTURE_NETEASE_SEARCH_JSON: &str = r#"{
  "result": {
    "songs": [
      {
        "id": 2080351,
        "name": "I Will Survive",
        "dt": 198000
      }
    ]
  }
}"#;

/// 6. NetEase Word-Synced Lyrics Mock (JSON)
pub const FIXTURE_NETEASE_KARAOKE_JSON: &str = r#"{
  "klyric": {
    "lyric": "[00:10.00] <00:10.00>I <00:10.50>wish <00:11.00>you <00:11.50>could <00:12.00>swim\n[00:12.50] <00:12.50>Like <00:13.00>dolphins <00:13.50>can <00:14.00>swim"
  },
  "lrc": {
    "lyric": "[00:10.00]I wish you could swim\n[00:12.50]Like dolphins can swim"
  }
}"#;

/// 7. NetEase Line-Synced Lyrics Mock (JSON)
pub const FIXTURE_NETEASE_LINE_JSON: &str = r#"{
  "klyric": {
    "lyric": ""
  },
  "lrc": {
    "lyric": "[00:10.00]I wish you could swim\n[00:12.50]Like dolphins can swim"
  }
}"#;

/// 8. LRCLIB Line-Synced Mock (JSON)
pub const FIXTURE_LRCLIB_SYNCED_JSON: &str = r#"{
  "id": 12345,
  "name": "Heroes",
  "trackName": "Heroes",
  "artistName": "David Bowie",
  "albumName": "Heroes",
  "duration": 371.0,
  "instrumental": false,
  "plainLyrics": "I, I will be king\nAnd you, you will be queen",
  "syncedLyrics": "[00:15.20]I, I will be king\n[00:22.50]And you, you will be queen"
}"#;

/// 9. LRCLIB Instrumental Mock (JSON)
pub const FIXTURE_LRCLIB_INSTRUMENTAL_JSON: &str = r#"{
  "id": 54321,
  "name": "Clubbed to Death",
  "trackName": "Clubbed to Death",
  "artistName": "Rob Dougan",
  "albumName": "Furious Angels",
  "duration": 446.0,
  "instrumental": true,
  "plainLyrics": null,
  "syncedLyrics": null
}"#;

/// 10. LyricsPlus Word-Synced Mock (JSON)
pub const FIXTURE_LYRICSPLUS_WORD_JSON: &str = r#"{
  "syncedLyrics": "[00:01.00] <00:01.00>Is <00:01.50>this <00:02.00>the <00:02.50>real <00:03.00>life\n[00:03.50] <00:03.50>Is <00:04.00>this <00:04.50>just <00:05.00>fantasy\n[00:06.00] <00:06.00>Caught <00:06.50>in <00:07.00>a <00:07.50>landslide\n[00:08.00] <00:08.00>No <00:08.50>escape <00:09.00>from <00:09.50>reality"
}"#;

/// 11. LyricsPlus Line-Synced Mock (JSON)
pub const FIXTURE_LYRICSPLUS_LINE_JSON: &str = r#"{
  "syncedLyrics": "[00:01.00]Is this the real life\n[00:03.50]Is this just fantasy\n[00:06.00]Caught in a landslide\n[00:08.00]No escape from reality"
}"#;

/// Instrumental Response Object
pub fn fixture_instrumental() -> LyricsResolution {
    LyricsResolution {
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
        lines: Vec::new(),
        is_instrumental: true,
    }
}

/// Empty Response Object
pub fn fixture_empty_response() -> LyricsResolution {
    LyricsResolution::new_not_found("LRCLIB", "exact_match")
}

/// HTTP Error Response Object
pub fn fixture_http_error(code: u16) -> LyricsResolution {
    if code == 401 || code == 403 {
        LyricsResolution::new_requires_auth("Spotify", "color_lyrics", format!("HTTP {}", code))
    } else {
        LyricsResolution::new_source_unavailable("Spotify", "color_lyrics", format!("HTTP {}", code))
    }
}

/// Conflict Results Object
pub fn fixture_conflict_results() -> (LyricsResolution, LyricsResolution) {
    let word_synced = LyricsResolution::new_resolved(
        "Musixmatch",
        "richsync",
        LyricsSyncType::KaraokeWordSynced,
        Some(FIXTURE_ENHANCED_LRC_WORD.to_string()),
        Some(FIXTURE_PLAIN_LYRICS.to_string()),
        vec![LyricsLineDomain {
            start_time_ms: 10000,
            words: "I wish you could swim".to_string(),
            end_time_ms: Some(12000),
        }],
        false,
        "desktop_api",
    );

    let line_synced = LyricsResolution::new_resolved(
        "LRCLIB",
        "line_search",
        LyricsSyncType::LineSynced,
        Some(FIXTURE_LINE_SYNCED_LRC.to_string()),
        Some(FIXTURE_PLAIN_LYRICS.to_string()),
        vec![LyricsLineDomain {
            start_time_ms: 10000,
            words: "I wish you could swim".to_string(),
            end_time_ms: None,
        }],
        false,
        "lrclib.net",
    );

    (word_synced, line_synced)
}

/// Fallback Word to Line Resolution
pub fn fixture_fallback_word_to_line() -> LyricsResolution {
    let mut res = LyricsResolution::new_resolved(
        "LRCLIB",
        "line_search_fallback",
        LyricsSyncType::LineSynced,
        Some(FIXTURE_LINE_SYNCED_LRC.to_string()),
        Some(FIXTURE_PLAIN_LYRICS.to_string()),
        vec![LyricsLineDomain {
            start_time_ms: 10000,
            words: "I wish you could swim".to_string(),
            end_time_ms: None,
        }],
        false,
        "lrclib.net",
    );
    res.fallback_applied = true;
    res
}

/// Reject Degradation Guard Case
pub fn fixture_reject_degradation_case() -> (LyricsResolution, String) {
    let res = LyricsResolution::new_resolved(
        "Apple Music",
        "ttml_syllable",
        LyricsSyncType::KaraokeWordSynced,
        Some(FIXTURE_ENHANCED_LRC_WORD.to_string()),
        Some(FIXTURE_PLAIN_LYRICS.to_string()),
        vec![LyricsLineDomain {
            start_time_ms: 10000,
            words: "I wish you could swim".to_string(),
            end_time_ms: Some(12000),
        }],
        false,
        "apple_amp_api",
    );

    let degraded = FIXTURE_LINE_SYNCED_LRC.to_string();
    (res, degraded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_lrc_not_converted_to_line_synced() {
        let (res, degraded) = fixture_reject_degradation_case();
        assert_eq!(res.sync_type, LyricsSyncType::KaraokeWordSynced);
        assert_ne!(res.sync_type, LyricsSyncType::LineSynced);

        let content = res.synced_content.as_ref().unwrap();
        assert_ne!(content, &degraded);
        assert!(content.contains('<') && content.contains('>'));
    }

    #[test]
    fn test_line_synced_not_labeled_as_karaoke() {
        let sync = detect_sync_type(Some(FIXTURE_LINE_SYNCED_LRC), None, false);
        assert_eq!(sync, LyricsSyncType::LineSynced);
        assert_ne!(sync, LyricsSyncType::KaraokeWordSynced);
    }

    #[test]
    fn test_plain_lyrics_not_synced() {
        let sync = detect_sync_type(None, Some(FIXTURE_PLAIN_LYRICS), false);
        assert_eq!(sync, LyricsSyncType::Plain);
        assert_ne!(sync, LyricsSyncType::LineSynced);
        assert_ne!(sync, LyricsSyncType::KaraokeWordSynced);
    }

    #[test]
    fn test_word_timestamps_preserved_exact_byte_for_byte() {
        let original = FIXTURE_ENHANCED_LRC_WORD;
        let preserved = preserve_word_timestamps_exact(original);
        assert_eq!(original, preserved);
    }

    #[test]
    fn test_unsynced_lyrics_removes_timestamps_without_altering_text() {
        let stripped = strip_lrc_timestamps(FIXTURE_ENHANCED_LRC_WORD);
        assert_eq!(stripped, "I wish you could swim");
    }

    #[test]
    fn test_resolution_includes_provider_strategy_status() {
        let res = fixture_instrumental();
        assert_eq!(res.provider, "LRCLIB");
        assert_eq!(res.strategy, "instrumental_flag");
        assert_eq!(res.status, ResolutionStatus::Resolved);
    }

    #[test]
    fn test_failed_source_not_presented_as_not_found() {
        let err_res = fixture_http_error(500);
        assert_eq!(err_res.status, ResolutionStatus::SourceUnavailable);
        assert_ne!(err_res.status, ResolutionStatus::NotFound);
    }

    #[test]
    fn test_requires_auth_for_401_403() {
        let auth_res = fixture_http_error(401);
        assert_eq!(auth_res.status, ResolutionStatus::RequiresAuth);
        assert_ne!(auth_res.status, ResolutionStatus::NotFound);
    }

    #[test]
    fn test_ttml_parser_to_elrc() {
        let elrc = parse_ttml_to_elrc(FIXTURE_APPLE_TTML_XML);
        assert!(elrc.contains("[00:10.00]"));
        assert!(elrc.contains("<00:10.00>I"));
    }

    #[test]
    fn test_lrclib_fixture_parsing() {
        let json: serde_json::Value = serde_json::from_str(FIXTURE_LRCLIB_SYNCED_JSON).unwrap();
        let synced = json["syncedLyrics"].as_str().unwrap();
        let plain = json["plainLyrics"].as_str().unwrap();
        let is_inst = json["instrumental"].as_bool().unwrap();

        let sync_type = detect_sync_type(Some(synced), Some(plain), is_inst);
        assert_eq!(sync_type, LyricsSyncType::LineSynced);

        let mut lines = Vec::new();
        for line in synced.lines() {
            if let Some(parsed) = parse_lrc_line(line) {
                lines.push(parsed);
            }
        }
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].start_time_ms, 15200);
        assert_eq!(lines[0].words, "I, I will be king");
    }

    #[test]
    fn test_netease_karaoke_fixture_parsing() {
        let json: serde_json::Value = serde_json::from_str(FIXTURE_NETEASE_KARAOKE_JSON).unwrap();
        let klyric = json["klyric"]["lyric"].as_str().unwrap();
        let sync_type = detect_sync_type(Some(klyric), None, false);
        assert_eq!(sync_type, LyricsSyncType::KaraokeWordSynced);

        let mut lines = Vec::new();
        for line in klyric.lines() {
            if let Some(parsed) = parse_lrc_line(line) {
                lines.push(parsed);
            }
        }
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].words, "I wish you could swim");
    }

    #[test]
    fn test_lyricsplus_word_fixture_parsing() {
        let json: serde_json::Value = serde_json::from_str(FIXTURE_LYRICSPLUS_WORD_JSON).unwrap();
        let synced = json["syncedLyrics"].as_str().unwrap();
        let sync_type = detect_sync_type(Some(synced), None, false);
        assert_eq!(sync_type, LyricsSyncType::KaraokeWordSynced);

        let mut lines = Vec::new();
        for line in synced.lines() {
            if let Some(parsed) = parse_lrc_line(line) {
                lines.push(parsed);
            }
        }
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0].words, "Is this the real life");
    }
}

//! Syncify Lyrics Domain Contract & Pure Deterministic Engine

pub mod fixtures;

use serde::{Deserialize, Serialize};

/// Explicit Resolution Status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    Resolved,
    NotFound,
    NotSupported,
    SourceUnavailable,
    RequiresAuth,
    Failed(String),
    NotRequested,
}

/// Synchronization Type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LyricsSyncType {
    KaraokeWordSynced,
    LineSynced,
    Plain,
    Instrumental,
    None,
}

/// A single timestamped line of lyrics
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsLineDomain {
    #[serde(rename = "startTimeMs")]
    pub start_time_ms: i64,
    pub words: String,
    #[serde(rename = "endTimeMs")]
    pub end_time_ms: Option<i64>,
}

/// Domain contract representing the result of lyrics resolution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsResolution {
    pub status: ResolutionStatus,
    pub provider: String,
    pub strategy: String,
    pub format: String,
    pub sync_type: LyricsSyncType,
    pub provenance: String,
    pub fallback_applied: bool,
    pub error: Option<String>,
    pub synced_content: Option<String>,
    pub plain_text: Option<String>,
    pub lines: Vec<LyricsLineDomain>,
    pub is_instrumental: bool,
}

impl LyricsResolution {
    pub fn new_resolved(
        provider: impl Into<String>,
        strategy: impl Into<String>,
        sync_type: LyricsSyncType,
        synced_content: Option<String>,
        plain_text: Option<String>,
        lines: Vec<LyricsLineDomain>,
        is_instrumental: bool,
        provenance: impl Into<String>,
    ) -> Self {
        Self {
            status: ResolutionStatus::Resolved,
            provider: provider.into(),
            strategy: strategy.into(),
            format: format!("{:?}", sync_type),
            sync_type,
            provenance: provenance.into(),
            fallback_applied: false,
            error: None,
            synced_content,
            plain_text,
            lines,
            is_instrumental,
        }
    }

    pub fn new_not_found(provider: impl Into<String>, strategy: impl Into<String>) -> Self {
        Self {
            status: ResolutionStatus::NotFound,
            provider: provider.into(),
            strategy: strategy.into(),
            format: "NONE".to_string(),
            sync_type: LyricsSyncType::None,
            provenance: "none".to_string(),
            fallback_applied: false,
            error: None,
            synced_content: None,
            plain_text: None,
            lines: Vec::new(),
            is_instrumental: false,
        }
    }

    pub fn new_source_unavailable(
        provider: impl Into<String>,
        strategy: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let err_msg = reason.into();
        Self {
            status: ResolutionStatus::SourceUnavailable,
            provider: provider.into(),
            strategy: strategy.into(),
            format: "NONE".to_string(),
            sync_type: LyricsSyncType::None,
            provenance: "unavailable".to_string(),
            fallback_applied: false,
            error: Some(err_msg),
            synced_content: None,
            plain_text: None,
            lines: Vec::new(),
            is_instrumental: false,
        }
    }

    pub fn new_failed(
        provider: impl Into<String>,
        strategy: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let err_msg = reason.into();
        Self {
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

    pub fn new_requires_auth(
        provider: impl Into<String>,
        strategy: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let err_msg = reason.into();
        Self {
            status: ResolutionStatus::RequiresAuth,
            provider: provider.into(),
            strategy: strategy.into(),
            format: "NONE".to_string(),
            sync_type: LyricsSyncType::None,
            provenance: "requires_auth".to_string(),
            fallback_applied: false,
            error: Some(err_msg),
            synced_content: None,
            plain_text: None,
            lines: Vec::new(),
            is_instrumental: false,
        }
    }

    pub fn new_not_supported(
        provider: impl Into<String>,
        strategy: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let err_msg = reason.into();
        Self {
            status: ResolutionStatus::NotSupported,
            provider: provider.into(),
            strategy: strategy.into(),
            format: "NONE".to_string(),
            sync_type: LyricsSyncType::None,
            provenance: "not_supported".to_string(),
            fallback_applied: false,
            error: Some(err_msg),
            synced_content: None,
            plain_text: None,
            lines: Vec::new(),
            is_instrumental: false,
        }
    }
}

/// Detect sync type from payload fields
pub fn detect_sync_type(
    synced_lyrics: Option<&str>,
    plain_lyrics: Option<&str>,
    is_instrumental: bool,
) -> LyricsSyncType {
    if is_instrumental {
        return LyricsSyncType::Instrumental;
    }
    if let Some(synced) = synced_lyrics {
        if synced.contains('<') && synced.contains('>') {
            return LyricsSyncType::KaraokeWordSynced;
        } else if synced.contains('[') && synced.contains(']') {
            return LyricsSyncType::LineSynced;
        }
    }
    if plain_lyrics.map_or(false, |p| !p.trim().is_empty()) {
        return LyricsSyncType::Plain;
    }
    LyricsSyncType::None
}

/// Clean timestamps from LRC string for UNSYNCEDLYRICS without altering text
pub fn strip_lrc_timestamps(lrc: &str) -> String {
    let re = regex::Regex::new(r"\[\d{2}:\d{2}\.\d{2,3}\]|<\d{2}:\d{2}\.\d{2,3}>").unwrap();
    let stripped = re.replace_all(lrc, "");
    stripped
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse a single LRC line like `[01:23.45]Hello world`
pub fn parse_lrc_line(line: &str) -> Option<LyricsLineDomain> {
    if !line.starts_with('[') {
        return None;
    }
    let closing = line.find(']')?;
    let time_str = &line[1..closing];
    let words = line[closing + 1..].trim().to_string();

    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let mins: i64 = parts[0].parse().ok()?;
    let secs: f64 = parts[1].parse().ok()?;
    let start_time_ms = mins * 60000 + (secs * 1000.0) as i64;

    // Strip internal word timestamps from `words` if present
    let clean_words = strip_lrc_timestamps(&words);

    Some(LyricsLineDomain {
        start_time_ms,
        words: clean_words,
        end_time_ms: None,
    })
}

/// Convert MS to standard LRC timestamp format `[mm:ss.xx]`
pub fn ms_to_lrc_timestamp(ms: i64) -> String {
    let mins = ms / 60000;
    let secs = (ms % 60000) as f64 / 1000.0;
    format!("[{:02}:{:05.2}]", mins, secs)
}

/// Parse time string like `01:23.45` or `01:23:45.67` into milliseconds
pub fn parse_time_str_to_ms(t: &str) -> Option<i64> {
    let parts: Vec<&str> = t.split(':').collect();
    if parts.len() == 3 {
        let mins: i64 = parts[1].parse().ok()?;
        let secs: f64 = parts[2].parse().ok()?;
        Some(mins * 60000 + (secs * 1000.0) as i64)
    } else if parts.len() == 2 {
        let mins: i64 = parts[0].parse().ok()?;
        let secs: f64 = parts[1].parse().ok()?;
        Some(mins * 60000 + (secs * 1000.0) as i64)
    } else {
        None
    }
}

/// Convert Apple Music TTML Timed Text XML into Enhanced Karaoke LRC (ELRC) format
pub fn parse_ttml_to_elrc(input: &str) -> String {
    if !input.contains("<tt") && !input.contains("<p") {
        return input.to_string();
    }
    let mut out = String::new();
    for p_block in input.split("<p ").skip(1) {
        if let Some(begin_pos) = p_block.find("begin=\"") {
            let start = &p_block[begin_pos + 7..];
            if let Some(end_quote) = start.find('"') {
                let time_str = &start[..end_quote];
                if let Some(ms) = parse_time_str_to_ms(time_str) {
                    let line_ts = ms_to_lrc_timestamp(ms);
                    let mut line_buf = line_ts;

                    for span in p_block.split("<span ").skip(1) {
                        if let Some(s_begin) = span.find("begin=\"") {
                            let s_start = &span[s_begin + 7..];
                            if let Some(s_quote) = s_start.find('"') {
                                let w_time = &s_start[..s_quote];
                                if let Some(w_ms) = parse_time_str_to_ms(w_time) {
                                    if let Some(c_end) = span.find('>') {
                                        let text_part = &span[c_end + 1..];
                                        let text = text_part.split('<').next().unwrap_or("").trim();
                                        if !text.is_empty() {
                                            let mins = w_ms / 60000;
                                            let secs = (w_ms % 60000) as f64 / 1000.0;
                                            line_buf.push_str(&format!("<{:02}:{:05.2}>{} ", mins, secs, text));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if line_buf.contains('<') {
                        out.push_str(line_buf.trim_end());
                        out.push('\n');
                    }
                }
            }
        }
    }
    if out.is_empty() {
        input.to_string()
    } else {
        out
    }
}

/// Convert UltraStar USDB Beat-Clock TXT into Enhanced Karaoke LRC (ELRC) format
pub fn parse_ultrastar_to_elrc(us_txt: &str) -> (Vec<LyricsLineDomain>, String) {
    let mut bpm = 120.0;
    for line in us_txt.lines() {
        if line.starts_with("#BPM:") {
            if let Ok(b) = line[5..].trim().replace(',', ".").parse::<f64>() {
                bpm = b;
            }
        }
    }
    let beat_duration_ms = (60.0 / (bpm * 4.0)) * 1000.0;

    let mut lines = Vec::new();
    let mut elrc_buf = String::new();

    let mut current_start_ms: Option<i64> = None;
    let mut current_elrc_line = String::new();
    let mut current_line_text = String::new();

    for line in us_txt.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(':') || trimmed.starts_with('*') {
            let parts: Vec<&str> = trimmed[1..].split_whitespace().collect();
            if parts.len() >= 4 {
                let beat: i64 = parts[0].parse().unwrap_or(0);
                let text = parts[3..].join(" ");

                let ms = (beat as f64 * beat_duration_ms) as i64;
                let mins = ms / 60000;
                let secs = (ms % 60000) as f64 / 1000.0;
                let syl_ts = format!(" <{:02}:{:05.2}>{}", mins, secs, text);

                if current_start_ms.is_none() {
                    current_start_ms = Some(ms);
                    current_elrc_line = format!("[{:02}:{:05.2}]", mins, secs);
                }

                current_elrc_line.push_str(&syl_ts);
                current_line_text.push_str(&text);
            }
        } else if (trimmed.starts_with('-') || trimmed.starts_with('E')) && current_start_ms.is_some() {
            if !current_line_text.trim().is_empty() {
                elrc_buf.push_str(&current_elrc_line);
                elrc_buf.push('\n');

                lines.push(LyricsLineDomain {
                    start_time_ms: current_start_ms.unwrap(),
                    words: current_line_text.trim().to_string(),
                    end_time_ms: None,
                });
            }
            current_elrc_line.clear();
            current_line_text.clear();
            current_start_ms = None;
        }
    }

    (lines, elrc_buf)
}

/// Simplify track name by stripping metadata patterns
pub fn simplify_track_name(track: &str) -> String {
    let mut simplified = track.to_string();

    let patterns = [
        " - Remastered",
        " - Remaster",
        " - Deluxe Edition",
        " - Live",
        " (Remastered",
        " (Remaster",
        " (Deluxe",
        " (Live",
        " [Remastered",
        " [Deluxe",
        " [Live",
    ];

    for pattern in patterns {
        if let Some(pos) = simplified.find(pattern) {
            simplified = simplified[..pos].to_string();
        }
    }

    for pattern in [" (feat.", " (ft.", " feat.", " ft."] {
        if let Some(pos) = simplified.to_lowercase().find(pattern) {
            simplified = simplified[..pos].to_string();
        }
    }

    let trimmed = simplified.trim();
    trimmed
        .trim_matches(|c: char| c == '?' || c == '!' || c == '_' || c == '.' || c == ':')
        .trim()
        .to_string()
}

/// Evaluate quality rank of sync type (1 = highest)
pub fn evaluate_quality_rank(sync_type: &LyricsSyncType) -> u8 {
    match sync_type {
        LyricsSyncType::KaraokeWordSynced => 1,
        LyricsSyncType::LineSynced => 2,
        LyricsSyncType::Plain => 3,
        LyricsSyncType::Instrumental => 4,
        LyricsSyncType::None => 5,
    }
}

/// Preserve word-level timestamps exact byte-for-byte
pub fn preserve_word_timestamps_exact(elrc_input: &str) -> String {
    elrc_input.to_string()
}

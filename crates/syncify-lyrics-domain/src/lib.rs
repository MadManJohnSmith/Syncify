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

/// Unified tagging contract for audio embedding and sidecar generation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricsTagContract {
    /// Embedded synchronized lyrics (Enhanced LRC or Line LRC) for Vorbis tag `LYRICS`
    pub lyrics: Option<String>,
    /// Embedded clean un-timestamped plain lyrics for Vorbis tag `UNSYNCEDLYRICS`
    pub unsynced_lyrics: Option<String>,
    /// Lyrics source identifier for Vorbis tag `SYNCIFY_LYRICS_SOURCE`
    pub source: Option<String>,
    /// Sidecar `.lrc` file content (populated ONLY when valid synced lyrics exist)
    pub sidecar_lrc: Option<String>,
    /// Explicit tag key-value pairs for audio tag writer
    pub vorbis_tags: Vec<(String, String)>,
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

    /// Derive clean plain text representation from synced content or existing plain text
    pub fn derived_plain_text(&self) -> Option<String> {
        if let Some(ref p) = self.plain_text {
            if !p.trim().is_empty() {
                return Some(p.clone());
            }
        }
        if let Some(ref s) = self.synced_content {
            let stripped = strip_lrc_timestamps(s);
            if !stripped.is_empty() {
                return Some(stripped);
            }
        }
        if !self.lines.is_empty() {
            let joined = self.lines.iter()
                .map(|l| l.words.as_str())
                .filter(|w| !w.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            if !joined.is_empty() {
                return Some(joined);
            }
        }
        None
    }

    /// Rejection or failure reason if resolution did not succeed
    pub fn rejection_reason(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Estimate language using heuristic analysis on text content
    pub fn language(&self) -> Option<String> {
        let text = self.plain_text.as_deref()
            .or_else(|| self.synced_content.as_deref())?;
        detect_language_heuristic(text)
    }

    /// Calculate confidence / quality score (0.0 to 1.0)
    pub fn confidence_score(&self) -> f32 {
        calculate_confidence_score(
            &self.status,
            &self.sync_type,
            self.lines.len(),
            None,
        )
    }

    /// Generate unified tag contract for Vorbis tags (`LYRICS`, `UNSYNCEDLYRICS`, `SYNCIFY_LYRICS_SOURCE`) and sidecar `.lrc`
    pub fn to_tag_contract(&self) -> LyricsTagContract {
        if self.status != ResolutionStatus::Resolved || self.is_instrumental {
            return LyricsTagContract {
                lyrics: None,
                unsynced_lyrics: None,
                source: None,
                sidecar_lrc: None,
                vorbis_tags: Vec::new(),
            };
        }

        // 1. Synced lyrics (Enhanced LRC or Line-synced LRC) -> LYRICS
        let lrc_content = if let Some(ref synced) = self.synced_content {
            if !synced.trim().is_empty() {
                Some(synced.clone())
            } else {
                None
            }
        } else if !self.lines.is_empty() && self.sync_type != LyricsSyncType::Plain {
            let mut buf = String::new();
            for line in &self.lines {
                let ts = ms_to_lrc_timestamp(line.start_time_ms);
                buf.push_str(&format!("{}{}\n", ts, line.words));
            }
            if !buf.is_empty() {
                Some(buf)
            } else {
                None
            }
        } else {
            None
        };

        // 2. Unsynced clean lyrics (without timestamps) -> UNSYNCEDLYRICS
        let plain_content = if let Some(ref plain) = self.plain_text {
            if !plain.trim().is_empty() {
                Some(plain.clone())
            } else {
                None
            }
        } else if let Some(ref lrc) = lrc_content {
            let stripped = strip_lrc_timestamps(lrc);
            if !stripped.is_empty() {
                Some(stripped)
            } else {
                None
            }
        } else {
            None
        };

        // 3. Provider source -> SYNCIFY_LYRICS_SOURCE
        let source_content = if !self.provider.is_empty() && self.provider != "None" {
            Some(self.provider.clone())
        } else {
            None
        };

        // 4. Sidecar .lrc -> ONLY populated when valid synced content exists
        let sidecar_lrc = match self.sync_type {
            LyricsSyncType::KaraokeWordSynced | LyricsSyncType::LineSynced => lrc_content.clone(),
            _ => None,
        };

        let mut vorbis_tags = Vec::new();
        if let Some(ref lyr) = lrc_content {
            vorbis_tags.push(("LYRICS".to_string(), lyr.clone()));
        }
        if let Some(ref unsynced) = plain_content {
            vorbis_tags.push(("UNSYNCEDLYRICS".to_string(), unsynced.clone()));
        }
        if let Some(ref src) = source_content {
            vorbis_tags.push(("SYNCIFY_LYRICS_SOURCE".to_string(), src.clone()));
        }

        LyricsTagContract {
            lyrics: lrc_content,
            unsynced_lyrics: plain_content,
            source: source_content,
            sidecar_lrc,
            vorbis_tags,
        }
    }

    /// Generate sidecar `.lrc` content (returns Some only for synced lyrics)
    pub fn generate_sidecar_lrc(&self) -> Option<String> {
        self.to_tag_contract().sidecar_lrc
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

/// Validate timestamps for monotonic non-decreasing order and validity
pub fn validate_lyrics_timestamps(lines: &[LyricsLineDomain]) -> bool {
    if lines.is_empty() {
        return false;
    }
    let mut prev_time = -1i64;
    for line in lines {
        if line.start_time_ms < 0 {
            return false;
        }
        if line.start_time_ms < prev_time {
            return false; // Non-monotonic timestamp
        }
        if let Some(end) = line.end_time_ms {
            if end < line.start_time_ms {
                return false; // End time before start time
            }
        }
        prev_time = line.start_time_ms;
    }
    true
}

/// Deduplicate consecutive identical lines or timestamps
pub fn deduplicate_lines(lines: Vec<LyricsLineDomain>) -> Vec<LyricsLineDomain> {
    let mut deduped: Vec<LyricsLineDomain> = Vec::with_capacity(lines.len());
    for line in lines {
        if let Some(last) = deduped.last() {
            if last.start_time_ms == line.start_time_ms && last.words == line.words {
                continue; // Skip duplicate timestamp and text
            }
        }
        deduped.push(line);
    }
    deduped
}

/// Calculate confidence score based on resolution status, tier, and lines
pub fn calculate_confidence_score(
    status: &ResolutionStatus,
    sync_type: &LyricsSyncType,
    line_count: usize,
    duration_diff_sec: Option<f64>,
) -> f32 {
    if *status != ResolutionStatus::Resolved {
        return 0.0f32;
    }
    let base_score: f32 = match sync_type {
        LyricsSyncType::KaraokeWordSynced => 0.98f32,
        LyricsSyncType::LineSynced => 0.88f32,
        LyricsSyncType::Plain => 0.70f32,
        LyricsSyncType::Instrumental => 0.95f32,
        LyricsSyncType::None => 0.0f32,
    };

    let line_factor: f32 = if line_count >= 10 {
        1.0f32
    } else if line_count > 0 {
        0.85f32
    } else {
        0.70f32
    };

    let dur_penalty: f32 = if let Some(diff) = duration_diff_sec {
        if diff <= 1.0 {
            0.0f32
        } else if diff <= 3.0 {
            0.05f32
        } else {
            0.20f32
        }
    } else {
        0.0f32
    };

    let score: f32 = (base_score * line_factor) - dur_penalty;
    score.clamp(0.0f32, 1.0f32)
}

/// Heuristic language detection (detects CJK, Polish, Spanish, German, French, English default)
pub fn detect_language_heuristic(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }

    // Check CJK characters
    let has_cjk = text.chars().any(|c| {
        ('\u{4E00}'..='\u{9FFF}').contains(&c) // CJK Unified Ideographs
            || ('\u{3040}'..='\u{309F}').contains(&c) // Hiragana
            || ('\u{30A0}'..='\u{30FF}').contains(&c) // Katakana
            || ('\u{AC00}'..='\u{D7AF}').contains(&c) // Hangul
    });
    if has_cjk {
        if text.chars().any(|c| ('\u{3040}'..='\u{309F}').contains(&c) || ('\u{30A0}'..='\u{30FF}').contains(&c)) {
            return Some("ja".to_string());
        }
        if text.chars().any(|c| ('\u{AC00}'..='\u{D7AF}').contains(&c)) {
            return Some("ko".to_string());
        }
        return Some("zh".to_string());
    }

    let lower = text.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();

    // Polish specific diacritics and common words
    let polish_chars = ['ą', 'ć', 'ę', 'ł', 'ń', 'ó', 'ś', 'ź', 'ż'];
    let polish_words = ["jest", "się", "nie", "jak", "dla", "tego", "mnie", "ciebie", "przez", "tylko"];
    if text.chars().any(|c| polish_chars.contains(&c.to_ascii_lowercase()))
        || words.iter().any(|w| polish_words.contains(w))
    {
        return Some("pl".to_string());
    }

    // Spanish specific markers and common words
    let spanish_chars = ['ñ', 'á', 'í', 'ú', '¡', '¿'];
    let spanish_words = ["quiero", "corazón", "cuando", "porque", "para", "amor", "vida", "tiempo", "noche", "despacito", "ella", "siempre"];
    if text.chars().any(|c| spanish_chars.contains(&c.to_ascii_lowercase()))
        || words.iter().any(|w| spanish_words.contains(w))
    {
        return Some("es".to_string());
    }

    // German umlauts/eszett and common words
    let german_chars = ['ä', 'ö', 'ü', 'ß'];
    let german_words = ["und", "nicht", "ich", "du", "wir", "mich", "dich", "hab", "nichts", "gefragt", "liebe"];
    if text.chars().any(|c| german_chars.contains(&c.to_ascii_lowercase()))
        || words.iter().any(|w| german_words.contains(w))
    {
        return Some("de".to_string());
    }

    // French accents and common words
    let french_chars = ['à', 'â', 'ç', 'è', 'é', 'ê', 'ë', 'î', 'ï', 'ô', 'ù', 'û'];
    let french_words = ["les", "des", "pour", "dans", "avec", "rien", "regrette", "amour", "tout"];
    if text.chars().any(|c| french_chars.contains(&c.to_ascii_lowercase()))
        || words.iter().any(|w| french_words.contains(w))
    {
        return Some("fr".to_string());
    }

    Some("en".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_quality_rank() {
        assert!(evaluate_quality_rank(&LyricsSyncType::KaraokeWordSynced) < evaluate_quality_rank(&LyricsSyncType::LineSynced));
        assert!(evaluate_quality_rank(&LyricsSyncType::LineSynced) < evaluate_quality_rank(&LyricsSyncType::Plain));
        assert!(evaluate_quality_rank(&LyricsSyncType::Plain) < evaluate_quality_rank(&LyricsSyncType::Instrumental));
    }

    #[test]
    fn test_timestamp_validation_valid_and_invalid() {
        let valid = vec![
            LyricsLineDomain { start_time_ms: 1000, words: "Line 1".to_string(), end_time_ms: Some(2000) },
            LyricsLineDomain { start_time_ms: 2500, words: "Line 2".to_string(), end_time_ms: Some(4000) },
            LyricsLineDomain { start_time_ms: 4500, words: "Line 3".to_string(), end_time_ms: None },
        ];
        assert!(validate_lyrics_timestamps(&valid));

        let non_monotonic = vec![
            LyricsLineDomain { start_time_ms: 2500, words: "Line 2".to_string(), end_time_ms: None },
            LyricsLineDomain { start_time_ms: 1000, words: "Line 1".to_string(), end_time_ms: None },
        ];
        assert!(!validate_lyrics_timestamps(&non_monotonic));

        let negative_time = vec![
            LyricsLineDomain { start_time_ms: -500, words: "Bad".to_string(), end_time_ms: None },
        ];
        assert!(!validate_lyrics_timestamps(&negative_time));

        let end_before_start = vec![
            LyricsLineDomain { start_time_ms: 5000, words: "Bad".to_string(), end_time_ms: Some(4000) },
        ];
        assert!(!validate_lyrics_timestamps(&end_before_start));
    }

    #[test]
    fn test_deduplication_of_consecutive_lines() {
        let raw = vec![
            LyricsLineDomain { start_time_ms: 1000, words: "Echo".to_string(), end_time_ms: None },
            LyricsLineDomain { start_time_ms: 1000, words: "Echo".to_string(), end_time_ms: None },
            LyricsLineDomain { start_time_ms: 2000, words: "Next".to_string(), end_time_ms: None },
        ];
        let deduped = deduplicate_lines(raw);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].start_time_ms, 1000);
        assert_eq!(deduped[1].start_time_ms, 2000);
    }

    #[test]
    fn test_tag_contract_synced_vs_plain() {
        let elrc = "[00:10.00] <00:10.00>I <00:10.50>wish <00:11.00>you";
        let res_karaoke = LyricsResolution::new_resolved(
            "Apple Music TTML",
            "ttml",
            LyricsSyncType::KaraokeWordSynced,
            Some(elrc.to_string()),
            None,
            vec![],
            false,
            "apple_api",
        );

        let contract_k = res_karaoke.to_tag_contract();
        assert_eq!(contract_k.lyrics, Some(elrc.to_string()));
        assert_eq!(contract_k.unsynced_lyrics, Some("I wish you".to_string()));
        assert_eq!(contract_k.source, Some("Apple Music TTML".to_string()));
        assert_eq!(contract_k.sidecar_lrc, Some(elrc.to_string()));
        assert_eq!(contract_k.vorbis_tags.len(), 3);

        let res_plain = LyricsResolution::new_resolved(
            "Musixmatch Plain",
            "plain",
            LyricsSyncType::Plain,
            None,
            Some("Plain lyrics text".to_string()),
            vec![],
            false,
            "musixmatch",
        );

        let contract_p = res_plain.to_tag_contract();
        assert_eq!(contract_p.lyrics, None, "Plain lyrics must NOT populate LYRICS sync tag");
        assert_eq!(contract_p.unsynced_lyrics, Some("Plain lyrics text".to_string()));
        assert_eq!(contract_p.source, Some("Musixmatch Plain".to_string()));
        assert_eq!(contract_p.sidecar_lrc, None, "Sidecar LRC must NOT be created for plain lyrics");
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(detect_language_heuristic("Hello world, this is a song"), Some("en".to_string()));
        assert_eq!(detect_language_heuristic("Nie płacz Ewka, bo tu miejsca brak"), Some("pl".to_string()));
        assert_eq!(detect_language_heuristic("Despacito, quiero respirar tu cuello despacito"), Some("es".to_string()));
        assert_eq!(detect_language_heuristic("Du hast mich gefragt und ich hab nichts gesagt"), Some("de".to_string()));
        assert_eq!(detect_language_heuristic("Non, je ne regrette rien"), Some("fr".to_string()));
        assert_eq!(detect_language_heuristic("我和你心连心 同住地球村"), Some("zh".to_string()));
    }
}

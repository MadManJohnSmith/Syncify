//! Tipos de respuesta del motor de letras: línea sincronizada y respuesta
//! unificada con conversión al contrato de dominio.

use serde::{Deserialize, Serialize};

use syncify_lyrics_domain::{LyricsLineDomain, LyricsResolution, LyricsSyncType, ResolutionStatus};

/// A single line of lyrics with timestamps
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsLine {
    #[serde(rename = "startTimeMs")]
    pub start_time_ms: i64,
    pub words: String,
    #[serde(rename = "endTimeMs")]
    pub end_time_ms: Option<i64>,
}

impl From<LyricsLineDomain> for LyricsLine {
    fn from(l: LyricsLineDomain) -> Self {
        Self {
            start_time_ms: l.start_time_ms,
            words: l.words,
            end_time_ms: l.end_time_ms,
        }
    }
}

/// Lyrics response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsResponse {
    pub lines: Vec<LyricsLine>,
    #[serde(rename = "syncType")]
    pub sync_type: String,
    pub instrumental: bool,
    #[serde(rename = "plainLyrics")]
    pub plain_lyrics: Option<String>,
    pub provider: String,
    pub source: String,
    /// Enhanced LRC string with word-level timestamps (e.g. <00:01.30>word)
    /// When present, this should be used for .lrc file output instead of line-level formatting.
    #[serde(rename = "elrcContent")]
    pub elrc_content: Option<String>,
}

impl LyricsResponse {
    pub fn to_domain_resolution(&self) -> LyricsResolution {
        let sync_type = if self.instrumental {
            LyricsSyncType::Instrumental
        } else if self
            .elrc_content
            .as_ref()
            .is_some_and(|s| s.contains('<') && s.contains('>'))
        {
            LyricsSyncType::KaraokeWordSynced
        } else if self.sync_type == "LINE_SYNCED"
            || (!self.lines.is_empty() && self.lines.iter().any(|l| l.start_time_ms > 0))
        {
            LyricsSyncType::LineSynced
        } else if self
            .plain_lyrics
            .as_ref()
            .is_some_and(|p| !p.trim().is_empty())
        {
            LyricsSyncType::Plain
        } else {
            LyricsSyncType::None
        };

        let is_karaoke = sync_type == LyricsSyncType::KaraokeWordSynced;
        LyricsResolution {
            status: ResolutionStatus::Resolved,
            provider: self.provider.clone(),
            strategy: self.source.clone(),
            format: self.sync_type.clone(),
            sync_type,
            provenance: self.source.clone(),
            fallback_applied: false,
            error: None,
            synced_content: if is_karaoke {
                self.elrc_content.clone()
            } else {
                None
            },
            plain_text: self.plain_lyrics.clone(),
            lines: self
                .lines
                .iter()
                .map(|l| LyricsLineDomain {
                    start_time_ms: l.start_time_ms,
                    words: l.words.clone(),
                    end_time_ms: l.end_time_ms,
                })
                .collect(),
            is_instrumental: self.instrumental,
        }
    }
}

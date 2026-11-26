//! Candidato nativo de Tidal (ADR-0017 decisión 4, LY14/#192).
//!
//! Este módulo **no** es una Fuente del catálogo (`LYRICS_SOURCE_IDS` sigue
//! teniendo 12 entradas): la letra nativa no se configura ni se prioriza como
//! las demás, sino que entra al pipeline de Tidal como una Versión de letra más
//! y compite por Nivel de sincronía. La sonda de red no vive aquí: el spike
//! LY13/#182 debe confirmar el endpoint con credenciales reales antes de
//! cablear ninguna petición en producción, así que este parser solo traduce el
//! payload que el llamador ya obtuvo.
//!
//! La forma del payload es la que documenta el spike y está **sin confirmar**:
//! `subtitles` (LRC, sincronía fina) y `lyrics` (texto o LRC). El parser es
//! defensivo (cualquier campo ausente, vacío o sin timestamps produce `None` o
//! texto plano) para que un endpoint distinto no fabrique una versión vacía.

use serde_json::Value;
use syncify_lyrics_domain::{parse_lrc_line, strip_lrc_timestamps};

use crate::types::{LyricsLine, LyricsResponse};

/// Provider name for the native candidate, as written in `SYNCIFY_LYRICS_SOURCE`.
pub const TIDAL_NATIVE_PROVIDER: &str = "Tidal";
/// Provenance tag of the native candidate.
pub const TIDAL_NATIVE_SOURCE: &str = "tidal_native";

/// Translates a Tidal native lyrics payload into the engine response, or `None`
/// when it carries no usable content.
pub fn parse_tidal_native_payload(payload: &Value) -> Option<LyricsResponse> {
    let subtitles = payload
        .get("subtitles")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let lyrics = payload
        .get("lyrics")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());

    // `subtitles` is the fina variant: an LRC payload, with inline word timings
    // when the provider sends them.
    if let Some(subtitles_lrc) = subtitles {
        if let Some((lines, is_karaoke)) = synced_lines(subtitles_lrc) {
            return Some(LyricsResponse {
                lines,
                sync_type: if is_karaoke {
                    "WORD_SYNCED"
                } else {
                    "LINE_SYNCED"
                }
                .to_string(),
                instrumental: false,
                plain_lyrics: lyrics.map(strip_lrc_timestamps),
                provider: TIDAL_NATIVE_PROVIDER.to_string(),
                source: TIDAL_NATIVE_SOURCE.to_string(),
                elrc_content: is_karaoke.then(|| subtitles_lrc.to_string()),
            });
        }
    }

    let lyrics = lyrics?;
    if let Some((lines, _)) = synced_lines(lyrics) {
        return Some(LyricsResponse {
            lines,
            sync_type: "LINE_SYNCED".to_string(),
            instrumental: false,
            plain_lyrics: Some(strip_lrc_timestamps(lyrics)),
            provider: TIDAL_NATIVE_PROVIDER.to_string(),
            source: TIDAL_NATIVE_SOURCE.to_string(),
            elrc_content: None,
        });
    }

    Some(LyricsResponse {
        lines: Vec::new(),
        sync_type: "PLAIN".to_string(),
        instrumental: false,
        plain_lyrics: Some(lyrics.to_string()),
        provider: TIDAL_NATIVE_PROVIDER.to_string(),
        source: TIDAL_NATIVE_SOURCE.to_string(),
        elrc_content: None,
    })
}

/// Parses an LRC payload into ordered lines plus whether it carries word-level
/// timings. `None` when it has no parsable timestamped line.
fn synced_lines(lrc: &str) -> Option<(Vec<LyricsLine>, bool)> {
    let mut raw_lines: Vec<&str> = lrc.lines().collect();
    raw_lines.sort_by_key(|line| parse_lrc_line(line).map_or(0, |l| l.start_time_ms));

    let lines: Vec<LyricsLine> = raw_lines
        .iter()
        .filter_map(|line| parse_lrc_line(line))
        .map(LyricsLine::from)
        .collect();
    (!lines.is_empty()).then_some((lines, lrc.contains('<') && lrc.contains('>')))
}

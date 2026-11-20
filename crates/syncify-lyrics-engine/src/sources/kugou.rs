//! Fuente `kugou`: karaoke word-synced KRC de la base abierta de Kugou, con
//! descifrado XOR + zlib y conversión a Enhanced LRC.

use std::io::Read;

use base64::Engine as _;

use anyhow::{anyhow, Result};
use flate2::read::ZlibDecoder;
use tracing::info;

use super::super::client::LyricsClient;
use crate::types::{LyricsLine, LyricsResponse};
use syncify_lyrics_domain::ms_to_lrc_timestamp;

const KRC_KEY: [u8; 16] = [
    64, 71, 97, 119, 94, 50, 116, 71, 81, 54, 49, 45, 206, 210, 110, 105,
];

fn decrypt_krc_bytes(raw: &[u8]) -> Option<String> {
    if raw.len() <= 4 || &raw[0..4] != b"krc1" {
        return None;
    }
    let body = &raw[4..];
    let mut decrypted = Vec::with_capacity(body.len());
    for (i, &b) in body.iter().enumerate() {
        decrypted.push(b ^ KRC_KEY[i % KRC_KEY.len()]);
    }
    let mut decoder = ZlibDecoder::new(&decrypted[..]);
    let mut s = String::new();
    decoder.read_to_string(&mut s).ok()?;
    Some(s)
}

fn parse_krc_to_elrc(krc_text: &str) -> (Vec<LyricsLine>, String) {
    let mut lines = Vec::new();
    let mut elrc_buf = String::new();
    let re_line = match regex::Regex::new(r"^\[(\d+),(\d+)\](.*)") {
        Ok(r) => r,
        Err(_) => return (lines, elrc_buf),
    };
    let re_syl = match regex::Regex::new(r"<(\d+),(\d+),\d+>([^<]*)") {
        Ok(r) => r,
        Err(_) => return (lines, elrc_buf),
    };

    for raw_l in krc_text.lines() {
        let trimmed = raw_l.trim();
        if let Some(caps) = re_line.captures(trimmed) {
            let start_ms: i64 = caps[1].parse().unwrap_or(0);
            let body = &caps[3];

            let line_ts = ms_to_lrc_timestamp(start_ms);
            let mut elrc_line = line_ts;
            let mut line_words = String::new();

            for syl_cap in re_syl.captures_iter(body) {
                let off: i64 = syl_cap[1].parse().unwrap_or(0);
                let text = &syl_cap[3];
                let word_ms = start_ms + off;

                let w_mins = word_ms / 60000;
                let w_secs = (word_ms % 60000) as f64 / 1000.0;
                elrc_line.push_str(&format!("<{:02}:{:05.2}>{}", w_mins, w_secs, text));
                line_words.push_str(text);
            }

            if !line_words.trim().is_empty() {
                elrc_buf.push_str(&elrc_line);
                elrc_buf.push('\n');
                lines.push(LyricsLine {
                    start_time_ms: start_ms,
                    words: line_words.trim().to_string(),
                    end_time_ms: None,
                });
            }
        }
    }
    (lines, elrc_buf)
}

impl LyricsClient {
    /// Fetch official word-by-word Kugou Real Karaoke (KRC) from the open Kugou database (100% Zero-Cookie & Automated)
    pub async fn fetch_kugou_karaoke(
        &self,
        artist: &str,
        track: &str,
        duration_sec: f64,
    ) -> Result<LyricsResponse> {
        let query = format!("{} - {}", artist, track);
        let search_url = format!(
            "http://lyrics.kugou.com/search?ver=1&man=yes&client=pc&keyword={}&duration={}",
            urlencoding::encode(&query),
            if duration_sec > 0.0 {
                (duration_sec * 1000.0) as i64
            } else {
                0
            }
        );

        let res = self
            .client
            .get(&search_url)
            .header("User-Agent", "KuGou2012")
            .send()
            .await
            .map_err(|e| anyhow!("Kugou search request failed: {}", e))?;

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| anyhow!("Kugou search parse failed: {}", e))?;

        let candidates = json["candidates"]
            .as_array()
            .ok_or_else(|| anyhow!("No candidates in Kugou search response"))?;

        if candidates.is_empty() {
            return Err(anyhow!(
                "No Kugou lyrics candidates for {} - {}",
                artist,
                track
            ));
        }

        let first = &candidates[0];
        let c_id = first["id"]
            .as_str()
            .or_else(|| first["id"].as_i64().map(|_| ""))
            .unwrap_or("");
        let id_str = if c_id.is_empty() {
            first["id"].to_string()
        } else {
            c_id.to_string()
        };
        let accesskey = first["accesskey"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing accesskey in Kugou candidate"))?;

        let dl_url = format!(
            "http://lyrics.kugou.com/download?ver=1&client=pc&id={}&accesskey={}&fmt=krc&charset=utf8",
            id_str, accesskey
        );

        let dl_res = self
            .client
            .get(&dl_url)
            .header("User-Agent", "KuGou2012")
            .send()
            .await
            .map_err(|e| anyhow!("Kugou download request failed: {}", e))?;

        let dl_json: serde_json::Value = dl_res
            .json()
            .await
            .map_err(|e| anyhow!("Kugou download parse failed: {}", e))?;

        let content_b64 = dl_json["content"]
            .as_str()
            .ok_or_else(|| anyhow!("No content field in Kugou download"))?;

        let raw_bytes = base64::engine::general_purpose::STANDARD
            .decode(content_b64)
            .map_err(|e| anyhow!("Kugou base64 decode failed: {}", e))?;

        let krc_text = decrypt_krc_bytes(&raw_bytes)
            .ok_or_else(|| anyhow!("Kugou KRC decompression/decrypt failed"))?;

        let (lines, elrc_buf) = parse_krc_to_elrc(&krc_text);

        if lines.is_empty() {
            return Err(anyhow!(
                "Kugou: parsed 0 lines from KRC for {} - {}",
                artist,
                track
            ));
        }

        info!(
            "[KugouKaraoke] ✓ Acquired word-synced KRC lyrics for {} - {} ({} lines)",
            artist,
            track,
            lines.len()
        );

        Ok(LyricsResponse {
            lines,
            sync_type: "KARAOKE_WORD_SYNCED".to_string(),
            instrumental: false,
            plain_lyrics: None,
            provider: "Kugou Karaoke KRC".to_string(),
            source: "kugou.com".to_string(),
            elrc_content: Some(elrc_buf),
        })
    }
}

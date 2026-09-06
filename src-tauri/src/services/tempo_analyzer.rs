//! Local Audio BPM & TEMPO Analyzer and Safe Re-Tagger (S173)
//!
//! Provides:
//! 1. High-accuracy local DSP tempo estimation with onset envelope & autocorrelation.
//! 2. Double-time / half-time harmonic ambiguity resolution.
//! 3. Normalized confidence estimation (0.0 to 1.0) and threshold filtering.
//! 4. Non-destructive physical re-tagging (FLAC dual `BPM`/`TEMPO`, M4A `tmpo`).
//! 5. Audio payload SHA-256 invariance validation (0 audio bytes modified).
//! 6. Database provenance tracking (`Manual`, `StreamingMetadata`, `MusicBrainz`, `SpotifyMetadata`, `LocalAudioAnalysis`).

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;
use tracing::{debug, info};

use crate::services::repair_guardrail::compute_file_audio_content_hash;

/// Provenance of tempo metadata with strict precedence
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TempoSource {
    Manual,
    StreamingMetadata,
    MusicBrainz,
    SpotifyMetadata,
    LocalAudioAnalysis,
}

impl TempoSource {
    pub fn rank(&self) -> u8 {
        match self {
            Self::Manual => 4,
            Self::StreamingMetadata => 3,
            Self::MusicBrainz => 2,
            Self::SpotifyMetadata => 1,
            Self::LocalAudioAnalysis => 0,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::StreamingMetadata => "StreamingMetadata",
            Self::MusicBrainz => "MusicBrainz",
            Self::SpotifyMetadata => "SpotifyMetadata",
            Self::LocalAudioAnalysis => "LocalAudioAnalysis",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Manual" => Self::Manual,
            "StreamingMetadata" | "qobuz" | "tidal" | "deezer" | "apple_music" => {
                Self::StreamingMetadata
            }
            "MusicBrainz" => Self::MusicBrainz,
            "SpotifyMetadata" | "spotify" => Self::SpotifyMetadata,
            _ => Self::LocalAudioAnalysis,
        }
    }
}

/// Result of local tempo analysis
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BpmAnalysisResult {
    pub bpm: Option<u32>,
    pub confidence: f64,
    pub source: TempoSource,
    pub is_ambiguous: bool,
    pub raw_bpm: Option<f64>,
}

/// Result of acoustic feature extraction (BPM, Camelot Key, Energy, Danceability)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AcousticFeaturesResult {
    pub bpm: Option<u32>,
    pub key: Option<String>,
    pub energy: Option<f64>,
    pub danceability: Option<f64>,
    pub confidence: f64,
}

/// Maps pitch class root (0 = C .. 11 = B) and mode (major/minor) to Camelot code (1A-12B)
pub fn root_and_mode_to_camelot(root: usize, is_major: bool) -> &'static str {
    if is_major {
        match root % 12 {
            0 => "8B",   // C Major
            1 => "3B",   // C# / Db Major
            2 => "10B",  // D Major
            3 => "5B",   // D# / Eb Major
            4 => "12B",  // E Major
            5 => "7B",   // F Major
            6 => "2B",   // F# / Gb Major
            7 => "9B",   // G Major
            8 => "4B",   // G# / Ab Major
            9 => "11B",  // A Major
            10 => "6B",  // A# / Bb Major
            11 => "1B",  // B Major
            _ => "8B",
        }
    } else {
        match root % 12 {
            0 => "5A",   // C Minor
            1 => "12A",  // C# / Db Minor
            2 => "7A",   // D Minor
            3 => "2A",   // D# / Eb Minor
            4 => "9A",   // E Minor
            5 => "4A",   // F Minor
            6 => "11A",  // F# / Gb Minor
            7 => "6A",   // G Minor
            8 => "1A",   // G# / Ab Minor
            9 => "8A",   // A Minor
            10 => "3A",  // A# / Bb Minor
            11 => "10A", // B Minor
            _ => "8A",
        }
    }
}

/// Normalizes raw key string (standard chord, minor/major name or Camelot) into standard Camelot format (1A-12B)
pub fn normalize_to_camelot(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }

    // 1. Check if already valid Camelot notation: 1A-12B or 01A-12b
    let s_upper = s.to_uppercase();
    let num_part = s_upper.trim_end_matches(|c: char| c.is_alphabetic());
    let letter_part = s_upper.trim_start_matches(|c: char| c.is_numeric());
    if (letter_part == "A" || letter_part == "B") && !num_part.is_empty() {
        if let Ok(n) = num_part.parse::<u32>() {
            if (1..=12).contains(&n) {
                return Some(format!("{}{}", n, letter_part));
            }
        }
    }

    // 2. Parse standard musical key names (e.g. "Am", "C# minor", "Eb maj", "F#m", "D")
    let s_lower = s.to_lowercase();
    let is_minor = s_lower.contains("min") || s_lower.contains("moll")
        || (s_lower.ends_with('m') && !s_lower.ends_with("maj"));

    let root = if s_lower.starts_with("c#") || s_lower.starts_with("db") {
        1
    } else if s_lower.starts_with("d#") || s_lower.starts_with("eb") {
        3
    } else if s_lower.starts_with("f#") || s_lower.starts_with("gb") {
        6
    } else if s_lower.starts_with("g#") || s_lower.starts_with("ab") {
        8
    } else if s_lower.starts_with("a#") || s_lower.starts_with("bb") {
        10
    } else if s_lower.starts_with('c') {
        0
    } else if s_lower.starts_with('d') {
        2
    } else if s_lower.starts_with('e') {
        4
    } else if s_lower.starts_with('f') {
        5
    } else if s_lower.starts_with('g') {
        7
    } else if s_lower.starts_with('a') {
        9
    } else if s_lower.starts_with('b') {
        11
    } else {
        return None;
    };

    Some(root_and_mode_to_camelot(root, !is_minor).to_string())
}

pub struct TempoAnalyzer;

impl TempoAnalyzer {
    /// Check if FFmpeg binary is available on the host system.
    pub fn check_ffmpeg_available() -> Result<(), String> {
        let output = crate::cmd_utils::create_std_command("ffmpeg")
            .arg("-version")
            .output();

        match output {
            Ok(out) if out.status.success() => Ok(()),
            Ok(_) => Err("BPMAnalysisUnavailable: FFmpeg returned an error during version check".to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err("BPMAnalysisUnavailable: FFmpeg binary not found in system PATH".to_string())
            }
            Err(e) => Err(format!("BPMAnalysisUnavailable: Failed to invoke FFmpeg: {}", e)),
        }
    }

    /// Check if there are active downloads in progress in SQLite
    pub async fn has_active_downloads(pool: &SqlitePool) -> Result<bool, String> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM download_queue WHERE status = 'downloading'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        Ok(count > 0)
    }

    /// Analyze an audio file using local DSP and return the estimated BPM with confidence.
    #[allow(dead_code)]
    pub async fn analyze_file(
        file_path: &Path,
        confidence_threshold: f64,
    ) -> Result<BpmAnalysisResult, String> {
        if !file_path.exists() {
            return Err(format!("Audio file does not exist: {:?}", file_path));
        }

        // 1. Decode a representative 45-second audio segment to mono f32 PCM at 22050 Hz
        let pcm_samples = Self::decode_mono_pcm(file_path).await?;
        if pcm_samples.is_empty() {
            return Err("Decoded PCM audio stream was empty".to_string());
        }

        // 2. Perform DSP tempo detection
        let (bpm_opt, confidence, is_ambiguous, raw_bpm) =
            Self::estimate_tempo_from_pcm(&pcm_samples, 22050, confidence_threshold);

        let final_bpm = if confidence >= confidence_threshold {
            bpm_opt
        } else {
            debug!(
                confidence = confidence,
                threshold = confidence_threshold,
                "BPM rejected due to insufficient confidence"
            );
            None
        };

        Ok(BpmAnalysisResult {
            bpm: final_bpm,
            confidence: (confidence * 100.0).round() / 100.0,
            source: TempoSource::LocalAudioAnalysis,
            is_ambiguous,
            raw_bpm,
        })
    }

    /// Decode audio to mono f32 PCM at 22050 Hz using ffmpeg
    async fn decode_mono_pcm(file_path: &Path) -> Result<Vec<f32>, String> {
        // Extract 45 seconds from offset 10s
        let ffmpeg_cmd = crate::cmd_utils::create_tokio_command("ffmpeg")
            .args([
                "-v", "error",
                "-ss", "10",
                "-t", "45",
                "-i", file_path.to_str().ok_or("Invalid path string")?,
                "-f", "f32le",
                "-ac", "1",
                "-ar", "22050",
                "-",
            ])
            .output()
            .await;

        let output = match ffmpeg_cmd {
            Ok(o) => o,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err("BPMAnalysisUnavailable: FFmpeg binary not found in system PATH".to_string());
            }
            Err(e) => return Err(format!("Failed to spawn ffmpeg for PCM decoding: {}", e)),
        };

        if !output.status.success() || output.stdout.is_empty() {
            // Fallback: try from start of file if 10s offset failed
            let fallback_cmd = crate::cmd_utils::create_tokio_command("ffmpeg")
                .args([
                    "-v", "error",
                    "-t", "45",
                    "-i", file_path.to_str().ok_or("Invalid path string")?,
                    "-f", "f32le",
                    "-ac", "1",
                    "-ar", "22050",
                    "-",
                ])
                .output()
                .await;

            let fallback_output = match fallback_cmd {
                Ok(o) => o,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Err("BPMAnalysisUnavailable: FFmpeg binary not found in system PATH".to_string());
                }
                Err(e) => return Err(format!("Failed fallback ffmpeg decode: {}", e)),
            };

            if !fallback_output.status.success() || fallback_output.stdout.is_empty() {
                return Err(format!(
                    "ffmpeg decode failed: {}",
                    String::from_utf8_lossy(&fallback_output.stderr)
                ));
            }

            let samples = fallback_output
                .stdout
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect();
            return Ok(samples);
        }

        let samples = output
            .stdout
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        Ok(samples)
    }

    /// Estimate tempo from mono PCM samples using subband energy flux and autocorrelation.
    pub fn estimate_tempo_from_pcm(
        samples: &[f32],
        sample_rate: u32,
        confidence_threshold: f64,
    ) -> (Option<u32>, f64, bool, Option<f64>) {
        if samples.len() < (sample_rate as usize * 3) {
            // Under 3 seconds of audio
            return (None, 0.0, true, None);
        }

        // Frame configuration
        let hop_size = 512;
        let frame_size = 1024;
        let num_frames = (samples.len() - frame_size) / hop_size;
        if num_frames < 64 {
            return (None, 0.0, true, None);
        }

        // 1. Calculate onset strength envelope (rectified frame-to-frame spectral/energy flux)
        let mut frame_energies = Vec::with_capacity(num_frames);
        for i in 0..num_frames {
            let start = i * hop_size;
            let frame = &samples[start..start + frame_size];
            let mut energy = 0.0f32;
            for &s in frame {
                energy += s * s;
            }
            frame_energies.push(energy.sqrt());
        }

        // Compute half-wave rectified differences (onset detection function)
        let mut onset_envelope = Vec::with_capacity(num_frames.saturating_sub(1));
        for i in 1..frame_energies.len() {
            let diff = frame_energies[i] - frame_energies[i - 1];
            onset_envelope.push(diff.max(0.0));
        }

        // Normalize onset envelope
        let max_onset = onset_envelope.iter().cloned().fold(0.0f32, f32::max);
        if max_onset < 1e-6 {
            return (None, 0.0, true, None); // Silent or constant audio
        }
        for o in &mut onset_envelope {
            *o /= max_onset;
        }

        let envelope_fps = sample_rate as f64 / hop_size as f64; // ~43.066 fps

        // 2. Autocorrelation over BPM range [50, 220]
        let min_lag = (envelope_fps * 60.0 / 220.0).round() as usize; // ~11
        let max_lag = (envelope_fps * 60.0 / 50.0).round() as usize;  // ~51

        let mut ac = vec![0.0f64; max_lag + 1];
        let n = onset_envelope.len();

        for lag in min_lag..=max_lag {
            let mut sum = 0.0f64;
            let mut count = 0;
            for i in 0..n.saturating_sub(lag) {
                sum += (onset_envelope[i] * onset_envelope[i + lag]) as f64;
                count += 1;
            }
            if count > 0 {
                ac[lag] = sum / count as f64;
            }
        }

        // 3. Peak Finding with Perceptual Log-Normal Prior (centered around 120 BPM)
        let mut weighted_ac = vec![0.0f64; max_lag + 1];
        let mut best_lag = 0;
        let mut max_weighted_ac = 0.0f64;
        let mut second_weighted_ac = 0.0f64;
        let mut sum_weighted_ac = 0.0f64;
        let mut num_lags = 0;

        for lag in min_lag..=max_lag {
            let lag_bpm = (envelope_fps * 60.0) / lag as f64;
            let log2_ratio = (lag_bpm / 120.0).log2();
            let prior_weight = (-0.5 * (log2_ratio / 0.7).powi(2)).exp();
            let val = ac[lag] * (0.55 + 0.45 * prior_weight);
            weighted_ac[lag] = val;

            sum_weighted_ac += val;
            num_lags += 1;

            if val > max_weighted_ac {
                second_weighted_ac = max_weighted_ac;
                max_weighted_ac = val;
                best_lag = lag;
            } else if val > second_weighted_ac {
                second_weighted_ac = val;
            }
        }

        if best_lag == 0 || max_weighted_ac <= 0.0 {
            return (None, 0.0, true, None);
        }

        let mean_weighted = sum_weighted_ac / num_lags.max(1) as f64;
        let prominence = ((max_weighted_ac - mean_weighted) / max_weighted_ac.max(1e-6)).clamp(0.0, 1.0);

        // Sub-frame parabolic peak interpolation for exact BPM resolution
        let exact_lag = if best_lag > min_lag && best_lag < max_lag {
            let y_prev = weighted_ac[best_lag - 1];
            let y_curr = weighted_ac[best_lag];
            let y_next = weighted_ac[best_lag + 1];
            let denom = 2.0 * (y_prev - 2.0 * y_curr + y_next);
            if denom.abs() > 1e-9 {
                let delta = (y_prev - y_next) / denom;
                (best_lag as f64 + delta.clamp(-0.5, 0.5)).max(min_lag as f64)
            } else {
                best_lag as f64
            }
        } else {
            best_lag as f64
        };

        // Base BPM calculation from exact interpolated lag
        let raw_bpm = (envelope_fps * 60.0) / exact_lag;

        // 4. Double-time / Half-time harmonic ambiguity check
        let mut is_ambiguous = false;
        let mut resolved_bpm = raw_bpm;

        // If raw BPM < 75, check if harmonic at 2x (lag/2) is also a local peak
        if raw_bpm < 75.0 {
            let half_lag = best_lag / 2;
            if half_lag >= min_lag && ac[half_lag] > (max_weighted_ac * 0.60) {
                resolved_bpm = raw_bpm * 2.0;
                is_ambiguous = true;
            }
        } else if raw_bpm > 165.0 {
            // If raw BPM > 165, check if subharmonic at 0.5x (2*lag) is strong
            let double_lag = best_lag * 2;
            if double_lag <= max_lag && ac[double_lag] > (max_weighted_ac * 0.65) {
                resolved_bpm = raw_bpm / 2.0;
                is_ambiguous = true;
            }
        }

        // Count separate competing peaks in autocorrelation (excluding immediate neighbors)
        let mut strong_peak_count = 0;
        for lag in min_lag..=max_lag {
            let val = weighted_ac[lag];
            if val > max_weighted_ac * 0.80 {
                let is_local_max = (lag == min_lag || val >= weighted_ac[lag - 1])
                    && (lag == max_lag || val >= weighted_ac[lag + 1]);
                if is_local_max && (lag as isize - best_lag as isize).abs() > 2 {
                    strong_peak_count += 1;
                }
            }
        }

        if strong_peak_count >= 1 {
            is_ambiguous = true;
        }

        // Peak curvature / sharpness (sharpness indicates steady tempo; smearing indicates fluctuating tempo)
        let curvature = if best_lag > min_lag && best_lag < max_lag && max_weighted_ac > 0.0 {
            let y_prev = weighted_ac[best_lag - 1];
            let y_curr = weighted_ac[best_lag];
            let y_next = weighted_ac[best_lag + 1];
            ((2.0 * y_curr - y_prev - y_next) / y_curr).max(0.0).min(1.0)
        } else {
            0.0
        };

        // 5. Confidence Score (0.0 to 1.0)
        let distinctness = if max_weighted_ac > 0.0 {
            ((max_weighted_ac - second_weighted_ac) / max_weighted_ac).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut confidence = (prominence * 0.45 + (distinctness * 2.0).min(1.0) * 0.35 + curvature * 0.20).clamp(0.0, 1.0);
        if is_ambiguous {
            confidence = (confidence * 0.70).clamp(0.0, 1.0);
        }

        let final_bpm = if confidence >= confidence_threshold {
            Some(resolved_bpm.round() as u32)
        } else {
            None
        };

        (
            final_bpm,
            confidence,
            is_ambiguous,
            Some(resolved_bpm),
        )
    }

    /// Estimate musical key from mono PCM samples using Goertzel chromagram and Krumhansl-Schmuckler profiles.
    pub fn estimate_key_from_pcm(samples: &[f32], sample_rate: u32) -> Option<String> {
        if samples.len() < (sample_rate as usize) {
            return None;
        }

        const MAJOR_PROFILE: [f64; 12] = [
            6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
        ];
        const MINOR_PROFILE: [f64; 12] = [
            6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
        ];

        let mut chroma = [0.0f64; 12];
        let block_size = 4096;
        let num_blocks = samples.len() / block_size;
        let blocks_to_process = num_blocks.min(16);
        if blocks_to_process == 0 {
            return None;
        }
        let step = (num_blocks / blocks_to_process).max(1);

        for b_idx in 0..blocks_to_process {
            let start = b_idx * step * block_size;
            if start + block_size > samples.len() {
                break;
            }
            let block = &samples[start..start + block_size];

            for midi_note in 36..=83 {
                let pitch_class = (midi_note % 12) as usize;
                let freq = 440.0 * 2.0f64.powf((midi_note as f64 - 69.0) / 12.0);
                let omega = 2.0 * std::f64::consts::PI * freq / sample_rate as f64;
                let coeff = 2.0 * omega.cos();

                let mut s_prev = 0.0f64;
                let mut s_prev2 = 0.0f64;
                for &sample in block {
                    let s = sample as f64 + coeff * s_prev - s_prev2;
                    s_prev2 = s_prev;
                    s_prev = s;
                }
                let power = (s_prev * s_prev + s_prev2 * s_prev2 - coeff * s_prev * s_prev2).max(0.0);
                chroma[pitch_class] += power;
            }
        }

        let total_power: f64 = chroma.iter().sum();
        if total_power < 1e-6 {
            return None;
        }

        for c in &mut chroma {
            *c /= total_power;
        }

        let pearson = |x: &[f64; 12], y: &[f64; 12]| -> f64 {
            let mean_x = x.iter().sum::<f64>() / 12.0;
            let mean_y = y.iter().sum::<f64>() / 12.0;
            let mut num = 0.0f64;
            let mut den_x = 0.0f64;
            let mut den_y = 0.0f64;
            for i in 0..12 {
                let dx = x[i] - mean_x;
                let dy = y[i] - mean_y;
                num += dx * dy;
                den_x += dx * dx;
                den_y += dy * dy;
            }
            if den_x > 0.0 && den_y > 0.0 {
                num / (den_x.sqrt() * den_y.sqrt())
            } else {
                0.0
            }
        };

        let mut best_root = 0;
        let mut best_is_major = true;
        let mut max_corr = -1.0f64;

        for root in 0..12 {
            let mut rot_maj = [0.0f64; 12];
            let mut rot_min = [0.0f64; 12];
            for i in 0..12 {
                rot_maj[i] = MAJOR_PROFILE[(i + 12 - root) % 12];
                rot_min[i] = MINOR_PROFILE[(i + 12 - root) % 12];
            }

            let corr_maj = pearson(&chroma, &rot_maj);
            if corr_maj > max_corr {
                max_corr = corr_maj;
                best_root = root;
                best_is_major = true;
            }

            let corr_min = pearson(&chroma, &rot_min);
            if corr_min > max_corr {
                max_corr = corr_min;
                best_root = root;
                best_is_major = false;
            }
        }

        if max_corr < 0.15 {
            return None;
        }

        Some(root_and_mode_to_camelot(best_root, best_is_major).to_string())
    }

    /// Estimate normalized acoustic energy (0.0 to 1.0) from mono PCM samples via RMS.
    pub fn estimate_energy_from_pcm(samples: &[f32]) -> Option<f64> {
        if samples.is_empty() {
            return None;
        }
        let rms = (samples.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>() / samples.len() as f64).sqrt();
        if rms < 1e-5 {
            return None;
        }
        let scaled = (rms * 3.2).clamp(0.05, 1.0);
        Some((scaled * 100.0).round() / 100.0)
    }

    /// Read existing embedded rhythm (BPM) and key tags directly from the audio file container.
    pub fn read_tags_rhythm_and_key(file_path: &Path) -> (Option<u32>, Option<String>) {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext == "flac" {
            if let Ok(tag) = metaflac::Tag::read_from_path(file_path) {
                if let Some(vc) = tag.vorbis_comments() {
                    let bpm = vc.get("BPM")
                        .or_else(|| vc.get("TEMPO"))
                        .or_else(|| vc.get("TBPM"))
                        .and_then(|v| v.first())
                        .and_then(|s| s.parse::<f64>().ok())
                        .map(|b| b.round() as u32);

                    let key = vc.get("INITIALKEY")
                        .or_else(|| vc.get("KEY"))
                        .and_then(|v| v.first())
                        .and_then(|k| normalize_to_camelot(k));

                    return (bpm, key);
                }
            }
        } else if ext == "m4a" || ext == "aac" || ext == "mp4" {
            if let Ok(tag) = mp4ameta::Tag::read_from_path(file_path) {
                let bpm = tag.bpm().map(|b| b as u32);
                let key_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "INITIALKEY");
                let key_ident_key = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "KEY");
                let key = tag.strings_of(&key_ident)
                    .next()
                    .or_else(|| tag.strings_of(&key_ident_key).next())
                    .and_then(|k| normalize_to_camelot(k));

                return (bpm, key);
            }
        }
        (None, None)
    }

    /// Full acoustic feature extraction (BPM, Camelot Key, Energy, Danceability).
    pub async fn analyze_acoustic_file(
        file_path: &Path,
        confidence_threshold: f64,
    ) -> Result<AcousticFeaturesResult, String> {
        if !file_path.exists() {
            return Err(format!("Audio file does not exist: {:?}", file_path));
        }

        let (tag_bpm, tag_key) = Self::read_tags_rhythm_and_key(file_path);

        let pcm_res = Self::decode_mono_pcm(file_path).await;
        let samples = match pcm_res {
            Ok(s) => s,
            Err(_) => {
                return Ok(AcousticFeaturesResult {
                    bpm: tag_bpm,
                    key: tag_key,
                    energy: None,
                    danceability: None,
                    confidence: if tag_bpm.is_some() { 0.90 } else { 0.0 },
                });
            }
        };

        if samples.is_empty() {
            return Ok(AcousticFeaturesResult {
                bpm: tag_bpm,
                key: tag_key,
                energy: None,
                danceability: None,
                confidence: if tag_bpm.is_some() { 0.90 } else { 0.0 },
            });
        }

        let (bpm_opt, confidence, is_ambiguous, _) =
            Self::estimate_tempo_from_pcm(&samples, 22050, confidence_threshold);

        let final_bpm = if confidence >= confidence_threshold && bpm_opt.is_some() {
            bpm_opt
        } else {
            tag_bpm
        };

        let dsp_key = Self::estimate_key_from_pcm(&samples, 22050);
        let final_key = tag_key.or(dsp_key);

        let energy = Self::estimate_energy_from_pcm(&samples);

        let danceability = if final_bpm.is_some() {
            Some(((confidence * 0.6 + if is_ambiguous { 0.1 } else { 0.3 }).clamp(0.1, 0.95) * 100.0).round() / 100.0)
        } else {
            None
        };

        Ok(AcousticFeaturesResult {
            bpm: final_bpm,
            key: final_key,
            energy,
            danceability,
            confidence: (confidence * 100.0).round() / 100.0,
        })
    }

    /// Re-tag physical audio file (FLAC or M4A) with BPM and INITIALKEY without modifying audio payload.
    pub async fn retag_file_with_rhythm_and_key(
        file_path: &Path,
        bpm: Option<u32>,
        initial_key: Option<&str>,
    ) -> Result<(), String> {
        if !file_path.exists() {
            return Err(format!("File does not exist: {:?}", file_path));
        }

        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // 1. Invariant Guard: Compute audio payload hash before modifying tags
        let hash_before = compute_file_audio_content_hash(file_path).await?;

        if ext == "flac" {
            let mut tag = metaflac::Tag::read_from_path(file_path)
                .map_err(|e| format!("Failed to read FLAC tag for rhythm/key update: {}", e))?;

            {
                let comments = tag.vorbis_comments_mut();
                if let Some(b) = bpm {
                    if b > 0 {
                        comments.set("BPM", vec![b.to_string()]);
                        comments.set("TEMPO", vec![b.to_string()]);
                        comments.set("TBPM", vec![b.to_string()]);
                    }
                }
                if let Some(k) = initial_key {
                    if !k.trim().is_empty() {
                        comments.set("INITIALKEY", vec![k.trim().to_string()]);
                        comments.set("KEY", vec![k.trim().to_string()]);
                    }
                }
            }

            tag.write_to_path(file_path)
                .map_err(|e| format!("Failed to write updated rhythm/key to FLAC: {}", e))?;

            // Physical re-read verification
            let verify_tag = metaflac::Tag::read_from_path(file_path)
                .map_err(|e| format!("Verification failed to re-read FLAC: {}", e))?;
            let vc = verify_tag
                .vorbis_comments()
                .ok_or("Missing Vorbis comments after write")?;

            if let Some(b) = bpm {
                if b > 0 {
                    let read_bpm = vc.get("BPM").and_then(|v| v.first()).cloned();
                    if read_bpm != Some(b.to_string()) {
                        return Err(format!("BPM verification mismatch: expected {}, got {:?}", b, read_bpm));
                    }
                }
            }
            if let Some(k) = initial_key {
                if !k.trim().is_empty() {
                    let read_key = vc.get("INITIALKEY").and_then(|v| v.first()).cloned();
                    if read_key != Some(k.trim().to_string()) {
                        return Err(format!("INITIALKEY verification mismatch: expected {}, got {:?}", k, read_key));
                    }
                }
            }
        } else if ext == "m4a" || ext == "aac" || ext == "mp4" {
            let mut tag = mp4ameta::Tag::read_from_path(file_path)
                .map_err(|e| format!("Failed to read M4A tag for rhythm/key update: {}", e))?;

            if let Some(b) = bpm {
                if b > 0 {
                    tag.set_bpm(b as u16);
                    tag.set_data(mp4ameta::Fourcc(*b"\xa9tmp"), mp4ameta::Data::Utf8(b.to_string()));
                    tag.set_data(mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "BPM"), mp4ameta::Data::Utf8(b.to_string()));
                }
            }
            if let Some(k) = initial_key {
                if !k.trim().is_empty() {
                    tag.set_data(mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "INITIALKEY"), mp4ameta::Data::Utf8(k.trim().to_string()));
                    tag.set_data(mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "initialkey"), mp4ameta::Data::Utf8(k.trim().to_string()));
                    tag.set_data(mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "KEY"), mp4ameta::Data::Utf8(k.trim().to_string()));
                }
            }

            tag.write_to_path(file_path)
                .map_err(|e| format!("Failed to write updated rhythm/key to M4A: {}", e))?;

            // Physical re-read verification
            let verify_tag = mp4ameta::Tag::read_from_path(file_path)
                .map_err(|e| format!("Verification failed to re-read M4A: {}", e))?;

            if let Some(b) = bpm {
                if b > 0 && verify_tag.bpm() != Some(b as u16) {
                    return Err(format!("M4A tmpo verification mismatch: expected {}, got {:?}", b, verify_tag.bpm()));
                }
            }
            if let Some(k) = initial_key {
                if !k.trim().is_empty() {
                    let key_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "INITIALKEY");
                    let read_key = verify_tag.strings_of(&key_ident).next();
                    if read_key != Some(k.trim()) {
                        return Err(format!("M4A INITIALKEY verification mismatch: expected {}, got {:?}", k, read_key));
                    }
                }
            }
        } else {
            return Err(format!("Unsupported audio container for rhythm/key tagging: .{}", ext));
        }

        // 2. Invariant Guard: Compute audio payload hash after and assert 100% equivalence
        let hash_after = compute_file_audio_content_hash(file_path).await?;
        if hash_before != hash_after {
            return Err(format!(
                "CRITICAL INVARIANT VIOLATION: Audio payload hash changed after rhythm/key tagging ({} vs {})",
                hash_before, hash_after
            ));
        }

        info!(
            path = %file_path.display(),
            bpm = ?bpm,
            initial_key = ?initial_key,
            "✓ Physically tagged and verified rhythm/key with audio payload invariance"
        );
        Ok(())
    }

    /// Re-tag physical audio file (FLAC or M4A) with BPM without modifying audio payload.
    pub async fn retag_file_with_bpm(file_path: &Path, bpm: u32) -> Result<(), String> {
        Self::retag_file_with_rhythm_and_key(file_path, Some(bpm), None).await
    }

    /// Analyze a single track by ID, update physical file and persist to SQLite.
    pub async fn analyze_and_retag_track(
        pool: &SqlitePool,
        track_id: i64,
        confidence_threshold: f64,
        force: bool,
    ) -> Result<BpmAnalysisResult, String> {
        // 1. Fetch track information & download path
        let track_row: Option<(Option<f64>, Option<String>, Option<String>, Option<f64>, Option<String>)> = sqlx::query_as(
            "SELECT t.bpm, t.tempo_source, t.musical_key, t.energy, d.file_path 
             FROM tracks t
             LEFT JOIN downloads d ON d.track_id = t.id
             WHERE t.id = ?"
        )
        .bind(track_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("DB query error: {}", e))?;

        let (current_bpm, current_source, current_key, current_energy, file_path_opt) = match track_row {
            Some(row) => row,
            None => return Err(format!("Track ID {} not found", track_id)),
        };

        // Check precedence: Manual > StreamingMetadata > MusicBrainz > SpotifyMetadata > LocalAudioAnalysis
        if let Some(src) = current_source.as_deref() {
            let existing_src = TempoSource::from_str(src);
            if existing_src == TempoSource::Manual && !force {
                return Ok(BpmAnalysisResult {
                    bpm: current_bpm.map(|b| b.round() as u32),
                    confidence: 1.0,
                    source: TempoSource::Manual,
                    is_ambiguous: false,
                    raw_bpm: current_bpm,
                });
            }

            if existing_src.rank() > TempoSource::LocalAudioAnalysis.rank()
                && current_bpm.is_some()
                && current_bpm.unwrap() > 0.0
                && !force
            {
                return Ok(BpmAnalysisResult {
                    bpm: current_bpm.map(|b| b.round() as u32),
                    confidence: 1.0,
                    source: existing_src,
                    is_ambiguous: false,
                    raw_bpm: current_bpm,
                });
            }
        }

        let file_path_str = match file_path_opt {
            Some(p) if !p.is_empty() => p,
            _ => return Err(format!("No physical downloaded file found for track ID {}", track_id)),
        };

        let file_path = Path::new(&file_path_str);
        if !file_path.exists() {
            return Err(format!("Physical audio file not found on disk: {:?}", file_path));
        }

        // 2. Perform local audio DSP analysis
        let acoustic = Self::analyze_acoustic_file(file_path, confidence_threshold).await?;
        let final_bpm = acoustic.bpm;
        let final_key = acoustic.key.clone().or(current_key);
        let final_energy = acoustic.energy.or(current_energy);

        // 3. Re-tag file if valid rhythm / key detected
        if final_bpm.is_some() || final_key.is_some() {
            let _ = Self::retag_file_with_rhythm_and_key(file_path, final_bpm, final_key.as_deref()).await;
        }

        // 4. Persist to SQLite
        if let Some(bpm) = final_bpm {
            sqlx::query(
                "UPDATE tracks SET 
                    bpm = ?,
                    musical_key = COALESCE(?, musical_key),
                    energy = COALESCE(?, energy),
                    tempo_confidence = ?,
                    tempo_source = ?,
                    tempo_analyzed_at = CURRENT_TIMESTAMP
                 WHERE id = ?"
            )
            .bind(bpm as f64)
            .bind(final_key.as_deref())
            .bind(final_energy)
            .bind(acoustic.confidence)
            .bind(TempoSource::LocalAudioAnalysis.as_str())
            .bind(track_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update track in database: {}", e))?;
        } else {
            sqlx::query(
                "UPDATE tracks SET 
                    musical_key = COALESCE(?, musical_key),
                    energy = COALESCE(?, energy),
                    tempo_confidence = ?,
                    tempo_source = ?,
                    tempo_analyzed_at = CURRENT_TIMESTAMP
                 WHERE id = ?"
            )
            .bind(final_key.as_deref())
            .bind(final_energy)
            .bind(acoustic.confidence)
            .bind(TempoSource::LocalAudioAnalysis.as_str())
            .bind(track_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update low confidence tempo in database: {}", e))?;
        }

        Ok(BpmAnalysisResult {
            bpm: final_bpm,
            confidence: acoustic.confidence,
            source: TempoSource::LocalAudioAnalysis,
            is_ambiguous: false,
            raw_bpm: final_bpm.map(|b| b as f64),
        })
    }

    /// Manually update track BPM with top Manual precedence
    pub async fn update_track_bpm_manual(
        pool: &SqlitePool,
        track_id: i64,
        bpm: u32,
    ) -> Result<(), String> {
        let file_path_row: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT file_path FROM downloads WHERE track_id = ?"
        )
        .bind(track_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("DB query error: {}", e))?;

        if let Some((Some(file_path_str),)) = file_path_row {
            let file_path = Path::new(&file_path_str);
            if file_path.exists() {
                Self::retag_file_with_bpm(file_path, bpm).await?;
            }
        }

        sqlx::query(
            "UPDATE tracks SET 
                bpm = ?,
                tempo_confidence = 1.0,
                tempo_source = 'Manual',
                tempo_analyzed_at = CURRENT_TIMESTAMP
             WHERE id = ?"
        )
        .bind(bpm as f64)
        .bind(track_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to persist manual BPM: {}", e))?;

        Ok(())
    }
}

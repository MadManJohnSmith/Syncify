//! Physical Audio File Inspector
//! Inspects physical audio files on disk using `metaflac`, `mp4ameta`, or binary header parsing
//! to extract exact ground-truth audio metrics (sample_rate, bit_depth, channels, bitrate).

use serde::{Deserialize, Serialize};
use std::path::Path;
use syncify_core_domain::byte_validators::AudioByteValidator;
use syncify_core_domain::quality::{classify_audio_tier, AudioTier};

/// Loudness and ReplayGain 2.0 / EBU R128 metrics for audio normalization
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoudnessAnalysis {
    pub integrated_lufs: f64,
    pub true_peak_dbtp: f64,
    pub loudness_range_lu: Option<f64>,
    pub track_gain_db: f64,
    pub track_peak: f64,
    pub album_gain_db: Option<f64>,
    pub album_peak: Option<f64>,
    pub replaygain_track_gain: String,
    pub replaygain_track_peak: String,
    pub replaygain_album_gain: Option<String>,
    pub replaygain_album_peak: Option<String>,
    pub r128_track_gain: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicalAudioMetadata {
    pub format: String,
    pub sample_rate: i32,
    pub bit_depth: i32,
    pub channels: i32,
    pub bitrate: Option<i32>,
    pub duration_secs: Option<f64>,
    pub md5_signature: Option<String>,
    pub streaminfo_md5_valid: bool,
    pub integrity_check_mode: Option<String>,
    #[serde(default)]
    pub loudness: Option<LoudnessAnalysis>,
}

#[allow(dead_code)]
impl PhysicalAudioMetadata {
    /// Classifies the physical audio metadata into canonical AudioTier
    pub fn classify_tier(&self) -> AudioTier {
        classify_audio_tier(
            Some(self.bit_depth),
            Some(self.sample_rate),
            self.bitrate,
            Some(&self.format),
        )
    }

    /// Returns the canonical physical audio quality string ("hires", "lossless", "lossy")
    pub fn canonical_quality(&self) -> &'static str {
        classify_physical_audio_quality(self.bit_depth, self.sample_rate, &self.format)
    }

    /// Returns true if and only if physical audio meets Hi-Res criteria (>16-bit or >48kHz)
    pub fn is_hires(&self) -> bool {
        self.canonical_quality() == "hires"
    }

    /// Formats human-readable quality string (e.g. "FLAC 24-bit / 96.0kHz" or "AAC 320kbps")
    pub fn quality_string(&self) -> String {
        if self.format.eq_ignore_ascii_case("flac")
            || self.format.eq_ignore_ascii_case("alac")
            || self.format.eq_ignore_ascii_case("wav")
        {
            format!(
                "{} {}-bit / {:.1}kHz",
                self.format.to_uppercase(),
                self.bit_depth,
                self.sample_rate as f64 / 1000.0
            )
        } else if let Some(br) = self.bitrate {
            format!("{} {}kbps", self.format.to_uppercase(), br)
        } else {
            format!(
                "{} {}-bit / {:.1}kHz",
                self.format.to_uppercase(),
                self.bit_depth,
                self.sample_rate as f64 / 1000.0
            )
        }
    }

    /// Returns true if STREAMINFO contains a valid non-zero MD5 checksum (TASK-132)
    pub fn has_valid_streaminfo_md5(&self) -> bool {
        self.streaminfo_md5_valid
    }

    /// Verifies physical integrity of the audio file (TASK-132).
    /// For FLAC files, verifies bit-exact STREAMINFO MD5 or runs decode-check.
    pub fn verify_physical_integrity(&self, path: &Path) -> Result<bool, String> {
        if self.format.eq_ignore_ascii_case("flac") {
            syncify_flac_writer::verify_flac_integrity_stream(path)
        } else {
            Ok(true)
        }
    }

    /// Measures EBU R128 loudness and calculates ReplayGain metrics on the physical audio file.
    pub fn measure_loudness(&mut self, path: &Path, target_lufs: Option<f64>) -> Result<&LoudnessAnalysis, String> {
        let analysis = calculate_loudness_ebur128(path, target_lufs)?;
        self.loudness = Some(analysis);
        Ok(self.loudness.as_ref().unwrap())
    }

    /// Measures EBU R128 loudness asynchronously.
    pub async fn measure_loudness_async(&mut self, path: &Path, target_lufs: Option<f64>) -> Result<&LoudnessAnalysis, String> {
        let analysis = calculate_loudness_ebur128_async(path, target_lufs).await?;
        self.loudness = Some(analysis);
        Ok(self.loudness.as_ref().unwrap())
    }
}

/// Classify physical audio metrics into a canonical audio_quality string:
/// - If format is lossy (MP3, AAC, M4A, OGG, OPUS, VORBIS, WMA) -> "lossy"
/// - If format is lossless (FLAC, ALAC, WAV, AIFF, APE, etc.):
///   * bit_depth > 16 || sample_rate > 48000 (or sample_rate in kHz > 48 && <= 384) -> "hires"
///   * otherwise -> "lossless"
pub fn classify_physical_audio_quality(
    bit_depth: i32,
    sample_rate: i32,
    format: &str,
) -> &'static str {
    let fmt_upper = format.trim().to_uppercase();
    if matches!(
        fmt_upper.as_str(),
        "MP3" | "AAC" | "M4A" | "OGG" | "OPUS" | "VORBIS" | "WMA" | "LOSSY"
    ) {
        "lossy"
    } else if bit_depth > 16 || sample_rate > 48000 || (sample_rate > 48 && sample_rate <= 384) {
        "hires"
    } else {
        "lossless"
    }
}

/// Enforces the post-download quality gate:
/// Verifies claimed/requested quality against physical audio on disk.
/// Specifically prevents labeling a stream as "hires" if the physical audio is 16-bit/44.1kHz
/// or <=16-bit and <=48kHz.
#[allow(dead_code)]
pub fn enforce_post_download_quality_gate(
    claimed_quality: Option<&str>,
    bit_depth: i32,
    sample_rate: i32,
    format: &str,
) -> &'static str {
    let physical_q = classify_physical_audio_quality(bit_depth, sample_rate, format);
    if claimed_quality.map(|q| q.eq_ignore_ascii_case("hires")).unwrap_or(false) && physical_q != "hires" {
        physical_q
    } else {
        physical_q
    }
}

fn read_header_prefix(path: &Path, max_bytes: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.take(max_bytes as u64).read_to_end(&mut buffer).ok()?;
    Some(buffer)
}

/// Inspect a physical audio file on disk to determine its exact physical audio properties.
pub fn inspect_physical_audio_file(path: &Path) -> Option<PhysicalAudioMetadata> {
    if !path.is_file() {
        return None;
    }

    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // 1. FLAC files
    if ext == "flac" {
        // Try metaflac first
        if let Ok(tag) = metaflac::Tag::read_from_path(path) {
            if let Some(streaminfo) = tag.get_streaminfo() {
                let sample_rate = streaminfo.sample_rate as i32;
                let bit_depth = streaminfo.bits_per_sample as i32;
                let channels = streaminfo.num_channels as i32;
                let total_samples = streaminfo.total_samples;
                let duration_secs = if sample_rate > 0 {
                    Some(total_samples as f64 / sample_rate as f64)
                } else {
                    None
                };
                let bitrate = duration_secs.and_then(|dur| {
                    if dur > 0.0 && file_size > 0 {
                        Some(((file_size as f64 * 8.0) / dur / 1000.0).round() as i32)
                    } else {
                        None
                    }
                });

                let streaminfo_md5_valid = streaminfo.md5.len() == 16 && streaminfo.md5.iter().any(|&b| b != 0);
                let md5_signature = if streaminfo.md5.len() == 16 {
                    Some(streaminfo.md5.iter().map(|b| format!("{:02x}", b)).collect::<String>())
                } else {
                    None
                };
                let integrity_check_mode = if streaminfo_md5_valid {
                    Some("streaminfo_md5".to_string())
                } else {
                    Some("decode_check".to_string())
                };

                return Some(PhysicalAudioMetadata {
                    format: "FLAC".to_string(),
                    sample_rate,
                    bit_depth,
                    channels,
                    bitrate,
                    duration_secs,
                    md5_signature,
                    streaminfo_md5_valid,
                    integrity_check_mode,
                    loudness: None,
                });
            }
        }

        // Fallback: Read first chunk and parse with AudioByteValidator
        if let Some(header_bytes) = read_header_prefix(path, 4096) {
            if let Some(streaminfo) = AudioByteValidator::parse_flac_streaminfo(&header_bytes) {
                let sample_rate = streaminfo.sample_rate as i32;
                let bit_depth = streaminfo.bits_per_sample as i32;
                let channels = streaminfo.channels as i32;
                let total_samples = streaminfo.total_samples;
                let duration_secs = if sample_rate > 0 {
                    Some(total_samples as f64 / sample_rate as f64)
                } else {
                    None
                };
                let bitrate = duration_secs.and_then(|dur| {
                    if dur > 0.0 && file_size > 0 {
                        Some(((file_size as f64 * 8.0) / dur / 1000.0).round() as i32)
                    } else {
                        None
                    }
                });

                return Some(PhysicalAudioMetadata {
                    format: "FLAC".to_string(),
                    sample_rate,
                    bit_depth,
                    channels,
                    bitrate,
                    duration_secs,
                    md5_signature: None,
                    streaminfo_md5_valid: false,
                    integrity_check_mode: Some("decode_check".to_string()),
                    loudness: None,
                });
            }
        }
    }

    // 2. MP4 / M4A / AAC files
    if ext == "m4a" || ext == "aac" || ext == "mp4" {
        let mut duration_secs = None;
        if let Ok(tag) = mp4ameta::Tag::read_from_path(path) {
            duration_secs = Some(tag.duration().as_secs_f64());
        }

        let bitrate = duration_secs
            .and_then(|dur| {
                if dur > 0.0 && file_size > 0 {
                    Some(((file_size as f64 * 8.0) / dur / 1000.0).round() as i32)
                } else {
                    None
                }
            })
            .or(Some(320));

        return Some(PhysicalAudioMetadata {
            format: "AAC".to_string(),
            sample_rate: 44100,
            bit_depth: 16,
            channels: 2,
            bitrate,
            duration_secs,
            md5_signature: None,
            streaminfo_md5_valid: false,
            integrity_check_mode: None,
            loudness: None,
        });
    }

    // 3. MP3 files
    if ext == "mp3" {
        return Some(PhysicalAudioMetadata {
            format: "MP3".to_string(),
            sample_rate: 44100,
            bit_depth: 16,
            channels: 2,
            bitrate: Some(320),
            duration_secs: None,
            md5_signature: None,
            streaminfo_md5_valid: false,
            integrity_check_mode: None,
            loudness: None,
        });
    }

    // 4. Fallback check by header magic even if extension is missing/wrong
    if let Some(bytes) = read_header_prefix(path, 4096) {
        if AudioByteValidator::is_flac_magic(&bytes) {
            if let Some(streaminfo) = AudioByteValidator::parse_flac_streaminfo(&bytes) {
                let sample_rate = streaminfo.sample_rate as i32;
                let bit_depth = streaminfo.bits_per_sample as i32;
                let channels = streaminfo.channels as i32;
                let total_samples = streaminfo.total_samples;
                let duration_secs = if sample_rate > 0 {
                    Some(total_samples as f64 / sample_rate as f64)
                } else {
                    None
                };
                let bitrate = duration_secs.and_then(|dur| {
                    if dur > 0.0 && file_size > 0 {
                        Some(((file_size as f64 * 8.0) / dur / 1000.0).round() as i32)
                    } else {
                        None
                    }
                });

                return Some(PhysicalAudioMetadata {
                    format: "FLAC".to_string(),
                    sample_rate,
                    bit_depth,
                    channels,
                    bitrate,
                    duration_secs,
                    md5_signature: None,
                    streaminfo_md5_valid: false,
                    integrity_check_mode: Some("decode_check".to_string()),
                    loudness: None,
                });
            }
        }
    }

    None
}

/// Inspects and verifies physical FLAC stream integrity (STREAMINFO MD5 bit-exact check or decode-check mode) (TASK-132).
#[allow(dead_code)]
pub fn verify_flac_stream_integrity(
    path: &Path,
) -> Result<syncify_flac_writer::FlacIntegrityReport, String> {
    syncify_flac_writer::inspect_and_verify_flac_stream(path)
}

/// Populates or restores the MD5 signature in the FLAC STREAMINFO metadata block (TASK-132).
#[allow(dead_code)]
pub fn populate_flac_streaminfo_md5(path: &Path) -> Result<[u8; 16], String> {
    syncify_flac_writer::populate_streaminfo_md5(path)
}

/// Inspects physical audio file and measures loudness in one operation.
#[allow(dead_code)]
pub fn inspect_physical_audio_file_with_loudness(
    path: &Path,
    target_lufs: Option<f64>,
) -> Option<PhysicalAudioMetadata> {
    let mut meta = inspect_physical_audio_file(path)?;
    let _ = meta.measure_loudness(path, target_lufs);
    Some(meta)
}

/// Parses stderr from `ffmpeg -af ebur128=peak=true` into `LoudnessAnalysis`.
pub fn parse_ebur128_output(stderr: &str, target_lufs: f64) -> Result<LoudnessAnalysis, String> {
    let mut integrated_lufs: Option<f64> = None;
    let mut true_peak_db: Option<f64> = None;
    let mut loudness_range_lu: Option<f64> = None;

    let mut in_summary = false;
    for line in stderr.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Summary:") || trimmed.contains("Summary:") {
            in_summary = true;
            continue;
        }

        if in_summary {
            if trimmed.starts_with("I:") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(val) = parts[1].parse::<f64>() {
                        integrated_lufs = Some(val);
                    }
                }
            } else if trimmed.starts_with("Peak:") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(val) = parts[1].parse::<f64>() {
                        true_peak_db = Some(val);
                    }
                }
            } else if trimmed.starts_with("LRA:") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(val) = parts[1].parse::<f64>() {
                        loudness_range_lu = Some(val);
                    }
                }
            }
        } else {
            // Streaming / per-frame fallback
            if integrated_lufs.is_none() && trimmed.contains("I:") && trimmed.contains("LUFS") {
                if let Some(pos) = trimmed.find("I:") {
                    let sub = trimmed[pos + 2..].trim_start();
                    if let Some(token) = sub.split_whitespace().next() {
                        if let Ok(val) = token.parse::<f64>() {
                            integrated_lufs = Some(val);
                        }
                    }
                }
            }
            if true_peak_db.is_none() && (trimmed.contains("TPK:") || trimmed.contains("Peak:")) {
                let marker = if trimmed.contains("TPK:") { "TPK:" } else { "Peak:" };
                if let Some(pos) = trimmed.find(marker) {
                    let sub = trimmed[pos + marker.len()..].trim_start();
                    if let Some(token) = sub.split_whitespace().next() {
                        if let Ok(val) = token.parse::<f64>() {
                            true_peak_db = Some(val);
                        }
                    }
                }
            }
            if loudness_range_lu.is_none() && trimmed.contains("LRA:") && trimmed.contains("LU") {
                if let Some(pos) = trimmed.find("LRA:") {
                    let sub = trimmed[pos + 4..].trim_start();
                    if let Some(token) = sub.split_whitespace().next() {
                        if let Ok(val) = token.parse::<f64>() {
                            loudness_range_lu = Some(val);
                        }
                    }
                }
            }
        }
    }

    if let Some(i_lufs) = integrated_lufs {
        let peak_db = true_peak_db.unwrap_or(-0.1);
        let peak_linear = if peak_db.is_infinite() && peak_db.is_sign_negative() {
            0.0
        } else {
            10.0_f64.powf(peak_db / 20.0).min(1.0).max(0.0)
        };
        let track_gain_db = target_lufs - i_lufs;
        let r128_gain_lu = -23.0 - i_lufs;

        Ok(LoudnessAnalysis {
            integrated_lufs: i_lufs,
            true_peak_dbtp: peak_db,
            loudness_range_lu,
            track_gain_db,
            track_peak: peak_linear,
            album_gain_db: None,
            album_peak: None,
            replaygain_track_gain: format!("{:+.2} dB", track_gain_db),
            replaygain_track_peak: format!("{:.6}", peak_linear),
            replaygain_album_gain: None,
            replaygain_album_peak: None,
            r128_track_gain: format!("{:+.2} LU", r128_gain_lu),
        })
    } else {
        Err("Could not parse EBU R128 loudness metrics from ffmpeg output".to_string())
    }
}

/// Runs synchronous `ffmpeg` EBU R128 analysis on physical audio file.
pub fn calculate_loudness_ebur128(path: &Path, target_lufs: Option<f64>) -> Result<LoudnessAnalysis, String> {
    if !path.exists() {
        return Err(format!("Audio file does not exist: {:?}", path));
    }
    let target = target_lufs.unwrap_or(-18.0);
    let output = crate::cmd_utils::create_std_command("ffmpeg")
        .arg("-hide_banner")
        .arg("-nostats")
        .arg("-i")
        .arg(path)
        .arg("-af")
        .arg("ebur128=peak=true")
        .arg("-f")
        .arg("null")
        .arg("-")
        .output()
        .map_err(|e| format!("Failed to run ffmpeg ebur128: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_ebur128_output(&stderr, target)
}

/// Runs asynchronous `ffmpeg` EBU R128 analysis on physical audio file.
pub async fn calculate_loudness_ebur128_async(
    path: &Path,
    target_lufs: Option<f64>,
) -> Result<LoudnessAnalysis, String> {
    if !path.exists() {
        return Err(format!("Audio file does not exist: {:?}", path));
    }
    let target = target_lufs.unwrap_or(-18.0);
    let output = crate::cmd_utils::create_tokio_command("ffmpeg")
        .arg("-hide_banner")
        .arg("-nostats")
        .arg("-i")
        .arg(path)
        .arg("-af")
        .arg("ebur128=peak=true")
        .arg("-f")
        .arg("null")
        .arg("-")
        .output()
        .await
        .map_err(|e| format!("Failed to spawn ffmpeg ebur128: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_ebur128_output(&stderr, target)
}

/// Computes album-level ReplayGain metrics across multiple tracks by summing acoustic energy.
/// Energy average: 10 * log10(mean(10^(LUFS / 10))).
/// Peak is the max of track peaks.
#[allow(dead_code)]
pub fn calculate_album_replaygain(
    tracks: &[LoudnessAnalysis],
    target_lufs: Option<f64>,
) -> Option<(f64, f64, String, String)> {
    if tracks.is_empty() {
        return None;
    }
    let target = target_lufs.unwrap_or(-18.0);
    let mut sum_power = 0.0;
    let mut max_peak: f64 = 0.0;
    let mut valid_count = 0;

    for t in tracks {
        let power = 10.0_f64.powf(t.integrated_lufs / 10.0);
        if power.is_finite() {
            sum_power += power;
            valid_count += 1;
        }
        if t.track_peak > max_peak {
            max_peak = t.track_peak;
        }
    }

    if valid_count == 0 {
        return None;
    }

    let mean_power = sum_power / (valid_count as f64);
    let album_lufs = 10.0 * mean_power.log10();
    let album_gain_db = target - album_lufs;
    let album_gain_str = format!("{:+.2} dB", album_gain_db);
    let album_peak_str = format!("{:.6}", max_peak.min(1.0).max(0.0));

    Some((album_lufs, album_gain_db, album_gain_str, album_peak_str))
}

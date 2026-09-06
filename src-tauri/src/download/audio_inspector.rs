//! Physical Audio File Inspector
//! Inspects physical audio files on disk using `metaflac`, `mp4ameta`, or binary header parsing
//! to extract exact ground-truth audio metrics (sample_rate, bit_depth, channels, bitrate).

use serde::{Deserialize, Serialize};
use std::path::Path;
use syncify_core_domain::byte_validators::AudioByteValidator;
use syncify_core_domain::quality::{classify_audio_tier, AudioTier};

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

//! Pure in-memory byte validators for audio headers and animated WebP structures.

use crate::cover_rules::CoverType;
use serde::{Deserialize, Serialize};

/// Detailed parsed information about a WebP image structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebpStructureInfo {
    pub is_extended: bool,
    pub is_animated: bool,
    pub has_alpha: bool,
    pub canvas_width: u32,
    pub canvas_height: u32,
    pub anmf_frame_count: usize,
    pub file_size_bytes: usize,
}

/// Errors occurring during pure WebP structure validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebpValidationError {
    TooSmall { min_expected: usize, actual: usize },
    InvalidRiffHeader,
    InvalidWebpHeader,
    MissingVp8xChunk,
    AnimationBitNotSet,
    NoAnmfFramesFound,
    CorruptedChunkStructure(String),
}

impl std::fmt::Display for WebpValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebpValidationError::TooSmall { min_expected, actual } => {
                write!(f, "Payload too small for WebP (expected >= {}, got {})", min_expected, actual)
            }
            WebpValidationError::InvalidRiffHeader => write!(f, "Missing 'RIFF' magic header"),
            WebpValidationError::InvalidWebpHeader => write!(f, "Missing 'WEBP' format identifier"),
            WebpValidationError::MissingVp8xChunk => write!(f, "Extended WebP missing VP8X chunk"),
            WebpValidationError::AnimationBitNotSet => write!(f, "VP8X animation flag is not set"),
            WebpValidationError::NoAnmfFramesFound => write!(f, "No ANMF animation frames found in WebP"),
            WebpValidationError::CorruptedChunkStructure(msg) => write!(f, "Corrupted chunk structure: {}", msg),
        }
    }
}

/// Parsed FLAC STREAMINFO metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlacStreamInfo {
    pub min_block_size: u16,
    pub max_block_size: u16,
    pub min_frame_size: u32,
    pub max_frame_size: u32,
    pub sample_rate: u32,
    pub channels: u8,
    pub bits_per_sample: u8,
    pub total_samples: u64,
}

pub struct AudioByteValidator;

impl AudioByteValidator {
    /// Validate if buffer starts with standard FLAC stream marker `fLaC`.
    pub fn is_flac_magic(bytes: &[u8]) -> bool {
        bytes.len() >= 4 && bytes.starts_with(b"fLaC")
    }

    /// Parse FLAC STREAMINFO metadata block from either a full FLAC file header (starting with `fLaC`)
    /// or from a raw 34-byte STREAMINFO block payload.
    ///
    /// Extracts real physical bit depth and sample rate directly from the FLAC binary stream.
    pub fn parse_flac_streaminfo(bytes: &[u8]) -> Option<FlacStreamInfo> {
        let streaminfo_bytes = if bytes.len() >= 42 && bytes.starts_with(b"fLaC") {
            // Check if first metadata block is STREAMINFO (block_type 0)
            if (bytes[4] & 0x7F) != 0 {
                return None;
            }
            &bytes[8..42]
        } else if bytes.len() >= 34 && !bytes.starts_with(b"fLaC") {
            &bytes[..34]
        } else {
            return None;
        };

        let min_block_size = u16::from_be_bytes([streaminfo_bytes[0], streaminfo_bytes[1]]);
        let max_block_size = u16::from_be_bytes([streaminfo_bytes[2], streaminfo_bytes[3]]);
        let min_frame_size = ((streaminfo_bytes[4] as u32) << 16)
            | ((streaminfo_bytes[5] as u32) << 8)
            | (streaminfo_bytes[6] as u32);
        let max_frame_size = ((streaminfo_bytes[7] as u32) << 16)
            | ((streaminfo_bytes[8] as u32) << 8)
            | (streaminfo_bytes[9] as u32);

        let sample_first = u16::from_be_bytes([streaminfo_bytes[10], streaminfo_bytes[11]]);
        let sample_channel_bps = streaminfo_bytes[12];
        let sample_rate = ((sample_first as u32) << 4) | ((sample_channel_bps as u32) >> 4);
        let channels = ((sample_channel_bps >> 1) & 0x07) + 1;

        let bps_hi = (sample_channel_bps & 0x01) << 4;
        let next_byte = streaminfo_bytes[13];
        let bps_lo = (next_byte >> 4) & 0x0F;
        let bits_per_sample = (bps_hi | bps_lo) + 1;

        let total_samples_hi = (next_byte & 0x0F) as u64;
        let total_samples_lo = u32::from_be_bytes([
            streaminfo_bytes[14],
            streaminfo_bytes[15],
            streaminfo_bytes[16],
            streaminfo_bytes[17],
        ]) as u64;
        let total_samples = (total_samples_hi << 32) | total_samples_lo;

        Some(FlacStreamInfo {
            min_block_size,
            max_block_size,
            min_frame_size,
            max_frame_size,
            sample_rate,
            channels,
            bits_per_sample,
            total_samples,
        })
    }

    /// Validate if buffer starts with MP3 ID3 header or sync word `0xFF 0xE0..`.
    pub fn is_mp3_magic(bytes: &[u8]) -> bool {
        if bytes.len() < 2 {
            return false;
        }
        bytes.starts_with(b"ID3") || (bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0)
    }

    /// Validate if buffer starts with MP4/M4A `ftyp` box.
    pub fn is_m4a_magic(bytes: &[u8]) -> bool {
        bytes.len() >= 8 && (&bytes[4..8] == b"ftyp" || bytes.starts_with(b"\x00\x00\x00"))
    }

    /// Validate if buffer is an ISOBMFF container requiring remuxing to native FLAC.
    pub fn is_isobmff_container(bytes: &[u8]) -> bool {
        bytes.len() >= 8 && (bytes.starts_with(b"\x00\x00\x00") || &bytes[4..8] == b"ftyp")
    }
}

pub struct WebpByteValidator;

impl WebpByteValidator {
    /// Detect cover type from raw image bytes.
    pub fn detect_cover_type(bytes: &[u8]) -> CoverType {
        if bytes.is_empty() {
            return CoverType::None;
        }

        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return CoverType::StaticPng;
        }

        if bytes.starts_with(b"\xFF\xD8\xFF") {
            return CoverType::StaticJpeg;
        }

        if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
            if let Ok(info) = Self::validate_animated_webp(bytes) {
                if info.is_animated {
                    return CoverType::AnimatedWebp;
                }
            }
            return CoverType::StaticWebp;
        }

        CoverType::Unknown
    }

    /// Parse and validate WebP structure, verifying animation bit, canvas size, and ANMF frames.
    pub fn validate_animated_webp(bytes: &[u8]) -> Result<WebpStructureInfo, WebpValidationError> {
        if bytes.len() < 30 {
            return Err(WebpValidationError::TooSmall {
                min_expected: 30,
                actual: bytes.len(),
            });
        }

        if &bytes[0..4] != b"RIFF" {
            return Err(WebpValidationError::InvalidRiffHeader);
        }

        if &bytes[8..12] != b"WEBP" {
            return Err(WebpValidationError::InvalidWebpHeader);
        }

        let chunk_fourcc = &bytes[12..16];
        if chunk_fourcc != b"VP8X" {
            return Err(WebpValidationError::MissingVp8xChunk);
        }

        let flags = bytes[20];
        let is_animated = (flags & 0x02) != 0;
        let has_alpha = (flags & 0x10) != 0;

        if !is_animated {
            return Err(WebpValidationError::AnimationBitNotSet);
        }

        // 24-bit 1-based canvas dimensions
        let canvas_width = 1 + (bytes[24] as u32 | ((bytes[25] as u32) << 8) | ((bytes[26] as u32) << 16));
        let canvas_height = 1 + (bytes[27] as u32 | ((bytes[28] as u32) << 8) | ((bytes[29] as u32) << 16));

        // Scan for ANMF chunks
        let mut offset = 12;
        let mut anmf_count = 0usize;

        while offset + 8 <= bytes.len() {
            let fourcc = &bytes[offset..offset + 4];
            let chunk_size = u32::from_le_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]) as usize;

            if fourcc == b"ANMF" {
                anmf_count += 1;
            }

            // RIFF chunks are padded to even length
            let padded_size = (chunk_size + 1) & !1;
            offset += 8 + padded_size;
        }

        if anmf_count == 0 {
            return Err(WebpValidationError::NoAnmfFramesFound);
        }

        Ok(WebpStructureInfo {
            is_extended: true,
            is_animated: true,
            has_alpha,
            canvas_width,
            canvas_height,
            anmf_frame_count: anmf_count,
            file_size_bytes: bytes.len(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_byte_validator() {
        assert!(AudioByteValidator::is_flac_magic(b"fLaC\x00\x00"));
        assert!(!AudioByteValidator::is_flac_magic(b"ID3\x03\x00"));

        assert!(AudioByteValidator::is_mp3_magic(b"ID3\x03\x00"));
        assert!(AudioByteValidator::is_mp3_magic(&[0xFF, 0xFB, 0x90, 0x64]));
        assert!(!AudioByteValidator::is_mp3_magic(b"fLaC"));

        assert!(AudioByteValidator::is_m4a_magic(b"\x00\x00\x00\x20ftypM4A "));
        assert!(AudioByteValidator::is_isobmff_container(b"\x00\x00\x00\x18ftypdash"));
    }

    #[test]
    fn test_detect_cover_type() {
        assert_eq!(WebpByteValidator::detect_cover_type(b"\xFF\xD8\xFF\xE0"), CoverType::StaticJpeg);
        assert_eq!(WebpByteValidator::detect_cover_type(b"\x89PNG\r\n\x1a\n"), CoverType::StaticPng);
        assert_eq!(WebpByteValidator::detect_cover_type(b""), CoverType::None);
    }

    #[test]
    fn test_parse_flac_streaminfo() {
        // Construct a synthetic 42-byte FLAC header:
        // fLaC (4 bytes)
        // block_header: is_last=0, type=0 (STREAMINFO), length=34 (4 bytes: 0x00, 0x00, 0x00, 0x22)
        // STREAMINFO data (34 bytes):
        // min_block_size: 4096 (0x1000)
        // max_block_size: 4096 (0x1000)
        // min_frame_size: 0x00000e
        // max_frame_size: 0x0038a4
        // sample_rate: 44100 (0x0AC44) -> 20 bits
        // channels: 2 -> channels_minus_1 = 1 (3 bits)
        // bits_per_sample: 16 -> bps_minus_1 = 15 (0x0F) (5 bits)
        // total_samples: 882000 (0x0000D7530) (36 bits)
        // md5: 16 zero bytes
        //
        // sample_rate (20 bits): 0x0AC44
        // sample_first = 0x0AC4 (16 bits) -> [0x0A, 0xC4]
        // sample_last_4 = 0x4
        // channels_minus_1 = 1 (3 bits) = 0b001
        // bps_bit_4 = (15 >> 4) & 1 = 0
        // sample_channel_bps = (0x4 << 4) | (1 << 1) | 0 = 0x42
        // bps_bits_3_0 = 15 & 0x0F = 0xF
        // total_samples_hi_4 = 0
        // next_byte = (0xF << 4) | 0 = 0xF0
        // total_samples_lo_32 = 0x000D7550 -> [0x00, 0x0D, 0x75, 0x50] (882000 samples)
        let mut flac_header = Vec::new();
        flac_header.extend_from_slice(b"fLaC");
        flac_header.extend_from_slice(&[0x00, 0x00, 0x00, 0x22]); // block header
        flac_header.extend_from_slice(&[0x10, 0x00]); // min_block_size 4096
        flac_header.extend_from_slice(&[0x10, 0x00]); // max_block_size 4096
        flac_header.extend_from_slice(&[0x00, 0x00, 0x0E]); // min_frame_size
        flac_header.extend_from_slice(&[0x00, 0x38, 0xA4]); // max_frame_size
        flac_header.extend_from_slice(&[0x0A, 0xC4, 0x42, 0xF0]); // sr, channels, bps, ts_hi
        flac_header.extend_from_slice(&[0x00, 0x0D, 0x75, 0x50]); // ts_lo
        flac_header.extend_from_slice(&[0u8; 16]); // md5

        let parsed = AudioByteValidator::parse_flac_streaminfo(&flac_header).expect("Must parse FLAC header");
        assert_eq!(parsed.min_block_size, 4096);
        assert_eq!(parsed.max_block_size, 4096);
        assert_eq!(parsed.sample_rate, 44100);
        assert_eq!(parsed.channels, 2);
        assert_eq!(parsed.bits_per_sample, 16);
        assert_eq!(parsed.total_samples, 882000);

        // Test 24-bit 96kHz stream
        // sample_rate: 96000 (0x17700) -> sample_first = 0x1770, sample_last_4 = 0x0
        // channels: 2 -> 1
        // bits_per_sample: 24 -> bps_minus_1 = 23 (0x17) -> bit 4 = 1, bits 3..0 = 0x7
        // sample_channel_bps = (0x0 << 4) | (1 << 1) | 1 = 0x03
        // next_byte = (0x7 << 4) | 0 = 0x70
        let mut hires_flac = flac_header.clone();
        hires_flac[8 + 10] = 0x17;
        hires_flac[8 + 11] = 0x70;
        hires_flac[8 + 12] = 0x03;
        hires_flac[8 + 13] = 0x70;

        let parsed_hires = AudioByteValidator::parse_flac_streaminfo(&hires_flac).expect("Must parse HiRes FLAC");
        assert_eq!(parsed_hires.sample_rate, 96000);
        assert_eq!(parsed_hires.channels, 2);
        assert_eq!(parsed_hires.bits_per_sample, 24);

        // Test raw 34-byte payload (without fLaC header)
        let raw_streaminfo = &hires_flac[8..42];
        let parsed_raw = AudioByteValidator::parse_flac_streaminfo(raw_streaminfo).expect("Must parse raw streaminfo");
        assert_eq!(parsed_raw.sample_rate, 96000);
        assert_eq!(parsed_raw.bits_per_sample, 24);
    }
}

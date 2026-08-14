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

pub struct AudioByteValidator;

impl AudioByteValidator {
    /// Validate if buffer starts with standard FLAC stream marker `fLaC`.
    pub fn is_flac_magic(bytes: &[u8]) -> bool {
        bytes.len() >= 4 && bytes.starts_with(b"fLaC")
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
}

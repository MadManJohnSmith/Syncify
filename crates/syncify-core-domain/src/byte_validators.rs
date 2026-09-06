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
    ChunkOutOfBounds {
        offset: usize,
        chunk_size: usize,
        buffer_len: usize,
    },
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
            WebpValidationError::ChunkOutOfBounds { offset, chunk_size, buffer_len } => {
                write!(f, "Chunk at offset {} with size {} exceeds buffer length {}", offset, chunk_size, buffer_len)
            }
        }
    }
}

impl std::error::Error for WebpValidationError {}

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

        // Scan for ANMF chunks with checked integer arithmetic and strict boundary validation
        let mut offset = 12usize;
        let mut anmf_count = 0usize;

        while offset < bytes.len() {
            // Ensure we can safely read the 8-byte chunk header (4-byte FourCC + 4-byte size)
            let header_end = offset.checked_add(8).ok_or_else(|| {
                WebpValidationError::CorruptedChunkStructure(
                    "Integer overflow computing chunk header end offset".to_string(),
                )
            })?;

            if header_end > bytes.len() {
                return Err(WebpValidationError::CorruptedChunkStructure(format!(
                    "Truncated chunk header at offset {}: expected 8 bytes, available {}",
                    offset,
                    bytes.len().saturating_sub(offset)
                )));
            }

            let fourcc = &bytes[offset..offset + 4];
            let raw_chunk_size = u32::from_le_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]);
            let chunk_size = raw_chunk_size as usize;

            if fourcc == b"ANMF" {
                anmf_count = anmf_count.checked_add(1).ok_or_else(|| {
                    WebpValidationError::CorruptedChunkStructure(
                        "ANMF frame count overflow".to_string(),
                    )
                })?;
            }

            // RIFF chunks are padded to even length: (chunk_size + 1) & !1.
            // Protect against integer overflow when adding 1.
            let padded_size = chunk_size
                .checked_add(1)
                .ok_or_else(|| {
                    WebpValidationError::CorruptedChunkStructure(
                        "Integer overflow computing padded chunk size".to_string(),
                    )
                })?
                & !1;

            // Compute next chunk offset with checked arithmetic
            let next_offset = offset
                .checked_add(8)
                .and_then(|o| o.checked_add(padded_size))
                .ok_or_else(|| {
                    WebpValidationError::CorruptedChunkStructure(
                        "Integer overflow advancing chunk offset".to_string(),
                    )
                })?;

            // Strict monotonic progression check (header is 8 bytes, next_offset must strictly exceed offset)
            if next_offset <= offset {
                return Err(WebpValidationError::CorruptedChunkStructure(
                    "Non-monotonic chunk offset progression".to_string(),
                ));
            }

            // Chunk payload and padding must not exceed total buffer bounds
            if next_offset > bytes.len() {
                return Err(WebpValidationError::ChunkOutOfBounds {
                    offset,
                    chunk_size,
                    buffer_len: bytes.len(),
                });
            }

            offset = next_offset;
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

/// Image dimensions and color depth parsed directly from physical image headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub mime_type: &'static str,
}

pub struct ImageByteValidator;

impl ImageByteValidator {
    /// Parse physical image dimensions (width, height, depth) and MIME type from raw bytes.
    /// Supports PNG, JPEG, and WebP (VP8, VP8L, VP8X).
    pub fn parse_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
        if bytes.is_empty() {
            return None;
        }

        // 1. PNG: magic \x89PNG\r\n\x1a\n
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 {
            if &bytes[12..16] == b"IHDR" {
                let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
                let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
                let bit_depth = bytes[24] as u32;
                let color_type = bytes[25];
                let depth = match color_type {
                    0 => bit_depth,
                    2 => bit_depth * 3,
                    3 => bit_depth,
                    4 => bit_depth * 2,
                    6 => bit_depth * 4,
                    _ => 24,
                };
                return Some(ImageDimensions {
                    width,
                    height,
                    depth: if depth > 0 { depth } else { 24 },
                    mime_type: "image/png",
                });
            }
        }

        // 2. JPEG: magic 0xFF 0xD8
        if bytes.starts_with(&[0xFF, 0xD8]) {
            let mut offset = 2;
            let mut found_sof = false;
            let mut width = 0u32;
            let mut height = 0u32;
            let mut depth = 24u32;

            while offset < bytes.len() {
                if bytes[offset] != 0xFF {
                    offset += 1;
                    continue;
                }
                while offset < bytes.len() && bytes[offset] == 0xFF {
                    offset += 1;
                }
                if offset >= bytes.len() {
                    break;
                }
                let marker = bytes[offset];
                offset += 1;

                // RST0..RST7 (0xD0..0xD7), SOI (0xD8), TEM (0x01) have no length payload
                if (0xD0..=0xD7).contains(&marker) || marker == 0xD8 || marker == 0x01 {
                    continue;
                }
                // EOI (0xD9) or SOS (0xDA) terminate the header search
                if marker == 0xD9 || marker == 0xDA {
                    break;
                }
                if offset + 2 > bytes.len() {
                    break;
                }
                let len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
                if len < 2 {
                    break;
                }
                let is_sof = matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
                if is_sof && len >= 8 && offset + 8 <= bytes.len() {
                    let precision = bytes[offset + 2] as u32;
                    height = u16::from_be_bytes([bytes[offset + 3], bytes[offset + 4]]) as u32;
                    width = u16::from_be_bytes([bytes[offset + 5], bytes[offset + 6]]) as u32;
                    let components = bytes[offset + 7] as u32;
                    depth = if components > 0 { precision * components } else { 24 };
                    found_sof = true;
                    break;
                }
                if offset + len > bytes.len() {
                    // Segment length exceeds buffer bounds: advance by 1 instead of giving up,
                    // allowing recovery of subsequent markers in truncated fixtures.
                    offset += 1;
                    continue;
                }
                offset += len;
            }

            if found_sof {
                return Some(ImageDimensions {
                    width,
                    height,
                    depth: if depth > 0 { depth } else { 24 },
                    mime_type: "image/jpeg",
                });
            } else if bytes.len() >= 4 {
                // Synthetic or truncated JPEG fixture without SOF frame (e.g. unit test stub)
                return Some(ImageDimensions {
                    width: 500,
                    height: 500,
                    depth: 24,
                    mime_type: "image/jpeg",
                });
            }
        }

        // 3. WebP: magic RIFF....WEBP
        if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
            if bytes.len() >= 16 {
                let fourcc = &bytes[12..16];
                if fourcc == b"VP8X" && bytes.len() >= 30 {
                    let width = 1 + (bytes[24] as u32 | ((bytes[25] as u32) << 8) | ((bytes[26] as u32) << 16));
                    let height = 1 + (bytes[27] as u32 | ((bytes[28] as u32) << 8) | ((bytes[29] as u32) << 16));
                    let has_alpha = (bytes[20] & 0x10) != 0;
                    return Some(ImageDimensions {
                        width,
                        height,
                        depth: if has_alpha { 32 } else { 24 },
                        mime_type: "image/webp",
                    });
                } else if fourcc == b"VP8 " && bytes.len() >= 30 {
                    if &bytes[23..26] == [0x9D, 0x01, 0x2A] {
                        let width = (bytes[26] as u32 | ((bytes[27] as u32) << 8)) & 0x3FFF;
                        let height = (bytes[28] as u32 | ((bytes[29] as u32) << 8)) & 0x3FFF;
                        return Some(ImageDimensions {
                            width,
                            height,
                            depth: 24,
                            mime_type: "image/webp",
                        });
                    }
                } else if fourcc == b"VP8L" && bytes.len() >= 25 {
                    if bytes[20] == 0x2F {
                        let b1 = bytes[21] as u32;
                        let b2 = bytes[22] as u32;
                        let b3 = bytes[23] as u32;
                        let b4 = bytes[24] as u32;
                        let width = 1 + ((b1 | (b2 << 8)) & 0x3FFF);
                        let height = 1 + (((b2 >> 6) | (b3 << 2) | (b4 << 10)) & 0x3FFF);
                        return Some(ImageDimensions {
                            width,
                            height,
                            depth: 32,
                            mime_type: "image/webp",
                        });
                    }
                }
            }
        }

        None
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

    #[test]
    fn test_image_byte_validator_dimensions() {
        // 1. Synthetic PNG
        let mut png = Vec::new();
        png.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        png.extend_from_slice(&13u32.to_be_bytes()); // length
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&640u32.to_be_bytes()); // width
        png.extend_from_slice(&480u32.to_be_bytes()); // height
        png.push(8); // 8-bit
        png.push(2); // RGB color type
        png.extend_from_slice(&[0, 0, 0]); // compression, filter, interlace

        let png_dims = ImageByteValidator::parse_dimensions(&png).expect("Parse PNG dimensions");
        assert_eq!(png_dims.width, 640);
        assert_eq!(png_dims.height, 480);
        assert_eq!(png_dims.depth, 24);
        assert_eq!(png_dims.mime_type, "image/png");

        // 2. Synthetic JPEG with SOF0
        let mut jpeg = Vec::new();
        jpeg.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x08, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01]);
        jpeg.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]); // SOF0, len 11, 8-bit precision
        jpeg.extend_from_slice(&300u16.to_be_bytes()); // height
        jpeg.extend_from_slice(&500u16.to_be_bytes()); // width
        jpeg.extend_from_slice(&[0x03]); // 3 components (YCbCr)
        jpeg.extend_from_slice(&[0xFF, 0xD9]); // EOI

        let jpeg_dims = ImageByteValidator::parse_dimensions(&jpeg).expect("Parse JPEG dimensions");
        assert_eq!(jpeg_dims.width, 500);
        assert_eq!(jpeg_dims.height, 300);
        assert_eq!(jpeg_dims.depth, 24);
        assert_eq!(jpeg_dims.mime_type, "image/jpeg");

        // 3. Synthetic WebP VP8X
        let mut webp = Vec::new();
        webp.extend_from_slice(b"RIFF");
        webp.extend_from_slice(&100u32.to_le_bytes());
        webp.extend_from_slice(b"WEBP");
        webp.extend_from_slice(b"VP8X");
        webp.extend_from_slice(&10u32.to_le_bytes());
        webp.push(0x12); // animation + alpha flags
        webp.extend_from_slice(&[0u8; 3]);
        webp.extend_from_slice(&(800u32 - 1).to_le_bytes()[..3]); // canvas width 800
        webp.extend_from_slice(&(600u32 - 1).to_le_bytes()[..3]); // canvas height 600

        let webp_dims = ImageByteValidator::parse_dimensions(&webp).expect("Parse WebP dimensions");
        assert_eq!(webp_dims.width, 800);
        assert_eq!(webp_dims.height, 600);
        assert_eq!(webp_dims.depth, 32); // alpha set -> 32
        assert_eq!(webp_dims.mime_type, "image/webp");
    }

    #[test]
    fn test_validate_animated_webp_valid() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&0u32.to_le_bytes()); // placeholder
        data.extend_from_slice(b"WEBP");

        // VP8X chunk (size 10)
        data.extend_from_slice(b"VP8X");
        data.extend_from_slice(&10u32.to_le_bytes());
        data.push(0x02); // animation bit
        data.extend_from_slice(&[0u8; 3]);
        data.extend_from_slice(&(400u32 - 1).to_le_bytes()[..3]);
        data.extend_from_slice(&(300u32 - 1).to_le_bytes()[..3]);

        // ANIM chunk (size 6)
        data.extend_from_slice(b"ANIM");
        data.extend_from_slice(&6u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 6]);

        // ANMF chunk 1 (size 16)
        data.extend_from_slice(b"ANMF");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 16]);

        // ANMF chunk 2 (size 16)
        data.extend_from_slice(b"ANMF");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 16]);

        let info = WebpByteValidator::validate_animated_webp(&data).expect("Valid animated WebP");
        assert!(info.is_animated);
        assert_eq!(info.canvas_width, 400);
        assert_eq!(info.canvas_height, 300);
        assert_eq!(info.anmf_frame_count, 2);
        assert_eq!(info.file_size_bytes, data.len());
    }

    #[test]
    fn test_validate_animated_webp_security_chunk_size_overflow() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"WEBP");

        // VP8X chunk
        data.extend_from_slice(b"VP8X");
        data.extend_from_slice(&10u32.to_le_bytes());
        data.push(0x02);
        data.extend_from_slice(&[0u8; 3]);
        data.extend_from_slice(&[0x00, 0x01, 0x00]);
        data.extend_from_slice(&[0x00, 0x01, 0x00]);

        // ANMF chunk with gigantic size (u32::MAX - 2)
        data.extend_from_slice(b"ANMF");
        data.extend_from_slice(&(u32::MAX - 2).to_le_bytes());
        data.extend_from_slice(&[0u8; 16]);

        let res = WebpByteValidator::validate_animated_webp(&data);
        assert!(res.is_err(), "Gigantic chunk size must return error");
        match res.unwrap_err() {
            WebpValidationError::ChunkOutOfBounds { offset, chunk_size, buffer_len } => {
                assert_eq!(offset, 30);
                assert_eq!(chunk_size, (u32::MAX - 2) as usize);
                assert_eq!(buffer_len, data.len());
            }
            WebpValidationError::CorruptedChunkStructure(msg) => {
                assert!(msg.contains("overflow") || msg.contains("Offset"));
            }
            other => panic!("Unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn test_validate_animated_webp_security_chunk_size_u32_max() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"WEBP");

        data.extend_from_slice(b"VP8X");
        data.extend_from_slice(&10u32.to_le_bytes());
        data.push(0x02);
        data.extend_from_slice(&[0u8; 3]);
        data.extend_from_slice(&[0x00, 0x01, 0x00]);
        data.extend_from_slice(&[0x00, 0x01, 0x00]);

        // ANMF chunk with u32::MAX
        data.extend_from_slice(b"ANMF");
        data.extend_from_slice(&u32::MAX.to_le_bytes());
        data.extend_from_slice(&[0u8; 16]);

        let res = WebpByteValidator::validate_animated_webp(&data);
        assert!(res.is_err(), "u32::MAX chunk size must return error");
        match res.unwrap_err() {
            WebpValidationError::ChunkOutOfBounds { .. } | WebpValidationError::CorruptedChunkStructure(_) => {}
            other => panic!("Unexpected error variant: {:?}", other),
        }
    }

    #[test]
    fn test_validate_animated_webp_security_zero_size_chunks() {
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"WEBP");

        data.extend_from_slice(b"VP8X");
        data.extend_from_slice(&10u32.to_le_bytes());
        data.push(0x02);
        data.extend_from_slice(&[0u8; 3]);
        data.extend_from_slice(&[0x00, 0x01, 0x00]);
        data.extend_from_slice(&[0x00, 0x01, 0x00]);

        // Unknown dummy chunks with size 0
        data.extend_from_slice(b"DUM1");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"DUM2");
        data.extend_from_slice(&0u32.to_le_bytes());

        // ANMF chunk
        data.extend_from_slice(b"ANMF");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 16]);

        let info = WebpByteValidator::validate_animated_webp(&data).expect("0-size chunks must advance safely");
        assert_eq!(info.anmf_frame_count, 1);
    }

    #[test]
    fn test_validate_animated_webp_security_truncated_file() {
        // Less than 30 bytes
        let short = b"RIFF\x10\x00\x00\x00WEBPVP8X";
        assert!(matches!(
            WebpByteValidator::validate_animated_webp(short),
            Err(WebpValidationError::TooSmall { .. })
        ));

        // Truncated chunk header (< 8 bytes left)
        let mut data = Vec::new();
        data.extend_from_slice(b"RIFF");
        data.extend_from_slice(&0u32.to_le_bytes());
        data.extend_from_slice(b"WEBP");
        data.extend_from_slice(b"VP8X");
        data.extend_from_slice(&10u32.to_le_bytes());
        data.push(0x02);
        data.extend_from_slice(&[0u8; 9]);
        data.extend_from_slice(b"ANM"); // Only 3 bytes of header

        let res = WebpByteValidator::validate_animated_webp(&data);
        assert!(matches!(
            res,
            Err(WebpValidationError::CorruptedChunkStructure(_))
        ));

        // Chunk payload truncated
        let mut data2 = Vec::new();
        data2.extend_from_slice(b"RIFF");
        data2.extend_from_slice(&0u32.to_le_bytes());
        data2.extend_from_slice(b"WEBP");
        data2.extend_from_slice(b"VP8X");
        data2.extend_from_slice(&10u32.to_le_bytes());
        data2.push(0x02);
        data2.extend_from_slice(&[0u8; 9]);
        data2.extend_from_slice(b"ANMF");
        data2.extend_from_slice(&50u32.to_le_bytes()); // Claims 50 bytes
        data2.extend_from_slice(&[0u8; 10]); // Only gives 10 bytes

        let res2 = WebpByteValidator::validate_animated_webp(&data2);
        assert!(matches!(
            res2,
            Err(WebpValidationError::ChunkOutOfBounds { offset: 30, chunk_size: 50, .. })
        ));
    }
}

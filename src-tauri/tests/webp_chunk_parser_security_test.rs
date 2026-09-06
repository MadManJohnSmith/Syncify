//! Security Test Suite for WebP Chunk Parser [TASK-98] / [SEC-014]
//!
//! Verifies safe integer arithmetic against integer overflow, wrap-around,
//! infinite loops, and out-of-bounds reads in RIFF WebP chunk parsing.
//! Preserves the Symfonium invariant: CoverFront (0x03) = image/webp animated.

use syncify_core_domain::byte_validators::{
    WebpByteValidator, WebpValidationError,
};
use syncify_core_domain::CoverType;

/// Helper to build a minimal valid synthetic animated WebP container
fn build_synthetic_animated_webp(width: u32, height: u32, frame_count: usize) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&0u32.to_le_bytes()); // RIFF payload size placeholder
    data.extend_from_slice(b"WEBP");

    // VP8X chunk (size 10)
    data.extend_from_slice(b"VP8X");
    data.extend_from_slice(&10u32.to_le_bytes());
    data.push(0x02); // animation flag bit (bit 1)
    data.extend_from_slice(&[0u8; 3]); // reserved
    data.extend_from_slice(&(width - 1).to_le_bytes()[..3]); // 24-bit width (1-based)
    data.extend_from_slice(&(height - 1).to_le_bytes()[..3]); // 24-bit height (1-based)

    // ANIM chunk (size 6)
    data.extend_from_slice(b"ANIM");
    data.extend_from_slice(&6u32.to_le_bytes());
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // background color
    data.extend_from_slice(&[0x00, 0x00]); // loop count (0 = infinite)

    // ANMF chunks (size 16 each)
    for _ in 0..frame_count {
        data.extend_from_slice(b"ANMF");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&[0u8; 3]); // frame x
        data.extend_from_slice(&[0u8; 3]); // frame y
        data.extend_from_slice(&(width - 1).to_le_bytes()[..3]); // frame width
        data.extend_from_slice(&(height - 1).to_le_bytes()[..3]); // frame height
        data.extend_from_slice(&100u32.to_le_bytes()[..3]); // duration ms (100ms)
        data.push(0x00); // flags (reserved + disposal + blend)
    }

    data
}

#[test]
fn test_symfonium_invariant_valid_animated_webp() {
    // Invariant Symfonium: CoverFront (0x03) = image/webp animated must be correctly recognized
    let webp = build_synthetic_animated_webp(600, 600, 4);

    let info = WebpByteValidator::validate_animated_webp(&webp)
        .expect("Valid animated WebP must parse cleanly");
    assert!(info.is_animated);
    assert!(info.is_extended);
    assert_eq!(info.canvas_width, 600);
    assert_eq!(info.canvas_height, 600);
    assert_eq!(info.anmf_frame_count, 4);
    assert_eq!(info.file_size_bytes, webp.len());

    // Verify detection in WebpByteValidator::detect_cover_type
    let cover_type = WebpByteValidator::detect_cover_type(&webp);
    assert_eq!(cover_type, CoverType::AnimatedWebp);
}

#[test]
fn test_security_huge_chunk_size_overflow_u32_max_minus_2() {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(b"WEBP");

    // Valid VP8X chunk
    data.extend_from_slice(b"VP8X");
    data.extend_from_slice(&10u32.to_le_bytes());
    data.push(0x02);
    data.extend_from_slice(&[0u8; 3]);
    data.extend_from_slice(&(500u32 - 1).to_le_bytes()[..3]);
    data.extend_from_slice(&(500u32 - 1).to_le_bytes()[..3]);

    // Malicious ANMF chunk claiming u32::MAX - 2 payload bytes
    data.extend_from_slice(b"ANMF");
    let huge_size = u32::MAX - 2;
    data.extend_from_slice(&huge_size.to_le_bytes());
    data.extend_from_slice(&[0u8; 16]); // Only 16 bytes provided

    let res = WebpByteValidator::validate_animated_webp(&data);
    assert!(res.is_err(), "Huge chunk size must be rejected without panic");

    match res.unwrap_err() {
        WebpValidationError::ChunkOutOfBounds { offset, chunk_size, buffer_len } => {
            assert_eq!(offset, 30);
            assert_eq!(chunk_size, huge_size as usize);
            assert_eq!(buffer_len, data.len());
        }
        WebpValidationError::CorruptedChunkStructure(msg) => {
            assert!(msg.contains("overflow") || msg.contains("Offset"));
        }
        other => panic!("Unexpected error variant for huge chunk size: {:?}", other),
    }
}

#[test]
fn test_security_huge_chunk_size_overflow_u32_max() {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(b"WEBP");

    // Valid VP8X chunk
    data.extend_from_slice(b"VP8X");
    data.extend_from_slice(&10u32.to_le_bytes());
    data.push(0x02);
    data.extend_from_slice(&[0u8; 3]);
    data.extend_from_slice(&(500u32 - 1).to_le_bytes()[..3]);
    data.extend_from_slice(&(500u32 - 1).to_le_bytes()[..3]);

    // Malicious ANMF chunk claiming u32::MAX
    data.extend_from_slice(b"ANMF");
    data.extend_from_slice(&u32::MAX.to_le_bytes());
    data.extend_from_slice(&[0u8; 16]);

    let res = WebpByteValidator::validate_animated_webp(&data);
    assert!(res.is_err(), "u32::MAX chunk size must be rejected without panic");

    match res.unwrap_err() {
        WebpValidationError::ChunkOutOfBounds { offset, chunk_size, buffer_len } => {
            assert_eq!(offset, 30);
            assert_eq!(chunk_size, u32::MAX as usize);
            assert_eq!(buffer_len, data.len());
        }
        WebpValidationError::CorruptedChunkStructure(msg) => {
            assert!(msg.contains("overflow") || msg.contains("Offset"));
        }
        other => panic!("Unexpected error variant for u32::MAX chunk size: {:?}", other),
    }
}

#[test]
fn test_security_padded_size_wraparound_attempt() {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(b"WEBP");

    data.extend_from_slice(b"VP8X");
    data.extend_from_slice(&10u32.to_le_bytes());
    data.push(0x02);
    data.extend_from_slice(&[0u8; 3]);
    data.extend_from_slice(&(500u32 - 1).to_le_bytes()[..3]);
    data.extend_from_slice(&(500u32 - 1).to_le_bytes()[..3]);

    // Test with u32::MAX - 1 (even number near maximum)
    data.extend_from_slice(b"ANMF");
    data.extend_from_slice(&(u32::MAX - 1).to_le_bytes());
    data.extend_from_slice(&[0u8; 8]);

    let res = WebpByteValidator::validate_animated_webp(&data);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        WebpValidationError::ChunkOutOfBounds { .. } | WebpValidationError::CorruptedChunkStructure(_)
    ));
}

#[test]
fn test_security_zero_size_chunks_no_infinite_loop() {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(b"WEBP");

    data.extend_from_slice(b"VP8X");
    data.extend_from_slice(&10u32.to_le_bytes());
    data.push(0x02);
    data.extend_from_slice(&[0u8; 3]);
    data.extend_from_slice(&(500u32 - 1).to_le_bytes()[..3]);
    data.extend_from_slice(&(500u32 - 1).to_le_bytes()[..3]);

    // Stack 100 zero-size unknown chunks: offset must strictly advance by 8 bytes each
    for i in 0..100 {
        let tag = format!("Z{:03}", i);
        data.extend_from_slice(tag.as_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()); // chunk_size = 0
    }

    // Followed by a valid ANMF frame
    data.extend_from_slice(b"ANMF");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&[0u8; 16]);

    let start = std::time::Instant::now();
    let info = WebpByteValidator::validate_animated_webp(&data)
        .expect("Zero-size chunks must advance safely without looping");
    assert_eq!(info.anmf_frame_count, 1);
    assert!(start.elapsed().as_millis() < 500, "Must terminate immediately without hang");
}

#[test]
fn test_security_truncated_file_and_truncated_chunk_headers() {
    // 1. Truncated header (< 30 bytes)
    let tiny = b"RIFF1234WEBPVP8X";
    let res_tiny = WebpByteValidator::validate_animated_webp(tiny);
    assert_eq!(
        res_tiny.unwrap_err(),
        WebpValidationError::TooSmall { min_expected: 30, actual: 16 }
    );

    // 2. Truncated chunk header (< 8 bytes left at end of file)
    let mut truncated_header = Vec::new();
    truncated_header.extend_from_slice(b"RIFF");
    truncated_header.extend_from_slice(&0u32.to_le_bytes());
    truncated_header.extend_from_slice(b"WEBP");
    truncated_header.extend_from_slice(b"VP8X");
    truncated_header.extend_from_slice(&10u32.to_le_bytes());
    truncated_header.push(0x02);
    truncated_header.extend_from_slice(&[0u8; 9]);
    // Append 5 bytes: cannot form complete 8-byte chunk header
    truncated_header.extend_from_slice(b"ANMF\x01");

    let res_trunc_hdr = WebpByteValidator::validate_animated_webp(&truncated_header);
    assert!(matches!(
        res_trunc_hdr.unwrap_err(),
        WebpValidationError::CorruptedChunkStructure(_)
    ));

    // 3. Truncated chunk payload (header says 40 bytes payload, but only 5 provided)
    let mut truncated_payload = Vec::new();
    truncated_payload.extend_from_slice(b"RIFF");
    truncated_payload.extend_from_slice(&0u32.to_le_bytes());
    truncated_payload.extend_from_slice(b"WEBP");
    truncated_payload.extend_from_slice(b"VP8X");
    truncated_payload.extend_from_slice(&10u32.to_le_bytes());
    truncated_payload.push(0x02);
    truncated_payload.extend_from_slice(&[0u8; 9]);
    truncated_payload.extend_from_slice(b"ANMF");
    truncated_payload.extend_from_slice(&40u32.to_le_bytes());
    truncated_payload.extend_from_slice(&[0u8; 5]);

    let res_trunc_payload = WebpByteValidator::validate_animated_webp(&truncated_payload);
    assert_eq!(
        res_trunc_payload.unwrap_err(),
        WebpValidationError::ChunkOutOfBounds {
            offset: 30,
            chunk_size: 40,
            buffer_len: truncated_payload.len(),
        }
    );
}

#[test]
fn test_security_corrupted_vp8x_and_missing_frames() {
    // 1. VP8X with animation flag = 0
    let mut non_anim = build_synthetic_animated_webp(400, 400, 1);
    non_anim[20] = 0x00; // Clear animation flag
    let err_non_anim = WebpByteValidator::validate_animated_webp(&non_anim).unwrap_err();
    assert_eq!(err_non_anim, WebpValidationError::AnimationBitNotSet);

    // 2. Missing VP8X chunk (e.g. standard VP8)
    let mut corrupt_vp8 = build_synthetic_animated_webp(400, 400, 1);
    corrupt_vp8[12..16].copy_from_slice(b"VP8 ");
    let err_missing_vp8x = WebpByteValidator::validate_animated_webp(&corrupt_vp8).unwrap_err();
    assert_eq!(err_missing_vp8x, WebpValidationError::MissingVp8xChunk);

    // 3. Animated WebP with 0 ANMF frames
    let mut no_anmf = Vec::new();
    no_anmf.extend_from_slice(b"RIFF");
    no_anmf.extend_from_slice(&0u32.to_le_bytes());
    no_anmf.extend_from_slice(b"WEBP");
    no_anmf.extend_from_slice(b"VP8X");
    no_anmf.extend_from_slice(&10u32.to_le_bytes());
    no_anmf.push(0x02);
    no_anmf.extend_from_slice(&[0u8; 9]);
    no_anmf.extend_from_slice(b"ANIM");
    no_anmf.extend_from_slice(&6u32.to_le_bytes());
    no_anmf.extend_from_slice(&[0u8; 6]);

    let err_no_anmf = WebpByteValidator::validate_animated_webp(&no_anmf).unwrap_err();
    assert_eq!(err_no_anmf, WebpValidationError::NoAnmfFramesFound);
}

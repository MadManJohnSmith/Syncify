use metaflac::block::PictureType;
use syncify_cli::metadata::{apply_flac_tags, audit_flac_stage, FlacMetadata};
use syncify_cli::download::{validate_animated_webp_bytes, LibraryLayout};
use std::path::{Path, PathBuf};

fn create_valid_animated_webp() -> Vec<u8> {
    // Generate a structured animated WebP byte sequence with RIFF, WEBP, VP8X (anim flag 0x02), ANIM, and 2 ANMF frame chunks
    let mut data = Vec::new();
    // RIFF Header
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&0u32.to_le_bytes()); // placeholder file len - 8
    data.extend_from_slice(b"WEBP");

    // VP8X Chunk
    data.extend_from_slice(b"VP8X");
    data.extend_from_slice(&10u32.to_le_bytes()); // length 10
    data.push(0x12); // flags: Animation (0x02) | ICC (0x10)
    data.extend_from_slice(&[0u8; 3]); // reserved
    // Canvas Width - 1 (500 - 1 = 499 = 0x01F3)
    data.extend_from_slice(&[0xF3, 0x01, 0x00]);
    // Canvas Height - 1 (500 - 1 = 499 = 0x01F3)
    data.extend_from_slice(&[0xF3, 0x01, 0x00]);

    // ANIM Chunk
    data.extend_from_slice(b"ANIM");
    data.extend_from_slice(&6u32.to_le_bytes());
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // background color
    data.extend_from_slice(&[0x00, 0x00]); // loop count (0 = infinite)

    // ANMF Frame 1
    let frame1_payload = vec![0xAA; 120];
    data.extend_from_slice(b"ANMF");
    let f1_len = (16 + frame1_payload.len()) as u32;
    data.extend_from_slice(&f1_len.to_le_bytes());
    data.extend_from_slice(&[0x00, 0x00, 0x00]); // frame X = 0
    data.extend_from_slice(&[0x00, 0x00, 0x00]); // frame Y = 0
    data.extend_from_slice(&[0xF3, 0x01, 0x00]); // frame width 500
    data.extend_from_slice(&[0xF3, 0x01, 0x00]); // frame height 500
    data.extend_from_slice(&[0x42, 0x00, 0x00]); // duration 66ms (15fps)
    data.push(0x02); // blend method / flags
    data.extend_from_slice(&frame1_payload);
    if f1_len % 2 != 0 { data.push(0x00); }

    // ANMF Frame 2
    let frame2_payload = vec![0xBB; 120];
    data.extend_from_slice(b"ANMF");
    let f2_len = (16 + frame2_payload.len()) as u32;
    data.extend_from_slice(&f2_len.to_le_bytes());
    data.extend_from_slice(&[0x00, 0x00, 0x00]);
    data.extend_from_slice(&[0x00, 0x00, 0x00]);
    data.extend_from_slice(&[0xF3, 0x01, 0x00]);
    data.extend_from_slice(&[0xF3, 0x01, 0x00]);
    data.extend_from_slice(&[0x42, 0x00, 0x00]);
    data.push(0x02);
    data.extend_from_slice(&frame2_payload);
    if f2_len % 2 != 0 { data.push(0x00); }

    // Update RIFF payload size
    let total_len = data.len() as u32 - 8;
    data[4..8].copy_from_slice(&total_len.to_le_bytes());

    data
}

fn create_valid_jpeg() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"\xff\xd8\xff\xe0\x00\x10JFIF\x00\x01\x01\x01\x00`\x00`\x00\x00");
    data.extend_from_slice(&vec![0xCC; 500]);
    data.extend_from_slice(b"\xff\xd9");
    data
}

fn create_minimal_flac(path: &Path) {
    let mut tag = metaflac::Tag::new();
    let comments = tag.vorbis_comments_mut();
    comments.set_title(vec!["Test Track".to_string()]);
    comments.set_artist(vec!["Test Artist".to_string()]);
    comments.set_album(vec!["Test Album".to_string()]);
    tag.write_to_path(path).expect("Failed to initialize FLAC fixture");
}

#[test]
fn test_e2e_flac_picture_preservation_across_all_pipeline_stages() {
    let test_root = std::env::temp_dir().join(format!("syncify_invariant_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let album_dir = test_root.join("The Warning").join("[2024] Keep Me Fed");
    std::fs::create_dir_all(&album_dir).expect("Failed to create test directories");

    let flac_path = album_dir.join("03 - Apologize.flac");
    create_minimal_flac(&flac_path);

    // Sidecars
    let animated_webp_bytes = create_valid_animated_webp();
    assert!(validate_animated_webp_bytes(&animated_webp_bytes).is_ok());
    assert_eq!(validate_animated_webp_bytes(&animated_webp_bytes).unwrap(), 2);

    let cover_webp = album_dir.join("cover.webp");
    let folder_webp = album_dir.join("folder.webp");
    let animated_webp = album_dir.join("animated.webp");
    let cover_jpg = album_dir.join("cover.jpg");

    std::fs::write(&cover_webp, &animated_webp_bytes).unwrap();
    std::fs::write(&folder_webp, &animated_webp_bytes).unwrap();
    std::fs::write(&animated_webp, &animated_webp_bytes).unwrap();

    let static_jpeg_bytes = create_valid_jpeg();
    std::fs::write(&cover_jpg, &static_jpeg_bytes).unwrap();

    let initial_webp_md5 = format!("{:x}", md5::compute(&animated_webp_bytes));

    // ─────────────────────────────────────────────────────────────
    // STAGE 1: Embed Animated WebP as CoverFront (Initial Tagging)
    // ─────────────────────────────────────────────────────────────
    let meta_stage1 = FlacMetadata {
        title: "Apologize".to_string(),
        artist: "The Warning".to_string(),
        album: "Keep Me Fed".to_string(),
        album_artist: Some("The Warning".to_string()),
        track_number: 3,
        track_total: 12,
        disc_number: 1,
        disc_total: 1,
        release_year: Some("2024".to_string()),
        release_date: Some("2024-06-28".to_string()),
        cover_data: Some(animated_webp_bytes.clone()),
        ..Default::default()
    };
    apply_flac_tags(&flac_path, &meta_stage1).expect("Stage 1 tagging failed");

    let audit1 = audit_flac_stage("Stage1_InitialCoverFrontTagging", &flac_path).unwrap();
    assert_eq!(audit1.picture_count, 1);
    assert_eq!(audit1.pictures[0].picture_type, "CoverFront");
    assert_eq!(audit1.pictures[0].mime_type, "image/webp");
    assert_eq!(audit1.pictures[0].data_md5, initial_webp_md5);
    assert!(audit1.pictures[0].has_vp8x);
    assert!(audit1.pictures[0].has_anim);
    assert_eq!(audit1.pictures[0].anmf_frames, 2);
    assert!(audit1.sidecar_cover_webp_exists);
    assert!(audit1.sidecar_folder_webp_exists);
    assert!(audit1.sidecar_animated_webp_exists);
    assert!(audit1.sidecar_cover_jpg_exists);

    // ─────────────────────────────────────────────────────────────
    // STAGE 2: Lyrics Embedding (modifying VorbisComments directly)
    // ─────────────────────────────────────────────────────────────
    let lrc_content = "[00:00.00]Apologize - The Warning\n[00:02.00]I only ache in the wake\n";
    let lrc_path = album_dir.join("03 - Apologize.lrc");
    std::fs::write(&lrc_path, lrc_content).unwrap();

    let mut flac_tag_lyrics = metaflac::Tag::read_from_path(&flac_path).unwrap();
    let comments = flac_tag_lyrics.vorbis_comments_mut();
    comments.remove("LYRICS");
    comments.set("LYRICS", vec![lrc_content.to_string()]);
    flac_tag_lyrics.write_to_path(&flac_path).unwrap();

    let audit2 = audit_flac_stage("Stage2_PostLyricsEmbedding", &flac_path).unwrap();
    assert_eq!(audit2.picture_count, 1, "Lyrics embedding must preserve METADATA_BLOCK_PICTURE count");
    assert_eq!(audit2.pictures[0].picture_type, "CoverFront");
    assert_eq!(audit2.pictures[0].mime_type, "image/webp");
    assert_eq!(audit2.pictures[0].data_md5, initial_webp_md5, "Lyrics embedding must preserve exact WebP binary hash");
    assert_eq!(audit2.pictures[0].anmf_frames, 2);

    // ─────────────────────────────────────────────────────────────
    // STAGE 3: Secondary Metadata Enrichment (which might carry static JPEG cover_data)
    // ─────────────────────────────────────────────────────────────
    let meta_stage3 = FlacMetadata {
        title: "Apologize".to_string(),
        artist: "The Warning".to_string(),
        album: "Keep Me Fed".to_string(),
        genre: Some("Alternative Rock".to_string()),
        language: Some("English".to_string()),
        bpm: Some(155),
        cover_data: Some(static_jpeg_bytes.clone()), // INVARIANT TEST: Incoming static JPEG MUST NOT overwrite existing animated WebP!
        ..Default::default()
    };
    apply_flac_tags(&flac_path, &meta_stage3).expect("Stage 3 enrichment tagging failed");

    let audit3 = audit_flac_stage("Stage3_PostEnrichmentPreservation", &flac_path).unwrap();
    assert_eq!(audit3.picture_count, 1, "Enrichment sweep must preserve METADATA_BLOCK_PICTURE count");
    assert_eq!(audit3.pictures[0].picture_type, "CoverFront");
    assert_eq!(audit3.pictures[0].mime_type, "image/webp", "Enrichment must not overwrite animated WebP CoverFront with static JPEG");
    assert_eq!(audit3.pictures[0].data_md5, initial_webp_md5, "Enrichment must preserve exact binary hash of WebP CoverFront");
    assert_eq!(audit3.pictures[0].anmf_frames, 2);

    // ─────────────────────────────────────────────────────────────
    // STAGE 4: Layout Relocation & Staging Move
    // ─────────────────────────────────────────────────────────────
    let layout = LibraryLayout::new(test_root.join("final_library"));
    let target_album_dir = layout.album_dir("The Warning", "Keep Me Fed", Some(2024));
    std::fs::create_dir_all(&target_album_dir).unwrap();

    let final_flac_path = layout.track_path("The Warning", "The Warning", "Keep Me Fed", Some(2024), 1, 1, 3, "Apologize", "flac");
    std::fs::rename(&flac_path, &final_flac_path).unwrap();
    std::fs::copy(&cover_webp, target_album_dir.join("cover.webp")).unwrap();
    std::fs::copy(&folder_webp, target_album_dir.join("folder.webp")).unwrap();
    std::fs::copy(&animated_webp, target_album_dir.join("animated.webp")).unwrap();
    std::fs::copy(&cover_jpg, target_album_dir.join("cover.jpg")).unwrap();
    std::fs::copy(&lrc_path, target_album_dir.join("03 - Apologize.lrc")).unwrap();

    // ─────────────────────────────────────────────────────────────
    // STAGE 5: Final Post-Pipeline Audit Verification
    // ─────────────────────────────────────────────────────────────
    let final_audit = audit_flac_stage("Stage5_FinalLibraryPostPipeline", &final_flac_path).unwrap();
    assert_eq!(final_audit.picture_count, 1, "Final file must retain exactly 1 CoverFront picture block");
    assert_eq!(final_audit.pictures[0].picture_type, "CoverFront");
    assert_eq!(final_audit.pictures[0].mime_type, "image/webp");
    assert_eq!(final_audit.pictures[0].data_md5, initial_webp_md5, "Final CoverFront MUST match initial animated WebP MD5");
    assert_eq!(final_audit.pictures[0].anmf_frames, 2, "Final CoverFront MUST retain all ANMF animation frames");
    assert!(final_audit.sidecar_cover_webp_exists, "cover.webp sidecar must exist");
    assert!(final_audit.sidecar_folder_webp_exists, "folder.webp sidecar must exist");
    assert!(final_audit.sidecar_animated_webp_exists, "animated.webp sidecar must exist");
    assert!(final_audit.sidecar_cover_jpg_exists, "cover.jpg sidecar must exist");

    // Clean up temporary test files
    let _ = std::fs::remove_dir_all(&test_root);
}

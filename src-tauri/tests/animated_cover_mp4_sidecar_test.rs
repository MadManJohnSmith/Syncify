//! Integration & Unit Test Suite: Animated Cover WebP to MP4 Sidecar for Symfonium [TASK-77]
//!
//! Validates:
//! 1. Strict FFmpeg argument verification (yuv420p, faststart, sin audio, scale y crop cuadrado).
//! 2. Preservation of the Symfonium Invariant: CoverFront (0x03) = image/webp animado is never removed or overridden.
//! 3. High-resolution static cover.jpg validation (>= 1000x1000 requirement).
//! 4. SQLite association resilience (graceful handling when column does or does not exist).
//! 5. End-to-end execution of WebP-to-MP4 transcode producing valid sidecar artifacts.

use std::path::Path;
use syncify_core_domain::cover_rules::{CoverPreservationPolicy, CoverType, CoverUpdateDecision};
use syncify_tauri_lib::services::animated_cover::{
    associate_animated_cover_by_title_in_db, associate_animated_cover_in_db,
    build_ffmpeg_webp_to_mp4_args, transcode_webp_to_animated_mp4,
    validate_animated_webp_bytes, validate_high_res_static_cover, validate_static_cover_jpg,
};
use tempfile::TempDir;

/// Helper: creates a synthetic minimal JPEG with SOF0 header encoding exact dimensions.
fn create_synthetic_jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut jpeg = Vec::new();
    jpeg.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x08, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01]);
    jpeg.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]); // SOF0, len 11, 8-bit precision
    jpeg.extend_from_slice(&height.to_be_bytes()); // height
    jpeg.extend_from_slice(&width.to_be_bytes()); // width
    jpeg.extend_from_slice(&[0x03]); // 3 components (YCbCr)
    jpeg.extend_from_slice(&[0xFF, 0xD9]); // EOI
    jpeg
}

/// Helper: creates a valid synthetic animated WebP container with RIFF, VP8X, ANIM, and ANMF frames.
fn create_synthetic_animated_webp(width: u16, height: u16, frame_count: u16) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&0u32.to_le_bytes()); // placeholder size
    data.extend_from_slice(b"WEBP");
    data.extend_from_slice(b"VP8X");
    data.extend_from_slice(&10u32.to_le_bytes()); // VP8X chunk size
    data.push(0x02); // animation flag set (bit 1)
    data.extend_from_slice(&[0u8; 3]); // reserved
    data.extend_from_slice(&(width as u32 - 1).to_le_bytes()[..3]);
    data.extend_from_slice(&(height as u32 - 1).to_le_bytes()[..3]);

    data.extend_from_slice(b"ANIM");
    data.extend_from_slice(&6u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes()); // bg color
    data.extend_from_slice(&0u16.to_le_bytes()); // loop count

    for _ in 0..frame_count {
        data.extend_from_slice(b"ANMF");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()[..3]); // frame x
        data.extend_from_slice(&0u32.to_le_bytes()[..3]); // frame y
        data.extend_from_slice(&(width as u32 - 1).to_le_bytes()[..3]);
        data.extend_from_slice(&(height as u32 - 1).to_le_bytes()[..3]);
        data.extend_from_slice(&100u32.to_le_bytes()[..3]); // duration ms
        data.push(0x00); // flags
    }

    let file_size = (data.len() - 8) as u32;
    data[4..8].copy_from_slice(&file_size.to_le_bytes());
    data
}

#[test]
fn test_ffmpeg_args_strict_parameters() {
    let input = "music/Album/cover.webp";
    let output = "music/Album/animated_cover.mp4";
    let args = build_ffmpeg_webp_to_mp4_args(input, output);

    // 1. Output overwrite flag
    assert!(args.contains(&"-y"), "Must contain -y for non-interactive overwrite");

    // 2. Input file specification
    let input_pos = args.iter().position(|&a| a == "-i").expect("Must contain -i");
    assert_eq!(args[input_pos + 1], input, "-i must be immediately followed by input webp path");

    // 3. Faststart flag for progressive streaming
    let movflags_pos = args.iter().position(|&a| a == "-movflags").expect("Must contain -movflags");
    assert_eq!(args[movflags_pos + 1], "+faststart", "-movflags must be +faststart");

    // 4. Pixel format yuv420p for universal decoder compatibility
    let pix_fmt_pos = args.iter().position(|&a| a == "-pix_fmt").expect("Must contain -pix_fmt");
    assert_eq!(args[pix_fmt_pos + 1], "yuv420p", "-pix_fmt must be yuv420p");

    // 5. Video filter: square crop, <= 1000px scale, 30 fps
    let vf_pos = args.iter().position(|&a| a == "-vf").expect("Must contain -vf");
    let vf_value = args[vf_pos + 1];
    assert!(vf_value.contains("scale='min(1000,iw)':-2"), "Must enforce min(1000,iw) scale: {}", vf_value);
    assert!(vf_value.contains("crop='trunc(iw/2)*2':'trunc(ih/2)*2'"), "Must enforce even dimensions crop: {}", vf_value);
    assert!(vf_value.contains("fps=30"), "Must enforce 30 fps: {}", vf_value);

    // 6. Video codec libx264
    let cv_pos = args.iter().position(|&a| a == "-c:v").expect("Must contain -c:v");
    assert_eq!(args[cv_pos + 1], "libx264", "-c:v must be libx264");

    // 7. CRF 23
    let crf_pos = args.iter().position(|&a| a == "-crf").expect("Must contain -crf");
    assert_eq!(args[crf_pos + 1], "23", "-crf must be 23");

    // 8. Audio disabled (-an)
    assert!(args.contains(&"-an"), "Must disable audio (-an)");

    // 9. Output path at end
    assert_eq!(*args.last().unwrap(), output, "Last argument must be the output mp4 path");
}

#[test]
fn test_symfonium_coverfront_webp_invariant_preserved() {
    // SYMFONIUM INVARIANT: CoverFront (0x03) = image/webp animated is the ONLY
    // configuration that activates animation in Now Playing of Symfonium.
    // The MP4 sidecar is purely complementary and MUST NOT remove or replace
    // the embedded CoverFront image/webp.

    let webp_bytes = create_synthetic_animated_webp(500, 500, 10);
    let frames = validate_animated_webp_bytes(&webp_bytes).expect("Valid animated WebP");
    assert_eq!(frames, 10, "Must detect 10 animation frames");

    // Policy must protect existing animated WebP against incoming static JPEG
    let decision = CoverPreservationPolicy::evaluate(CoverType::AnimatedWebp, CoverType::StaticJpeg);
    assert_eq!(
        decision,
        CoverUpdateDecision::PreserveExisting,
        "CoverFront image/webp MUST be preserved against static image overwrites"
    );

    // Also protect against incoming static PNG
    let decision_png = CoverPreservationPolicy::evaluate(CoverType::AnimatedWebp, CoverType::StaticPng);
    assert_eq!(
        decision_png,
        CoverUpdateDecision::PreserveExisting,
        "CoverFront image/webp MUST be preserved against static PNG overwrites"
    );

    // FLAC PictureType validation
    let mut flac_tag = metaflac::Tag::new();
    flac_tag.add_picture("image/webp", metaflac::block::PictureType::CoverFront, webp_bytes.clone());

    let pics: Vec<_> = flac_tag.pictures().collect();
    assert_eq!(pics.len(), 1);
    assert_eq!(pics[0].picture_type, metaflac::block::PictureType::CoverFront);
    assert_eq!(pics[0].mime_type, "image/webp");
    assert_eq!(pics[0].data, webp_bytes);
}

#[test]
fn test_static_cover_validation_high_res() {
    let temp = TempDir::new().unwrap();

    // 1. Valid high-res cover (1200x1200)
    let valid_cover = temp.path().join("cover.jpg");
    let valid_jpeg_bytes = create_synthetic_jpeg(1200, 1200);
    std::fs::write(&valid_cover, &valid_jpeg_bytes).unwrap();

    let res = validate_static_cover_jpg(&valid_cover);
    assert!(res.is_ok(), "1200x1200 cover must pass validation: {:?}", res);
    let (w, h) = res.unwrap();
    assert_eq!(w, 1200);
    assert_eq!(h, 1200);

    // Also test directory-level validator
    let dir_res = validate_high_res_static_cover(temp.path());
    assert!(dir_res.is_ok(), "Directory-level validation must find cover.jpg");

    // 2. Exactly 1000x1000 must pass (threshold boundary)
    let exact_boundary_cover = temp.path().join("boundary_cover.jpg");
    let boundary_bytes = create_synthetic_jpeg(1000, 1000);
    std::fs::write(&exact_boundary_cover, &boundary_bytes).unwrap();
    assert!(validate_static_cover_jpg(&exact_boundary_cover).is_ok());

    // 3. Low-res cover (500x500) must fail
    let low_res_cover = temp.path().join("low_res_cover.jpg");
    let low_res_bytes = create_synthetic_jpeg(500, 500);
    std::fs::write(&low_res_cover, &low_res_bytes).unwrap();

    let low_res_result = validate_static_cover_jpg(&low_res_cover);
    assert!(low_res_result.is_err(), "500x500 cover must be rejected as low resolution");
    let err_msg = low_res_result.err().unwrap();
    assert!(err_msg.contains("below the minimum required 1000x1000"), "Error must specify resolution requirement: {}", err_msg);

    // 4. Missing cover file must fail
    let missing_cover = temp.path().join("non_existent_cover.jpg");
    let missing_res = validate_static_cover_jpg(&missing_cover);
    assert!(missing_res.is_err());
    assert!(missing_res.err().unwrap().contains("does not exist"));

    // 5. Non-JPEG file must fail
    let fake_cover = temp.path().join("fake_cover.jpg");
    std::fs::write(&fake_cover, b"NOT_A_JPEG_FILE").unwrap();
    let fake_res = validate_static_cover_jpg(&fake_cover);
    assert!(fake_res.is_err());
}

#[tokio::test]
async fn test_sqlite_animated_cover_association_resilience() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

    // Setup base schema without animated_cover_path column
    sqlx::query("CREATE TABLE albums (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Test Album')")
        .execute(&pool)
        .await
        .unwrap();

    let mp4_path = Path::new("/home/music/Test Album/animated_cover.mp4");

    // 1. Without column: returns Ok(false) without crashing
    let res = associate_animated_cover_in_db(&pool, 1, mp4_path).await;
    assert_eq!(res, Ok(false), "Must safely return Ok(false) when column is not present");

    let res_title = associate_animated_cover_by_title_in_db(&pool, "Test Album", mp4_path).await;
    assert_eq!(res_title, Ok(false), "Must safely return Ok(false) by title when column is not present");

    // 2. Add column dynamically (as a future migration or ledger table would provide)
    sqlx::query("ALTER TABLE albums ADD COLUMN animated_cover_path TEXT")
        .execute(&pool)
        .await
        .unwrap();

    // 3. With column: successfully associates and updates the database row
    let res_with_col = associate_animated_cover_in_db(&pool, 1, mp4_path).await;
    assert_eq!(res_with_col, Ok(true), "Must successfully associate when column exists");

    let saved_path: Option<String> = sqlx::query_scalar("SELECT animated_cover_path FROM albums WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(saved_path, Some(mp4_path.to_string_lossy().to_string()));

    // 4. Test title-based association
    let new_mp4_path = Path::new("/home/music/Test Album/updated_animated_cover.mp4");
    let res_by_title = associate_animated_cover_by_title_in_db(&pool, "test album", new_mp4_path).await;
    assert_eq!(res_by_title, Ok(true));

    let updated_path: Option<String> = sqlx::query_scalar("SELECT animated_cover_path FROM albums WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(updated_path, Some(new_mp4_path.to_string_lossy().to_string()));
}

#[tokio::test]
async fn test_transcode_webp_to_mp4_execution_and_coexistence() {
    let temp = TempDir::new().unwrap();
    let album_dir = temp.path().join("Artist - Album");
    tokio::fs::create_dir_all(&album_dir).await.unwrap();

    let webp_path = album_dir.join("cover.webp");
    let cover_jpg_path = album_dir.join("cover.jpg");
    let mp4_path = album_dir.join("animated_cover.mp4");

    let webp_bytes = create_synthetic_animated_webp(300, 300, 5);
    tokio::fs::write(&webp_path, &webp_bytes).await.unwrap();

    let static_jpeg = create_synthetic_jpeg(1000, 1000);
    tokio::fs::write(&cover_jpg_path, &static_jpeg).await.unwrap();

    // Execute transcoding with static cover validation enabled
    let transcode_res = transcode_webp_to_animated_mp4(
        &webp_path,
        &mp4_path,
        true, // require static cover validation
        None,
        None,
    ).await;

    // FFmpeg is present on system so the transcode should produce a valid MP4
    assert!(
        transcode_res.is_ok(),
        "Transcoding animated WebP to MP4 must succeed: {:?}",
        transcode_res
    );
    let out_path = transcode_res.unwrap();
    assert_eq!(out_path, mp4_path);
    assert!(mp4_path.exists(), "animated_cover.mp4 must exist on disk");

    let mp4_meta = std::fs::metadata(&mp4_path).expect("Read metadata of generated MP4");
    assert!(mp4_meta.len() > 100, "MP4 file size must be > 100 bytes (got {})", mp4_meta.len());

    // Invariant check: original WebP and static JPEG sidecars are NEVER deleted
    assert!(webp_path.exists(), "Original cover.webp must be preserved");
    assert!(cover_jpg_path.exists(), "Original cover.jpg must be preserved");
}

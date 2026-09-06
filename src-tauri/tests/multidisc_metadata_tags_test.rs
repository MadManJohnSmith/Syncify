//! Tests for TASK-139: Columna total_discs + Emisión DISCTOTAL/TPOS y TRACKTOTAL por Disco (41 FLAC con Total de Caja)
//!
//! Validates:
//! 1. Migration 0069 applies cleanly, adds `total_discs` to `albums`, backfills existing albums, and maintains it via durable triggers.
//! 2. FLAC Vorbis Comments emit `DISCTOTAL` and `TOTALDISCS` when multidisc metadata is present.
//! 3. FLAC Vorbis Comments emit local disc track total in `TRACKTOTAL` rather than box set overall track total.
//! 4. MP4 `disk` and `trkn` atoms correctly encode `(disc_number, total_discs)` and `(track_number, disc_track_total)`.
//! 5. Domain structures in `syncify-core-domain` (`Album`, `Metadata`, `TidalAlbum::total_discs`) adhere to multidisc contracts.

use sqlx::sqlite::SqlitePoolOptions;
use std::path::{Path, PathBuf};
use syncify_core_domain::metadata::{Album, Metadata, TidalAlbum};
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};

struct TestFlacFile {
    path: PathBuf,
}

impl Drop for TestFlacFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn create_test_flac_file() -> TestFlacFile {
    let path = std::env::temp_dir().join(format!(
        "test_multidisc_flac_{}_{}.flac",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[
        0x80, 0x00, 0x00, 0x22, // Last metadata block (STREAMINFO), length 34
        0x10, 0x00, 0x10, 0x00, // min/max block size
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // min/max frame size
        0x0A, 0xC4, 0x42, 0xF0, // 44.1kHz, 2 channels, 16 bits, 0 samples
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    std::fs::write(&path, &flac_bytes).expect("Failed to write initial FLAC bytes");
    TestFlacFile { path }
}

async fn create_test_mp4_file(path: &Path) -> bool {
    let ffmpeg_out = tokio::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "lavfi",
            "-i", "anullsrc=r=44100:cl=stereo",
            "-t", "1",
            "-c:a", "aac",
            "-b:a", "320k",
            path.to_str().unwrap(),
        ])
        .output()
        .await;

    match ffmpeg_out {
        Ok(out) if out.status.success() => true,
        _ => false,
    }
}

#[tokio::test]
async fn test_migration_0069_albums_total_discs_application_and_triggers() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Connect to in-memory SQLite");

    // 1. Run all migrations up to 0069
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrations must apply cleanly");

    // 2. Verify total_discs column exists on albums
    let col_check: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM pragma_table_info('albums') WHERE name = 'total_discs'",
    )
    .fetch_one(&pool)
    .await
    .expect("Query table_info");
    assert_eq!(col_check.0, 1, "albums table must contain total_discs column");

    // 3. Insert an album and multidisc tracks
    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks) VALUES ('The Wall (Remastered)', 26) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert album");

    // Insert track on Disc 1
    sqlx::query(
        "INSERT INTO tracks (title, album_id, disc_number, track_number) VALUES ('In The Flesh?', ?, 1, 1)",
    )
    .bind(album_id)
    .execute(&pool)
    .await
    .expect("Insert disc 1 track");

    let total_discs_d1: Option<i64> = sqlx::query_scalar("SELECT total_discs FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .expect("Fetch total_discs after disc 1");
    assert_eq!(total_discs_d1, Some(1), "total_discs should be updated to 1");

    // Insert track on Disc 2 -> trigger should update total_discs to 2
    sqlx::query(
        "INSERT INTO tracks (title, album_id, disc_number, track_number) VALUES ('Hey You', ?, 2, 1)",
    )
    .bind(album_id)
    .execute(&pool)
    .await
    .expect("Insert disc 2 track");

    let total_discs_d2: Option<i64> = sqlx::query_scalar("SELECT total_discs FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .expect("Fetch total_discs after disc 2");
    assert_eq!(total_discs_d2, Some(2), "total_discs should automatically update to 2 via trigger");

    // Insert track on Disc 3 -> trigger should update total_discs to 3
    sqlx::query(
        "INSERT INTO tracks (title, album_id, disc_number, track_number) VALUES ('Bonus Disc Track', ?, 3, 1)",
    )
    .bind(album_id)
    .execute(&pool)
    .await
    .expect("Insert disc 3 track");

    let total_discs_d3: Option<i64> = sqlx::query_scalar("SELECT total_discs FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .expect("Fetch total_discs after disc 3");
    assert_eq!(total_discs_d3, Some(3), "total_discs should automatically update to 3 via trigger");

    // Test backfill query directly against an album with total_discs = NULL
    let album2_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, total_discs) VALUES ('Backfill Boxset', 41, NULL) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert album2");

    sqlx::query("INSERT INTO tracks (title, album_id, disc_number) VALUES ('T1', ?, 1)")
        .bind(album2_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO tracks (title, album_id, disc_number) VALUES ('T2', ?, 4)")
        .bind(album2_id)
        .execute(&pool)
        .await
        .unwrap();

    // Re-run backfill logic
    sqlx::query(
        "UPDATE albums SET total_discs = (SELECT MAX(COALESCE(disc_number, 1)) FROM tracks WHERE tracks.album_id = albums.id) WHERE id = ?",
    )
    .bind(album2_id)
    .execute(&pool)
    .await
    .expect("Run backfill query");

    let backfilled_total: Option<i64> = sqlx::query_scalar("SELECT total_discs FROM albums WHERE id = ?")
        .bind(album2_id)
        .fetch_one(&pool)
        .await
        .expect("Fetch backfilled total_discs");
    assert_eq!(backfilled_total, Some(4), "Backfill query should compute MAX disc_number (4)");
}

#[test]
fn test_flac_multidisc_boxset_tracktotal_and_disctotal_emission() {
    let temp_file = create_test_flac_file();
    let path = &temp_file.path;

    // 41-track box set (3 CDs): Disc 2 has 14 tracks, current track is Track 5 on Disc 2
    let meta = FlacMetadata {
        title: "Comfortably Numb".to_string(),
        artist: "Pink Floyd".to_string(),
        album: "The Wall (Experience Edition)".to_string(),
        track_number: 5,
        track_total: 41,              // Overall box set total
        disc_track_total: Some(14),   // Local Disc 2 track total
        disc_number: 2,
        total_discs: Some(3),         // 3 CDs in set
        disc_total: 0,
        ..Default::default()
    };

    let ver = apply_and_verify_flac_tags(path, &meta).expect("apply_and_verify_flac_tags must succeed");
    assert!(ver.tags_match, "Tags must match: {:?}", ver.mismatches);

    let read_tag = metaflac::Tag::read_from_path(path).expect("Read FLAC tags");
    let comments = read_tag.vorbis_comments().expect("Vorbis comments");

    // Multi-disc verification
    assert_eq!(comments.get("DISCNUMBER"), Some(&vec!["2".to_string()]));
    assert_eq!(comments.get("DISCTOTAL"), Some(&vec!["3".to_string()]));
    assert_eq!(comments.get("TOTALDISCS"), Some(&vec!["3".to_string()]));

    // TRACKTOTAL MUST reflect local disc total (14), NOT box set total (41)
    assert_eq!(comments.get("TRACKNUMBER"), Some(&vec!["5".to_string()]));
    assert_eq!(comments.get("TRACKTOTAL"), Some(&vec!["14".to_string()]));
}

#[test]
fn test_flac_fallback_to_disc_total_and_track_total() {
    let temp_file = create_test_flac_file();
    let path = &temp_file.path;

    // Legacy / single-disc structure where disc_track_total and total_discs are None
    let meta = FlacMetadata {
        title: "Single Disc Track".to_string(),
        artist: "Artist".to_string(),
        album: "Single Album".to_string(),
        track_number: 3,
        track_total: 10,
        disc_track_total: None,
        disc_number: 1,
        disc_total: 2,
        total_discs: None,
        ..Default::default()
    };

    let ver = apply_and_verify_flac_tags(path, &meta).expect("apply_and_verify_flac_tags must succeed");
    assert!(ver.tags_match, "Tags must match: {:?}", ver.mismatches);

    let read_tag = metaflac::Tag::read_from_path(path).expect("Read FLAC tags");
    let comments = read_tag.vorbis_comments().expect("Vorbis comments");

    assert_eq!(comments.get("DISCTOTAL"), Some(&vec!["2".to_string()]));
    assert_eq!(comments.get("TOTALDISCS"), Some(&vec!["2".to_string()]));
    assert_eq!(comments.get("TRACKTOTAL"), Some(&vec!["10".to_string()]));
}

#[tokio::test]
async fn test_mp4_multidisc_boxset_disk_atom_and_trkn_atom() {
    let temp_dir = std::env::temp_dir().join(format!("syncify_test_mp4_multidisc_{}", std::process::id()));
    let _ = tokio::fs::create_dir_all(&temp_dir).await;
    let m4a_path = temp_dir.join("test_multidisc.m4a");

    if !create_test_mp4_file(&m4a_path).await {
        eprintln!("ffmpeg not available or failed to create test M4A; skipping MP4 test");
        return;
    }

    let meta = Mp4Metadata {
        title: "Track on Disc 2".to_string(),
        artist: "Boxset Artist".to_string(),
        album: "Complete Anthology Box".to_string(),
        track_number: 7,
        track_total: 41,              // Overall boxset track count
        disc_track_total: Some(13),   // Local Disc 2 track count
        disc_number: 2,
        total_discs: Some(3),         // 3 CDs
        disc_total: 0,
        ..Default::default()
    };

    let ver = apply_and_verify_mp4_tags(&m4a_path, &meta).expect("apply_and_verify_mp4_tags must succeed");
    assert!(ver.tags_match, "Tags must match: {:?}", ver.mismatches);

    // Inspect underlying atoms using mp4ameta
    let tag = mp4ameta::Tag::read_from_path(&m4a_path).expect("Read MP4 tags");
    assert_eq!(tag.disc_number(), Some(2), "MP4 disk atom disc_number");
    assert_eq!(tag.total_discs(), Some(3), "MP4 disk atom total_discs");
    assert_eq!(tag.track_number(), Some(7), "MP4 trkn atom track_number");
    assert_eq!(tag.total_tracks(), Some(13), "MP4 trkn atom total_tracks should reflect local disc total");

    let _ = tokio::fs::remove_file(&m4a_path).await;
    let _ = tokio::fs::remove_dir(&temp_dir).await;
}

/// TASK-75: MP4/M4A files must carry `MUSICBRAINZ_ARTISTID` and `ACOUSTID_ID`
/// as `----:com.apple.iTunes:*` freeform atoms readable by Symfonium, enabling
/// MusicBrainz discography navigation and smart radio.
#[tokio::test]
async fn test_mp4_acoustid_and_musicbrainz_artistid_freeform_atoms() {
    let temp_dir = std::env::temp_dir().join(format!("syncify_test_mp4_acoustid_{}", std::process::id()));
    let _ = tokio::fs::create_dir_all(&temp_dir).await;
    let m4a_path = temp_dir.join("test_acoustid_identity.m4a");

    if !create_test_mp4_file(&m4a_path).await {
        eprintln!("ffmpeg not available or failed to create test M4A; skipping MP4 test");
        return;
    }

    let meta = Mp4Metadata {
        title: "Acoustic Identity Track".to_string(),
        artist: "Identity Artist".to_string(),
        album: "Identity Album".to_string(),
        musicbrainz_track_id: Some("11111111-2222-3333-4444-555555555555".to_string()),
        musicbrainz_artist_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()),
        acoustid_id: Some("0e0a8a5c-8d93-4ce5-8b0a-1f2e3d4c5b6a".to_string()),
        acoustid_fingerprint: Some("AQAA0bmSQIhQJEAiFBCSEceE5McJ8kieBE-OP9qBo0C0".to_string()),
        ..Default::default()
    };

    let ver = apply_and_verify_mp4_tags(&m4a_path, &meta).expect("apply_and_verify_mp4_tags must succeed");
    assert!(ver.tags_match, "Tags must match: {:?}", ver.mismatches);
    assert!(ver.musicbrainz_present, "musicbrainz_present must be reported");
    assert!(ver.acoustid_present, "acoustid_present must be reported when ACOUSTID_ID is expected");

    // Inspect underlying freeform atoms using mp4ameta
    let tag = mp4ameta::Tag::read_from_path(&m4a_path).expect("Read MP4 tags");

    let mbid_upper = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "MUSICBRAINZ_ARTISTID");
    assert_eq!(
        tag.strings_of(&mbid_upper).next(),
        Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
        "----:com.apple.iTunes:MUSICBRAINZ_ARTISTID freeform atom must carry the artist MBID"
    );
    // Legacy pinned variant must coexist (readers pinned to the iTunes-style name).
    let mbid_legacy = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "MusicBrainz Artist Id");
    assert_eq!(
        tag.strings_of(&mbid_legacy).next(),
        Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"),
        "Legacy 'MusicBrainz Artist Id' atom must remain written"
    );

    let acoustid_upper = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "ACOUSTID_ID");
    assert_eq!(
        tag.strings_of(&acoustid_upper).next(),
        Some("0e0a8a5c-8d93-4ce5-8b0a-1f2e3d4c5b6a"),
        "----:com.apple.iTunes:ACOUSTID_ID freeform atom must carry the AcoustID"
    );
    let fingerprint_upper = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "ACOUSTID_FINGERPRINT");
    assert_eq!(
        tag.strings_of(&fingerprint_upper).next(),
        Some("AQAA0bmSQIhQJEAiFBCSEceE5McJ8kieBE-OP9qBo0C0"),
        "----:com.apple.iTunes:ACOUSTID_FINGERPRINT freeform atom must carry the Chromaprint"
    );

    let _ = tokio::fs::remove_file(&m4a_path).await;
    let _ = tokio::fs::remove_dir(&temp_dir).await;
}

#[test]
fn test_domain_structures_multidisc_contracts() {
    // 1. Album struct in syncify-core-domain
    let album = Album::new("Anthology Box Set", Some(4));
    assert_eq!(album.title, "Anthology Box Set");
    assert_eq!(album.total_discs, Some(4));

    // 2. Metadata struct in syncify-core-domain
    let meta = Metadata {
        title: "Song 1".to_string(),
        artist: "Band".to_string(),
        album: Some("Anthology Box Set".to_string()),
        album_artist: Some("Band".to_string()),
        track_number: Some(1),
        track_total: Some(12),
        disc_number: Some(2),
        total_discs: Some(4),
        isrc: Some("USUM71702778".to_string()),
        release_year: Some("2024".to_string()),
        release_date: Some("2024-01-01".to_string()),
    };
    assert_eq!(meta.effective_disc_total(), Some(4));
    assert_eq!(meta.effective_track_total(), Some(12));

    // 3. TidalAlbum total_discs method
    let tidal_album = TidalAlbum {
        id: Some(123456),
        title: "Tidal Multidisc Album".to_string(),
        release_date: Some("2024-05-10".to_string()),
        cover: None,
        artist: None,
        artists: None,
        number_of_tracks: Some(41),
        number_of_volumes: Some(3),
        copyright: None,
        upc: None,
        album_type: None,
    };
    assert_eq!(tidal_album.total_discs(), Some(3));
}

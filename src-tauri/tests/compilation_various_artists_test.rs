//! TASK-69: Compilation & Various Artists Unification Integration Test Suite
//!
//! Validates:
//! 1. Multi-artist albums write VorbisComments with ALBUMARTIST=Various Artists and COMPILATION=1.
//! 2. Compilations with a designated compiler preserve that artist in ALBUMARTIST while emitting COMPILATION=1.
//! 3. Mono-artist albums preserve their normal artist in ALBUMARTIST without writing COMPILATION=1.
//! 4. Database sync-time persistence unifies divergent artist tracks under a single album entry
//!    with "Various Artists" as primary album artist, preventing album fragmentation.
//! 5. Mono-artist releases in DB persist their individual artist as primary without adding Various Artists.

use std::fs;
use std::path::{Path, PathBuf};
use syncify_flac_writer::{
    apply_and_verify_flac_tags, apply_flac_tags, detect_album_is_compilation,
    unify_album_compilation_metadata, FlacMetadata,
};
use syncify_tauri_lib::services::enrichment::{
    detect_compilation_from_origin_tracks, is_multi_artist_compilation,
    unify_origin_album_tracks, EnrichmentEngine, OriginTrackMetadata, SyncTrackInput,
};
use tempfile::tempdir;

struct TestFlacGuard {
    path: PathBuf,
}

impl Drop for TestFlacGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn create_synthetic_flac(dir: &Path, name: &str) -> TestFlacGuard {
    let path = dir.join(name);
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
    fs::write(&path, &flac_bytes).expect("Failed to write synthetic FLAC header");
    TestFlacGuard { path }
}

#[test]
fn test_multi_artist_album_emits_various_artists_and_compilation_flag() {
    let dir = tempdir().expect("tempdir");
    let track1_file = create_synthetic_flac(dir.path(), "track1_multi.flac");
    let track2_file = create_synthetic_flac(dir.path(), "track2_multi.flac");

    let mut tracks = vec![
        FlacMetadata {
            title: "Midnight City".to_string(),
            artist: "M83".to_string(),
            album: "Synthwave Classics 2024".to_string(),
            track_number: 1,
            track_total: 2,
            ..Default::default()
        },
        FlacMetadata {
            title: "Nightcall".to_string(),
            artist: "Kavinsky".to_string(),
            album: "Synthwave Classics 2024".to_string(),
            track_number: 2,
            track_total: 2,
            ..Default::default()
        },
    ];

    // Verify detection
    assert!(
        detect_album_is_compilation(&tracks),
        "Album with M83 and Kavinsky must be detected as compilation"
    );

    // Unify album metadata
    unify_album_compilation_metadata(&mut tracks, None);

    assert_eq!(tracks[0].compilation, Some(true));
    assert_eq!(tracks[0].album_artist, Some("Various Artists".to_string()));
    assert_eq!(tracks[1].compilation, Some(true));
    assert_eq!(tracks[1].album_artist, Some("Various Artists".to_string()));

    // Apply FLAC tags and verify roundtrip
    let rep1 = apply_and_verify_flac_tags(&track1_file.path, &tracks[0]).expect("tag track 1");
    assert!(rep1.tags_match, "Tags match failure: {:?}", rep1.mismatches);

    let rep2 = apply_and_verify_flac_tags(&track2_file.path, &tracks[1]).expect("tag track 2");
    assert!(rep2.tags_match, "Tags match failure: {:?}", rep2.mismatches);

    // Read back physical VorbisComments from track 1
    let tag1 = metaflac::Tag::read_from_path(&track1_file.path).expect("read tag1");
    let comments1 = tag1.vorbis_comments().expect("comments1");
    assert_eq!(
        comments1.get("ALBUMARTIST"),
        Some(&vec!["Various Artists".to_string()]),
        "Track 1 ALBUMARTIST must be 'Various Artists'"
    );
    assert_eq!(
        comments1.get("COMPILATION"),
        Some(&vec!["1".to_string()]),
        "Track 1 COMPILATION must be '1'"
    );
    assert_eq!(
        comments1.get("ARTIST"),
        Some(&vec!["M83".to_string()]),
        "Track 1 ARTIST must preserve 'M83'"
    );

    // Read back physical VorbisComments from track 2
    let tag2 = metaflac::Tag::read_from_path(&track2_file.path).expect("read tag2");
    let comments2 = tag2.vorbis_comments().expect("comments2");
    assert_eq!(
        comments2.get("ALBUMARTIST"),
        Some(&vec!["Various Artists".to_string()]),
        "Track 2 ALBUMARTIST must be 'Various Artists'"
    );
    assert_eq!(
        comments2.get("COMPILATION"),
        Some(&vec!["1".to_string()]),
        "Track 2 COMPILATION must be '1'"
    );
    assert_eq!(
        comments2.get("ARTIST"),
        Some(&vec!["Kavinsky".to_string()]),
        "Track 2 ARTIST must preserve 'Kavinsky'"
    );
}

#[test]
fn test_compilation_with_compiler_artist_preserved() {
    let dir = tempdir().expect("tempdir");
    let track_file = create_synthetic_flac(dir.path(), "soundtrack_track.flac");

    let mut tracks = vec![
        FlacMetadata {
            title: "Misirlou".to_string(),
            artist: "Dick Dale & The Del-Tones".to_string(),
            album: "Pulp Fiction OST".to_string(),
            track_number: 1,
            ..Default::default()
        },
        FlacMetadata {
            title: "Jungle Boogie".to_string(),
            artist: "Kool & The Gang".to_string(),
            album: "Pulp Fiction OST".to_string(),
            track_number: 2,
            ..Default::default()
        },
    ];

    // Unify with custom compiler artist
    unify_album_compilation_metadata(&mut tracks, Some("Quentin Tarantino"));

    assert_eq!(tracks[0].compilation, Some(true));
    assert_eq!(tracks[0].album_artist, Some("Quentin Tarantino".to_string()));

    apply_flac_tags(&track_file.path, &tracks[0]).expect("apply flac tags");

    let tag = metaflac::Tag::read_from_path(&track_file.path).expect("read tag");
    let comments = tag.vorbis_comments().expect("comments");

    assert_eq!(
        comments.get("ALBUMARTIST"),
        Some(&vec!["Quentin Tarantino".to_string()]),
        "Compiler artist must be preserved in ALBUMARTIST"
    );
    assert_eq!(
        comments.get("COMPILATION"),
        Some(&vec!["1".to_string()]),
        "COMPILATION=1 must be set for compilation releases"
    );
    assert_eq!(
        comments.get("ARTIST"),
        Some(&vec!["Dick Dale & The Del-Tones".to_string()])
    );
}

#[test]
fn test_mono_artist_album_preserves_artist_and_omits_compilation() {
    let dir = tempdir().expect("tempdir");
    let track1_file = create_synthetic_flac(dir.path(), "track1_mono.flac");
    let track2_file = create_synthetic_flac(dir.path(), "track2_mono.flac");

    let mut tracks = vec![
        FlacMetadata {
            title: "Speak to Me".to_string(),
            artist: "Pink Floyd".to_string(),
            album: "The Dark Side of the Moon".to_string(),
            album_artist: Some("Pink Floyd".to_string()),
            compilation: None,
            track_number: 1,
            track_total: 2,
            ..Default::default()
        },
        FlacMetadata {
            title: "Breathe".to_string(),
            artist: "Pink Floyd".to_string(),
            album: "The Dark Side of the Moon".to_string(),
            album_artist: Some("Pink Floyd".to_string()),
            compilation: None,
            track_number: 2,
            track_total: 2,
            ..Default::default()
        },
    ];

    // Verify detection
    assert!(
        !detect_album_is_compilation(&tracks),
        "Mono-artist album must NOT be detected as compilation"
    );

    unify_album_compilation_metadata(&mut tracks, None);

    assert_ne!(tracks[0].compilation, Some(true));
    assert_eq!(tracks[0].album_artist, Some("Pink Floyd".to_string()));
    assert_ne!(tracks[1].compilation, Some(true));
    assert_eq!(tracks[1].album_artist, Some("Pink Floyd".to_string()));

    let rep1 = apply_and_verify_flac_tags(&track1_file.path, &tracks[0]).expect("tag track 1");
    assert!(rep1.tags_match, "Tags match failure: {:?}", rep1.mismatches);

    let rep2 = apply_and_verify_flac_tags(&track2_file.path, &tracks[1]).expect("tag track 2");
    assert!(rep2.tags_match, "Tags match failure: {:?}", rep2.mismatches);

    // Read back physical VorbisComments
    let tag1 = metaflac::Tag::read_from_path(&track1_file.path).expect("read tag1");
    let comments1 = tag1.vorbis_comments().expect("comments1");
    assert_eq!(
        comments1.get("ALBUMARTIST"),
        Some(&vec!["Pink Floyd".to_string()]),
        "Mono-artist ALBUMARTIST must be 'Pink Floyd'"
    );
    assert!(
        comments1.get("COMPILATION").is_none(),
        "COMPILATION tag must NOT be present on mono-artist album"
    );

    let tag2 = metaflac::Tag::read_from_path(&track2_file.path).expect("read tag2");
    let comments2 = tag2.vorbis_comments().expect("comments2");
    assert_eq!(
        comments2.get("ALBUMARTIST"),
        Some(&vec!["Pink Floyd".to_string()]),
        "Mono-artist ALBUMARTIST must be 'Pink Floyd'"
    );
    assert!(
        comments2.get("COMPILATION").is_none(),
        "COMPILATION tag must NOT be present on mono-artist album"
    );
}

#[test]
fn test_origin_tracks_compilation_helpers() {
    let mut comp_tracks = vec![
        OriginTrackMetadata {
            title: Some("Song 1".to_string()),
            artist: Some("Artist Alpha".to_string()),
            album: Some("Summer Festival 2024".to_string()),
            ..Default::default()
        },
        OriginTrackMetadata {
            title: Some("Song 2".to_string()),
            artist: Some("Artist Beta".to_string()),
            album: Some("Summer Festival 2024".to_string()),
            ..Default::default()
        },
    ];

    assert!(is_multi_artist_compilation(&["Artist Alpha", "Artist Beta"]));
    assert!(!is_multi_artist_compilation(&["Artist Alpha", "Artist Alpha"]));
    assert!(detect_compilation_from_origin_tracks(&comp_tracks));

    unify_origin_album_tracks(&mut comp_tracks, None);

    for t in &comp_tracks {
        assert_eq!(t.album_artist.as_deref(), Some("Various Artists"));
        assert_eq!(t.release_type.as_deref(), Some("compilation"));
    }
}

async fn setup_test_db() -> sqlx::SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

async fn create_test_account(pool: &sqlx::SqlitePool, service_name: &str, display_name: &str) -> (i64, i64) {
    let service_id: i64 = match sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service_name)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
    {
        Some(id) => id,
        None => {
            sqlx::query_scalar("INSERT OR IGNORE INTO services (name) VALUES (?) RETURNING id")
                .bind(service_name)
                .fetch_one(pool)
                .await
                .unwrap_or(3)
        }
    };

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, ?, 1) RETURNING id"
    )
    .bind(service_id)
    .bind(display_name)
    .fetch_one(pool)
    .await
    .unwrap();

    (service_id, account_id)
}

#[tokio::test]
async fn test_db_persistence_unifies_compilation_album_and_sets_various_artists() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "tidal", "Tidal User 1").await;

    let engine = EnrichmentEngine::new();

    // Track 1: Artist "Daft Punk" on "Greatest Electro Hits"
    let input1 = SyncTrackInput {
        service_id,
        account_id,
        service_name: "tidal".to_string(),
        service_track_id: "td-tr-001".to_string(),
        origin_meta: OriginTrackMetadata {
            title: Some("One More Time".to_string()),
            artist: Some("Daft Punk".to_string()),
            album: Some("Greatest Electro Hits".to_string()),
            album_artist: None,
            release_type: Some("compilation".to_string()),
            track_number: Some(1),
            track_total: Some(2),
            source_name: "tidal".to_string(),
            ..Default::default()
        },
        cover_art_url: None,
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: Some("td-alb-999".to_string()),
        ..Default::default()
    };

    let res1 = engine
        .enrich_and_persist_sync_track(&pool, input1)
        .await
        .expect("persist track 1");

    assert!(res1.album_id.is_some());

    // Track 2: Artist "Justice" on "Greatest Electro Hits" (same album provider ID td-alb-999)
    let input2 = SyncTrackInput {
        service_id,
        account_id,
        service_name: "tidal".to_string(),
        service_track_id: "td-tr-002".to_string(),
        origin_meta: OriginTrackMetadata {
            title: Some("D.A.N.C.E.".to_string()),
            artist: Some("Justice".to_string()),
            album: Some("Greatest Electro Hits".to_string()),
            album_artist: None,
            track_number: Some(2),
            track_total: Some(2),
            source_name: "tidal".to_string(),
            ..Default::default()
        },
        cover_art_url: None,
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: Some("td-alb-999".to_string()),
        ..Default::default()
    };

    let res2 = engine
        .enrich_and_persist_sync_track(&pool, input2)
        .await
        .expect("persist track 2");

    assert!(res2.album_id.is_some());

    // Verify albums table: exactly 1 unified album entry, NOT fragmented!
    let album_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums")
        .fetch_one(&pool)
        .await
        .expect("count albums");
    assert_eq!(
        album_count, 1,
        "Compilation tracks must unite under exactly 1 album row, no fragmentation"
    );

    // Verify both tracks link to the same album
    let track1_alb: (i64,) = sqlx::query_as("SELECT album_id FROM tracks WHERE id = ?")
        .bind(res1.track_id)
        .fetch_one(&pool)
        .await
        .expect("track1 album");
    let track2_alb: (i64,) = sqlx::query_as("SELECT album_id FROM tracks WHERE id = ?")
        .bind(res2.track_id)
        .fetch_one(&pool)
        .await
        .expect("track2 album");
    assert_eq!(track1_alb.0, track2_alb.0, "Both tracks must point to the same album_id");

    // Verify primary album artist is "Various Artists"
    let primary_album_artist: (String,) = sqlx::query_as(
        r#"
        SELECT ar.name FROM album_artists aa
        JOIN artists ar ON ar.id = aa.artist_id
        WHERE aa.album_id = ? AND aa.is_primary = 1
        LIMIT 1
        "#
    )
    .bind(track1_alb.0)
    .fetch_one(&pool)
    .await
    .expect("primary album artist");

    assert_eq!(
        primary_album_artist.0, "Various Artists",
        "Primary album artist in album_artists must be 'Various Artists'"
    );
}

#[tokio::test]
async fn test_db_persistence_preserves_mono_artist_album() {
    let pool = setup_test_db().await;
    let (service_id, account_id) = create_test_account(&pool, "tidal", "Tidal User 2").await;

    let engine = EnrichmentEngine::new();

    let input1 = SyncTrackInput {
        service_id,
        account_id,
        service_name: "tidal".to_string(),
        service_track_id: "td-tr-101".to_string(),
        origin_meta: OriginTrackMetadata {
            title: Some("Airbag".to_string()),
            artist: Some("Radiohead".to_string()),
            album: Some("OK Computer".to_string()),
            album_artist: Some("Radiohead".to_string()),
            track_number: Some(1),
            track_total: Some(12),
            source_name: "tidal".to_string(),
            ..Default::default()
        },
        cover_art_url: None,
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: Some("td-alb-okc".to_string()),
        ..Default::default()
    };

    let res1 = engine
        .enrich_and_persist_sync_track(&pool, input1)
        .await
        .expect("persist track 1");

    assert!(res1.album_id.is_some());

    let input2 = SyncTrackInput {
        service_id,
        account_id,
        service_name: "tidal".to_string(),
        service_track_id: "td-tr-102".to_string(),
        origin_meta: OriginTrackMetadata {
            title: Some("Paranoid Android".to_string()),
            artist: Some("Radiohead".to_string()),
            album: Some("OK Computer".to_string()),
            album_artist: Some("Radiohead".to_string()),
            track_number: Some(2),
            track_total: Some(12),
            source_name: "tidal".to_string(),
            ..Default::default()
        },
        cover_art_url: None,
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: Some("td-alb-okc".to_string()),
        ..Default::default()
    };

    let res2 = engine
        .enrich_and_persist_sync_track(&pool, input2)
        .await
        .expect("persist track 2");

    assert!(res2.album_id.is_some());

    // Exactly 1 album
    let album_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums")
        .fetch_one(&pool)
        .await
        .expect("count albums");
    assert_eq!(album_count, 1);

    // Primary album artist is "Radiohead"
    let primary_album_artist: (String,) = sqlx::query_as(
        r#"
        SELECT ar.name FROM album_artists aa
        JOIN artists ar ON ar.id = aa.artist_id
        WHERE aa.album_id = (SELECT album_id FROM tracks WHERE id = ?) AND aa.is_primary = 1
        LIMIT 1
        "#
    )
    .bind(res1.track_id)
    .fetch_one(&pool)
    .await
    .expect("primary album artist");

    assert_eq!(primary_album_artist.0, "Radiohead");

    // No "Various Artists" in artists table
    let va_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artists WHERE name = 'Various Artists' COLLATE NOCASE")
        .fetch_one(&pool)
        .await
        .expect("count va");
    assert_eq!(va_count, 0, "Mono-artist album must not insert 'Various Artists'");
}

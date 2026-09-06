//! Integration & Unit Test Suite for TASK-141:
//! MusicBrainz artist-credit deserialization and MUSICBRAINZ_ARTISTID tag emission.
//!
//! Verifies:
//! 1. Kebab-case `artist-credit` deserialization in `MusicBrainzRecording` from real API payloads.
//! 2. Snake-case `artist_credit` deserialization via serde aliases.
//! 3. Deserialization of `artist-credit` in `Release`, `MusicBrainzReleaseWithMedia`, and `MusicBrainzReleaseTrack`.
//! 4. Physical Vorbis Comment emission of `MUSICBRAINZ_ARTISTID` on FLAC files.
//! 5. Trimming of whitespace on `MUSICBRAINZ_ARTISTID` during tagging.
//! 6. Database persistence of `artists.musicbrainz_id` during metadata enrichment.
//! 7. Enforcement of TASK-127: rejection of synthetic apocryphal MBIDs during database persistence.
//! 8. Repair/backfill of stale or invalid (e.g. 'NOT_FOUND') `artists.musicbrainz_id`.

use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_metadata_domain::{chrono_now_iso, EnrichedMetadata};
use syncify_tauri_lib::services::enrichment::EnrichmentEngine;
use syncify_tauri_lib::services::musicbrainz::{
    MusicBrainzRecording, MusicBrainzReleaseWithMedia,
};

const REAL_MUSICBRAINZ_RECORDING_JSON: &str = r#"{
  "id": "b32810a9-2b81-4279-bbd1-580ea52e729a",
  "title": "Bohemian Rhapsody",
  "length": 354000,
  "video": false,
  "artist-credit": [
    {
      "name": "Queen",
      "artist": {
        "id": "0383dadf-2a4e-4d10-a46a-e6e041da8eb3",
        "name": "Queen",
        "sort-name": "Queen",
        "disambiguation": "UK rock band"
      },
      "joinphrase": ""
    }
  ],
  "releases": [
    {
      "id": "71e54911-3990-410d-85f0-612d7c0bb5bb",
      "title": "A Night at the Opera",
      "status": "Official",
      "country": "GB",
      "date": "1975-11-21",
      "barcode": "5099968460724",
      "release-group": {
        "id": "270adba8-a734-3c66-8800-474cf481c5d0",
        "title": "A Night at the Opera",
        "primary-type": "Album"
      },
      "artist-credit": [
        {
          "name": "Queen",
          "artist": {
            "id": "0383dadf-2a4e-4d10-a46a-e6e041da8eb3",
            "name": "Queen",
            "sort-name": "Queen"
          }
        }
      ]
    }
  ],
  "genres": [
    {
      "name": "rock"
    },
    {
      "name": "progressive rock"
    }
  ],
  "tags": [
    {
      "name": "classic rock"
    }
  ]
}"#;

const SNAKE_CASE_RECORDING_JSON: &str = r#"{
  "id": "5441c29d-3602-48f7-b1a9-30704df52227",
  "title": "Heroes",
  "artist_credit": [
    {
      "name": "David Bowie",
      "artist": {
        "id": "5441c29d-3602-48f7-b1a9-30704df52227",
        "name": "David Bowie",
        "sort_name": "Bowie, David"
      }
    }
  ]
}"#;

const REAL_MUSICBRAINZ_RELEASE_JSON: &str = r#"{
  "id": "71e54911-3990-410d-85f0-612d7c0bb5bb",
  "title": "A Night at the Opera",
  "date": "1975-11-21",
  "barcode": "5099968460724",
  "artist-credit": [
    {
      "name": "Queen",
      "artist": {
        "id": "0383dadf-2a4e-4d10-a46a-e6e041da8eb3",
        "name": "Queen",
        "sort-name": "Queen"
      }
    }
  ],
  "media": [
    {
      "position": 1,
      "format": "12\" Vinyl",
      "track-count": 12,
      "tracks": [
        {
          "id": "c0556272-3be9-4da4-8b63-c7743d2c8821",
          "position": 11,
          "number": "A11",
          "title": "Bohemian Rhapsody",
          "length": 354000,
          "recording": {
            "id": "b32810a9-2b81-4279-bbd1-580ea52e729a",
            "title": "Bohemian Rhapsody",
            "length": 354000,
            "first-release-date": "1975-10-31"
          },
          "artist-credit": [
            {
              "name": "Queen",
              "artist": {
                "id": "0383dadf-2a4e-4d10-a46a-e6e041da8eb3",
                "name": "Queen"
              }
            }
          ]
        }
      ]
    }
  ]
}"#;

/// Create a minimal valid FLAC file containing a valid STREAMINFO block
fn create_test_flac_file() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let path = temp_dir.path().join("test_track.flac");

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
    (temp_dir, path)
}

#[test]
fn test_musicbrainz_recording_kebab_case_artist_credit_deserialization() {
    let rec: MusicBrainzRecording = serde_json::from_str(REAL_MUSICBRAINZ_RECORDING_JSON)
        .expect("Failed to deserialize MusicBrainzRecording with kebab-case artist-credit");

    assert_eq!(rec.id, "b32810a9-2b81-4279-bbd1-580ea52e729a");
    assert_eq!(rec.title, "Bohemian Rhapsody");

    let credits = rec.artist_credit.expect("artist_credit was None; kebab-case 'artist-credit' deserialization broken!");
    assert_eq!(credits.len(), 1);

    let first = &credits[0];
    assert_eq!(first.name, "Queen");
    assert_eq!(first.artist.id, "0383dadf-2a4e-4d10-a46a-e6e041da8eb3");
    assert_eq!(first.artist.name, "Queen");
    assert_eq!(first.artist.sort_name.as_deref(), Some("Queen"));
    assert_eq!(first.artist.disambiguation.as_deref(), Some("UK rock band"));

    // Releases artist-credit check
    let releases = rec.releases.expect("releases was None");
    assert_eq!(releases.len(), 1);
    let rel_credits = releases[0].artist_credit.as_ref().expect("release artist-credit was None");
    assert_eq!(rel_credits[0].artist.id, "0383dadf-2a4e-4d10-a46a-e6e041da8eb3");
}

#[test]
fn test_musicbrainz_recording_snake_case_alias() {
    let rec: MusicBrainzRecording = serde_json::from_str(SNAKE_CASE_RECORDING_JSON)
        .expect("Failed to deserialize MusicBrainzRecording with snake_case artist_credit");

    let credits = rec.artist_credit.expect("artist_credit alias failed to deserialize");
    assert_eq!(credits.len(), 1);
    assert_eq!(credits[0].name, "David Bowie");
    assert_eq!(credits[0].artist.id, "5441c29d-3602-48f7-b1a9-30704df52227");
    assert_eq!(credits[0].artist.name, "David Bowie");
    assert_eq!(credits[0].artist.sort_name.as_deref(), Some("Bowie, David"));
}

#[test]
fn test_musicbrainz_release_and_tracks_artist_credit() {
    let rel: MusicBrainzReleaseWithMedia = serde_json::from_str(REAL_MUSICBRAINZ_RELEASE_JSON)
        .expect("Failed to deserialize MusicBrainzReleaseWithMedia");

    let rel_credits = rel.artist_credit.expect("Release artist-credit was None");
    assert_eq!(rel_credits[0].artist.id, "0383dadf-2a4e-4d10-a46a-e6e041da8eb3");

    let media = rel.media.expect("Media was None");
    let tracks = media[0].tracks.as_ref().expect("Tracks was None");
    let track_credits = tracks[0].artist_credit.as_ref().expect("Track artist-credit was None");
    assert_eq!(track_credits[0].artist.id, "0383dadf-2a4e-4d10-a46a-e6e041da8eb3");
}

#[test]
fn test_flac_vorbis_comment_musicbrainz_artistid_emission() {
    let (_temp_dir, flac_path) = create_test_flac_file();

    let meta = FlacMetadata {
        title: "Bohemian Rhapsody".to_string(),
        artist: "Queen".to_string(),
        album: "A Night at the Opera".to_string(),
        musicbrainz_artist_id: Some("0383dadf-2a4e-4d10-a46a-e6e041da8eb3".to_string()),
        musicbrainz_track_id: Some("b32810a9-2b81-4279-bbd1-580ea52e729a".to_string()),
        musicbrainz_album_id: Some("71e54911-3990-410d-85f0-612d7c0bb5bb".to_string()),
        ..Default::default()
    };

    let res = apply_and_verify_flac_tags(&flac_path, &meta);
    assert!(res.is_ok(), "Failed to apply and verify FLAC tags: {:?}", res.err());
    let verification = res.unwrap();
    assert!(verification.flac_valid, "FLAC file invalid after tagging");
    assert!(verification.tags_match, "Tags mismatch: {:?}", verification.mismatches);

    // Verify directly with metaflac low-level reader
    let tag = metaflac::Tag::read_from_path(&flac_path).expect("Failed to read tag with metaflac");
    let vorbis = tag.vorbis_comments().expect("No Vorbis comments found in FLAC");

    let artist_mbids = vorbis.get("MUSICBRAINZ_ARTISTID").expect("MUSICBRAINZ_ARTISTID not written!");
    assert_eq!(artist_mbids, &["0383dadf-2a4e-4d10-a46a-e6e041da8eb3".to_string()]);

    let track_mbids = vorbis.get("MUSICBRAINZ_TRACKID").expect("MUSICBRAINZ_TRACKID not written!");
    assert_eq!(track_mbids, &["b32810a9-2b81-4279-bbd1-580ea52e729a".to_string()]);
}

#[test]
fn test_flac_vorbis_comment_musicbrainz_artistid_trimmed() {
    let (_temp_dir, flac_path) = create_test_flac_file();

    let meta = FlacMetadata {
        title: "Space Oddity".to_string(),
        artist: "David Bowie".to_string(),
        album: "David Bowie".to_string(),
        musicbrainz_artist_id: Some("  5441c29d-3602-48f7-b1a9-30704df52227 \n\t".to_string()),
        ..Default::default()
    };

    let res = apply_and_verify_flac_tags(&flac_path, &meta);
    assert!(res.is_ok(), "apply_and_verify_flac_tags failed: {:?}", res.err());

    let tag = metaflac::Tag::read_from_path(&flac_path).expect("Failed to read tag with metaflac");
    let vorbis = tag.vorbis_comments().expect("No Vorbis comments found in FLAC");
    let artist_mbids = vorbis.get("MUSICBRAINZ_ARTISTID").expect("MUSICBRAINZ_ARTISTID not found");
    assert_eq!(artist_mbids, &["5441c29d-3602-48f7-b1a9-30704df52227".to_string()]);
}

#[tokio::test]
async fn test_database_persistence_and_backfill_of_artist_mbid() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

    // Setup minimal schema matching syncify.db
    sqlx::query("CREATE TABLE artists (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, musicbrainz_id TEXT);")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE tracks (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, album_id INTEGER, track_number INTEGER, disc_number INTEGER, isrc TEXT, release_year INTEGER, record_label TEXT, musicbrainz_id TEXT, enrichment_status TEXT, enriched_at TEXT);")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE track_artists (track_id INTEGER, artist_id INTEGER, role TEXT, PRIMARY KEY(track_id, artist_id));")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE albums (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, release_date TEXT, upc TEXT, total_tracks INTEGER, label TEXT, musicbrainz_id TEXT);")
        .execute(&pool).await.unwrap();

    // Insert artist initially without MBID
    sqlx::query("INSERT INTO artists (name) VALUES ('Queen');").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tracks (title, enrichment_status) VALUES ('Bohemian Rhapsody', 'pending');").execute(&pool).await.unwrap();

    let engine = EnrichmentEngine::new();
    let mut meta = EnrichedMetadata::default();
    let now = chrono_now_iso();

    meta.artist.merge_candidate(Some("Queen".to_string()), "stream", 1.0, &now);
    meta.musicbrainz_artist_id.merge_candidate(
        Some("0383dadf-2a4e-4d10-a46a-e6e041da8eb3".to_string()),
        "musicbrainz",
        0.95,
        &now,
    );

    let res = engine.apply_to_database(&pool, 1, &meta, None).await;
    assert!(res.is_ok(), "apply_to_database failed: {:?}", res.err());

    let (name, mbid): (String, Option<String>) = sqlx::query_as("SELECT name, musicbrainz_id FROM artists WHERE id = 1")
        .fetch_one(&pool).await.unwrap();

    assert_eq!(name, "Queen");
    assert_eq!(mbid.as_deref(), Some("0383dadf-2a4e-4d10-a46a-e6e041da8eb3"));
}

#[tokio::test]
async fn test_database_persistence_rejects_synthetic_apocryphal_mbid() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

    sqlx::query("CREATE TABLE artists (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, musicbrainz_id TEXT);")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE tracks (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, album_id INTEGER, track_number INTEGER, disc_number INTEGER, isrc TEXT, release_year INTEGER, record_label TEXT, musicbrainz_id TEXT, enrichment_status TEXT, enriched_at TEXT);")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE track_artists (track_id INTEGER, artist_id INTEGER, role TEXT, PRIMARY KEY(track_id, artist_id));")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE albums (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, release_date TEXT, upc TEXT, total_tracks INTEGER, label TEXT, musicbrainz_id TEXT);")
        .execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO artists (name) VALUES ('Alan Mearns');").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tracks (title, enrichment_status) VALUES ('Track 1', 'pending');").execute(&pool).await.unwrap();

    let engine = EnrichmentEngine::new();
    let mut meta = EnrichedMetadata::default();
    let now = chrono_now_iso();

    meta.artist.merge_candidate(Some("Alan Mearns".to_string()), "stream", 1.0, &now);
    // Synthetic apocryphal ID from TASK-127
    meta.musicbrainz_artist_id.merge_candidate(
        Some("e774d650-ebf2-5345-acff-8a5ad5cb0ce9".to_string()),
        "musicbrainz",
        0.95,
        &now,
    );

    let res = engine.apply_to_database(&pool, 1, &meta, None).await;
    assert!(res.is_ok());

    let (name, mbid): (String, Option<String>) = sqlx::query_as("SELECT name, musicbrainz_id FROM artists WHERE id = 1")
        .fetch_one(&pool).await.unwrap();

    assert_eq!(name, "Alan Mearns");
    assert_eq!(mbid, None, "Synthetic apocryphal MBID must NOT be saved to artists table");
}

#[tokio::test]
async fn test_database_persistence_repairs_stale_not_found_mbid() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

    sqlx::query("CREATE TABLE artists (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, musicbrainz_id TEXT);")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE tracks (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, album_id INTEGER, track_number INTEGER, disc_number INTEGER, isrc TEXT, release_year INTEGER, record_label TEXT, musicbrainz_id TEXT, enrichment_status TEXT, enriched_at TEXT);")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE track_artists (track_id INTEGER, artist_id INTEGER, role TEXT, PRIMARY KEY(track_id, artist_id));")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE albums (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, release_date TEXT, upc TEXT, total_tracks INTEGER, label TEXT, musicbrainz_id TEXT);")
        .execute(&pool).await.unwrap();

    // Existing artist had 'NOT_FOUND' sentinel in musicbrainz_id
    sqlx::query("INSERT INTO artists (name, musicbrainz_id) VALUES ('Queen', 'NOT_FOUND');").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tracks (title, enrichment_status) VALUES ('Bohemian Rhapsody', 'pending');").execute(&pool).await.unwrap();

    let engine = EnrichmentEngine::new();
    let mut meta = EnrichedMetadata::default();
    let now = chrono_now_iso();

    meta.artist.merge_candidate(Some("Queen".to_string()), "stream", 1.0, &now);
    meta.musicbrainz_artist_id.merge_candidate(
        Some("0383dadf-2a4e-4d10-a46a-e6e041da8eb3".to_string()),
        "musicbrainz",
        0.95,
        &now,
    );

    let res = engine.apply_to_database(&pool, 1, &meta, None).await;
    assert!(res.is_ok());

    let (name, mbid): (String, Option<String>) = sqlx::query_as("SELECT name, musicbrainz_id FROM artists WHERE id = 1")
        .fetch_one(&pool).await.unwrap();

    assert_eq!(name, "Queen");
    assert_eq!(
        mbid.as_deref(),
        Some("0383dadf-2a4e-4d10-a46a-e6e041da8eb3"),
        "Sentinel 'NOT_FOUND' must be replaced by valid authentic MBID"
    );
}

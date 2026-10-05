//! SoundCloud unified-engine coverage (F3-1/F3-3, DO-1).
//!
//! The `"soundcloud"` arm of `perform_sync_service_with_emitter` used to delegate
//! to the legacy likes importer, which inserted `tracks` raw with
//! `INSERT OR IGNORE (title, duration_ms)` and attributed every track to the
//! uploader's `username`. These tests pin the replacement: the shared
//! `sync_soundcloud_likes_with_engine` core, which runs every like through
//! `EnrichmentEngine`.
//!
//! Two layers, following the S187/S189 pattern of a local mock HTTP server:
//!
//! 1. Client layer — `publisher_metadata` parsing, artist attribution, cover-art
//!    normalization, `next_href` pagination and the 401 shape the engine arm
//!    turns into `RequiresAuth`.
//! 2. Catalog layer — real migrations on in-memory SQLite: canonical identity
//!    (no title+duration collapse), publisher artist, album + cover rows,
//!    `library_entries` and idempotency on a second run.

use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;
use syncify_tauri_lib::commands::{
    sync_soundcloud_likes_with_engine, sync_soundcloud_playlists_with_engine,
};
use syncify_tauri_lib::services::SoundCloudClient;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

type Responder = Arc<dyn Fn(&str, &str) -> (u16, String) + Send + Sync>;

/// Answer one connection: parse the request line, hand (method, target) to the
/// responder and write a complete JSON response before closing.
async fn serve_one(mut socket: TcpStream, responder: Responder) {
    let mut buf = vec![0u8; 32768];
    let n = socket.read(&mut buf).await.unwrap_or(0);
    let raw = String::from_utf8_lossy(&buf[..n]);
    let mut parts = raw.lines().next().unwrap_or("").split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("").to_string();
    let (status, body) = responder(&method, &target);
    let resp = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        if status == 200 { "OK" } else { "Error" },
        body.len(),
        body
    );
    let _ = socket.write_all(resp.as_bytes()).await;
    let _ = socket.flush().await;
}

/// Spawn a local mock server; responder gets (method, path-with-query).
async fn spawn_mock(responder: Responder) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let responder = responder.clone();
            tokio::spawn(serve_one(socket, responder));
        }
    });
    format!("http://{}", addr)
}

/// Two-page likes endpoint: page 1 advertises a `next_href` pointing back at the
/// mock itself, so the `next_href` pagination loop can be driven end to end.
async fn spawn_two_page_mock(page1: serde_json::Value, page2: serde_json::Value) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let next_href = format!("http://{}/users/7/likes?cursor=page2", addr);
    let responder: Responder = Arc::new(move |_method, target| {
        if target.contains("cursor=page2") {
            (200, page2.to_string())
        } else {
            let mut first = page1.clone();
            first["next_href"] = serde_json::Value::String(next_href.clone());
            (200, first.to_string())
        }
    });
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let responder = responder.clone();
            tokio::spawn(serve_one(socket, responder));
        }
    });
    format!("http://{}", addr)
}

/// In-memory DB with the real schema, plus a SoundCloud account to import into.
async fn setup_test_db() -> (sqlx::SqlitePool, i64, i64) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory db");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");

    let soundcloud_service_id: i64 =
        sqlx::query_scalar("SELECT id FROM services WHERE name = 'soundcloud'")
            .fetch_one(&pool)
            .await
            .expect("soundcloud service row seeded by migrations");

    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, display_name, is_active) VALUES (?, 'SoundCloud Test', 1) RETURNING id",
    )
    .bind(soundcloud_service_id)
    .fetch_one(&pool)
    .await
    .expect("insert soundcloud account");

    (pool, account_id, soundcloud_service_id)
}

// ============================================================
// Client layer
// ============================================================

#[tokio::test]
async fn test_publisher_metadata_artist_wins_over_uploader() {
    let base = spawn_mock(Arc::new(|_method, target| {
        assert!(
            target.starts_with("/users/7/likes"),
            "unexpected path: {}",
            target
        );
        (
            200,
            serde_json::json!({
                "collection": [{
                    "created_at": "2019-04-01T10:00:00Z",
                    "track": {
                        "id": 1001,
                        "title": "Midnight City",
                        "duration": 240000,
                        "user": {"id": 42, "username": "m83fanaccount"},
                        "artwork_url": "https://i1.sndcdn.com/artworks-abc-t500x500.jpg",
                        "genre": "Electronic",
                        "publisher_metadata": {
                            "artist": "M83",
                            "album_title": "Hurry Up, We're Dreaming",
                            "isrc": "GB-KPL-11-00001",
                            "label_name": "Mute Records",
                            "year": 2011
                        }
                    }
                }],
                "next_href": null
            })
            .to_string(),
        )
    }))
    .await;

    let client = SoundCloudClient::new("token".into())
        .with_user_id(7)
        .with_api_base(base);
    let page = client.get_likes(None).await.expect("likes");

    let like = &page.collection[0];
    assert_eq!(
        like.created_at.as_deref(),
        Some("2019-04-01T10:00:00Z"),
        "the like timestamp is the added_at the catalog stores"
    );
    let track = like.track.as_ref().expect("track in like");

    assert_eq!(
        track.attributed_artist(),
        Some("M83"),
        "the label's performing artist wins over the uploader account"
    );
    assert_eq!(track.uploader_name(), Some("m83fanaccount"));
    assert_eq!(track.album_title(), Some("Hurry Up, We're Dreaming"));
    assert_eq!(track.isrc(), Some("GB-KPL-11-00001"));
    assert_eq!(track.label(), Some("Mute Records"));
    assert_eq!(track.release_year(), Some(2011));
}

#[tokio::test]
async fn test_track_without_publisher_metadata_falls_back_to_uploader() {
    // User-uploaded material (DJ set, bootleg): SoundCloud sends no
    // publisher_metadata, so there is no label artist and — by the service's own
    // capacity — no ISRC. The attribution falls back to the uploader and the
    // identity is resolved by Check A (the SoundCloud track id).
    let base = spawn_mock(Arc::new(|_method, _target| {
        (
            200,
            serde_json::json!({
                "collection": [{
                    "created_at": "2020-02-02T08:30:00Z",
                    "track": {
                        "id": 2002,
                        "title": "Midnight City (Bootleg)",
                        "duration": 241000,
                        "user": {"id": 77, "username": "some_dj"}
                    }
                }],
                "next_href": null
            })
            .to_string(),
        )
    }))
    .await;

    let client = SoundCloudClient::new("token".into())
        .with_user_id(7)
        .with_api_base(base);
    let page = client.get_likes(None).await.expect("likes");
    let track = page.collection[0].track.as_ref().expect("track in like");

    assert_eq!(track.publisher_artist(), None);
    assert_eq!(
        track.attributed_artist(),
        Some("some_dj"),
        "without label metadata the uploader is the only attribution available"
    );
    assert_eq!(track.isrc(), None, "no publisher_metadata means no ISRC");
    assert_eq!(track.album_title(), None);
    assert_eq!(track.cover_art_url(), None);
}

#[tokio::test]
async fn test_cover_art_is_normalized_to_the_largest_rendition() {
    let base = spawn_mock(Arc::new(|_method, _target| {
        (
            200,
            serde_json::json!({
                "collection": [{
                    "track": {
                        "id": 1, "title": "T", "duration": 1000,
                        "user": {"id": 1, "username": "u"},
                        "artwork_url": "https://i1.sndcdn.com/artworks-abc-t500x500.jpg"
                    }
                }],
                "next_href": null
            })
            .to_string(),
        )
    }))
    .await;

    let client = SoundCloudClient::new("token".into())
        .with_user_id(7)
        .with_api_base(base);
    let page = client.get_likes(None).await.expect("likes");
    let track = page.collection[0].track.as_ref().expect("track in like");

    assert_eq!(
        track.cover_art_url().as_deref(),
        Some("https://i1.sndcdn.com/artworks-abc-large.jpg"),
        "the 500x500 rendition is upgraded to -large, like the other engine arms persist"
    );
}

#[tokio::test]
async fn test_likes_pagination_follows_next_href() {
    let base = spawn_two_page_mock(
        serde_json::json!({
            "collection": [{
                "track": {"id": 1, "title": "T1", "duration": 1000,
                          "user": {"id": 1, "username": "u1"}}
            }]
        }),
        serde_json::json!({
            "collection": [{
                "track": {"id": 2, "title": "T2", "duration": 2000,
                          "user": {"id": 1, "username": "u2"}}
            }],
            "next_href": null
        }),
    )
    .await;

    let client = SoundCloudClient::new("token".into())
        .with_user_id(7)
        .with_api_base(base);

    let mut seen: Vec<(i64, String)> = Vec::new();
    let mut next_url: Option<String> = None;
    loop {
        let page = client.get_likes(next_url.as_deref()).await.expect("page");
        if page.collection.is_empty() {
            break;
        }
        for like in &page.collection {
            let track = like.track.as_ref().expect("track");
            seen.push((track.id, track.title.clone()));
        }
        next_url = page.next_href.clone();
        if next_url.is_none() {
            break;
        }
    }

    assert_eq!(
        seen,
        vec![(1, "T1".to_string()), (2, "T2".to_string())],
        "every next_href page is consumed, and the last page ends the loop"
    );
}

#[tokio::test]
async fn test_401_surfaces_the_shape_the_engine_turns_into_requires_auth() {
    let base = spawn_mock(Arc::new(|_method, _target| {
        (401, "{\"errors\":[{\"error_code\":401}]}".to_string())
    }))
    .await;

    let client = SoundCloudClient::new("expired".into())
        .with_user_id(7)
        .with_api_base(base);
    let err = client
        .get_likes(None)
        .await
        .expect_err("401 must not parse as an empty page");

    // `commands::service::is_soundcloud_auth_error` keys on exactly this prefix;
    // the sync arm turns a match into RequiresAuth + credential invalidation.
    assert!(
        err.contains("SoundCloud API error 401"),
        "401 must be recognizable as an auth failure: {}",
        err
    );
}

// ============================================================
// Catalog layer
// ============================================================

/// Two likes of the same title and duration from different SoundCloud tracks:
/// one delivered by a label, one a raw upload. The pre-F3 dedup
/// (`INSERT OR IGNORE (title, duration_ms)`) merged them into a single track.
#[tokio::test]
async fn test_same_title_and_duration_do_not_collapse_into_one_track() {
    let base = spawn_mock(Arc::new(|_method, _target| {
        (
            200,
            serde_json::json!({
                "collection": [
                    {
                        "created_at": "2019-04-01T10:00:00Z",
                        "track": {
                            "id": 1001,
                            "title": "Midnight City",
                            "duration": 240000,
                            "user": {"id": 42, "username": "m83fanaccount"},
                            "artwork_url": "https://i1.sndcdn.com/artworks-abc-t500x500.jpg",
                            "publisher_metadata": {
                                "artist": "M83",
                                "album_title": "Hurry Up, We're Dreaming",
                                "isrc": "GB-KPL-11-00001",
                                "label_name": "Mute Records",
                                "year": 2011
                            }
                        }
                    },
                    {
                        "created_at": "2020-06-06T20:15:00Z",
                        "track": {
                            "id": 1002,
                            "title": "Midnight City",
                            "duration": 240000,
                            "user": {"id": 77, "username": "another_fan"}
                        }
                    }
                ],
                "next_href": null
            })
            .to_string(),
        )
    }))
    .await;

    let (pool, account_id, soundcloud_service_id) = setup_test_db().await;
    let client = SoundCloudClient::new("token".into())
        .with_user_id(7)
        .with_api_base(base);

    let totals = sync_soundcloud_likes_with_engine(
        &pool,
        account_id,
        soundcloud_service_id,
        &client,
        |_res, _processed, _page_finished| {},
    )
    .await
    .expect("likes sync");

    assert!(totals.errors.is_empty(), "errors: {:?}", totals.errors);
    assert_eq!(totals.seen, 2);
    assert_eq!(totals.imported, 2, "both likes are new library entries");
    assert_eq!(totals.skipped, 0);

    let track_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE title = ?")
        .bind("Midnight City")
        .fetch_one(&pool)
        .await
        .expect("count tracks");
    assert_eq!(
        track_rows, 2,
        "canonical identity keys on the SoundCloud track id, so a label release \
         and a raw upload with the same title and duration stay separate tracks"
    );

    // Both are linked to the service through track_sources (Check A).
    let source_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM track_sources WHERE service_id = ? AND service_track_id IN ('1001','1002')",
    )
    .bind(soundcloud_service_id)
    .fetch_one(&pool)
    .await
    .expect("count sources");
    assert_eq!(source_rows, 2);

    // The label track keeps its ISRC; the raw upload has none — the service's
    // documented capacity gap, not a persistence failure.
    let with_isrc: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE title = ? AND isrc = ?")
            .bind("Midnight City")
            .bind("GB-KPL-11-00001")
            .fetch_one(&pool)
            .await
            .expect("count isrc tracks");
    assert_eq!(with_isrc, 1, "publisher_metadata.isrc is persisted");

    let label_track_id: i64 = sqlx::query_scalar(
        "SELECT track_id FROM track_sources WHERE service_id = ? AND service_track_id = '1001'",
    )
    .bind(soundcloud_service_id)
    .fetch_one(&pool)
    .await
    .expect("label track id");

    let primary_artist: Option<String> = sqlx::query_scalar(
        "SELECT ar.name FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id \
         WHERE ta.track_id = ? AND ta.role = 'primary'",
    )
    .bind(label_track_id)
    .fetch_one(&pool)
    .await
    .expect("primary artist of the label track");
    assert_eq!(
        primary_artist.as_deref(),
        Some("M83"),
        "the track is attributed to the label's artist, not the uploader account"
    );

    let uploader_artist_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM artists WHERE name = ?")
            .bind("m83fanaccount")
            .fetch_one(&pool)
            .await
            .expect("count uploader artists");
    assert_eq!(
        uploader_artist_rows, 0,
        "the uploader is not credited as the performing artist"
    );

    // Album + cover come from publisher_metadata when the service provides them.
    let album: (String, Option<String>) = sqlx::query_as(
        "SELECT a.title, a.cover_art_url FROM albums a JOIN tracks t ON t.album_id = a.id WHERE t.id = ?",
    )
    .bind(label_track_id)
    .fetch_one(&pool)
    .await
    .expect("album of the label track");
    assert_eq!(album.0, "Hurry Up, We're Dreaming");
    assert_eq!(
        album.1.as_deref(),
        Some("https://i1.sndcdn.com/artworks-abc-large.jpg"),
        "cover is persisted at the largest rendition the client resolves"
    );

    let entries: Vec<(i32, Option<String>)> = sqlx::query_as(
        "SELECT is_liked, added_at FROM library_entries WHERE account_id = ? ORDER BY added_at",
    )
    .bind(account_id)
    .fetch_all(&pool)
    .await
    .expect("library entries");
    assert_eq!(entries.len(), 2, "one liked library entry per like");
    assert!(entries.iter().all(|(liked, _)| *liked == 1));
    assert_eq!(
        entries[0].1.as_deref(),
        // `import_pagination::normalize_added_at` standardizes to RFC 3339 UTC,
        // so the like's `created_at` reaches the catalog as +00:00, never 1970.
        Some("2019-04-01T10:00:00+00:00"),
        "the like's created_at becomes library_entries.added_at"
    );
}

#[tokio::test]
async fn test_second_run_is_idempotent() {
    let base = spawn_mock(Arc::new(|_method, _target| {
        (
            200,
            serde_json::json!({
                "collection": [{
                    "created_at": "2019-04-01T10:00:00Z",
                    "track": {
                        "id": 1001, "title": "Midnight City", "duration": 240000,
                        "user": {"id": 42, "username": "m83fanaccount"},
                        "publisher_metadata": {
                            "artist": "M83",
                            "album_title": "Hurry Up, We're Dreaming",
                            "isrc": "GB-KPL-11-00001"
                        }
                    }
                }],
                "next_href": null
            })
            .to_string(),
        )
    }))
    .await;

    let (pool, account_id, soundcloud_service_id) = setup_test_db().await;
    let client = SoundCloudClient::new("token".into())
        .with_user_id(7)
        .with_api_base(base);
    let run = || async {
        sync_soundcloud_likes_with_engine(
            &pool,
            account_id,
            soundcloud_service_id,
            &client,
            |_res, _processed, _page_finished| {},
        )
        .await
    };

    let first = run().await.expect("first sync");
    assert_eq!(first.imported, 1);

    let second = run().await.expect("second sync");
    assert!(second.errors.is_empty(), "errors: {:?}", second.errors);
    assert_eq!(second.imported, 0, "the catalog already holds the track");
    assert_eq!(second.skipped, 1, "it is reported as already present");

    let track_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(&pool)
        .await
        .expect("count tracks");
    assert_eq!(track_rows, 1, "re-importing must not duplicate the track");
}

/// R4: the `"soundcloud"` arm used to warn that SoundCloud exposes no playlists
/// and skip the phase entirely. `/me/library/playlists_without_albums` does
/// exist (the Python service already walks it), so the arm must import the
/// account's playlists through the same engine as the likes: canonical
/// identity, `publisher_metadata` enrichment and `playlist_tracks` membership.
#[tokio::test]
async fn test_playlists_reach_the_catalog_through_the_engine() {
    let base = spawn_mock(Arc::new(|_method, target| {
        if target.starts_with("/me/library/playlists_without_albums") {
            (
                200,
                serde_json::json!({
                    "collection": [{
                        "playlist": {
                            "id": 77,
                            "title": "Late Night Drive",
                            "description": "Synthwave picks",
                            "track_count": 2,
                            "duration": 400_000,
                            "user": {"id": 42, "username": "curator"}
                        }
                    }],
                    "next_href": null
                })
                .to_string(),
            )
        } else if target.starts_with("/playlists/77/tracks") {
            (
                200,
                serde_json::json!({
                    "collection": [
                        {
                            "created_at": "2024-01-01T00:00:00Z",
                            "track": {
                                "id": 3001,
                                "title": "Nightcall",
                                "duration": 258000,
                                "user": {"id": 9, "username": " uploader_account "},
                                "artwork_url": "https://i1.sndcdn.com/artworks-xyz-t500x500.jpg",
                                "publisher_metadata": {
                                    "artist": "Kavinsky",
                                    "album_title": "OutRun",
                                    "isrc": "FR-10S8-13-00001"
                                }
                            }
                        },
                        {
                            "created_at": "2024-01-02T00:00:00Z",
                            "track": {
                                "id": 3002,
                                "title": "Resonance",
                                "duration": 212000,
                                "user": {"id": 11, "username": "home_recording"}
                            }
                        }
                    ],
                    "next_href": null
                })
                .to_string(),
            )
        } else {
            panic!("unexpected request: {}", target);
        }
    }))
    .await;

    let (pool, account_id, soundcloud_service_id) = setup_test_db().await;
    let client = SoundCloudClient::new("token".into())
        .with_user_id(7)
        .with_api_base(base);

    let mut finished_pages: Vec<(String, u64)> = Vec::new();
    let totals = sync_soundcloud_playlists_with_engine(
        &pool,
        account_id,
        soundcloud_service_id,
        &client,
        |res, playlist_title, processed, page_finished| {
            if page_finished {
                finished_pages.push((playlist_title.to_string(), processed));
            }
            assert!(res.track_id > 0, "every reported track is persisted");
        },
    )
    .await
    .expect("playlists sync");

    assert!(totals.errors.is_empty(), "errors: {:?}", totals.errors);
    assert_eq!(totals.playlists_seen, 1);
    assert_eq!(totals.playlists_synced, 1);
    assert_eq!(totals.tracks_seen, 2);
    assert_eq!(totals.imported, 2);
    assert_eq!(totals.skipped, 0);
    assert_eq!(
        finished_pages,
        vec![("Late Night Drive".to_string(), 2)],
        "the callback reports the last track of each page with its playlist"
    );

    // The playlist row exists and is linked by its remote identity.
    let playlist: (String, i64) = sqlx::query_as(
        "SELECT p.name, p.track_count FROM playlists p \
         JOIN playlist_sources ps ON ps.playlist_id = p.id \
         WHERE ps.account_id = ? AND ps.service_playlist_id = '77'",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .expect("playlist row linked by playlist_sources");
    assert_eq!(playlist.0, "Late Night Drive");
    assert_eq!(playlist.1, 2, "track_count is reconciled");

    // Membership rows keep the collection order (1..N).
    let membership: Vec<(String, i32)> = sqlx::query_as(
        "SELECT t.title, pt.position FROM playlist_tracks pt \
         JOIN tracks t ON t.id = pt.track_id \
         JOIN playlist_sources ps ON ps.playlist_id = pt.playlist_id \
         WHERE ps.account_id = ? AND ps.service_playlist_id = '77' \
         ORDER BY pt.position",
    )
    .bind(account_id)
    .fetch_all(&pool)
    .await
    .expect("membership rows");
    assert_eq!(
        membership,
        vec![("Nightcall".to_string(), 1), ("Resonance".to_string(), 2),],
        "playlist tracks keep the order the user saved them in"
    );

    // The label track keeps the same enrichment as the likes: publisher artist,
    // album and ISRC; the raw upload resolves by Check A without an ISRC.
    let kavinsky_track_id: i64 = sqlx::query_scalar(
        "SELECT track_id FROM track_sources WHERE service_id = ? AND service_track_id = '3001'",
    )
    .bind(soundcloud_service_id)
    .fetch_one(&pool)
    .await
    .expect("label track source");

    let primary_artist: Option<String> = sqlx::query_scalar(
        "SELECT ar.name FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id \
         WHERE ta.track_id = ? AND ta.role = 'primary'",
    )
    .bind(kavinsky_track_id)
    .fetch_one(&pool)
    .await
    .expect("primary artist");
    assert_eq!(
        primary_artist.as_deref(),
        Some("Kavinsky"),
        "publisher_metadata.artist wins over the uploader account"
    );

    let isrc: Option<String> = sqlx::query_scalar("SELECT isrc FROM tracks WHERE id = ?")
        .bind(kavinsky_track_id)
        .fetch_one(&pool)
        .await
        .expect("isrc");
    assert_eq!(isrc.as_deref(), Some("FR-10S8-13-00001"));

    // Playlist tracks are library entries, but not favorites.
    let liked_flags: Vec<(i32,)> =
        sqlx::query_as("SELECT is_liked FROM library_entries WHERE account_id = ?")
            .bind(account_id)
            .fetch_all(&pool)
            .await
            .expect("library entries");
    assert_eq!(liked_flags.len(), 2, "one library entry per playlist track");
    assert!(
        liked_flags.iter().all(|(liked,)| *liked == 0),
        "playlist membership is not a like"
    );

    // Second run: idempotent — no new tracks, no duplicated membership.
    let second = sync_soundcloud_playlists_with_engine(
        &pool,
        account_id,
        soundcloud_service_id,
        &client,
        |_res, _title, _processed, _page_finished| {},
    )
    .await
    .expect("second playlists sync");
    assert!(second.errors.is_empty(), "errors: {:?}", second.errors);
    assert_eq!(second.imported, 0, "the catalog already holds the tracks");
    assert_eq!(second.skipped, 2);

    let membership_rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playlist_tracks")
        .fetch_one(&pool)
        .await
        .expect("count membership");
    assert_eq!(membership_rows, 2, "membership is not duplicated");
}

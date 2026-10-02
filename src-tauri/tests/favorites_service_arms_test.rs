//! Fase-8 item 8.3 — coverage for the service arms that were missing.
//!
//! Three things were true before this item and each is pinned here:
//!
//! 1. `sync_favorites` / `push_favorite_to_service` only knew tidal, qobuz and
//!    spotify; anything else fell through to "Unsupported service".
//! 2. `perform_get_service_auth_status` reported `connected_valid` for soundcloud
//!    purely because some token was stored — the token was never presented to
//!    the API, so a revoked one kept looking healthy.
//! 3. Retryability had two criteria (a string matcher in tidal, a status matcher
//!    in http_retry, plus a third narrower one in the download path) and the
//!    soundcloud/apple_music clients bypassed the shared rate limiter.
//!
//! The provider-facing arms are exercised against a local mock HTTP server (the
//! S187/S189 pattern) through the `perform_*` entry points, so the command
//! wrappers keep only their Tauri-specific plumbing.
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    perform_get_service_auth_status, perform_push_favorite_to_service, perform_sync_favorites,
};
use syncify_tauri_lib::crypto;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// ============================================================
// Mock HTTP server
// ============================================================

/// What the mock saw: `(method, target)` for every request it answered.
type Seen = Arc<Mutex<Vec<(String, String)>>>;

struct MockServer {
    base: String,
    seen: Seen,
}

impl MockServer {
    fn requests(&self) -> Vec<(String, String)> {
        self.seen.lock().expect("mock recorder").clone()
    }
}

/// Answer one connection: read the request line (and headers, for anything the
/// client sends before the body), hand `(method, target)` to the responder and
/// write a complete response before closing.
async fn serve_one(mut socket: TcpStream, responder: Responder, seen: Seen) {
    let mut buf = vec![0u8; 32768];
    let n = socket.read(&mut buf).await.unwrap_or(0);
    let raw = String::from_utf8_lossy(&buf[..n]).to_string();
    let request_line = raw.lines().next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("").to_string();
    seen.lock()
        .expect("mock recorder")
        .push((method.clone(), target.clone()));

    let (status, extra_headers, body) = responder(&method, &target);
    let resp = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\n{}{}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        if (200..300).contains(&status) {
            "OK"
        } else {
            "Error"
        },
        extra_headers,
        if extra_headers.is_empty() {
            String::new()
        } else {
            "\r\n".to_string()
        },
        body.len(),
        body
    );
    let _ = socket.write_all(resp.as_bytes()).await;
    let _ = socket.flush().await;
}

type Responder = Arc<dyn Fn(&str, &str) -> (u16, String, String) + Send + Sync>;

/// Spawn a local mock server; the responder gets `(method, path-with-query)` and
/// returns `(status, extra raw headers, body)`.
async fn spawn_mock(responder: Responder) -> MockServer {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let recorder = seen.clone();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let responder = responder.clone();
            let seen = recorder.clone();
            tokio::spawn(serve_one(socket, responder, seen));
        }
    });
    MockServer {
        base: format!("http://{}", addr),
        seen,
    }
}

/// The env seams redirect the provider clients at the mock. They are
/// process-wide, so the tests that set them are serialised.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// ============================================================
// Database
// ============================================================

async fn setup_db() -> sqlx::SqlitePool {
    let _ = crypto::init_crypto([7u8; 32]);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory db");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    pool
}

/// Register a service + active account holding `creds` (encrypted at rest, the
/// way `load_service_credentials` expects to read them back).
async fn seed_account(pool: &sqlx::SqlitePool, service: &str, creds: serde_json::Value) -> i64 {
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(service)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("service '{}' must be seeded by migrations: {}", service, e));

    let encrypted = crypto::encrypt(&creds.to_string()).expect("encrypt credentials");
    sqlx::query_scalar(
        r#"INSERT INTO accounts (service_id, display_name, is_active, credentials_json)
           VALUES (?, ?, 1, ?) RETURNING id"#,
    )
    .bind(service_id)
    .bind(format!("{} Test", service))
    .bind(&encrypted)
    .fetch_one(pool)
    .await
    .expect("insert account")
}

/// Distinct `service_item_id`s of one item type for an account, sorted.
async fn favorite_rows(pool: &sqlx::SqlitePool, account_id: i64, item_type: &str) -> Vec<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT service_item_id FROM favorites
         WHERE account_id = ? AND item_type = ? ORDER BY service_item_id",
    )
    .bind(account_id)
    .bind(item_type)
    .fetch_all(pool)
    .await
    .expect("read favorites")
}

// ============================================================
// Deezer
// ============================================================

/// The gw-light payload `init()` needs to learn the user id and the api_token
/// (`results.checkForm` / `results.USER.USER_ID`, the fields the client reads).
const DEEZER_USER_DATA: &str =
    r#"{"results":{"checkForm":"api-token-xyz","USER":{"USER_ID":"4242"}}}"#;

#[tokio::test]
async fn deezer_push_track_add_hits_favorite_song_add() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|method, target| {
        if method == "POST" && target.contains("method=deezer.getUserData") {
            (200, String::new(), DEEZER_USER_DATA.to_string())
        } else if method == "POST" {
            (
                200,
                String::new(),
                r#"{"results":{"SNG_ID":"3135556"}}"#.to_string(),
            )
        } else {
            (404, String::new(), "{}".to_string())
        }
    }))
    .await;

    std::env::set_var("SYNCIFY_DEEZER_API_BASE", &mock.base);
    let pool = setup_db().await;
    let account_id = seed_account(&pool, "deezer", serde_json::json!({ "arl": "arl-test" })).await;

    let response = perform_push_favorite_to_service(&pool, "deezer", "track", "3135556", true)
        .await
        .expect("deezer track push must succeed");

    assert_eq!(response.status, "success");
    assert!(response.is_favorite);

    let targets: Vec<String> = mock
        .requests()
        .into_iter()
        .filter(|(m, _)| m == "POST")
        .map(|(_, t)| t)
        .collect();
    assert!(
        targets.iter().any(
            |t| t.contains("method=favorite_song.add") && t.contains("api_token=api-token-xyz")
        ),
        "expected favorite_song.add with the negotiated token, saw {:?}",
        targets
    );

    // Local mirror of the push.
    assert_eq!(
        favorite_rows(&pool, account_id, "track").await,
        vec!["3135556"]
    );
}

#[tokio::test]
async fn deezer_push_track_remove_hits_favorite_song_remove() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|method, target| {
        if method == "POST" && target.contains("method=deezer.getUserData") {
            (200, String::new(), DEEZER_USER_DATA.to_string())
        } else {
            (
                200,
                String::new(),
                r#"{"results":{"SNG_ID":"3135556"}}"#.to_string(),
            )
        }
    }))
    .await;

    std::env::set_var("SYNCIFY_DEEZER_API_BASE", &mock.base);
    let pool = setup_db().await;
    let account_id = seed_account(&pool, "deezer", serde_json::json!({ "arl": "arl-test" })).await;

    // Seed a local favorite so the removal has something to delete.
    sqlx::query(
        "INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title)
         VALUES (?, (SELECT id FROM services WHERE name='deezer'), 'track', '3135556', 'x')",
    )
    .bind(account_id)
    .execute(&pool)
    .await
    .unwrap();

    let response = perform_push_favorite_to_service(&pool, "deezer", "track", "3135556", false)
        .await
        .expect("deezer track un-push must succeed");
    assert!(!response.is_favorite);

    let targets: Vec<String> = mock.requests().into_iter().map(|(_, t)| t).collect();
    assert!(
        targets
            .iter()
            .any(|t| t.contains("method=favorite_song.remove")),
        "expected favorite_song.remove, saw {:?}",
        targets
    );

    assert!(favorite_rows(&pool, account_id, "track").await.is_empty());
}

#[tokio::test]
async fn deezer_push_rejects_item_types_deezer_has_no_endpoint_for() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|_m, _t| {
        (200, String::new(), DEEZER_USER_DATA.to_string())
    }))
    .await;
    std::env::set_var("SYNCIFY_DEEZER_API_BASE", &mock.base);

    let pool = setup_db().await;
    seed_account(&pool, "deezer", serde_json::json!({ "arl": "arl-test" })).await;

    let err = perform_push_favorite_to_service(&pool, "deezer", "album", "12345", true)
        .await
        .expect_err("Deezer must not claim it propagates album favorites");
    assert!(
        err.contains("only propagates track favorites"),
        "unexpected error: {}",
        err
    );
}

#[tokio::test]
async fn deezer_sync_favorites_imports_tracks_albums_and_artists() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|method, target| {
        if method == "POST" && target.contains("method=deezer.getUserData") {
            (200, String::new(), DEEZER_USER_DATA.to_string())
        } else if target.starts_with("/user/4242/tracks") {
            (
                200,
                String::new(),
                r#"{"total":1,"data":[{"id":3135556,"title":"Billie Jean","duration":294,
                     "isrc":"USARL5567361","artist":{"name":"Michael Jackson"},
                     "album":{"title":"Thriller"}}]}"#
                    .to_string(),
            )
        } else if target.starts_with("/user/4242/albums") {
            (
                200,
                String::new(),
                r#"{"total":1,"data":[{"id":9001,"title":"Thriller","nb_tracks":13,
                     "cover_medium":"https://e-cdns-images.dzcdn.net/cover/md.jpg",
                     "artist":{"id":27,"name":"Michael Jackson"}}]}"#
                    .to_string(),
            )
        } else if target.starts_with("/user/4242/artists") {
            (
                200,
                String::new(),
                r#"{"total":1,"data":[{"id":27,"name":"Michael Jackson"}]}"#.to_string(),
            )
        } else {
            (404, String::new(), "{}".to_string())
        }
    }))
    .await;

    std::env::set_var("SYNCIFY_DEEZER_API_BASE", &mock.base);
    std::env::set_var("SYNCIFY_DEEZER_PUBLIC_API_BASE", &mock.base);

    let pool = setup_db().await;
    let account_id = seed_account(&pool, "deezer", serde_json::json!({ "arl": "arl-test" })).await;

    let result = perform_sync_favorites(&pool, "deezer", Some("all"))
        .await
        .expect("deezer favorites sync must succeed");

    assert_eq!(result.service, "deezer");
    assert_eq!(result.total_found, 3, "1 track + 1 album + 1 artist");
    assert!(result.imported >= 3);

    assert_eq!(
        favorite_rows(&pool, account_id, "track").await,
        vec!["3135556"]
    );
    assert_eq!(
        favorite_rows(&pool, account_id, "album").await,
        vec!["9001"]
    );
    assert_eq!(favorite_rows(&pool, account_id, "artist").await, vec!["27"]);

    // Metadata reached the `favorites` row, not just the id.
    let (title, artist, isrc): (String, String, Option<String>) = sqlx::query_as(
        "SELECT title, artist_name, isrc FROM favorites
         WHERE account_id = ? AND item_type = 'track' AND service_item_id = '3135556'",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(title, "Billie Jean");
    assert_eq!(artist, "Michael Jackson");
    assert_eq!(isrc.as_deref(), Some("USARL5567361"));
}

// ============================================================
// SoundCloud
// ============================================================

#[tokio::test]
async fn soundcloud_auth_status_validates_the_token_against_the_api() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|_m, target| {
        assert_eq!(target, "/me", "the probe must hit /me");
        (
            200,
            String::new(),
            r#"{"id":7,"username":"alan"}"#.to_string(),
        )
    }))
    .await;
    std::env::set_var("SYNCIFY_SOUNDCLOUD_API_BASE", &mock.base);

    let pool = setup_db().await;
    let account_id = seed_account(
        &pool,
        "soundcloud",
        serde_json::json!({ "oauth_token": "oauth-1", "user_id": 7 }),
    )
    .await;
    // No stored display name: the one the API reports has to be used.
    sqlx::query("UPDATE accounts SET display_name = NULL WHERE id = ?")
        .bind(account_id)
        .execute(&pool)
        .await
        .unwrap();

    let status = perform_get_service_auth_status(&pool, "soundcloud", Some(account_id))
        .await
        .unwrap();

    assert_eq!(status.status, "connected_valid");
    assert!(status.is_authenticated);
    assert!(status.sync_available);
    // The API answered, so the display name is read back from it.
    assert_eq!(status.display_name.as_deref(), Some("alan"));
    assert_eq!(mock.requests().len(), 1, "the token must be presented");
}

#[tokio::test]
async fn soundcloud_auth_status_reports_a_rejected_token_as_requires_auth() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|_m, _t| {
        (
            401,
            String::new(),
            r#"{"errors":[{"errorMessage":"Invalid OAuth token"}]}"#.to_string(),
        )
    }))
    .await;
    std::env::set_var("SYNCIFY_SOUNDCLOUD_API_BASE", &mock.base);

    let pool = setup_db().await;
    let account_id = seed_account(
        &pool,
        "soundcloud",
        serde_json::json!({ "oauth_token": "revoked", "user_id": 7 }),
    )
    .await;

    let status = perform_get_service_auth_status(&pool, "soundcloud", Some(account_id))
        .await
        .unwrap();

    assert_eq!(
        status.status, "requires_auth",
        "a token the API refuses is not connected_valid"
    );
    assert!(!status.is_authenticated);
    assert!(status.credentials_invalid);
    assert!(!status.sync_available);
}

#[tokio::test]
async fn soundcloud_auth_status_without_a_token_never_calls_the_api() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|_m, _t| {
        panic!("no token stored, the API must not be called")
    }))
    .await;
    std::env::set_var("SYNCIFY_SOUNDCLOUD_API_BASE", &mock.base);

    let pool = setup_db().await;
    let account_id = seed_account(&pool, "soundcloud", serde_json::json!({ "user_id": 7 })).await;

    let status = perform_get_service_auth_status(&pool, "soundcloud", Some(account_id))
        .await
        .unwrap();

    assert_eq!(status.status, "requires_auth");
    assert!(status.credentials_invalid);
    assert!(mock.requests().is_empty());
}

#[tokio::test]
async fn soundcloud_push_track_uses_put_then_delete() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|_m, _t| (201, String::new(), "{}".to_string()))).await;
    std::env::set_var("SYNCIFY_SOUNDCLOUD_API_BASE", &mock.base);

    let pool = setup_db().await;
    let account_id = seed_account(
        &pool,
        "soundcloud",
        serde_json::json!({ "oauth_token": "oauth-1", "user_id": 7 }),
    )
    .await;

    perform_push_favorite_to_service(&pool, "soundcloud", "track", "555", true)
        .await
        .expect("like must propagate");
    perform_push_favorite_to_service(&pool, "soundcloud", "track", "555", false)
        .await
        .expect("unlike must propagate");

    let requests = mock.requests();
    assert!(
        requests
            .iter()
            .any(|(m, t)| m == "PUT" && t == "/users/7/track_likes/555"),
        "expected PUT /users/7/track_likes/555, saw {:?}",
        requests
    );
    assert!(
        requests
            .iter()
            .any(|(m, t)| m == "DELETE" && t == "/users/7/track_likes/555"),
        "expected DELETE /users/7/track_likes/555, saw {:?}",
        requests
    );
    assert!(favorite_rows(&pool, account_id, "track").await.is_empty());
}

#[tokio::test]
async fn soundcloud_push_rejects_item_types_soundcloud_has_no_endpoint_for() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|_m, _t| (201, String::new(), "{}".to_string()))).await;
    std::env::set_var("SYNCIFY_SOUNDCLOUD_API_BASE", &mock.base);

    let pool = setup_db().await;
    seed_account(
        &pool,
        "soundcloud",
        serde_json::json!({ "oauth_token": "oauth-1", "user_id": 7 }),
    )
    .await;

    let err = perform_push_favorite_to_service(&pool, "soundcloud", "album", "1", true)
        .await
        .expect_err("SoundCloud has no album likes");
    assert!(err.contains("only exposes track likes"), "got: {}", err);
}

/// The 429 branch has to do two things: keep the attempt alive (it is
/// transient) and feed the shared limiter so the retry waits out the penalty
/// the server asked for instead of hammering straight back.
#[tokio::test]
async fn soundcloud_like_honours_the_retry_after_penalty_on_429() {
    let _env = lock_env();
    let attempts = Arc::new(AtomicUsize::new(0));
    let counter = attempts.clone();
    let mock = spawn_mock(Arc::new(move |_m, _t| {
        if counter.fetch_add(1, Ordering::SeqCst) == 0 {
            (429, "Retry-After: 1\r\n".to_string(), "{}".to_string())
        } else {
            (201, String::new(), "{}".to_string())
        }
    }))
    .await;
    std::env::set_var("SYNCIFY_SOUNDCLOUD_API_BASE", &mock.base);

    let pool = setup_db().await;
    seed_account(
        &pool,
        "soundcloud",
        serde_json::json!({ "oauth_token": "oauth-1", "user_id": 7 }),
    )
    .await;

    let started = Instant::now();
    perform_push_favorite_to_service(&pool, "soundcloud", "track", "555", true)
        .await
        .expect("a 429 is transient: the like must still land");
    let elapsed = started.elapsed();

    assert_eq!(attempts.load(Ordering::SeqCst), 2, "429 then retry");
    assert!(
        elapsed >= Duration::from_secs(1),
        "the shared limiter must suspend the retry for the Retry-After delay, waited {:?}",
        elapsed
    );
}

#[tokio::test]
async fn soundcloud_sync_favorites_imports_liked_tracks() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|_m, target| {
        assert!(
            target.starts_with("/users/7/likes"),
            "unexpected: {}",
            target
        );
        (
            200,
            String::new(),
            r#"{"collection":[{"track":{"id":555,"title":"Billie Jean","duration":294000,
                 "user":{"id":9,"username":"uploader"},
                 "publisher_metadata":{"artist":"Michael Jackson","album_title":"Thriller",
                                       "isrc":"USARL5567361"},
                 "artwork_url":"https://i1.sndcdn.com/artworks-xyz-t500x500.jpg"},
                 "created_at":"2024-05-01T10:00:00Z"}],"next_href":null}"#
                .to_string(),
        )
    }))
    .await;
    std::env::set_var("SYNCIFY_SOUNDCLOUD_API_BASE", &mock.base);

    let pool = setup_db().await;
    let account_id = seed_account(
        &pool,
        "soundcloud",
        serde_json::json!({ "oauth_token": "oauth-1", "user_id": 7 }),
    )
    .await;

    let result = perform_sync_favorites(&pool, "soundcloud", Some("all"))
        .await
        .expect("soundcloud favorites sync must succeed");

    assert_eq!(result.total_found, 1);
    assert_eq!(favorite_rows(&pool, account_id, "track").await, vec!["555"]);

    // Publisher attribution, not the uploading account.
    let (title, artist, album, isrc): (String, String, Option<String>, Option<String>) =
        sqlx::query_as(
            "SELECT title, artist_name, album_name, isrc FROM favorites
             WHERE account_id = ? AND item_type = 'track' AND service_item_id = '555'",
        )
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(title, "Billie Jean");
    assert_eq!(artist, "Michael Jackson");
    assert_eq!(album.as_deref(), Some("Thriller"));
    assert_eq!(isrc.as_deref(), Some("USARL5567361"));
}

// ============================================================
// Apple Music
// ============================================================

#[tokio::test]
async fn apple_music_push_track_add_and_remove_use_the_library_endpoint() {
    let _env = lock_env();
    // `add_to_favorites` first asks whether the song is already in the library;
    // the writes themselves answer with an empty 204, as MusicKit does.
    let mock = spawn_mock(Arc::new(|method, _t| {
        if method == "GET" {
            (200, String::new(), r#"{"data":[]}"#.to_string())
        } else {
            (204, String::new(), String::new())
        }
    }))
    .await;
    std::env::set_var("SYNCIFY_APPLE_MUSIC_BASE_URL", format!("{}/v1", mock.base));

    let pool = setup_db().await;
    let account_id = seed_account(
        &pool,
        "apple_music",
        serde_json::json!({ "developer_token": "dev", "music_user_token": "user", "storefront": "es" }),
    )
    .await;

    perform_push_favorite_to_service(&pool, "apple_music", "track", "1440857781", true)
        .await
        .expect("add to library must succeed");
    perform_push_favorite_to_service(&pool, "apple_music", "track", "1440857781", false)
        .await
        .expect("remove from library must succeed");

    let requests = mock.requests();
    // The library song lookup guards the insert; the write itself follows.
    assert!(
        requests.iter().any(|(m, t)| m == "GET"
            && t.starts_with("/v1/me/library/songs?")
            && t.contains("ids[i]=1440857781")),
        "expected the library lookup, saw {:?}",
        requests
    );
    assert!(
        requests.iter().any(|(m, t)| m == "POST"
            && t.starts_with("/v1/me/library?")
            && t.contains("ids[songs]=1440857781")),
        "expected POST me/library?ids[songs], saw {:?}",
        requests
    );
    assert!(
        requests.iter().any(|(m, t)| m == "DELETE"
            && t.starts_with("/v1/me/library?")
            && t.contains("ids[songs]=1440857781")),
        "expected DELETE me/library?ids[songs], saw {:?}",
        requests
    );
    assert!(favorite_rows(&pool, account_id, "track").await.is_empty());
}

#[tokio::test]
async fn apple_music_push_album_uses_the_albums_collection() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|_m, _t| (204, String::new(), String::new()))).await;
    std::env::set_var("SYNCIFY_APPLE_MUSIC_BASE_URL", format!("{}/v1", mock.base));

    let pool = setup_db().await;
    seed_account(
        &pool,
        "apple_music",
        serde_json::json!({ "developer_token": "dev", "music_user_token": "user" }),
    )
    .await;

    perform_push_favorite_to_service(&pool, "apple_music", "album", "1440857780", true)
        .await
        .expect("add album to library must succeed");
    perform_push_favorite_to_service(&pool, "apple_music", "album", "1440857780", false)
        .await
        .expect("remove album from library must succeed");

    let requests = mock.requests();
    assert!(
        requests
            .iter()
            .any(|(m, t)| m == "POST" && t.contains("ids[albums]=1440857780")),
        "expected POST me/library?ids[albums], saw {:?}",
        requests
    );
    assert!(
        requests
            .iter()
            .any(|(m, t)| m == "DELETE" && t.contains("ids[albums]=1440857780")),
        "expected DELETE me/library?ids[albums], saw {:?}",
        requests
    );
}

#[tokio::test]
async fn apple_music_push_rejects_artists() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|_m, _t| (204, String::new(), String::new()))).await;
    std::env::set_var("SYNCIFY_APPLE_MUSIC_BASE_URL", format!("{}/v1", mock.base));

    let pool = setup_db().await;
    seed_account(
        &pool,
        "apple_music",
        serde_json::json!({ "developer_token": "dev", "music_user_token": "user" }),
    )
    .await;

    let err = perform_push_favorite_to_service(&pool, "apple_music", "artist", "1", true)
        .await
        .expect_err("the Apple Music library holds no artist favorites");
    assert!(err.contains("no artist favorites"), "got: {}", err);
}

#[tokio::test]
async fn apple_music_sync_favorites_imports_library_songs_and_albums() {
    let _env = lock_env();
    let mock = spawn_mock(Arc::new(|_m, target| {
        if target.starts_with("/v1/me/library/songs") {
            (
                200,
                String::new(),
                r#"{"data":[{"id":"1440857781","attributes":{"name":"Billie Jean",
                     "artistName":"Michael Jackson","albumName":"Thriller",
                     "durationInMillis":294000,"isrc":"USARL5567361",
                     "dateAdded":"2024-05-01T10:00:00Z"}}],"meta":{"total":1}}"#
                    .to_string(),
            )
        } else if target.starts_with("/v1/me/library/albums") {
            (
                200,
                String::new(),
                r#"{"data":[{"id":"1440857780","attributes":{"name":"Thriller",
                     "artistName":"Michael Jackson","trackCount":13,
                     "upc":"074646492620","dateAdded":"2024-05-01T10:00:00Z"}}],
                     "meta":{"total":1}}"#
                    .to_string(),
            )
        } else {
            (404, String::new(), "{}".to_string())
        }
    }))
    .await;
    std::env::set_var("SYNCIFY_APPLE_MUSIC_BASE_URL", format!("{}/v1", mock.base));

    let pool = setup_db().await;
    let account_id = seed_account(
        &pool,
        "apple_music",
        serde_json::json!({ "developer_token": "dev", "music_user_token": "user", "storefront": "es" }),
    )
    .await;

    let result = perform_sync_favorites(&pool, "apple_music", Some("all"))
        .await
        .expect("apple music favorites sync must succeed");

    assert_eq!(
        result.total_found, 1,
        "songs total drives the reported count"
    );
    assert_eq!(
        favorite_rows(&pool, account_id, "track").await,
        vec!["1440857781"]
    );
    assert_eq!(
        favorite_rows(&pool, account_id, "album").await,
        vec!["1440857780"]
    );

    let upc: Option<String> = sqlx::query_scalar(
        "SELECT upc FROM favorites
         WHERE account_id = ? AND item_type = 'album' AND service_item_id = '1440857780'",
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(upc.as_deref(), Some("074646492620"));
}

#[tokio::test]
async fn apple_music_sync_favorites_paginates_with_the_next_cursor() {
    let _env = lock_env();
    // The second page is only reachable through the `next` URL the first one
    // hands back; without following it the second song would never be seen.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let next_url = format!("http://{}/v1/me/library/songs?offset=1&limit=1", addr);

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let next_url = next_url.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 32768];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]).to_string();
                let target = raw
                    .lines()
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("")
                    .to_string();
                let body = if target.contains("offset=1") {
                    r#"{"data":[{"id":"2","attributes":{"name":"Second","artistName":"A"}}],"meta":{"total":2}}"#
                        .to_string()
                } else {
                    r#"{"data":[{"id":"1","attributes":{"name":"First","artistName":"A"}}],"next":"NEXT_URL","meta":{"total":2}}"#
                        .replace("NEXT_URL", &next_url)
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });

    std::env::set_var(
        "SYNCIFY_APPLE_MUSIC_BASE_URL",
        format!("http://{}/v1", addr),
    );

    let pool = setup_db().await;
    let account_id = seed_account(
        &pool,
        "apple_music",
        serde_json::json!({ "developer_token": "dev", "music_user_token": "user" }),
    )
    .await;

    perform_sync_favorites(&pool, "apple_music", Some("tracks"))
        .await
        .expect("apple music favorites sync must succeed");

    assert_eq!(
        favorite_rows(&pool, account_id, "track").await,
        vec!["1", "2"]
    );
}

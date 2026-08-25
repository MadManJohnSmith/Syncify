//! S197 regression tests — live ISRC resolution as last resort in
//! `evaluate_track_preflight` (integration level).
//!
//! Owner symptom: Spotify-origin tracks with an ISRC but no local Qobuz/Tidal
//! mapping were classified `NoDownloadProvider` even though a connected Tidal
//! account could have resolved them live. S197 wires a final-resolution round:
//! before the default verdict, the pipeline queries connected providers by ISRC
//! (Tidal first, then Qobuz), persists the first hit into `track_sources`
//! (same INSERT shape as the import paths) and re-runs the untouched preflight.
//!
//! Coverage here follows the S187 mock-HTTP pattern (`TcpListener` + injectable
//! base URL). The injection seam is the `SYNCIFY_S197_TIDAL_BASE_URL` env var
//! read only by the S197 helper in `commands/queue.rs`. QobuzClient exposes no
//! base-url override, so per spec the automated HTTP path covers the Tidal leg;
//! the pure decision helpers used on both legs are unit-tested directly.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::sync::{Arc, Mutex};
use syncify_tauri_lib::commands::{
    evaluate_track_preflight, s197_qobuz_quality_fields, s197_should_attempt_live_resolution,
    DownloadPreflightStatus,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type Responder = Arc<dyn Fn(&str) -> (u16, String) + Send + Sync>;

/// Spawn a local mock Tidal server; returns (base_url, request log).
/// Copied from tidal_s187_import_pagination_test.rs.
async fn spawn_mock_tidal(responder: Responder) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let requests = Arc::new(Mutex::new(Vec::<String>::new()));
    let reqs = requests.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { break };
            let responder = responder.clone();
            let reqs = reqs.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16384];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]);
                let request_line = raw.lines().next().unwrap_or("");
                let target = request_line.split_whitespace().nth(1).unwrap_or("").to_string();
                reqs.lock().unwrap().push(target.clone());
                let (status, body) = responder(&target);
                let reason = match status {
                    200 => "OK",
                    _ => "Error",
                };
                let resp = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status,
                    reason,
                    body.len(),
                    body
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });
    (format!("http://{}", addr), requests)
}

fn split_target(target: &str) -> (&str, &str) {
    match target.split_once('?') {
        Some((p, q)) => (p, q),
        None => (target, ""),
    }
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        if k == key { Some(v.to_string()) } else { None }
    })
}

/// Search response whose single hit carries `queried_isrc` as its own ISRC, so
/// `search_by_isrc` finds an exact identity match for whatever code the test uses.
fn s197_hit_body(queried_isrc: &str) -> String {
    serde_json::json!({
        "tracks": {
            "totalNumberOfItems": 1,
            "items": [{
                "id": 4242,
                "title": "S197 Live Hit",
                "duration": 210,
                "isrc": queried_isrc,
                "audioQuality": "LOSSLESS",
                "artist": {"id": 9, "name": "S197 Artist"},
                "album": {
                    "id": 3,
                    "title": "S197 Album",
                    "releaseDate": "2021-01-01",
                    "numberOfTracks": 10,
                    "cover": null
                },
                "trackNumber": 1,
                "volumeNumber": 1
            }]
        }
    })
    .to_string()
}

async fn fresh_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    // Idempotent service seed (mirrors ambiguous_source_dedupe_test.rs).
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    pool
}

/// A Spotify-origin track carrying an ISRC and no downloadable source yet —
/// exactly the owner scenario this sprint fixes.
async fn insert_spotify_origin_track(db: &SqlitePool, isrc: &str) -> i64 {
    let _artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('S197 Artist') RETURNING id")
            .fetch_one(db)
            .await
            .unwrap();
    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('S197 Album') RETURNING id")
            .fetch_one(db)
            .await
            .unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, isrc) VALUES ('S197 Track', ?, ?) RETURNING id",
    )
    .bind(album_id)
    .bind(isrc)
    .fetch_one(db)
    .await
    .unwrap();

    sqlx::query("INSERT OR REPLACE INTO track_sources (track_id, service_id, service_track_id, available) VALUES (?, 1, ?, 1)")
        .bind(track_id)
        .bind(format!("sp_{}", track_id))
        .execute(db)
        .await
        .unwrap();

    track_id
}

/// Active Tidal account whose encrypted credentials decrypt via the test crypto
/// key — required so `load_service_credentials` yields a usable client.
async fn insert_active_tidal_account(db: &SqlitePool) {
    let creds = serde_json::json!({
        "access_token": "s197-test-token",
        "user_id": "7",
        "country_code": "US"
    });
    let encrypted = syncify_tauri_lib::crypto::encrypt(&creds.to_string()).unwrap();
    sqlx::query(
        "INSERT INTO accounts (service_id, display_name, email, is_active, credentials_invalid, credentials_json) VALUES (3, 'S197 Tidal User', 's197@tidal.com', 1, 0, ?)",
    )
    .bind(encrypted)
    .execute(db)
    .await
    .unwrap();
}

async fn tidal_row_for_track(db: &SqlitePool, track_id: i64) -> Option<(String, Option<i64>, i64)> {
    sqlx::query_as(
        "SELECT ts.service_track_id, ts.bit_depth, ts.available FROM track_sources ts \
         JOIN services s ON s.id = ts.service_id \
         WHERE ts.track_id = ? AND LOWER(s.name) = 'tidal'",
    )
    .bind(track_id)
    .fetch_optional(db)
    .await
    .unwrap()
}

async fn tidal_row_count(db: &SqlitePool, track_id: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM track_sources ts JOIN services s ON s.id = ts.service_id \
         WHERE ts.track_id = ? AND LOWER(s.name) = 'tidal'",
    )
    .bind(track_id)
    .fetch_one(db)
    .await
    .unwrap()
}

/// The three S197 scenarios run sequentially inside ONE test because they share
/// the process-global base-url override env var (per-phase mock servers):
///   (i)   ISRC hit  → row persisted in track_sources + preflight eligible;
///   (ii)  double miss → NoDownloadProvider and zero inserted rows;
///   (iii) no active session → silent skip, zero provider calls, default kept.
#[tokio::test]
async fn s197_live_isrc_resolution_scenarios() {
    let _ = syncify_tauri_lib::crypto::init_crypto([42u8; 32]);
    const ISRC: &str = "US1977700001";

    // ---------- Phase (i): Tidal exact-ISRC hit ----------
    let hit_responder: Responder = Arc::new(|target: &str| {
        assert!(
            split_target(target).0.ends_with("/search/tracks"),
            "mock must only see search requests, got {}",
            target
        );
        // Echo the searched ISRC as the hit's own ISRC (exact identity match).
        let queried = query_param(split_target(target).1, "query").unwrap_or_default();
        (200, s197_hit_body(&queried))
    });
    let (base_hit, requests_hit) = spawn_mock_tidal(hit_responder).await;
    std::env::set_var("SYNCIFY_S197_TIDAL_BASE_URL", &base_hit);

    let db_hit = fresh_db().await;
    insert_active_tidal_account(&db_hit).await;
    let tid_hit = insert_spotify_origin_track(&db_hit, ISRC).await;

    let pf_hit = evaluate_track_preflight(&db_hit, tid_hit, None, None, false, true)
        .await
        .expect("preflight must not error");

    assert_eq!(
        pf_hit.status,
        DownloadPreflightStatus::ReadyExactSource,
        "live hit must upgrade the verdict to eligible (got {:?}: {})",
        pf_hit.status,
        pf_hit.reason
    );
    assert!(pf_hit.is_eligible);
    assert_eq!(pf_hit.resolved_service_name.as_deref(), Some("tidal"));
    assert_eq!(pf_hit.resolved_service_track_id.as_deref(), Some("4242"));

    let (stid, bit_depth, available) =
        tidal_row_for_track(&db_hit, tid_hit).await.expect("hit must be persisted");
    assert_eq!(stid, "4242");
    // LOSSLESS maps to 16/44.1 exactly like the Tidal import paths.
    assert_eq!(bit_depth, Some(16));
    assert_eq!(available, 1);
    assert!(!requests_hit.lock().unwrap().is_empty(), "provider was consulted");
    drop(db_hit);

    // ---------- Phase (ii): miss on Tidal (+ Qobuz absent) → default, zero rows ----------
    let miss_responder: Responder = Arc::new(|_target: &str| {
        (
            200,
            serde_json::json!({"tracks": {"totalNumberOfItems": 0, "items": []}}).to_string(),
        )
    });
    let (base_miss, requests_miss) = spawn_mock_tidal(miss_responder).await;
    std::env::set_var("SYNCIFY_S197_TIDAL_BASE_URL", &base_miss);

    let db_miss = fresh_db().await;
    insert_active_tidal_account(&db_miss).await;
    let tid_miss = insert_spotify_origin_track(&db_miss, ISRC).await;

    let pf_miss = evaluate_track_preflight(&db_miss, tid_miss, None, None, false, true)
        .await
        .expect("preflight must not error on a miss");

    assert_eq!(
        pf_miss.status,
        DownloadPreflightStatus::NoDownloadProvider,
        "double miss keeps the historical default (got {:?})",
        pf_miss.status
    );
    assert!(!pf_miss.is_eligible);
    assert_eq!(tidal_row_count(&db_miss, tid_miss).await, 0, "no rows may be inserted on a miss");
    assert_eq!(
        requests_miss.lock().unwrap().len(),
        1,
        "exactly one Tidal search attempt (Qobuz skipped: no session)"
    );
    drop(db_miss);

    // ---------- Phase (iii): no active session anywhere → silent skip ----------
    std::env::remove_var("SYNCIFY_S197_TIDAL_BASE_URL");
    let (base_idle, requests_idle) = spawn_mock_tidal(Arc::new(|_| (200, "{}".to_string()))).await;
    let _ = base_idle; // env var intentionally NOT pointed at it

    let db_nosess = fresh_db().await;
    let tid_nosess = insert_spotify_origin_track(&db_nosess, ISRC).await;

    let pf_nosess = evaluate_track_preflight(&db_nosess, tid_nosess, None, None, false, true)
        .await
        .expect("missing sessions must skip silently, never error");

    assert_eq!(
        pf_nosess.status,
        DownloadPreflightStatus::NoDownloadProvider,
        "no session ⇒ historical behavior fully intact"
    );
    assert!(!pf_nosess.is_eligible);
    assert_eq!(tidal_row_count(&db_nosess, tid_nosess).await, 0);
    assert_eq!(
        requests_idle.lock().unwrap().len(),
        0,
        "no HTTP call may happen without an active account"
    );
}

/// Pure gate: live resolution runs only with ISRC proof and when the explicit
/// service request does not exclude both live-capable providers.
#[test]
fn s197_gate_pure_decision_matrix() {
    assert!(!s197_should_attempt_live_resolution(false, None), "no ISRC ⇒ never");
    assert!(s197_should_attempt_live_resolution(true, None), "no restriction ⇒ allowed");
    assert!(
        !s197_should_attempt_live_resolution(true, Some("spotify")),
        "explicit spotify-only request must not trigger provider lookups"
    );
    assert!(s197_should_attempt_live_resolution(true, Some("tidal")));
    assert!(s197_should_attempt_live_resolution(true, Some("qobuz")));
    assert!(
        s197_should_attempt_live_resolution(true, Some("TIDAL")),
        "service names are case-insensitive like the SQL comparisons"
    );
}

/// Pure mapping: mirrors `QobuzClient::compute_quality_score` and the import
/// path column conversion (sample rate kHz → Hz). This covers the Qobuz leg,
/// which cannot be driven over mock HTTP without touching qobuz.rs.
#[test]
fn s197_qobuz_quality_fields_mirror_import_math() {
    // Hi-res: 24/96 ⇒ score 1000 + 24*10 + min(96.0,200)=96 ⇒ 1336; Hz stored.
    assert_eq!(s197_qobuz_quality_fields(Some(24), Some(96.0)), (Some(24), Some(96000), 1336));
    // CD-quality FLAC baseline; kHz truncates like the import math (44.1 ⇒ 44).
    assert_eq!(s197_qobuz_quality_fields(Some(16), Some(44.1)), (Some(16), Some(44100), 1204));
    // Unknown quality degrades gracefully to the FLAC base score.
    assert_eq!(s197_qobuz_quality_fields(None, None), (None, None, 1000));
}

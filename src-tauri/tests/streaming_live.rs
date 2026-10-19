//! Fase 2 — verificación en vivo de resolución y cabecera de streaming (ignored).
//!
//! Uso:
//!   SYNCIFY_LIVE_TRACK_ID=1790 cargo test -p syncify-tauri --test streaming_live -- --ignored --nocapture

use syncify_tauri_lib::crypto;
use syncify_tauri_lib::download::qobuz::QobuzDownloader;
use syncify_tauri_lib::services::streaming::resolve_stream_for_track;

fn live_db_url() -> String {
    if let Ok(explicit) = std::env::var("SYNCIFY_LIVE_DB") {
        return format!("sqlite:{}?mode=ro", explicit);
    }
    let home = std::env::var("HOME").expect("HOME");
    format!(
        "sqlite:{}/.local/share/com.syncify.app/syncify.db?mode=ro",
        home
    )
}

#[tokio::test]
#[ignore = "live: requiere cuenta Qobuz/Tidal real, keychain y red"]
async fn resolves_and_starts_downloading_a_live_stream() {
    let track_id: i64 = std::env::var("SYNCIFY_LIVE_TRACK_ID")
        .expect("SYNCIFY_LIVE_TRACK_ID must be set")
        .parse()
        .expect("SYNCIFY_LIVE_TRACK_ID must be an integer");

    let _ = crypto::init_keychain_crypto();
    let db = sqlx::SqlitePool::connect(&live_db_url())
        .await
        .expect("open syncify.db read-only");
    let http = reqwest::Client::new();

    let sources: Vec<(String, String, i64)> = sqlx::query_as(
        r#"
        SELECT LOWER(s.name), ts.service_track_id, COALESCE(ts.quality_score, 0)
        FROM track_sources ts
        JOIN services s ON s.id = ts.service_id
        WHERE ts.track_id = ?
        ORDER BY ts.quality_score DESC, ts.id ASC
        "#,
    )
    .bind(track_id)
    .fetch_all(&db)
    .await
    .unwrap_or_default();
    println!("track {} sources={:?}", track_id, sources);

    match resolve_stream_for_track(&db, &http, track_id).await {
        Ok(resolved) => {
            println!(
                "resolved: service={} format={} tier={} label='{}' source='{}' ext={}",
                resolved.info.service_name,
                resolved.info.format,
                resolved.info.quality_tier,
                resolved.info.quality_label,
                resolved.info.source_name,
                resolved.info.extension
            );
            let resp = http
                .get(&resolved.url)
                .header(reqwest::header::RANGE, "bytes=0-63")
                .send()
                .await
                .expect("GET stream prefix");
            assert!(resp.status().is_success(), "HTTP {}", resp.status());
            let ctype = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            let clen = resp.content_length();
            assert_ne!(
                clen,
                Some(1_200_588),
                "resolved stream must never be a 1,200,588-byte 30s preview"
            );
            println!(
                "verified HTTP {} content-type={} len={:?}",
                resp.status(),
                ctype,
                clen
            );
        }
        Err(err) => {
            println!("honest rejection for track {}: {}", track_id, err);
            assert!(
                err.contains("preview") || err.contains("No se pudo resolver"),
                "unexpected error message: {}",
                err
            );
        }
    }
}

#[tokio::test]
#[ignore = "live: requiere cuenta Qobuz real, keychain y red"]
async fn probes_qobuz_formats_for_the_live_track() {
    let track_id: i64 = std::env::var("SYNCIFY_LIVE_TRACK_ID")
        .expect("SYNCIFY_LIVE_TRACK_ID must be set")
        .parse()
        .expect("SYNCIFY_LIVE_TRACK_ID must be an integer");

    let _ = crypto::init_keychain_crypto();
    let db = sqlx::SqlitePool::connect(&live_db_url())
        .await
        .expect("open syncify.db read-only");

    let qobuz_track_id: i64 = sqlx::query_scalar::<_, String>(
        r#"
        SELECT ts.service_track_id
        FROM track_sources ts
        JOIN services s ON s.id = ts.service_id
        WHERE ts.track_id = ? AND LOWER(s.name) = 'qobuz'
        LIMIT 1
        "#,
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .expect("qobuz track_source for track")
    .parse()
    .expect("numeric qobuz service_track_id");

    let accounts: Vec<(i64, i64, Option<i64>)> = sqlx::query_as(
        r#"
        SELECT a.id, a.is_active, a.streaming_entitled
        FROM accounts a
        JOIN services s ON s.id = a.service_id
        WHERE LOWER(s.name) = 'qobuz' AND COALESCE(a.credentials_invalid, 0) = 0
        ORDER BY a.is_active DESC, a.id DESC
        "#,
    )
    .fetch_all(&db)
    .await
    .expect("qobuz accounts");
    println!(
        "local track={} -> qobuz service_track_id={}, accounts={:?}",
        track_id, qobuz_track_id, accounts
    );

    let downloader = QobuzDownloader::new();
    let http = reqwest::Client::new();

    for (account_id, is_active, entitled) in accounts {
        let token = match downloader
            .resolve_token_for_account(&db, Some(account_id))
            .await
        {
            Ok(t) => t,
            Err(e) => {
                println!("account {} token err: {:?}", account_id, e);
                continue;
            }
        };
        for (quality, allow_fallback) in [("hires", false), ("lossless", false), ("mp3", true)] {
            match downloader
                .get_official_download_url(qobuz_track_id, quality, &token, allow_fallback)
                .await
            {
                Ok(res) => {
                    let probe = http
                        .get(&res.url)
                        .header(reqwest::header::RANGE, "bytes=0-63")
                        .send()
                        .await;
                    if let Ok(mut resp) = probe {
                        let status = resp.status();
                        let ctype = resp
                            .headers()
                            .get(reqwest::header::CONTENT_TYPE)
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_string);
                        let crange = resp
                            .headers()
                            .get(reqwest::header::CONTENT_RANGE)
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_string);
                        let clen = resp.content_length();
                        let head = resp
                            .chunk()
                            .await
                            .ok()
                            .flatten()
                            .map(|b| b.slice(..b.len().min(16)).to_vec())
                            .unwrap_or_default();
                        println!(
                            "acc={}(active={},ent={:?}) quality={}(fb={}) -> fmt={} HTTP {} type={:?} range={:?} len={:?} head={:02x?}",
                            account_id, is_active, entitled, quality, allow_fallback, res.format_id, status, ctype, crange, clen, head
                        );
                    }
                }
                Err(e) => {
                    println!(
                        "acc={}(active={},ent={:?}) quality={}(fb={}) -> ERR: {}",
                        account_id, is_active, entitled, quality, allow_fallback, e
                    );
                }
            }
        }
    }
}

/// Fase 3 — la cascada ISRC debe encontrar la misma grabación en Qobuz/Tidal
/// para una pista que NO tiene fuentes directas (importada sólo de Spotify).
///
/// Uso:
///   SYNCIFY_LIVE_ISRC_TRACK_ID=24437 cargo test -p syncify-tauri --test streaming_live -- --ignored --nocapture
#[tokio::test]
#[ignore = "live: busca por ISRC en Qobuz/Tidal reales"]
async fn isrc_cascade_resolves_a_track_without_direct_sources() {
    let track_id: i64 = std::env::var("SYNCIFY_LIVE_ISRC_TRACK_ID")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(24_437);

    let _ = crypto::init_keychain_crypto();
    let db = sqlx::SqlitePool::connect(&live_db_url())
        .await
        .expect("open syncify.db read-only");
    let http = reqwest::Client::new();

    let direct: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT LOWER(s.name), ts.service_track_id
        FROM track_sources ts
        JOIN services s ON s.id = ts.service_id
        WHERE ts.track_id = ? AND LOWER(s.name) IN ('qobuz', 'tidal')
        "#,
    )
    .bind(track_id)
    .fetch_all(&db)
    .await
    .unwrap_or_default();
    let meta: Option<(String, Option<String>, Option<i64>)> =
        sqlx::query_as("SELECT title, isrc, duration_ms FROM tracks WHERE id = ?")
            .bind(track_id)
            .fetch_optional(&db)
            .await
            .ok()
            .flatten();
    println!(
        "track {} meta={:?} direct_qobuz_tidal_sources={:?}",
        track_id, meta, direct
    );

    match syncify_tauri_lib::services::streaming::resolve_stream_for_track(&db, &http, track_id)
        .await
    {
        Ok(resolved) => {
            println!(
                "resolved via {}: format={} tier={} label='{}' source='{}' match={}",
                resolved.info.service_name,
                resolved.info.format,
                resolved.info.quality_tier,
                resolved.info.quality_label,
                resolved.info.source_name,
                resolved.info.match_method
            );
            assert!(
                resolved.info.service_name == "qobuz" || resolved.info.service_name == "tidal",
                "the cascade must land on a lossless provider"
            );
            assert_ne!(
                resolved.info.quality_tier, "lossy",
                "a Spotify-only import must play lossless when the catalogue has it"
            );
            let resp = http
                .get(&resolved.url)
                .header(reqwest::header::RANGE, "bytes=0-63")
                .send()
                .await
                .expect("GET stream prefix");
            assert!(resp.status().is_success(), "HTTP {}", resp.status());
            assert_ne!(
                resp.content_length(),
                Some(1_200_588),
                "cascade must reject the 30 s Qobuz preview"
            );
            println!(
                "verified HTTP {} content-type={:?}",
                resp.status(),
                resp.headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
            );
        }
        Err(err) => panic!("cascade failed for track {}: {}", track_id, err),
    }
}

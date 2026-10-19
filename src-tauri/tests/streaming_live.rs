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
                "resolved: service={} format={} tier={} label='{}' source='{}' ext={} dash={}",
                resolved.info.service_name,
                resolved.info.format,
                resolved.info.quality_tier,
                resolved.info.quality_label,
                resolved.info.source_name,
                resolved.info.extension,
                resolved.dash.is_some()
            );
            if let Some(dash) = &resolved.dash {
                // Tidal lossless/hi-res: el manifiesto se verifica leyendo el
                // STREAMINFO real del init y demuxeando el primer segmento.
                println!(
                    "dash manifest: init={} segs={}",
                    dash.init_url, dash.total_segments
                );
                let init = http
                    .get(&dash.init_url)
                    .send()
                    .await
                    .expect("GET init")
                    .bytes()
                    .await
                    .expect("init bytes");
                let info = syncify_fmp4_flac::read_init(&init).expect("init FLAC");
                println!(
                    "init FLAC: rate={} depth={} channels={} total={} dur={:.1}s",
                    info.sample_rate,
                    info.bits_per_sample,
                    info.channels,
                    info.total_samples,
                    info.duration_secs()
                );
                assert!(info.duration_secs() > 30.0, "no debe ser un preview");
                let segment = http
                    .get(dash.segment_url(1))
                    .send()
                    .await
                    .expect("GET segment 1")
                    .bytes()
                    .await
                    .expect("segment bytes");
                let mut frames: Vec<u8> = Vec::new();
                let samples = syncify_fmp4_flac::demux_segment(&init, &segment, &mut |frame| {
                    frames.extend_from_slice(frame);
                    Ok(())
                })
                .expect("demux segment 1");
                assert!(samples > 0 && !frames.is_empty());
                println!(
                    "segment 1 demuxed: samples={} frames_bytes={}",
                    samples,
                    frames.len()
                );
                return;
            }
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
            if let Some(dash) = &resolved.dash {
                let init = http
                    .get(&dash.init_url)
                    .send()
                    .await
                    .expect("GET init")
                    .bytes()
                    .await
                    .expect("init bytes");
                let info = syncify_fmp4_flac::read_init(&init).expect("init FLAC");
                println!(
                    "dash FLAC verified: rate={} depth={} dur={:.1}s",
                    info.sample_rate,
                    info.bits_per_sample,
                    info.duration_secs()
                );
                assert!(info.duration_secs() > 30.0);
                return;
            }
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

/// Paso 1 del plan Tidal lossless: el bucle real de descarga DASH
/// (`download_dash_to_cache`) produce un FLAC nativo que el motor abre, con
/// duración y seek correctos.
///
/// Uso:
///   SYNCIFY_LIVE_DASH_TIDAL_ID=156600942 cargo test -p syncify-tauri --test streaming_live dash_download -- --ignored --nocapture
#[tokio::test]
#[ignore = "live: descarga un manifiesto DASH completo de Tidal"]
async fn dash_download_to_cache_produces_a_playable_flac() {
    let _ = crypto::init_keychain_crypto();
    let db = sqlx::SqlitePool::connect(&live_db_url())
        .await
        .expect("open syncify.db read-only");
    let http = reqwest::Client::new();

    let tidal_id: i64 = std::env::var("SYNCIFY_LIVE_DASH_TIDAL_ID")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(156_600_942);

    let creds =
        syncify_tauri_lib::services::tidal_pipeline::resolve_and_refresh_gui_credentials_for_account(
            &db, &http, None, false,
        )
        .await
        .0
        .expect("tidal gui credentials");
    let downloader = syncify_tidal_downloader::TidalDownloader::new();
    let resolution = downloader
        .get_stream_resolution_with_credentials(tidal_id, Some("hires"), Some(&creds), true)
        .await
        .expect("tidal resolution");
    let dash = syncify_tauri_lib::services::streaming::DashManifest::parse(&resolution.url)
        .expect("DASH spec");
    println!(
        "tidal {} -> DASH init={} segs={}",
        tidal_id, dash.init_url, dash.total_segments
    );

    let init = http
        .get(&dash.init_url)
        .send()
        .await
        .expect("GET init")
        .bytes()
        .await
        .expect("init bytes");
    let flac = syncify_fmp4_flac::read_init(&init).expect("init FLAC");

    let out_dir = std::path::PathBuf::from("/tmp/opencode/dash-live");
    std::fs::create_dir_all(&out_dir).expect("create out dir");
    let path = out_dir.join(format!("tidal-{}.flac", tidal_id));
    let info = syncify_tauri_lib::services::streaming::StreamSourceInfo {
        service_name: "tidal".to_string(),
        service_track_id: tidal_id.to_string(),
        format: "FLAC".to_string(),
        codec: Some("flac".to_string()),
        container: Some("ISOBMFF".to_string()),
        extension: "flac".to_string(),
        bit_depth: Some(flac.bits_per_sample as i32),
        sample_rate: Some(flac.sample_rate as i32),
        quality_label: format!(
            "FLAC {} bits / {} kHz",
            flac.bits_per_sample,
            flac.sample_rate / 1000
        ),
        quality_tier: if flac.bits_per_sample > 16 {
            "hires".to_string()
        } else {
            "lossless".to_string()
        },
        source_name: resolution.source_name.clone(),
        match_method: "direct".to_string(),
    };
    let session =
        syncify_tauri_lib::services::stream_cache::StreamSession::new(1, tidal_id, info, path.clone());

    let mut primed_at: Option<f64> = None;
    let mut prime = || {
        let written = session.downloaded_bytes.load(std::sync::atomic::Ordering::SeqCst);
        primed_at = Some(written as f64);
        Ok(())
    };
    let mut progress = || {};
    syncify_tauri_lib::services::stream_cache::download_dash_to_cache(
        &http,
        &session,
        &dash,
        &mut prime,
        &mut progress,
    )
    .await
    .expect("download DASH to cache");
    assert!(session.complete.load(std::sync::atomic::Ordering::SeqCst));

    let len = std::fs::metadata(&path).expect("produced flac").len();
    println!(
        "live DASH FLAC: {} bytes (prime en {} bytes), {}",
        len,
        primed_at.unwrap_or(0.0),
        path.display()
    );
    assert!(len > 10_000_000, "un FLAC hi-res completo pesa mucho más");
    assert!(
        primed_at.unwrap_or(0.0) > 0.0,
        "el prime debe ocurrir durante la descarga"
    );

    // El motor lo abre en pausa (sin sonar): duración real del STREAMINFO y
    // seek dentro de lo descargado, como haría la app.
    let mut engine = syncify_player::PlayerEngine::new().expect("audio output");
    let loaded = engine.load(&path, false).expect("decode native FLAC");
    let duration = loaded.duration.expect("duración del STREAMINFO").as_secs_f64();
    assert!(
        (duration - flac.duration_secs()).abs() < 1.0,
        "duración {duration} vs STREAMINFO {}",
        flac.duration_secs()
    );
    engine
        .seek(std::time::Duration::from_secs(120))
        .expect("seek 120s");
    engine
        .seek(std::time::Duration::from_secs(5))
        .expect("seek 5s");
    println!("motor: duración {:.1}s y seek ok", duration);
}

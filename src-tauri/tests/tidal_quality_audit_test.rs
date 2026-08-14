//! Diagnostic audit test for Tidal stream resolution parameters

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PlaybackInfoResp {
    #[serde(rename = "trackId")]
    _track_id: Option<u64>,
    #[serde(rename = "assetPresentation")]
    _asset_presentation: Option<String>,
    #[serde(rename = "audioMode")]
    audio_mode: Option<String>,
    #[serde(rename = "audioQuality")]
    audio_quality: Option<String>,
    #[serde(rename = "manifestMimeType")]
    manifest_mime_type: Option<String>,
    manifest: Option<String>,
    #[serde(rename = "albumReplayGain")]
    _album_replay_gain: Option<f64>,
    #[serde(rename = "albumPeakAmplitude")]
    _album_peak_amplitude: Option<f64>,
    #[serde(rename = "trackReplayGain")]
    track_replay_gain: Option<f64>,
    #[serde(rename = "trackPeakAmplitude")]
    _track_peak_amplitude: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct BtsManifest {
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    codecs: Option<String>,
    #[serde(rename = "encryptionType")]
    encryption_type: Option<String>,
    urls: Option<Vec<String>>,
}

#[tokio::test]
async fn test_audit_tidal_stream_resolution_matrix() {
    let app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let db_path = std::path::PathBuf::from(app_data)
        .join("com.syncify.app")
        .join("syncify.db");

    if !db_path.exists() {
        println!("DB does not exist at {:?}, skipping live test", db_path);
        return;
    }

    let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", db_path.display()))
        .await
        .expect("Connect DB");

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT a.credentials_json FROM accounts a JOIN services s ON s.id = a.service_id WHERE LOWER(s.name) = 'tidal' AND a.is_active = 1 LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();

    let creds_json = match row {
        Some((cj,)) => cj,
        None => {
            println!("No active Tidal credentials found in DB");
            return;
        }
    };

    let _ = syncify_tauri_lib::crypto::init_keychain_crypto();
    let dec_str = syncify_tauri_lib::crypto::decrypt(&creds_json).expect("Decrypt creds");
    let creds: syncify_tidal_downloader::TidalGuiCredentials = serde_json::from_str(&dec_str).expect("Deserialize creds");

    let access_token = creds.access_token.clone();
    let country_code = creds.country_code.clone().unwrap_or_else(|| "ES".to_string());
    let client = reqwest::Client::new();
    let track_ids = vec!["560266", "80654035", "77703642"];

    for track_id in track_ids {
        println!("\n=======================================================");
        println!("AUDITING TIDAL TRACK: {}", track_id);
        println!("=======================================================");

        // 1. Check Catalog Metadata
        let meta_url = format!("https://api.tidal.com/v1/tracks/{}?countryCode={}", track_id, country_code);
        if let Ok(resp) = client.get(&meta_url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("X-Tidal-SessionId", &access_token)
            .send()
            .await
        {
            if let Ok(json_val) = resp.json::<serde_json::Value>().await {
                println!("--- CATALOG METADATA ---");
                println!("Title: {:?}", json_val.get("title").and_then(|v| v.as_str()));
                println!("Artist: {:?}", json_val.get("artist").and_then(|a| a.get("name")).and_then(|v| v.as_str()));
                println!("Album: {:?}", json_val.get("album").and_then(|a| a.get("title")).and_then(|v| v.as_str()));
                println!("ISRC: {:?}", json_val.get("isrc").and_then(|v| v.as_str()));
                println!("Catalog audioQuality: {:?}", json_val.get("audioQuality").and_then(|v| v.as_str()));
                println!("Catalog audioModes: {:?}", json_val.get("audioModes"));
                println!("Catalog mediaMetadata: {:?}", json_val.get("mediaMetadata"));
                println!();
            }
        }

        let test_matrix = vec![
            ("LOSSLESS", None),
            ("LOSSLESS", Some("application/vnd.tidal.bts")),
            ("LOSSLESS", Some("application/dash+xml")),
            ("HI_RES_LOSSLESS", None),
            ("HI_RES_LOSSLESS", Some("application/dash+xml")),
            ("HIGH", None),
            ("HIGH", Some("application/vnd.tidal.bts")),
        ];

        for (audio_quality_param, manifest_mime_type_param) in test_matrix {
            let mut url = format!(
                "https://api.tidal.com/v1/tracks/{}/playbackinfopostpaywall?audioquality={}&playbackmode=STREAM&assetpresentation=FULL&countryCode={}",
                track_id, audio_quality_param, country_code
            );
            if let Some(m_mime) = manifest_mime_type_param {
                url.push_str(&format!("&manifestMimeType={}", m_mime));
            }

            let resp_res = client.get(&url)
                .header("Authorization", format!("Bearer {}", access_token))
                .header("X-Tidal-SessionId", &access_token)
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
                .send()
                .await;

            match resp_res {
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();

                    println!("--- REQUEST: audioquality={}, manifestMimeType={:?} ---", audio_quality_param, manifest_mime_type_param);
                    println!("HTTP Status: {}", status);

                    if status.is_success() {
                        if let Ok(info) = serde_json::from_str::<PlaybackInfoResp>(&text) {
                            println!("Response audioQuality: {:?}", info.audio_quality);
                            println!("Response manifestMimeType: {:?}", info.manifest_mime_type);
                            println!("Response audioMode: {:?}", info.audio_mode);
                            println!("Track ReplayGain: {:?}", info.track_replay_gain);

                            if let Some(b64_manifest) = info.manifest {
                                if let Ok(decoded_bytes) = BASE64.decode(&b64_manifest) {
                                    if let Ok(decoded_str) = String::from_utf8(decoded_bytes.clone()) {
                                        if decoded_str.starts_with('{') {
                                            if let Ok(bts) = serde_json::from_str::<BtsManifest>(&decoded_str) {
                                                println!("BTS JSON mimeType: {:?}", bts.mime_type);
                                                println!("BTS JSON codecs: {:?}", bts.codecs);
                                                println!("BTS JSON encryptionType: {:?}", bts.encryption_type);
                                                println!("BTS URLs count: {:?}", bts.urls.as_ref().map(|u| u.len()));
                                            } else {
                                                println!("JSON manifest: {}", &decoded_str[..decoded_str.len().min(120)]);
                                            }
                                        } else if decoded_str.contains("<MPD") || decoded_str.contains("<?xml") {
                                            println!("XML DASH manifest detected, len={}", decoded_str.len());
                                            if decoded_str.contains("codecs=\"flac\"") || decoded_str.contains("codecs=\"fLaC\"") {
                                                println!("DASH contains FLAC codec!");
                                            } else if decoded_str.contains("codecs=\"mp4a") {
                                                println!("DASH contains AAC codec!");
                                            }
                                        } else {
                                            println!("Decoded string format unknown, first 60 chars: {:?}", &decoded_str[..decoded_str.len().min(60)]);
                                        }
                                    }
                                }
                            }
                        } else {
                            println!("Non-standard JSON success: {}", &text[..text.len().min(200)]);
                        }
                    } else {
                        println!("Error response: {}", &text[..text.len().min(200)]);
                    }
                    println!();
                }
                Err(e) => {
                    println!("Request failed: {}\n", e);
                }
            }
        }
    }
}

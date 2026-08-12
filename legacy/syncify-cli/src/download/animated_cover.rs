//! Apple Music Motion / Animated Cover Art Downloader & Converter (CLI Standalone)

use reqwest::Client;
use std::path::{Path, PathBuf};
use tracing::info;

pub async fn download_animated_cover(
    client: &Client,
    artist: &str,
    album: &str,
    target_album_dir: &Path,
) -> Option<PathBuf> {
    tokio::fs::create_dir_all(target_album_dir).await.ok()?;
    let gif_target = target_album_dir.join("cover.gif");
    let webp_target = target_album_dir.join("cover.webp");
    if gif_target.exists() {
        return Some(gif_target);
    }
    if webp_target.exists() {
        return Some(webp_target);
    }

    let search_query = format!("{} {}", artist, album);
    let itunes_url = format!(
        "https://itunes.apple.com/search?term={}&entity=album&limit=3",
        urlencoding::encode(&search_query)
    );

    let res = client.get(&itunes_url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }

    let json: serde_json::Value = res.json().await.ok()?;
    let results = json["results"].as_array()?;
    if results.is_empty() {
        return None;
    }

    let artwork_url = results[0]["artworkUrl100"].as_str()?;
    let high_res_cover = artwork_url
        .replace("100x100bb.jpg", "1200x1200bb.jpg")
        .replace("100x100bb", "1200x1200bb");

    let img_res = client.get(&high_res_cover).send().await.ok()?;
    if img_res.status().is_success() {
        let bytes = img_res.bytes().await.ok()?;
        let img_path = target_album_dir.join("cover_hi.jpg");
        let _ = tokio::fs::write(&img_path, &bytes).await;

        let ffmpeg_status = std::process::Command::new("ffmpeg")
            .arg("-y")
            .arg("-i")
            .arg(&img_path)
            .arg(&webp_target)
            .output();

        if ffmpeg_status.is_ok() && webp_target.exists() {
            let _ = tokio::fs::remove_file(&img_path);
            info!("[AnimatedCover] Converted high-res artwork to {}", webp_target.display());
            return Some(webp_target);
        }
        return Some(img_path);
    }

    None
}

/// Extract Apple Music WebPlayKid token dynamically
pub async fn extract_apple_music_token(client: &Client) -> Option<String> {
    use regex::Regex;
    use std::sync::OnceLock;

    static CACHED_TOKEN: OnceLock<Option<String>> = OnceLock::new();
    if let Some(cached) = CACHED_TOKEN.get() {
        return cached.clone();
    }

    let page = match client
        .get("https://music.apple.com/")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => res.text().await.unwrap_or_default(),
        _ => {
            let _ = CACHED_TOKEN.set(None);
            return None;
        }
    };

    let js_re = Regex::new(r#"(/assets/index[^"'\s>]+\.js)"#).ok()?;
    let js_path = js_re.captures(&page)?.get(1)?.as_str();
    let js_url = format!("https://music.apple.com{}", js_path);

    let js_content = match client
        .get(&js_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => res.text().await.unwrap_or_default(),
        _ => {
            let _ = CACHED_TOKEN.set(None);
            return None;
        }
    };

    let token_re = Regex::new(r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+").ok()?;
    for cap in token_re.find_iter(&js_content) {
        let token = cap.as_str();
        if token.starts_with("eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzI1NiIsImtpZCI6IldlYlBsYXlLaWQifQ") {
            let result = Some(token.to_string());
            let _ = CACHED_TOKEN.set(result.clone());
            return result;
        }
    }

    let _ = CACHED_TOKEN.set(None);
    None
}

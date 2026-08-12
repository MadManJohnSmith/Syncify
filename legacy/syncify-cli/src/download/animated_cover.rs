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

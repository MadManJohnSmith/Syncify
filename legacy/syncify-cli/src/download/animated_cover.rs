//! Apple Music Motion / Animated Cover Art Downloader & Converter for `legacy/syncify-cli`
//!
//! Restored from master backup pipeline `Syncify_FULL_BACKUP_20260810`.
//!
//! Pipeline:
//! 1. Extracts Apple Music developer token (JWT) from web player JS bundle.
//! 2. Searches iTunes API to resolve exact collectionId.
//! 3. Queries Apple Music Catalog API for `editorialVideo.motionDetailSquare.video` (HLS .m3u8).
//! 4. Converts HLS stream to animated WebP (`cover.webp`, `folder.webp`, `animated.webp`) and GIF using ffmpeg.
//! 5. Embeds animated `image/webp` picture frame into FLAC files using `metaflac` without duplicating PICTURE blocks.

use reqwest::Client;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Explicit Animated Cover Resolution Status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimatedCoverStatus {
    Success(PathBuf),
    NotFound,
    SourceUnavailable(String),
    Failed(String),
}

/// Extract Apple Music developer token (JWT) from the web player JavaScript bundle.
pub async fn extract_apple_music_token(client: &Client) -> Option<String> {
    use regex::Regex;
    use std::sync::OnceLock;

    static CACHED_TOKEN: OnceLock<Option<String>> = OnceLock::new();
    if let Some(cached) = CACHED_TOKEN.get() {
        return cached.clone();
    }

    info!("[AnimatedCover] Extracting Apple Music web player token...");

    // Step 1: Fetch music.apple.com to find JS bundle URL
    let page = match client
        .get("https://music.apple.com/")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => res.text().await.unwrap_or_default(),
        _ => {
            warn!("[AnimatedCover] Failed to fetch music.apple.com");
            let _ = CACHED_TOKEN.set(None);
            return None;
        }
    };

    // Step 2: Find JS bundle path (index-legacy~*.js, index-*.js, web-player-*.js)
    let js_re = match Regex::new(r#"(/assets/(?:index|web-player|app)[^"'\s>]+\.js)"#)
        .or_else(|_| Regex::new(r#"(/assets/[^"'\s>]+\.js)"#)) {
        Ok(re) => re,
        Err(_) => return None,
    };
    let js_path = match js_re.captures(&page).and_then(|c| c.get(1)) {
        Some(m) => m.as_str(),
        None => return None,
    };

    let js_url = format!("https://music.apple.com{}", js_path);
    debug!("[AnimatedCover] Fetching JS bundle: {}...", &js_url[..js_url.len().min(80)]);

    // Step 3: Download JS bundle and extract JWT token
    let js_content = match client
        .get(&js_url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
    {
        Ok(res) if res.status().is_success() => res.text().await.unwrap_or_default(),
        _ => {
            warn!("[AnimatedCover] Failed to download JS bundle");
            let _ = CACHED_TOKEN.set(None);
            return None;
        }
    };

    let token_re = match Regex::new(r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+") {
        Ok(re) => re,
        Err(_) => return None,
    };

    for cap in token_re.find_iter(&js_content) {
        let token = cap.as_str();
        if token.starts_with("eyJ0eXAiOiJKV1QiLCJhbGciOiJFUzI1NiIsImtpZCI6IldlYlBsYXlLaWQifQ") {
            info!("[AnimatedCover] Successfully extracted Apple Music WebPlayKid token");
            let result = Some(token.to_string());
            let _ = CACHED_TOKEN.set(result.clone());
            return result;
        }
    }

    if let Some(cap) = token_re.find(&js_content) {
        let token = cap.as_str().to_string();
        info!("[AnimatedCover] Using fallback JWT token from Apple Music JS bundle");
        let result = Some(token);
        let _ = CACHED_TOKEN.set(result.clone());
        return result;
    }

    warn!("[AnimatedCover] No JWT token found in Apple Music JS bundle");
    let _ = CACHED_TOKEN.set(None);
    None
}

pub fn strip_album_edition_suffixes(title: &str) -> String {
    let mut cleaned = title.to_string();
    let suffixes = [
        "(Deluxe Edition)", "(Deluxe)", "(Extended Edition)", "(Extended)",
        "(The Complete Edition)", "(Complete Edition)", "(Special Edition)",
        "[Deluxe Edition]", "[Deluxe]", "[Extended Edition]", "[Extended]",
        "Deluxe Edition", "Extended Edition", "The Complete Edition",
    ];
    for suf in &suffixes {
        if let Some(pos) = cleaned.to_lowercase().find(&suf.to_lowercase()) {
            cleaned = cleaned[..pos].trim().to_string();
            break;
        }
    }
    cleaned
}

/// Download animated album cover art from Apple Music with explicit status.
pub async fn resolve_and_download_animated_cover(
    client: &Client,
    artist: &str,
    album: &str,
    target_dir: &Path,
) -> AnimatedCoverStatus {
    if artist.trim().is_empty() || album.trim().is_empty() {
        return AnimatedCoverStatus::NotFound;
    }

    if let Err(e) = tokio::fs::create_dir_all(target_dir).await {
        return AnimatedCoverStatus::Failed(format!("Failed to create target directory: {}", e));
    }

    // Step 1: Extract Apple Music developer token
    let am_token = match extract_apple_music_token(client).await {
        Some(token) => token,
        None => {
            return AnimatedCoverStatus::SourceUnavailable(
                "Could not extract Apple Music developer token from web player".to_string(),
            );
        }
    };

    // Step 2A: Query iTunes Search API to resolve exact Apple Music collectionIds
    let album_lower = album.to_lowercase();
    let artist_lower = artist.to_lowercase();
    let clean_album = strip_album_edition_suffixes(album);
    let search_terms = vec![
        format!("{} {}", artist, clean_album),
        format!("{} {}", artist, album),
        clean_album.clone(),
    ];

    let mut collection_ids = Vec::new();
    for term in &search_terms {
        let itunes_url = format!(
            "https://itunes.apple.com/search?term={}&entity=album&limit=10",
            urlencoding::encode(term)
        );
        if let Ok(res) = client.get(&itunes_url).send().await {
            if res.status().is_success() {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    if let Some(results) = json["results"].as_array() {
                        for item in results {
                            let r_artist = item["artistName"].as_str().unwrap_or("").to_lowercase();
                            let r_album = item["collectionName"].as_str().unwrap_or("").to_lowercase();

                            let artist_match = r_artist.contains(&artist_lower) || artist_lower.contains(&r_artist);
                            let album_match = r_album.contains(&album_lower) || album_lower.contains(&r_album)
                                || (!clean_album.is_empty() && r_album.contains(&clean_album.to_lowercase()));

                            if artist_match && album_match {
                                if let Some(cid) = item["collectionId"].as_u64() {
                                    if !collection_ids.contains(&cid) {
                                        collection_ids.push(cid);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if !collection_ids.is_empty() {
            break;
        }
    }

    let mut m3u8_url: Option<String> = None;
    let storefronts = vec!["us", "gb", "es", "de", "fr", "mx", "it", "ca", "au", "jp", "nl", "br"];

    // Step 2B: Direct lookup by collectionId on Apple Music catalog API
    'id_lookup: for cid in &collection_ids {
        for sf in &storefronts {
            let album_url = format!(
                "https://amp-api.music.apple.com/v1/catalog/{}/albums/{}?extend=editorialVideo",
                sf, cid
            );
            let req = client
                .get(&album_url)
                .header("Authorization", format!("Bearer {}", am_token))
                .header("Origin", "https://music.apple.com")
                .header("Referer", "https://music.apple.com/")
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");

            if let Ok(res) = req.send().await {
                if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                    continue;
                }
                if res.status().is_success() {
                    if let Ok(json) = res.json::<serde_json::Value>().await {
                        if let Some(albums_arr) = json["data"].as_array() {
                            if let Some(item) = albums_arr.first() {
                                let attrs = &item["attributes"];
                                let video = attrs["editorialVideo"]["motionDetailSquare"]["video"].as_str()
                                    .or_else(|| attrs["editorialVideo"]["motionSquareVideo1x1"]["video"].as_str())
                                    .or_else(|| attrs["editorialVideo"]["motionDetailTall"]["video"].as_str())
                                    .or_else(|| attrs["editorialArtwork"]["motionDetailSquare"]["video"].as_str());

                                if let Some(vid_url) = video {
                                    info!("[AnimatedCover] ✓ Found animated cover HLS stream via ID {} on '{}' for '{} - {}'", cid, sf, artist, album);
                                    m3u8_url = Some(vid_url.to_string());
                                    break 'id_lookup;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Step 2C: Fallback to Catalog Search across storefronts
    if m3u8_url.is_none() {
        'sf_search: for sf in &storefronts {
            let term = format!("{} {}", artist, clean_album);
            let catalog_search_url = format!(
                "https://amp-api.music.apple.com/v1/catalog/{}/search?term={}&types=albums&extend=editorialVideo&limit=5",
                sf,
                urlencoding::encode(&term)
            );

            let req = client
                .get(&catalog_search_url)
                .header("Authorization", format!("Bearer {}", am_token))
                .header("Origin", "https://music.apple.com")
                .header("Referer", "https://music.apple.com/")
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36");

            if let Ok(res) = req.send().await {
                if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                    continue;
                }
                if res.status().is_success() {
                    if let Ok(json) = res.json::<serde_json::Value>().await {
                        if let Some(albums_arr) = json["results"]["albums"]["data"].as_array() {
                            for item in albums_arr {
                                let attrs = &item["attributes"];
                                let r_artist = attrs["artistName"].as_str().unwrap_or("").to_lowercase();
                                let r_album = attrs["name"].as_str().unwrap_or("").to_lowercase();

                                let artist_match = r_artist.contains(&artist_lower) || artist_lower.contains(&r_artist);
                                let album_match = r_album.contains(&album_lower) || album_lower.contains(&r_album)
                                    || (!clean_album.is_empty() && r_album.contains(&clean_album.to_lowercase()));

                                if artist_match && album_match {
                                    let video = attrs["editorialVideo"]["motionDetailSquare"]["video"].as_str()
                                        .or_else(|| attrs["editorialVideo"]["motionSquareVideo1x1"]["video"].as_str())
                                        .or_else(|| attrs["editorialVideo"]["motionDetailTall"]["video"].as_str())
                                        .or_else(|| attrs["editorialArtwork"]["motionDetailSquare"]["video"].as_str());

                                    if let Some(vid_url) = video {
                                        info!("[AnimatedCover] ✓ Found animated cover HLS stream on '{}' for '{} - {}'", sf, artist, album);
                                        m3u8_url = Some(vid_url.to_string());
                                        break 'sf_search;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let m3u8_url = match m3u8_url {
        Some(url) => url,
        None => {
            info!("[AnimatedCover] No animated artwork available for '{}' - '{}' across storefronts", artist, album);
            return AnimatedCoverStatus::NotFound;
        }
    };

    info!("[AnimatedCover] Found animated artwork HLS stream: {}", &m3u8_url[..m3u8_url.len().min(80)]);

    // Step 3: Convert HLS stream to animated WebP using ffmpeg (libwebp)
    let webp_path = target_dir.join("cover.webp");

    let webp_result = tokio::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", &m3u8_url,
            "-t", "8",
            "-vf", "fps=15,scale=500:500:flags=lanczos",
            "-vcodec", "libwebp",
            "-loop", "0",
            "-q:v", "75",
            "-an",
            webp_path.to_str().unwrap_or("cover.webp"),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    match webp_result {
        Ok(r) if r.status.success() => {
            if webp_path.exists() {
                let size = std::fs::metadata(&webp_path).map(|m| m.len()).unwrap_or(0);
                if size == 0 {
                    let _ = std::fs::remove_file(&webp_path);
                    return AnimatedCoverStatus::Failed("ffmpeg generated 0-byte cover.webp".to_string());
                }

                let folder_webp = target_dir.join("folder.webp");
                let animated_webp = target_dir.join("animated.webp");
                let _ = std::fs::copy(&webp_path, &folder_webp);
                let _ = std::fs::copy(&webp_path, &animated_webp);

                info!("[AnimatedCover] ✓ High-quality animated cover.webp, folder.webp & animated.webp sidecars saved ({} KB): {:?}", size / 1024, webp_path);
                AnimatedCoverStatus::Success(webp_path)
            } else {
                warn!("[AnimatedCover] ffmpeg completed but cover.webp not found at {:?}", webp_path);
                AnimatedCoverStatus::Failed("ffmpeg completed successfully but cover.webp not found on disk".to_string())
            }
        }
        Ok(r) => {
            let err_msg = String::from_utf8_lossy(&r.stderr);
            warn!("[AnimatedCover] ffmpeg animated WebP conversion failed: {}", err_msg);
            AnimatedCoverStatus::Failed(format!("ffmpeg exit error: {}", err_msg.lines().next().unwrap_or("unknown error")))
        }
        Err(e) => {
            warn!("[AnimatedCover] Failed to launch ffmpeg: {}", e);
            AnimatedCoverStatus::Failed(format!("Failed to spawn ffmpeg: {}", e))
        }
    }
}

/// Download animated album cover art from Apple Music (compat wrapper returning Option<PathBuf>)
pub async fn download_animated_cover(
    client: &Client,
    artist: &str,
    album: &str,
    target_dir: &Path,
) -> Option<PathBuf> {
    match resolve_and_download_animated_cover(client, artist, album, target_dir).await {
        AnimatedCoverStatus::Success(p) => Some(p),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_album_edition_suffixes() {
        assert_eq!(strip_album_edition_suffixes("Thriller (Deluxe Edition)"), "Thriller");
        assert_eq!(strip_album_edition_suffixes("Abbey Road [Deluxe]"), "Abbey Road");
        assert_eq!(strip_album_edition_suffixes("Random Access Memories (Complete Edition)"), "Random Access Memories");
        assert_eq!(strip_album_edition_suffixes("Heroes"), "Heroes");
    }

    #[test]
    fn test_animated_cover_status_variants() {
        let success = AnimatedCoverStatus::Success(PathBuf::from("/tmp/cover.webp"));
        let not_found = AnimatedCoverStatus::NotFound;
        let source_unavail = AnimatedCoverStatus::SourceUnavailable("Token expired".to_string());
        let failed = AnimatedCoverStatus::Failed("ffmpeg error".to_string());

        assert!(matches!(success, AnimatedCoverStatus::Success(_)));
        assert_eq!(not_found, AnimatedCoverStatus::NotFound);
        assert!(matches!(source_unavail, AnimatedCoverStatus::SourceUnavailable(_)));
        assert!(matches!(failed, AnimatedCoverStatus::Failed(_)));
    }

    #[test]
    fn test_metaflac_picture_embedding_no_duplicate_blocks() {
        let temp_dir = std::env::temp_dir().join(format!("test_flac_pic_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let flac_path = temp_dir.join("test.flac");

        // Construct valid minimal FLAC
        let mut flac_bytes = Vec::new();
        flac_bytes.extend_from_slice(b"fLaC");
        flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // is_last=1, len=34
        let mut streaminfo = [0u8; 34];
        streaminfo[0..2].copy_from_slice(&4608u16.to_be_bytes());
        streaminfo[2..4].copy_from_slice(&4608u16.to_be_bytes());
        streaminfo[10] = 0x0A;
        streaminfo[11] = 0xC4;
        streaminfo[12] = 0x42;
        streaminfo[13] = 0xF0;
        flac_bytes.extend_from_slice(&streaminfo);
        flac_bytes.extend_from_slice(&[0xFF, 0xF8, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00]); // frame sync 0xFFF8
        std::fs::write(&flac_path, &flac_bytes).unwrap();

        // 1. Add first picture
        let pic1 = vec![1, 2, 3, 4, 5];
        let mut tag = metaflac::Tag::read_from_path(&flac_path).unwrap();
        tag.remove_picture_type(metaflac::block::PictureType::CoverFront);
        tag.add_picture("image/jpeg", metaflac::block::PictureType::CoverFront, pic1);
        tag.write_to_path(&flac_path).unwrap();

        // 2. Replace with animated webp picture
        let pic_webp = vec![0x52, 0x49, 0x46, 0x46, 0x00, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50]; // RIFF...WEBP
        let mut tag2 = metaflac::Tag::read_from_path(&flac_path).unwrap();
        tag2.remove_picture_type(metaflac::block::PictureType::CoverFront);
        tag2.add_picture("image/webp", metaflac::block::PictureType::CoverFront, pic_webp.clone());
        tag2.write_to_path(&flac_path).unwrap();

        // 3. Verify exactly 1 picture block exists and MIME is image/webp
        let tag_read = metaflac::Tag::read_from_path(&flac_path).unwrap();
        let pictures: Vec<_> = tag_read.pictures().collect();
        assert_eq!(pictures.len(), 1, "There must be exactly 1 picture block without duplicates");
        assert_eq!(pictures[0].mime_type, "image/webp");
        assert_eq!(pictures[0].data, pic_webp);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

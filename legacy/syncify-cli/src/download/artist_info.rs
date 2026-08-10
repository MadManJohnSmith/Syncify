// ArtistInfo Engine for Syncify & Symfonium
// Generates Kodi/Symfonium compatible artist.nfo XML and downloads genuine artist.jpg / fanart.jpg

use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use std::path::Path;
use tracing::{debug, info};

/// Download artist profile image (artist.jpg), fanart image (fanart.jpg),
/// and generate XML artist.nfo metadata file in the target artist directory.
pub async fn download_artist_info(
    client: &Client,
    artist: &str,
    target_artist_dir: &Path,
) -> Result<()> {
    download_artist_info_with_url(client, artist, target_artist_dir, None).await
}

/// Download artist info with optional high-resolution Qobuz artist portrait URL
pub async fn download_artist_info_with_url(
    client: &Client,
    artist: &str,
    target_artist_dir: &Path,
    qobuz_picture_url: Option<&str>,
) -> Result<()> {
    if artist.trim().is_empty() || artist.eq_ignore_ascii_case("Various Artists") {
        return Ok(());
    }

    tokio::fs::create_dir_all(target_artist_dir).await?;

    let artist_jpg_path = target_artist_dir.join("artist.jpg");
    let fanart_jpg_path = target_artist_dir.join("fanart.jpg");
    let artist_nfo_path = target_artist_dir.join("artist.nfo");

    // 1. Fetch Artist Metadata from MusicBrainz
    let mut bio = String::new();
    let mut country = String::new();
    let mut type_name = String::new();
    let mut mbid = String::new();
    let mut genres: Vec<String> = Vec::new();

    let mb_url = format!(
        "https://musicbrainz.org/ws/2/artist/?query=artist:{}&fmt=json",
        urlencoding::encode(artist)
    );

    if let Ok(res) = client
        .get(&mb_url)
        .header("User-Agent", "Syncify/1.0 (https://github.com/MadManJohnSmith/Syncify)")
        .send()
        .await
    {
        if res.status().is_success() {
            if let Ok(json) = res.json::<Value>().await {
                if let Some(artists_arr) = json["artists"].as_array() {
                    if let Some(first_artist) = artists_arr.first() {
                        mbid = first_artist["id"].as_str().unwrap_or("").to_string();
                        country = first_artist["country"].as_str().unwrap_or("").to_string();
                        type_name = first_artist["type"].as_str().unwrap_or("").to_string();
                        bio = first_artist["disambiguation"].as_str().unwrap_or("").to_string();

                        if let Some(tag_list) = first_artist["tags"].as_array() {
                            for tag in tag_list.iter().take(5) {
                                if let Some(t_name) = tag["name"].as_str() {
                                    genres.push(t_name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Fetch Genuine Artist Portrait (artist.jpg) - NO album cover fallback!
    if !artist_jpg_path.exists() {
        let mut portrait_saved = false;

        // Tier 0: Direct Qobuz Artist Portrait URL if provided
        if let Some(q_url) = qobuz_picture_url {
            if !q_url.is_empty() {
                if let Ok(img_res) = client.get(q_url).send().await {
                    if img_res.status().is_success() {
                        if let Ok(bytes) = img_res.bytes().await {
                            if !bytes.is_empty() {
                                let _ = tokio::fs::write(&artist_jpg_path, &bytes).await;
                                info!("[ArtistInfo] Saved official Qobuz artist.jpg for '{}'", artist);
                                portrait_saved = true;
                            }
                        }
                    }
                }
            }
        }

        // Tier 1: Deezer Artist API (1000x1000 Studio Portrait)
        if !portrait_saved {
            let deezer_url = format!("https://api.deezer.com/artist/{}", urlencoding::encode(artist));
            if let Ok(res) = client.get(&deezer_url).send().await {
                if res.status().is_success() {
                    if let Ok(json) = res.json::<Value>().await {
                        let picture_url = json["picture_xl"]
                            .as_str()
                            .or_else(|| json["picture_big"].as_str())
                            .or_else(|| json["picture_medium"].as_str());

                        if let Some(p_url) = picture_url {
                            if let Ok(img_res) = client.get(p_url).send().await {
                                if img_res.status().is_success() {
                                    if let Ok(bytes) = img_res.bytes().await {
                                        let _ = tokio::fs::write(&artist_jpg_path, &bytes).await;
                                        info!("[ArtistInfo] Saved 1000x1000 Deezer artist.jpg portrait for '{}'", artist);
                                        portrait_saved = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Tier 2: iTunes musicArtist Portrait
        if !portrait_saved {
            let itunes_url = format!(
                "https://itunes.apple.com/search?term={}&entity=musicArtist&limit=1",
                urlencoding::encode(artist)
            );

            if let Ok(res) = client.get(&itunes_url).send().await {
                if res.status().is_success() {
                    if let Ok(json) = res.json::<Value>().await {
                        if let Some(img_url) = json["results"][0]["artworkUrl100"].as_str() {
                            let highres_url = img_url.replace("100x100bb", "1000x1000bb");
                            if let Ok(img_res) = client.get(&highres_url).send().await {
                                if img_res.status().is_success() {
                                    if let Ok(bytes) = img_res.bytes().await {
                                        let _ = tokio::fs::write(&artist_jpg_path, &bytes).await;
                                        info!("[ArtistInfo] Saved iTunes artist.jpg for '{}'", artist);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Fetch Genuine 1920x1080 Horizontal Fanart (fanart.jpg) from TheAudioDB
    if !fanart_jpg_path.exists() {
        let audiodb_url = format!(
            "https://www.theaudiodb.com/api/v1/json/2/search.php?s={}",
            urlencoding::encode(artist)
        );

        let mut fanart_saved = false;
        if let Ok(res) = client.get(&audiodb_url).send().await {
            if res.status().is_success() {
                if let Ok(json) = res.json::<Value>().await {
                    if let Some(artists_arr) = json["artists"].as_array() {
                        if let Some(first_art) = artists_arr.first() {
                            let fanart_url = first_art["strArtistFanart"]
                                .as_str()
                                .or_else(|| first_art["strArtistWideThumb"].as_str())
                                .or_else(|| first_art["strArtistThumb"].as_str());

                            if let Some(f_url) = fanart_url {
                                if !f_url.is_empty() {
                                    if let Ok(f_res) = client.get(f_url).send().await {
                                        if f_res.status().is_success() {
                                            if let Ok(bytes) = f_res.bytes().await {
                                                let _ = tokio::fs::write(&fanart_jpg_path, &bytes).await;
                                                info!("[ArtistInfo] Saved 1080p horizontal fanart.jpg for '{}'", artist);
                                                fanart_saved = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback: If no horizontal fanart exists on TheAudioDB, copy the high-res artist portrait
        if !fanart_saved && artist_jpg_path.exists() {
            let _ = tokio::fs::copy(&artist_jpg_path, &fanart_jpg_path).await;
            debug!("[ArtistInfo] Used artist.jpg as fanart.jpg fallback for '{}'", artist);
        }
    }

    // Save standard Symfonium / Kodi image aliases for universal player detection
    if artist_jpg_path.exists() {
        let folder_jpg = target_artist_dir.join("folder.jpg");
        let poster_jpg = target_artist_dir.join("poster.jpg");
        let thumb_jpg = target_artist_dir.join("thumb.jpg");
        if !folder_jpg.exists() { let _ = tokio::fs::copy(&artist_jpg_path, &folder_jpg).await; }
        if !poster_jpg.exists() { let _ = tokio::fs::copy(&artist_jpg_path, &poster_jpg).await; }
        if !thumb_jpg.exists() { let _ = tokio::fs::copy(&artist_jpg_path, &thumb_jpg).await; }
    }

    if fanart_jpg_path.exists() {
        let backdrop_jpg = target_artist_dir.join("backdrop.jpg");
        if !backdrop_jpg.exists() { let _ = tokio::fs::copy(&fanart_jpg_path, &backdrop_jpg).await; }
    }

    // 4. Generate XML artist.nfo File (Kodi & Symfonium Compatible)
    let mut nfo_xml = String::new();
    nfo_xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>\n");
    nfo_xml.push_str("<artist>\n");
    nfo_xml.push_str(&format!("  <name>{}</name>\n", escape_xml(artist)));
    if !mbid.is_empty() {
        nfo_xml.push_str(&format!("  <musicbrainzartistid>{}</musicbrainzartistid>\n", mbid));
    }
    if !type_name.is_empty() {
        nfo_xml.push_str(&format!("  <type>{}</type>\n", escape_xml(&type_name)));
    }
    if !country.is_empty() {
        nfo_xml.push_str(&format!("  <country>{}</country>\n", escape_xml(&country)));
    }
    if !bio.is_empty() {
        nfo_xml.push_str(&format!("  <biography>{}</biography>\n", escape_xml(&bio)));
    }
    for g in &genres {
        nfo_xml.push_str(&format!("  <genre>{}</genre>\n", escape_xml(g)));
    }
    nfo_xml.push_str("  <thumb aspect=\"thumb\" preview=\"artist.jpg\">artist.jpg</thumb>\n");
    nfo_xml.push_str("  <fanart>\n");
    nfo_xml.push_str("    <thumb preview=\"fanart.jpg\">fanart.jpg</thumb>\n");
    nfo_xml.push_str("  </fanart>\n");
    nfo_xml.push_str("</artist>\n");

    let _ = tokio::fs::write(&artist_nfo_path, nfo_xml).await;
    info!("[ArtistInfo] Saved artist.nfo for '{}'", artist);

    Ok(())
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// URL Import Commands - parsing streaming service URLs
//
// This file is included via include!() in mod.rs

/// Parse a streaming service URL and extract service, content type, and ID
#[tauri::command]
pub async fn import_from_url(url: String) -> Result<ParsedUrl, String> {
    tracing::info!("import_from_url called with: {}", url);

    let url_lower = url.to_lowercase();

    // Spotify: open.spotify.com/{type}/{id} or spotify.com/{type}/{id}
    if url_lower.contains("spotify.com") {
        return parse_spotify_url(&url);
    }

    // Qobuz: play.qobuz.com/{type}/{id} or open.qobuz.com/{type}/{id}
    if url_lower.contains("qobuz.com") {
        return parse_qobuz_url(&url);
    }

    // Tidal: tidal.com/{type}/{id} or listen.tidal.com/{type}/{id}
    if url_lower.contains("tidal.com") {
        return parse_tidal_url(&url);
    }

    // Deezer: deezer.com/{type}/{id}
    if url_lower.contains("deezer.com") {
        return parse_deezer_url(&url);
    }

    Err("Unsupported URL. Please use a Spotify, Qobuz, Tidal, or Deezer link.".to_string())
}

fn parse_spotify_url(url: &str) -> Result<ParsedUrl, String> {
    let path = extract_path(url)?;
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.len() >= 2 {
        let content_type = parts[0].to_string();
        let id = parts[1].split('?').next().unwrap_or(parts[1]).to_string();

        if is_valid_content_type(&content_type) {
            return Ok(ParsedUrl {
                service: "spotify".to_string(),
                content_type,
                id,
                url: url.to_string(),
            });
        }
    }

    Err("Invalid Spotify URL format. Expected: spotify.com/{track|album|playlist|artist}/{id}".into())
}

fn parse_qobuz_url(url: &str) -> Result<ParsedUrl, String> {
    let path = extract_path(url)?;
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.len() >= 2 {
        let content_type = parts[0].to_string();
        let id = parts[1].split('?').next().unwrap_or(parts[1]).to_string();

        if is_valid_content_type(&content_type) {
            return Ok(ParsedUrl {
                service: "qobuz".to_string(),
                content_type,
                id,
                url: url.to_string(),
            });
        }
    }

    Err("Invalid Qobuz URL format. Expected: qobuz.com/{track|album|playlist|artist}/{id}".into())
}

fn parse_tidal_url(url: &str) -> Result<ParsedUrl, String> {
    let path = extract_path(url)?;
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    let (content_type, id) = if parts.len() >= 3 && parts[0] == "browse" {
        (parts[1].to_string(), parts[2].split('?').next().unwrap_or(parts[2]).to_string())
    } else if parts.len() >= 2 {
        (parts[0].to_string(), parts[1].split('?').next().unwrap_or(parts[1]).to_string())
    } else {
        return Err("Invalid Tidal URL format".into());
    };

    if is_valid_content_type(&content_type) {
        return Ok(ParsedUrl {
            service: "tidal".to_string(),
            content_type,
            id,
            url: url.to_string(),
        });
    }

    Err("Invalid Tidal URL format. Expected: tidal.com/{track|album|playlist|artist}/{id}".into())
}

fn parse_deezer_url(url: &str) -> Result<ParsedUrl, String> {
    let path = extract_path(url)?;
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    let (content_type, id) = if parts.len() >= 3 && parts[0].len() == 2 {
        (parts[1].to_string(), parts[2].split('?').next().unwrap_or(parts[2]).to_string())
    } else if parts.len() >= 2 {
        (parts[0].to_string(), parts[1].split('?').next().unwrap_or(parts[1]).to_string())
    } else {
        return Err("Invalid Deezer URL format".into());
    };

    if is_valid_content_type(&content_type) {
        return Ok(ParsedUrl {
            service: "deezer".to_string(),
            content_type,
            id,
            url: url.to_string(),
        });
    }

    Err("Invalid Deezer URL format. Expected: deezer.com/{track|album|playlist|artist}/{id}".into())
}

fn extract_path(url: &str) -> Result<String, String> {
    let url = url.trim();
    if let Some(pos) = url.find("://") {
        let after_protocol = &url[pos + 3..];
        if let Some(slash_pos) = after_protocol.find('/') {
            return Ok(after_protocol[slash_pos..].to_string());
        }
    }
    Err("Could not parse URL path".into())
}

fn is_valid_content_type(content_type: &str) -> bool {
    matches!(
        content_type.to_lowercase().as_str(),
        "track" | "album" | "playlist" | "artist"
    )
}

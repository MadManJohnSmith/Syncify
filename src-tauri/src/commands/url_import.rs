#[allow(unused_imports)]
use super::*;
use crate::commands::queue::perform_add_to_queue;
use crate::commands::types::ParsedUrl;
use crate::download::orchestrator::DownloadOrchestrator;
use crate::download::songlink::SongLinkClient;
use std::sync::Arc;
use tauri::State;

// URL Import Commands - parsing streaming service URLs and enqueuing them
//
// Submodule of crate::commands

/// Parse a streaming service URL and extract service, content type, and ID
pub fn parse_streaming_url(url: &str) -> Result<ParsedUrl, String> {
    let url_lower = url.to_lowercase();

    // Spotify: open.spotify.com/{type}/{id} or spotify.com/{type}/{id}
    if url_lower.contains("spotify.com") {
        return parse_spotify_url(url);
    }

    // Qobuz: play.qobuz.com/{type}/{id} or open.qobuz.com/{type}/{id}
    if url_lower.contains("qobuz.com") {
        return parse_qobuz_url(url);
    }

    // Tidal: tidal.com/{type}/{id} or listen.tidal.com/{type}/{id}
    if url_lower.contains("tidal.com") {
        return parse_tidal_url(url);
    }

    // Deezer: deezer.com/{type}/{id}
    if url_lower.contains("deezer.com") {
        return parse_deezer_url(url);
    }

    Err("Unsupported URL. Please use a Spotify, Qobuz, Tidal, or Deezer link.".to_string())
}

/// Perform resolution and transactional enqueuing of a URL into `download_queue`
pub async fn perform_import_from_url(
    db: &crate::DbPool,
    orchestrator: Option<&DownloadOrchestrator>,
    url: &str,
) -> Result<ParsedUrl, String> {
    perform_import_from_url_with_quality(db, orchestrator, url, None).await
}

/// Perform resolution and transactional enqueuing of a URL into `download_queue` with specified quality
pub async fn perform_import_from_url_with_quality(
    db: &crate::DbPool,
    orchestrator: Option<&DownloadOrchestrator>,
    url: &str,
    requested_quality: Option<&str>,
) -> Result<ParsedUrl, String> {
    tracing::info!("perform_import_from_url called with: {}", url);

    // 1. Parse URL
    let parsed = parse_streaming_url(url)?;

    if parsed.content_type != "track" {
        return Err(format!(
            "Import and enqueuing from URL currently supports individual tracks (found: '{}')",
            parsed.content_type
        ));
    }

    // 2. Query SongLink for cross-platform resolution and rich metadata
    let songlink_client = if let Some(orch) = orchestrator {
        orch.songlink()
    } else {
        Arc::new(SongLinkClient::new())
    };

    let priority_order: Vec<String> = if let Some(orch) = orchestrator {
        orch.service_priority().to_vec()
    } else {
        let prefs: Vec<(String,)> = sqlx::query_as(
            "SELECT service_name FROM service_preferences ORDER BY priority ASC",
        )
        .fetch_all(db)
        .await
        .unwrap_or_default();
        if !prefs.is_empty() {
            prefs.into_iter().map(|(p,)| p).collect()
        } else {
            vec!["qobuz".to_string(), "tidal".to_string(), "amazon".to_string()]
        }
    };

    let songlink_res = songlink_client.check_from_url(url).await;

    let (target_service, target_track_id, title, artist, is_cross_platform) = match songlink_res {
        Ok(avail) => {
            tracing::info!(
                "[import_from_url] SongLink resolved: title={:?}, artist={:?}, tidal={:?}, qobuz={:?}",
                avail.title,
                avail.artist_name,
                avail.tidal_id,
                avail.qobuz_id
            );

            // Determine candidate download service
            let (chosen_service, chosen_track_id, cross) = if parsed.service == "spotify" {
                let mut resolved = None;
                for pref in &priority_order {
                    match pref.to_lowercase().as_str() {
                        "qobuz" => {
                            if let Some(ref qid) = avail.qobuz_id {
                                resolved = Some(("qobuz".to_string(), qid.clone(), true));
                                break;
                            }
                        }
                        "tidal" => {
                            if let Some(ref tid) = avail.tidal_id {
                                resolved = Some(("tidal".to_string(), tid.clone(), true));
                                break;
                            }
                        }
                        _ => {}
                    }
                }

                resolved.unwrap_or_else(|| {
                    if let Some(ref qid) = avail.qobuz_id {
                        ("qobuz".to_string(), qid.clone(), true)
                    } else if let Some(ref tid) = avail.tidal_id {
                        ("tidal".to_string(), tid.clone(), true)
                    } else {
                        (parsed.service.clone(), parsed.id.clone(), false)
                    }
                })
            } else if parsed.service == "tidal" {
                ("tidal".to_string(), parsed.id.clone(), false)
            } else if parsed.service == "qobuz" {
                ("qobuz".to_string(), parsed.id.clone(), false)
            } else if parsed.service == "deezer" {
                if let Some(ref qid) = avail.qobuz_id {
                    ("qobuz".to_string(), qid.clone(), true)
                } else if let Some(ref tid) = avail.tidal_id {
                    ("tidal".to_string(), tid.clone(), true)
                } else {
                    ("deezer".to_string(), parsed.id.clone(), false)
                }
            } else {
                (parsed.service.clone(), parsed.id.clone(), false)
            };

            let title_str = avail.title.unwrap_or_else(|| {
                format!("{} Track {}", capitalize(&parsed.service), parsed.id)
            });
            let artist_str = avail
                .artist_name
                .unwrap_or_else(|| "Unknown Artist".to_string());

            (chosen_service, chosen_track_id, title_str, artist_str, cross)
        }
        Err(e) => {
            tracing::warn!(
                "[import_from_url] SongLink resolution failed for URL '{}': {}. Falling back to parsed URL.",
                url,
                e
            );
            (
                parsed.service.clone(),
                parsed.id.clone(),
                format!("{} Track {}", capitalize(&parsed.service), parsed.id),
                "Unknown Artist".to_string(),
                false,
            )
        }
    };

    // 3. Resolve or insert track entry in DB
    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
        .bind(&target_service)
        .fetch_optional(db)
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| match target_service.as_str() {
            "spotify" => 1,
            "qobuz" => 2,
            "tidal" => 3,
            "deezer" => 4,
            _ => 1,
        });

    // Check if track already exists via track_sources
    let existing_track_id: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT ts.track_id
        FROM track_sources ts
        WHERE ts.service_id = ? AND ts.service_track_id = ?
        LIMIT 1
        "#,
    )
    .bind(service_id)
    .bind(&target_track_id)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let track_id = if let Some(tid) = existing_track_id {
        tid
    } else {
        // Insert new track
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, created_at) VALUES (?, CURRENT_TIMESTAMP) RETURNING id",
        )
        .bind(&title)
        .fetch_one(db)
        .await
        .map_err(|e| format!("Failed to create track entry: {}", e))?;

        // Link artist if valid
        if !artist.is_empty() && artist != "Unknown Artist" {
            let _ = sqlx::query("INSERT OR IGNORE INTO artists (name) VALUES (?)")
                .bind(&artist)
                .execute(db)
                .await;

            let artist_id: Option<i64> = sqlx::query_scalar("SELECT id FROM artists WHERE name = ?")
                .bind(&artist)
                .fetch_optional(db)
                .await
                .unwrap_or(None);

            if let Some(aid) = artist_id {
                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
                )
                .bind(tid)
                .bind(aid)
                .execute(db)
                .await;
            }
        }

        // Insert track_sources entry
        let _ = sqlx::query(
            r#"
            INSERT INTO track_sources (track_id, service_id, service_track_id, available)
            VALUES (?, ?, ?, 1)
            ON CONFLICT(track_id, service_id) DO UPDATE SET service_track_id = excluded.service_track_id
            "#,
        )
        .bind(tid)
        .bind(service_id)
        .bind(&target_track_id)
        .execute(db)
        .await;

        // If origin service is different, record origin track source as well
        if parsed.service != target_service {
            if let Ok(Some(orig_sid)) = sqlx::query_scalar::<_, i64>("SELECT id FROM services WHERE name = ?")
                .bind(&parsed.service)
                .fetch_optional(db)
                .await
            {
                let _ = sqlx::query(
                    r#"
                    INSERT INTO track_sources (track_id, service_id, service_track_id, available)
                    VALUES (?, ?, ?, 1)
                    ON CONFLICT(track_id, service_id) DO UPDATE SET service_track_id = excluded.service_track_id
                    "#,
                )
                .bind(tid)
                .bind(orig_sid)
                .bind(&parsed.id)
                .execute(db)
                .await;
            }
        }

        tid
    };

    // 4. Enqueue into download_queue using perform_add_to_queue
    let final_quality = requested_quality.unwrap_or("lossless");

    let queue_id = perform_add_to_queue(
        db,
        track_id,
        Some(50),
        Some(final_quality.to_string()),
        None,
        Some(service_id),
        Some(target_service.clone()),
        None,
        Some(target_track_id.clone()),
        None,
        Some(title.clone()),
        Some(artist.clone()),
        None,
        None,
        Some(false),
        Some(true),
        None,
    )
    .await
    .map_err(|e| format!("Failed to enqueue track: {}", e))?;

    // Record fallback provenance & requested_quality
    let _ = sqlx::query(
        r#"
        UPDATE download_queue
        SET requested_quality = COALESCE(requested_quality, quality_preference),
            origin_service = ?,
            origin_service_track_id = ?,
            effective_service = ?,
            effective_service_track_id = ?,
            fallback_reason = ?,
            match_method = ?
        WHERE id = ?
        "#,
    )
    .bind(&parsed.service)
    .bind(&parsed.id)
    .bind(&target_service)
    .bind(&target_track_id)
    .bind(if is_cross_platform {
        Some("SongLink cross-platform URL import")
    } else {
        None::<&str>
    })
    .bind(if is_cross_platform {
        Some("songlink_cross_platform")
    } else {
        None::<&str>
    })
    .bind(queue_id)
    .execute(db)
    .await;

    // Fetch actual queue status
    let q_status: Option<String> = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(queue_id)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

    Ok(ParsedUrl {
        service: parsed.service,
        content_type: parsed.content_type,
        id: parsed.id,
        url: parsed.url,
        queue_id: Some(queue_id),
        track_id: Some(track_id),
        title: Some(title),
        artist: Some(artist),
        status: q_status.or_else(|| Some("queued".to_string())),
    })
}

/// Parse a streaming service URL and enqueue it into download_queue (Tauri command)
#[tauri::command]
pub async fn import_from_url(
    url: String,
    state: State<'_, AppState>,
) -> Result<ParsedUrl, String> {
    tracing::info!("import_from_url called with: {}", url);
    let res = perform_import_from_url(&state.db, None, &url).await?;
    state.worker_state.notify_available();
    Ok(res)
}

fn parse_spotify_url(url: &str) -> Result<ParsedUrl, String> {
    let path = extract_path(url)?;
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.len() >= 2 {
        let content_type = parts[0].to_string();
        let id = parts[1].split('?').next().unwrap_or(parts[1]).to_string();

        if is_valid_content_type(&content_type) {
            return Ok(ParsedUrl::new("spotify", content_type, id, url));
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
            return Ok(ParsedUrl::new("qobuz", content_type, id, url));
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
        return Ok(ParsedUrl::new("tidal", content_type, id, url));
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
        return Ok(ParsedUrl::new("deezer", content_type, id, url));
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

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

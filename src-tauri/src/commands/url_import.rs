#[allow(unused_imports)]
use super::*;
use crate::commands::queue::normalize_queue_quality_preference;
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
    tracing::debug!("perform_import_from_url called");

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
        let prefs: Vec<(String,)> =
            sqlx::query_as("SELECT service_name FROM service_preferences ORDER BY priority ASC")
                .fetch_all(db)
                .await
                .unwrap_or_default();
        if !prefs.is_empty() {
            prefs.into_iter().map(|(p,)| p).collect()
        } else {
            vec![
                "qobuz".to_string(),
                "tidal".to_string(),
                "amazon".to_string(),
            ]
        }
    };

    let songlink_res = songlink_client.check_from_url(url).await;

    let (target_service, target_track_id, title, artist, is_cross_platform) = match songlink_res {
        Ok(avail) => {
            tracing::debug!("[import_from_url] SongLink resolution succeeded");

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

            let title_str = avail
                .title
                .unwrap_or_else(|| format!("{} Track {}", capitalize(&parsed.service), parsed.id));
            let artist_str = avail
                .artist_name
                .unwrap_or_else(|| "Unknown Artist".to_string());

            (
                chosen_service,
                chosen_track_id,
                title_str,
                artist_str,
                cross,
            )
        }
        Err(_) => {
            tracing::warn!(
                "[import_from_url] SongLink resolution failed; falling back to parsed URL"
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
        .map_err(|e| format!("Failed to resolve target service: {e}"))?
        .ok_or_else(|| format!("Target service is not configured: {target_service}"))?;
    let mut tx = db
        .begin()
        .await
        .map_err(|e| format!("Failed to begin URL import: {e}"))?;

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
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("Failed to look up track source: {e}"))?;

    let track_id = if let Some(tid) = existing_track_id {
        tid
    } else {
        // Insert new track
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, created_at) VALUES (?, CURRENT_TIMESTAMP) RETURNING id",
        )
        .bind(&title)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("Failed to create track entry: {}", e))?;

        // Link artist if valid
        if !artist.is_empty() && artist != "Unknown Artist" {
            sqlx::query("INSERT OR IGNORE INTO artists (name) VALUES (?)")
                .bind(&artist)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to insert artist: {e}"))?;

            let artist_id: Option<i64> =
                sqlx::query_scalar("SELECT id FROM artists WHERE name = ?")
                    .bind(&artist)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to look up artist: {e}"))?;

            if let Some(aid) = artist_id {
                sqlx::query(
                    "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')",
                )
                .bind(tid)
                .bind(aid)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to link artist: {e}"))?;
            }
        }

        // Insert track_sources entry
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, available) VALUES (?, ?, ?, 1)",
        )
        .bind(tid)
        .bind(service_id)
        .bind(&target_track_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to link target track source: {e}"))?;

        tid
    };
    // Record the origin even when the resolved target source already existed.
    // A conflicting origin identity must remain unchanged and require review.
    if parsed.service != target_service {
        let orig_sid: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = ?")
            .bind(&parsed.service)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Failed to resolve origin service: {e}"))?
            .ok_or_else(|| format!("Origin service is not configured: {}", parsed.service))?;
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, available) VALUES (?, ?, ?, 1) ON CONFLICT(track_id, service_id) DO NOTHING")
            .bind(track_id)
            .bind(orig_sid)
            .bind(&parsed.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Failed to link origin track source: {e}"))?;
        let recorded: String = sqlx::query_scalar(
            "SELECT service_track_id FROM track_sources WHERE track_id = ? AND service_id = ?",
        )
        .bind(track_id)
        .bind(orig_sid)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("Failed to verify origin track source: {e}"))?;
        if recorded != parsed.id {
            return Err(
                "Origin source identity conflicts with existing track; review required".to_string(),
            );
        }
    }
    // URL imports own their queue insert: the generic queue helper uses a pool and
    // cannot join this transaction. Never expose track/source rows before provenance
    // is durably recorded. Reuse only an active row whose complete identity
    // belongs to this exact URL; another origin or terminal state needs review.
    let final_quality = normalize_queue_quality_preference(Some(
        requested_quality.unwrap_or("lossless").to_string(),
    ));
    let existing_queue: Option<(i64, String, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, status, service_id, service_name, service_track_id, effective_service, effective_service_track_id, origin_service, origin_service_track_id FROM download_queue WHERE track_id = ? ORDER BY id DESC LIMIT 1",
    )
    .bind(track_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("Failed to look up URL import queue: {e}"))?;
    let (queue_id, q_status) = if let Some((
        id,
        status,
        sid,
        service,
        service_track,
        effective,
        effective_track,
        origin,
        origin_track,
    )) = existing_queue
    {
        if !matches!(status.as_str(), "queued" | "downloading")
            || sid != Some(service_id)
            || service.as_deref() != Some(target_service.as_str())
            || service_track.as_deref() != Some(target_track_id.as_str())
            || effective.as_deref() != Some(target_service.as_str())
            || effective_track.as_deref() != Some(target_track_id.as_str())
            || origin.as_deref() != Some(parsed.service.as_str())
            || origin_track.as_deref() != Some(parsed.id.as_str())
        {
            return Err(
                "Existing queue identity or state conflicts with URL import; review required"
                    .to_string(),
            );
        }
        (id, status)
    } else {
        let position: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM download_queue WHERE status = 'queued'",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("Failed to determine URL queue position: {e}"))?;
        let id: i64 = sqlx::query_scalar(
            r#"INSERT INTO download_queue (
                track_id, priority, quality_preference, requested_quality, status,
                progress_percent, retry_count, position, resumable,
                service_id, service_name, service_track_id, target_title, target_artist,
                smart_studio_origin, allow_fallback,
                origin_service, origin_service_track_id, effective_service,
                effective_service_track_id, fallback_reason, match_method, created_at
            ) VALUES (?, 50, ?, ?, 'queued', 0.0, 0, ?, 1, ?, ?, ?, ?, ?, 0, 1,
                      ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP) RETURNING id"#,
        )
        .bind(track_id)
        .bind(&final_quality)
        .bind(&final_quality)
        .bind(position)
        .bind(service_id)
        .bind(&target_service)
        .bind(&target_track_id)
        .bind(&title)
        .bind(&artist)
        .bind(&parsed.service)
        .bind(&parsed.id)
        .bind(&target_service)
        .bind(&target_track_id)
        .bind(is_cross_platform.then_some("SongLink cross-platform URL import"))
        .bind(is_cross_platform.then_some("songlink_cross_platform"))
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("Failed to enqueue URL import with provenance: {e}"))?;
        (id, "queued".to_string())
    };
    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit URL import: {e}"))?;

    Ok(ParsedUrl {
        service: parsed.service,
        content_type: parsed.content_type,
        id: parsed.id,
        url: parsed.url,
        queue_id: Some(queue_id),
        track_id: Some(track_id),
        title: Some(title),
        artist: Some(artist),
        status: Some(q_status),
    })
}

/// Parse a streaming service URL and enqueue it into download_queue (Tauri command)
#[tauri::command]
pub async fn import_from_url(url: String, state: State<'_, AppState>) -> Result<ParsedUrl, String> {
    tracing::debug!("import_from_url called");
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

    Err(
        "Invalid Spotify URL format. Expected: spotify.com/{track|album|playlist|artist}/{id}"
            .into(),
    )
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
        (
            parts[1].to_string(),
            parts[2].split('?').next().unwrap_or(parts[2]).to_string(),
        )
    } else if parts.len() >= 2 {
        (
            parts[0].to_string(),
            parts[1].split('?').next().unwrap_or(parts[1]).to_string(),
        )
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
        (
            parts[1].to_string(),
            parts[2].split('?').next().unwrap_or(parts[2]).to_string(),
        )
    } else if parts.len() >= 2 {
        (
            parts[0].to_string(),
            parts[1].split('?').next().unwrap_or(parts[1]).to_string(),
        )
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

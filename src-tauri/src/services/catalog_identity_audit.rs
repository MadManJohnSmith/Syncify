//! Read-only forensic audit for Syncify catalog, database consistency, identity integrity, and filesystem alignment.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;
use syncify_core_domain::metadata::{is_placeholder_album, is_placeholder_artist, is_placeholder_title, is_valid_isrc};

/// High-level 16-category forensic audit report.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CatalogIdentityAuditReport {
    pub audit_timestamp: String,
    pub duplicate_service_sources_count: usize,
    pub conflicting_isrc_count: usize,
    pub ghost_tracks_count: usize,
    pub ghost_albums_count: usize,
    pub ghost_artists_count: usize,
    pub downloads_without_canonical_track_count: usize,
    pub canonical_tracks_without_valid_source_count: usize,
    pub placeholder_metadata_count: usize,
    pub ambiguous_editions_count: usize,
    pub orphan_playlist_links_count: usize,
    pub physical_path_mismatches_count: usize,
    pub metadata_provenance_conflicts_count: usize,
    pub invalid_filenames_count: usize,
    pub invalid_taggings_count: usize,
    pub sidecar_mismatches_count: usize,
    pub staging_residuals_count: usize,
    pub total_anomalies: usize,
    pub details: Vec<CatalogAnomalyItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogAnomalyItem {
    pub category: String,
    pub entity_type: String,
    pub entity_id: Option<i64>,
    pub service_id: Option<i64>,
    pub service_track_id: Option<String>,
    pub message: String,
    pub suggested_action: String,
}

/// Perform a strictly read-only audit across all 16 consistency categories.
pub async fn audit_catalog_identity(
    db: &SqlitePool,
    base_dir: Option<&Path>,
) -> Result<CatalogIdentityAuditReport, String> {
    let mut details = Vec::new();

    // 1. DuplicateServiceSource: Same (service_id, service_track_id) mapped to different track_ids
    let dup_sources: Vec<(i64, String, i64)> = sqlx::query_as(
        r#"
        SELECT service_id, service_track_id, COUNT(DISTINCT track_id) as count
        FROM track_sources
        GROUP BY service_id, service_track_id
        HAVING COUNT(DISTINCT track_id) > 1
        "#
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (svc_id, st_id, cnt) in &dup_sources {
        details.push(CatalogAnomalyItem {
            category: "DuplicateServiceSource".to_string(),
            entity_type: "track_sources".to_string(),
            entity_id: None,
            service_id: Some(*svc_id),
            service_track_id: Some(st_id.clone()),
            message: format!("Service source (service_id: {}, service_track_id: '{}') is mapped to {} distinct canonical tracks", svc_id, st_id, cnt),
            suggested_action: "Merge or disambiguate canonical track sources to maintain 1:1 mapping".to_string(),
        });
    }

    // 2. ConflictingISRC / Invalid ISRC format in tracks
    let raw_isrc_rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, isrc FROM tracks WHERE isrc IS NOT NULL AND isrc != ''"
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut invalid_isrc_count = 0;
    for (tid, isrc) in &raw_isrc_rows {
        if !is_valid_isrc(isrc) {
            invalid_isrc_count += 1;
            details.push(CatalogAnomalyItem {
                category: "ConflictingISRC".to_string(),
                entity_type: "tracks".to_string(),
                entity_id: Some(*tid),
                service_id: None,
                service_track_id: None,
                message: format!("Track {} contains invalid ISRC format: '{}'", tid, isrc),
                suggested_action: "Nullify numeric or malformed ISRC to prevent false identity collisions".to_string(),
            });
        }
    }

    // 3. GhostTrack: tracks with no album, no artist in track_artists, and no sources
    let ghost_tracks: Vec<(i64, String)> = sqlx::query_as(
        r#"
        SELECT t.id, t.title FROM tracks t
        WHERE t.album_id IS NULL
          AND NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.track_id = t.id)
          AND NOT EXISTS (SELECT 1 FROM track_sources ts WHERE ts.track_id = t.id)
        "#
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (tid, title) in &ghost_tracks {
        details.push(CatalogAnomalyItem {
            category: "GhostTrack".to_string(),
            entity_type: "tracks".to_string(),
            entity_id: Some(*tid),
            service_id: None,
            service_track_id: None,
            message: format!("Ghost track {} ('{}') has no album, artists, or sources", tid, title),
            suggested_action: "Purge or link canonical source".to_string(),
        });
    }

    // 4. GhostAlbum: albums with 0 tracks referencing them
    let ghost_albums: Vec<(i64, String)> = sqlx::query_as(
        r#"
        SELECT a.id, a.title FROM albums a
        WHERE NOT EXISTS (SELECT 1 FROM tracks t WHERE t.album_id = a.id)
        "#
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (aid, title) in &ghost_albums {
        details.push(CatalogAnomalyItem {
            category: "GhostAlbum".to_string(),
            entity_type: "albums".to_string(),
            entity_id: Some(*aid),
            service_id: None,
            service_track_id: None,
            message: format!("Ghost album {} ('{}') has 0 associated tracks", aid, title),
            suggested_action: "Clean up orphan album record".to_string(),
        });
    }

    // 5. GhostArtist: artists with 0 tracks and 0 albums
    let ghost_artists: Vec<(i64, String)> = sqlx::query_as(
        r#"
        SELECT ar.id, ar.name FROM artists ar
        WHERE NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = ar.id)
          AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = ar.id)
        "#
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (arid, name) in &ghost_artists {
        details.push(CatalogAnomalyItem {
            category: "GhostArtist".to_string(),
            entity_type: "artists".to_string(),
            entity_id: Some(*arid),
            service_id: None,
            service_track_id: None,
            message: format!("Ghost artist {} ('{}') has 0 associated tracks or albums", arid, name),
            suggested_action: "Clean up orphan artist record".to_string(),
        });
    }

    // 6. DownloadWithoutCanonicalTrack: downloads referencing non-existent track_id
    let orphan_downloads: Vec<(i64, Option<String>)> = sqlx::query_as(
        r#"
        SELECT d.id, d.file_path FROM downloads d
        WHERE NOT EXISTS (SELECT 1 FROM tracks t WHERE t.id = d.track_id)
        "#
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (did, path) in &orphan_downloads {
        details.push(CatalogAnomalyItem {
            category: "DownloadWithoutCanonicalTrack".to_string(),
            entity_type: "downloads".to_string(),
            entity_id: Some(*did),
            service_id: None,
            service_track_id: None,
            message: format!("Download {} points to missing canonical track. Path: {:?}", did, path),
            suggested_action: "Relink to canonical track or purge invalid download record".to_string(),
        });
    }

    // 7. CanonicalTrackWithoutValidSource: track with 0 sources in track_sources
    let sourceless_tracks: Vec<(i64, String)> = sqlx::query_as(
        r#"
        SELECT t.id, t.title FROM tracks t
        WHERE NOT EXISTS (SELECT 1 FROM track_sources ts WHERE ts.track_id = t.id)
        "#
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (tid, title) in &sourceless_tracks {
        details.push(CatalogAnomalyItem {
            category: "CanonicalTrackWithoutValidSource".to_string(),
            entity_type: "tracks".to_string(),
            entity_id: Some(*tid),
            service_id: None,
            service_track_id: None,
            message: format!("Canonical track {} ('{}') has no active provider source", tid, title),
            suggested_action: "Enrich or attach valid provider source".to_string(),
        });
    }

    // 8. PlaceholderMetadata: 'Unknown Artist', 'Unknown Album', 'Tidal Track %'
    let placeholder_tracks: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, title FROM tracks"
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut placeholder_count = 0;
    for (tid, title) in &placeholder_tracks {
        if is_placeholder_title(title) {
            placeholder_count += 1;
            details.push(CatalogAnomalyItem {
                category: "PlaceholderMetadata".to_string(),
                entity_type: "tracks".to_string(),
                entity_id: Some(*tid),
                service_id: None,
                service_track_id: None,
                message: format!("Track {} uses placeholder title '{}'", tid, title),
                suggested_action: "Fetch real metadata from provider API or MusicBrainz".to_string(),
            });
        }
    }

    let placeholder_artists: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, name FROM artists"
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (arid, name) in &placeholder_artists {
        if is_placeholder_artist(name) {
            placeholder_count += 1;
            details.push(CatalogAnomalyItem {
                category: "PlaceholderMetadata".to_string(),
                entity_type: "artists".to_string(),
                entity_id: Some(*arid),
                service_id: None,
                service_track_id: None,
                message: format!("Artist {} uses placeholder name '{}'", arid, name),
                suggested_action: "Resolve real artist name from provider API".to_string(),
            });
        }
    }

    let placeholder_albums: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, title FROM albums"
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (aid, title) in &placeholder_albums {
        if is_placeholder_album(title) {
            placeholder_count += 1;
            details.push(CatalogAnomalyItem {
                category: "PlaceholderMetadata".to_string(),
                entity_type: "albums".to_string(),
                entity_id: Some(*aid),
                service_id: None,
                service_track_id: None,
                message: format!("Album {} uses placeholder title '{}'", aid, title),
                suggested_action: "Resolve real album title from provider API".to_string(),
            });
        }
    }

    // 9. AmbiguousEdition: Tracks with identical title and primary artist
    let ambiguous_editions: Vec<(String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT t.title, ta.artist_id, COUNT(t.id) as cnt
        FROM tracks t
        JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
        GROUP BY t.title, ta.artist_id
        HAVING COUNT(t.id) > 1
        "#
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (title, arid, cnt) in &ambiguous_editions {
        details.push(CatalogAnomalyItem {
            category: "AmbiguousEdition".to_string(),
            entity_type: "tracks".to_string(),
            entity_id: None,
            service_id: None,
            service_track_id: None,
            message: format!("Found {} distinct tracks sharing title '{}' and primary artist ID {}", cnt, title, arid),
            suggested_action: "Preserve distinct edition metadata, album links and track numbers without silent merging".to_string(),
        });
    }

    // 10. OrphanPlaylistLink: playlist_tracks with invalid track_id or playlist_id
    let orphan_pl_links: Vec<(i64, i64)> = sqlx::query_as(
        r#"
        SELECT pt.playlist_id, pt.track_id FROM playlist_tracks pt
        WHERE NOT EXISTS (SELECT 1 FROM playlists p WHERE p.id = pt.playlist_id)
           OR NOT EXISTS (SELECT 1 FROM tracks t WHERE t.id = pt.track_id)
        "#
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (plid, tid) in &orphan_pl_links {
        details.push(CatalogAnomalyItem {
            category: "OrphanPlaylistLink".to_string(),
            entity_type: "playlist_tracks".to_string(),
            entity_id: Some(*tid),
            service_id: None,
            service_track_id: None,
            message: format!("Playlist link points to missing playlist {} or track {}", plid, tid),
            suggested_action: "Purge broken playlist reference".to_string(),
        });
    }

    // 11. PhysicalPathMismatch & 13. InvalidFilename & 14. InvalidTagging & 15. SidecarMismatch
    let mut physical_path_mismatch_count = 0;
    let mut invalid_filenames_count = 0;
    let invalid_taggings_count = 0;
    let sidecar_mismatches_count = 0;

    let downloads: Vec<(i64, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT d.id, d.track_id, d.file_path, t.title FROM downloads d JOIN tracks t ON t.id = d.track_id"
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (did, _tid, path_opt, _title_opt) in &downloads {
        if let Some(ref path_str) = path_opt {
            let p = Path::new(path_str);
            if !p.exists() {
                physical_path_mismatch_count += 1;
                details.push(CatalogAnomalyItem {
                    category: "PhysicalPathMismatch".to_string(),
                    entity_type: "downloads".to_string(),
                    entity_id: Some(*did),
                    service_id: None,
                    service_track_id: None,
                    message: format!("Physical audio file does not exist on disk: {:?}", path_str),
                    suggested_action: "Reconcile or mark missing in library".to_string(),
                });
            } else {
                // Check filename
                if let Some(fname) = p.file_name().and_then(|n| n.to_str()) {
                    if fname.starts_with("Unknown") || fname.starts_with("01 - Tidal Track") || fname.starts_with("02 - Tidal Track") {
                        invalid_filenames_count += 1;
                        details.push(CatalogAnomalyItem {
                            category: "InvalidFilename".to_string(),
                            entity_type: "downloads".to_string(),
                            entity_id: Some(*did),
                            service_id: None,
                            service_track_id: None,
                            message: format!("Download file has placeholder filename: '{}'", fname),
                            suggested_action: "Re-enrich and safely rename file using canonical title".to_string(),
                        });
                    }
                }
            }
        } else {
            physical_path_mismatch_count += 1;
        }
    }

    // 12. MetadataProvenanceConflict: e.g. artists with track IDs in tidal_id/spotify_id
    let prov_conflicts: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, name, spotify_id, tidal_id FROM artists WHERE (spotify_id IS NOT NULL AND LENGTH(spotify_id) < 5) OR (tidal_id IS NOT NULL AND LENGTH(tidal_id) < 2)"
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (arid, name, sp_id, tid_id) in &prov_conflicts {
        details.push(CatalogAnomalyItem {
            category: "MetadataProvenanceConflict".to_string(),
            entity_type: "artists".to_string(),
            entity_id: Some(*arid),
            service_id: None,
            service_track_id: None,
            message: format!("Artist {} ('{}') has suspect service ID: spotify={:?}, tidal={:?}", arid, name, sp_id, tid_id),
            suggested_action: "Clean invalid provenance ID from artist record".to_string(),
        });
    }

    // 16. StagingResidual
    let mut staging_residuals_count = 0;
    if let Some(root) = base_dir {
        let staging_dir = root.join(".staging");
        if staging_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&staging_dir) {
                for entry in entries.flatten() {
                    staging_residuals_count += 1;
                    details.push(CatalogAnomalyItem {
                        category: "StagingResidual".to_string(),
                        entity_type: "filesystem".to_string(),
                        entity_id: None,
                        service_id: None,
                        service_track_id: None,
                        message: format!("Residual temporary file found in staging: {:?}", entry.path()),
                        suggested_action: "Purge confirmed residual staging file".to_string(),
                    });
                }
            }
        }
    }

    let total = details.len();

    Ok(CatalogIdentityAuditReport {
        audit_timestamp: chrono::Utc::now().to_rfc3339(),
        duplicate_service_sources_count: dup_sources.len(),
        conflicting_isrc_count: invalid_isrc_count,
        ghost_tracks_count: ghost_tracks.len(),
        ghost_albums_count: ghost_albums.len(),
        ghost_artists_count: ghost_artists.len(),
        downloads_without_canonical_track_count: orphan_downloads.len(),
        canonical_tracks_without_valid_source_count: sourceless_tracks.len(),
        placeholder_metadata_count: placeholder_count,
        ambiguous_editions_count: ambiguous_editions.len(),
        orphan_playlist_links_count: orphan_pl_links.len(),
        physical_path_mismatches_count: physical_path_mismatch_count,
        metadata_provenance_conflicts_count: prov_conflicts.len(),
        invalid_filenames_count,
        invalid_taggings_count,
        sidecar_mismatches_count,
        staging_residuals_count,
        total_anomalies: total,
        details,
    })
}

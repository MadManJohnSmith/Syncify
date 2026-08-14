//! Domain contract & Metadata Enrichment Engine for `src-tauri`.
//!
//! Integrates `syncify-metadata-domain` precedence engine with `MusicBrainzClient`
//! to resolve the first group of enriched metadata fields safely and deterministically.

use crate::services::musicbrainz::{MusicBrainzClient, MusicBrainzRecording};
pub use syncify_metadata_domain::{
    chrono_now_iso, normalize_title, ConflictInfo, EnrichedMetadata, FieldResolution,
    FieldValidator, SourcePriority,
};

/// Origin streaming track metadata passed into the enrichment engine
#[derive(Debug, Clone, Default)]
pub struct OriginTrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub release_year: Option<String>,
    pub original_date: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
    pub isrc: Option<String>,
    pub barcode: Option<String>,
    pub source_name: String,
}

/// Metadata Enrichment Engine for `src-tauri`
pub struct EnrichmentEngine {
    musicbrainz: MusicBrainzClient,
}

impl EnrichmentEngine {
    pub fn new() -> Self {
        Self {
            musicbrainz: MusicBrainzClient::new(),
        }
    }

    /// Resolve enriched metadata for a track following strict precedence rules.
    pub async fn resolve_track_metadata(
        &self,
        artist: &str,
        album: &str,
        title: &str,
        isrc_hint: Option<&str>,
        origin_meta: Option<&OriginTrackMetadata>,
    ) -> EnrichedMetadata {
        let mut meta = EnrichedMetadata::default();
        let now_ts = chrono_now_iso();
        meta.enriched_at = now_ts.clone();

        // 1. Populate basic structure from arguments / origin
        let src_name = origin_meta.map(|o| o.source_name.as_str()).unwrap_or("stream");

        if FieldValidator::is_valid_title(title) {
            meta.title.merge_candidate(Some(title.to_string()), src_name, 1.0, &now_ts);
        }
        if FieldValidator::is_valid_artist(artist) {
            meta.artist.merge_candidate(Some(artist.to_string()), src_name, 1.0, &now_ts);
        }
        if !album.trim().is_empty() {
            meta.album.merge_candidate(Some(album.to_string()), src_name, 1.0, &now_ts);
        }

        if let Some(orig) = origin_meta {
            if let Some(ref aa) = orig.album_artist {
                if FieldValidator::is_valid_artist(aa) {
                    meta.album_artist.merge_candidate(Some(aa.clone()), &orig.source_name, 0.95, &now_ts);
                }
            }
            if let Some(tn) = orig.track_number {
                if tn > 0 {
                    meta.track_number.merge_candidate(Some(tn.to_string()), &orig.source_name, 1.0, &now_ts);
                }
            }
            if let Some(tt) = orig.track_total {
                if tt > 0 {
                    meta.track_total.merge_candidate(Some(tt.to_string()), &orig.source_name, 0.95, &now_ts);
                }
            }
            if let Some(dn) = orig.disc_number {
                if dn > 0 {
                    meta.disc_number.merge_candidate(Some(dn.to_string()), &orig.source_name, 1.0, &now_ts);
                }
            }
            if let Some(dt) = orig.disc_total {
                if dt > 0 {
                    meta.disc_total.merge_candidate(Some(dt.to_string()), &orig.source_name, 0.95, &now_ts);
                }
            }
            if let Some(ref yr) = orig.release_year {
                if FieldValidator::is_valid_year(yr) {
                    meta.release_year.merge_candidate(Some(yr.clone()), &orig.source_name, 0.90, &now_ts);
                }
            }
            if let Some(ref od) = orig.original_date {
                if FieldValidator::is_valid_year(od) {
                    meta.original_date.merge_candidate(Some(od.clone()), &orig.source_name, 0.90, &now_ts);
                }
            }
            if let Some(ref lbl) = orig.label {
                if FieldValidator::is_valid_label(lbl) {
                    meta.label.merge_candidate(Some(lbl.clone()), &orig.source_name, 0.85, &now_ts);
                }
            }
            if let Some(ref cat) = orig.catalog_number {
                if !cat.trim().is_empty() {
                    meta.catalog_number.merge_candidate(Some(cat.clone()), &orig.source_name, 0.85, &now_ts);
                }
            }
            if let Some(ref isrc_val) = orig.isrc {
                if FieldValidator::is_valid_identifier(isrc_val) {
                    meta.isrc.merge_candidate(Some(isrc_val.clone()), &orig.source_name, 0.95, &now_ts);
                }
            }
            if let Some(ref bar) = orig.barcode {
                if FieldValidator::is_valid_identifier(bar) {
                    meta.barcode.merge_candidate(Some(bar.clone()), &orig.source_name, 0.95, &now_ts);
                }
            }
        }

        // Apply ISRC hint if provided
        if let Some(isrc) = isrc_hint {
            if FieldValidator::is_valid_identifier(isrc) {
                meta.isrc.merge_candidate(Some(isrc.to_string()), src_name, 0.95, &now_ts);
            }
        }

        // 2. Query MusicBrainz
        let mb_recording = self.query_musicbrainz(artist, album, title, meta.isrc.value()).await;

        if let Some(ref rec) = mb_recording {
            meta.musicbrainz_recording_id.merge_candidate(
                Some(rec.id.clone()),
                "musicbrainz",
                0.95,
                &now_ts,
            );

            // Artist credit
            if let Some(ref acs) = rec.artist_credit {
                if let Some(first_ac) = acs.first() {
                    meta.musicbrainz_artist_id.merge_candidate(
                        Some(first_ac.artist.id.clone()),
                        "musicbrainz",
                        0.95,
                        &now_ts,
                    );
                }
            }

            // Select best release
            if let Some(ref releases) = rec.releases {
                let norm_album = normalize_title(album);
                let is_album_match = |r_title: &str| {
                    let t = normalize_title(r_title);
                    t == norm_album || t.starts_with(&norm_album) || norm_album.starts_with(&t)
                };

                let selected_release = releases
                    .iter()
                    .find(|r| is_album_match(&r.title))
                    .or_else(|| releases.first());

                if let Some(rel) = selected_release {
                    meta.musicbrainz_release_id.merge_candidate(
                        Some(rel.id.clone()),
                        "musicbrainz",
                        0.95,
                        &now_ts,
                    );

                    if let Some(ref rg) = rel.release_group {
                        meta.musicbrainz_release_group_id.merge_candidate(
                            Some(rg.id.clone()),
                            "musicbrainz",
                            0.95,
                            &now_ts,
                        );
                    }

                    if let Some(ref date_str) = rel.date {
                        if FieldValidator::is_valid_year(date_str) {
                            meta.original_date.merge_candidate(
                                Some(date_str.clone()),
                                "musicbrainz",
                                0.85,
                                &now_ts,
                            );
                        }
                    }

                    if let Some(ref labels) = rel.label_info {
                        if let Some(first_lbl) = labels.first() {
                            if let Some(ref l_obj) = first_lbl.label {
                                if FieldValidator::is_valid_label(&l_obj.name) {
                                    meta.label.merge_candidate(
                                        Some(l_obj.name.clone()),
                                        "musicbrainz",
                                        0.85,
                                        &now_ts,
                                    );
                                }
                            }
                            if let Some(ref cat_str) = first_lbl.catalog_number {
                                if !cat_str.trim().is_empty() {
                                    meta.catalog_number.merge_candidate(
                                        Some(cat_str.clone()),
                                        "musicbrainz",
                                        0.85,
                                        &now_ts,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        } else {
            meta.musicbrainz_recording_id = FieldResolution::NotFound {
                source: "musicbrainz".to_string(),
                checked_at: now_ts.clone(),
            };
        }

        meta
    }

    async fn query_musicbrainz(
        &self,
        artist: &str,
        album: &str,
        title: &str,
        isrc_opt: Option<&str>,
    ) -> Option<MusicBrainzRecording> {
        // 1. Try ISRC lookup first
        if let Some(isrc) = isrc_opt {
            if let Ok(Some(rec)) = self.musicbrainz.lookup_by_isrc(isrc).await {
                return Some(rec);
            }
        }

        // 2. Try text search
        if let Ok(recordings) = self.musicbrainz.search_recordings(title, artist, Some(album), 5).await {
            let recs_vec: Vec<_> = recordings.into_iter().collect();
            let norm_album = normalize_title(album);

            return recs_vec
                .iter()
                .cloned()
                .find(|r| {
                    if let Some(ref rels) = r.releases {
                        rels.iter().any(|rel| {
                            let t = normalize_title(&rel.title);
                            t == norm_album || t.starts_with(&norm_album) || norm_album.starts_with(&t)
                        })
                    } else {
                        false
                    }
                })
                .or_else(|| recs_vec.into_iter().next());
        }

        None
    }

    /// Safely persist resolved metadata to SQLite adhering to all relational safety invariants.
    pub async fn apply_to_database(
        &self,
        db: &sqlx::SqlitePool,
        track_id: i64,
        meta: &EnrichedMetadata,
        audio_file_path: Option<&std::path::Path>,
    ) -> Result<(), String> {
        // 1. If audio file is provided, apply and verify FLAC tags first
        if let Some(flac_path) = audio_file_path {
            let flac_meta = crate::services::tag_writer::FlacMetadata {
                title: meta.title.value().unwrap_or("").to_string(),
                artist: meta.artist.value().unwrap_or("").to_string(),
                album: meta.album.value().unwrap_or("").to_string(),
                album_artist: meta.album_artist.value().map(|s| s.to_string()),
                performers: meta.artist.value().map(|s| s.to_string()),
                label: meta.label.value().map(|s| s.to_string()),
                barcode: meta.barcode.value().map(|s| s.to_string()),
                catalog_number: meta.catalog_number.value().map(|s| s.to_string()),
                original_date: meta.original_date.value().map(|s| s.to_string()),
                track_number: meta.track_number.value().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0),
                track_total: meta.track_total.value().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0),
                disc_number: meta.disc_number.value().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1),
                disc_total: meta.disc_total.value().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1),
                isrc: meta.isrc.value().map(|s| s.to_string()),
                release_year: meta.release_year.value().map(|s| s.to_string()),
                musicbrainz_track_id: meta.musicbrainz_recording_id.value().map(|s| s.to_string()),
                musicbrainz_artist_id: meta.musicbrainz_artist_id.value().map(|s| s.to_string()),
                musicbrainz_album_id: meta.musicbrainz_release_id.value().map(|s| s.to_string()),
                musicbrainz_albumartist_id: meta.musicbrainz_artist_id.value().map(|s| s.to_string()),
                musicbrainz_release_group_id: meta.musicbrainz_release_group_id.value().map(|s| s.to_string()),
                ..Default::default()
            };

            crate::services::tag_writer::apply_flac_tags(flac_path, &flac_meta)
                .map_err(|e| format!("Failed to apply FLAC tags: {}", e))?;

            crate::services::tag_writer::verify_flac_tags(flac_path, &flac_meta)
                .map_err(|e| format!("FLAC re-read verification failed: {}", e))?;
        }

        // 2. Start SQLite transaction
        let mut tx = db.begin().await.map_err(|e| format!("DB transaction failed: {}", e))?;

        // 3. Update track record (only non-empty resolved values)
        if let Some(t) = meta.title.value() {
            let _ = sqlx::query("UPDATE tracks SET title = ? WHERE id = ?")
                .bind(t).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(tn) = meta.track_number.value().and_then(|s| s.parse::<i32>().ok()) {
            let _ = sqlx::query("UPDATE tracks SET track_number = ? WHERE id = ?")
                .bind(tn).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(dn) = meta.disc_number.value().and_then(|s| s.parse::<i32>().ok()) {
            let _ = sqlx::query("UPDATE tracks SET disc_number = ? WHERE id = ?")
                .bind(dn).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(i) = meta.isrc.value() {
            let _ = sqlx::query("UPDATE tracks SET isrc = ? WHERE id = ?")
                .bind(i).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(y) = meta.release_year.value().and_then(|s| s.chars().take(4).collect::<String>().parse::<i32>().ok()) {
            let _ = sqlx::query("UPDATE tracks SET release_year = ? WHERE id = ?")
                .bind(y).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(l) = meta.label.value() {
            let _ = sqlx::query("UPDATE tracks SET record_label = ? WHERE id = ?")
                .bind(l).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(mb_track) = meta.musicbrainz_recording_id.value() {
            let _ = sqlx::query("UPDATE tracks SET musicbrainz_id = ? WHERE id = ?")
                .bind(mb_track).bind(track_id).execute(&mut *tx).await;
        }

        // Update global job status and timestamp
        let _ = sqlx::query("UPDATE tracks SET enrichment_status = 'complete', enriched_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(track_id).execute(&mut *tx).await;

        // 4. Resolve / link Artist safely (NEVER rename artists.name)
        if let Some(art_name) = meta.artist.value() {
            let artist_row: Option<(i64, Option<String>)> = sqlx::query_as(
                "SELECT id, musicbrainz_id FROM artists WHERE name = ? COLLATE NOCASE LIMIT 1"
            )
            .bind(art_name)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten();

            let artist_id = if let Some((aid, mb_id)) = artist_row {
                if mb_id.is_none() {
                    if let Some(mb_art) = meta.musicbrainz_artist_id.value() {
                        let _ = sqlx::query("UPDATE artists SET musicbrainz_id = ? WHERE id = ?")
                            .bind(mb_art).bind(aid).execute(&mut *tx).await;
                    }
                }
                aid
            } else {
                let mb_art = meta.musicbrainz_artist_id.value();
                let res = sqlx::query("INSERT INTO artists (name, musicbrainz_id) VALUES (?, ?)")
                    .bind(art_name)
                    .bind(mb_art)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to insert artist: {}", e))?;
                res.last_insert_rowid()
            };

            // Link track_artists safely
            let _ = sqlx::query("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
                .bind(track_id).bind(artist_id).execute(&mut *tx).await;
        }

        // 5. Update Album if track has album_id
        let album_row: Option<(Option<i64>,)> = sqlx::query_as("SELECT album_id FROM tracks WHERE id = ?")
            .bind(track_id)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten();

        if let Some((Some(album_id),)) = album_row {
            if let Some(alb_title) = meta.album.value() {
                let _ = sqlx::query("UPDATE albums SET title = ? WHERE id = ?")
                    .bind(alb_title).bind(album_id).execute(&mut *tx).await;
            }
            if let Some(rel_date) = meta.original_date.value() {
                let _ = sqlx::query("UPDATE albums SET release_date = ? WHERE id = ?")
                    .bind(rel_date).bind(album_id).execute(&mut *tx).await;
            }
            if let Some(upc) = meta.barcode.value() {
                let _ = sqlx::query("UPDATE albums SET upc = ? WHERE id = ?")
                    .bind(upc).bind(album_id).execute(&mut *tx).await;
            }
            if let Some(tt) = meta.track_total.value().and_then(|s| s.parse::<i32>().ok()) {
                let _ = sqlx::query("UPDATE albums SET total_tracks = ? WHERE id = ?")
                    .bind(tt).bind(album_id).execute(&mut *tx).await;
            }
            if let Some(lbl) = meta.label.value() {
                let _ = sqlx::query("UPDATE albums SET label = ? WHERE id = ?")
                    .bind(lbl).bind(album_id).execute(&mut *tx).await;
            }
            if let Some(mb_rel) = meta.musicbrainz_release_id.value() {
                let _ = sqlx::query("UPDATE albums SET musicbrainz_id = ? WHERE id = ?")
                    .bind(mb_rel).bind(album_id).execute(&mut *tx).await;
            }
        }

        tx.commit().await.map_err(|e| format!("Failed to commit DB transaction: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syncify_metadata_domain::fixtures::*;

    #[test]
    fn test_enrichment_precedence_streaming_over_musicbrainz() {
        let mut meta = EnrichedMetadata::default();
        let now = chrono_now_iso();

        // MusicBrainz candidate first
        meta.title.merge_candidate(Some("MB Title".to_string()), "musicbrainz", 0.95, &now);
        // Streaming official title
        meta.title.merge_candidate(Some("Stream Official Title".to_string()), "qobuz", 0.90, &now);

        assert_eq!(meta.title.value(), Some("Stream Official Title"));
        assert_eq!(meta.title.source(), Some("qobuz"));
    }

    #[test]
    fn test_manual_override_preservation() {
        let mut meta = EnrichedMetadata::default();
        let now = chrono_now_iso();

        meta.label.merge_candidate(Some("User Label".to_string()), "manual", 1.0, &now);
        meta.label.merge_candidate(Some("MB Label".to_string()), "musicbrainz", 0.95, &now);

        assert_eq!(meta.label.value(), Some("User Label"));
        assert_eq!(meta.label.source(), Some("manual"));
    }

    #[test]
    fn test_field_validator_rejection_rules() {
        assert!(!FieldValidator::is_valid_year("0000"));
        assert!(!FieldValidator::is_valid_year("0"));
        assert!(FieldValidator::is_valid_year("1978"));

        assert!(!FieldValidator::is_valid_identifier(""));
        assert!(!FieldValidator::is_valid_identifier("0"));
        assert!(!FieldValidator::is_valid_identifier("null"));
        assert!(FieldValidator::is_valid_identifier("USRC12345678"));

        assert!(FieldValidator::is_valid_artist("Various Artists"));
        assert!(FieldValidator::is_valid_artist("Various"));
    }

    #[test]
    fn test_musicbrainz_exact_match_offline() {
        let json_val: serde_json::Value = serde_json::from_str(FIXTURE_MB_EXACT_RECORDING_JSON).unwrap();
        assert_eq!(json_val["id"].as_str(), Some("b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d"));
    }

    #[test]
    fn test_musicbrainz_alternative_release_offline() {
        let json_val: serde_json::Value = serde_json::from_str(FIXTURE_MB_ALTERNATIVE_RELEASE_JSON).unwrap();
        let releases = json_val["releases"].as_array().unwrap();
        let norm_album = normalize_title("Heroes");
        let matched = releases.iter().find(|r| normalize_title(r["title"].as_str().unwrap_or("")) == norm_album);
        assert!(matched.is_some());
    }

    #[tokio::test]
    async fn test_sqlite_rejection_on_nonexistent_flac_file() {
        let engine = EnrichmentEngine::new();
        let meta = EnrichedMetadata::default();
        let fake_path = std::path::Path::new("C:/nonexistent/path/track.flac");

        // Create temporary in-memory pool
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

        let result = engine.apply_to_database(&pool, 1, &meta, Some(fake_path)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_artist_global_name_safety() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        
        // Setup minimal schema
        sqlx::query("CREATE TABLE artists (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, musicbrainz_id TEXT);")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE tracks (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, album_id INTEGER, track_number INTEGER, disc_number INTEGER, isrc TEXT, release_year INTEGER, record_label TEXT, musicbrainz_id TEXT, enrichment_status TEXT, enriched_at TEXT);")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE track_artists (track_id INTEGER, artist_id INTEGER, role TEXT, PRIMARY KEY(track_id, artist_id));")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE albums (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, release_date TEXT, upc TEXT, total_tracks INTEGER, label TEXT, musicbrainz_id TEXT);")
            .execute(&pool).await.unwrap();

        // Insert canonical artist
        sqlx::query("INSERT INTO artists (name) VALUES ('Queen');").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO tracks (title, enrichment_status) VALUES ('Bohemian Rhapsody', 'pending');").execute(&pool).await.unwrap();

        let engine = EnrichmentEngine::new();
        let mut meta = EnrichedMetadata::default();
        let now = chrono_now_iso();
        // Enriched artist candidate with lowercase or alternate spelling
        meta.artist.merge_candidate(Some("queen".to_string()), "stream", 1.0, &now);
        meta.musicbrainz_artist_id.merge_candidate(Some("0383dadf-2a4e-4d10-a46a-e6e041dae229".to_string()), "musicbrainz", 0.95, &now);

        let res = engine.apply_to_database(&pool, 1, &meta, None).await;
        assert!(res.is_ok());

        // Assert canonical artist name was NOT modified / corrupted
        let (canonical_name, mbid): (String, Option<String>) = sqlx::query_as("SELECT name, musicbrainz_id FROM artists WHERE id = 1")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(canonical_name, "Queen"); // Preserved!
        assert_eq!(mbid.as_deref(), Some("0383dadf-2a4e-4d10-a46a-e6e041dae229")); // Populated safely
    }
}

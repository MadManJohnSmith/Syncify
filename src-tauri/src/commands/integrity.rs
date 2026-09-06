// Integrity Commands - included via include!() in mod.rs
// Audits library physical files, SQLite consistency, orphan/corrupt files, and abandoned staging

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityAuditReport {
    pub total_tracks_scanned: i64,
    pub verified_files: i64,
    pub missing_files: Vec<String>,
    pub orphan_files: Vec<String>,
    pub corrupt_or_zero_byte_files: Vec<String>,
    pub abandoned_staging_files: Vec<String>,
    pub database_inconsistencies: Vec<String>,
    pub is_healthy: bool,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityRepairResult {
    pub purged_staging_files: i64,
    pub cleaned_database_entries: i64,
    pub message: String,
}

/// Run a comprehensive integrity audit across physical files, metadata, and SQLite database
#[tauri::command]
pub async fn run_integrity_audit(
    state: State<'_, AppState>,
    download_dir: Option<String>,
) -> Result<IntegrityAuditReport, String> {
    let mut report = IntegrityAuditReport {
        total_tracks_scanned: 0,
        verified_files: 0,
        missing_files: Vec::new(),
        orphan_files: Vec::new(),
        corrupt_or_zero_byte_files: Vec::new(),
        abandoned_staging_files: Vec::new(),
        database_inconsistencies: Vec::new(),
        is_healthy: true,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    // 1. Audit downloaded files in SQLite against physical disk
    let downloads: Vec<(i64, Option<i64>, String, Option<String>)> = sqlx::query_as(
        "SELECT id, track_id, file_path, file_format FROM downloads"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to query downloads: {}", e))?;

    report.total_tracks_scanned = downloads.len() as i64;
    let mut known_file_paths: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (_id, _track_id, file_path, _format) in downloads {
        let p = std::path::Path::new(&file_path);
        known_file_paths.insert(file_path.clone());

        if !p.exists() {
            report.missing_files.push(format!("File registered in DB does not exist on disk: {}", file_path));
            report.is_healthy = false;
            continue;
        }

        // Check file size & magic header
        match std::fs::metadata(p) {
            Ok(meta) => {
                if meta.len() == 0 {
                    report.corrupt_or_zero_byte_files.push(format!("Zero-byte file detected: {}", file_path));
                    report.is_healthy = false;
                } else if meta.len() >= 4 {
                    // Check magic bytes
                    if let Ok(bytes) = std::fs::read(p) {
                        let is_flac = bytes.starts_with(b"fLaC");
                        let is_m4a = bytes.len() >= 8 && (&bytes[4..8] == b"ftyp" || &bytes[0..4] == b"ftyp");
                        let is_mp3 = bytes.starts_with(b"ID3") || (bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0);

                        if !is_flac && !is_m4a && !is_mp3 {
                            report.corrupt_or_zero_byte_files.push(format!("Invalid audio container magic header: {}", file_path));
                            report.is_healthy = false;
                        } else {
                            report.verified_files += 1;
                        }
                    } else {
                        report.corrupt_or_zero_byte_files.push(format!("Unreadable file: {}", file_path));
                        report.is_healthy = false;
                    }
                }
            }
            Err(e) => {
                report.missing_files.push(format!("Could not read metadata for {}: {}", file_path, e));
                report.is_healthy = false;
            }
        }
    }

    // 2. Check for abandoned staging (.part, .partial) files in staging / output directory
    let search_dir = download_dir
        .or_else(|| dirs::audio_dir().map(|p| p.to_string_lossy().to_string()))
        .or_else(|| std::env::current_dir().ok().map(|p| p.to_string_lossy().to_string()))
        .unwrap_or_else(|| ".".to_string());

    let search_path = std::path::Path::new(&search_dir);
    if search_path.exists() {
        for entry in walkdir::WalkDir::new(search_path).max_depth(3).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                if ext.eq_ignore_ascii_case("part") || ext.eq_ignore_ascii_case("partial") {
                    report.abandoned_staging_files.push(path.to_string_lossy().to_string());
                    report.is_healthy = false;
                }
            }
        }
    }

    // 3. Database referential consistency checks
    // 3a. Stuck downloading tasks in queue
    let stuck_queue: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, track_id FROM download_queue WHERE status = 'downloading'"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    for (qid, tid) in stuck_queue {
        report.database_inconsistencies.push(format!("Queue item #{} (track #{}) stuck in 'downloading' status", qid, tid));
    }

    // 3b. Tracks referencing non-existent albums
    let orphan_tracks: Vec<(i64, String)> = sqlx::query_as(
        "SELECT t.id, t.title FROM tracks t LEFT JOIN albums a ON a.id = t.album_id WHERE t.album_id IS NOT NULL AND a.id IS NULL"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    for (tid, title) in orphan_tracks {
        report.database_inconsistencies.push(format!("Track #{} ('{}') references non-existent album", tid, title));
        report.is_healthy = false;
    }

    Ok(report)
}

/// Repair detected integrity issues (reset stuck queue items, purge abandoned staging files)
pub async fn perform_repair_integrity_issues(
    db: &crate::DbPool,
    staging_files_to_purge: Option<Vec<String>>,
) -> Result<IntegrityRepairResult, String> {
    let mut purged = 0i64;
    let mut cleaned_db = 0i64;

    // Reset stuck queue items to queued
    let res = sqlx::query(
        "UPDATE download_queue SET status = 'queued' WHERE status = 'downloading'"
    )
    .execute(db)
    .await;

    if let Ok(r) = res {
        cleaned_db += r.rows_affected() as i64;
    }

    // Clean staging files if requested, strictly confined to the legitimate staging directory
    if let Some(files) = staging_files_to_purge {
        if !files.is_empty() {
            let eff = resolve_effective_download_paths(db)
                .await
                .map_err(|e| format!("Failed to resolve staging path: {}", e))?;
            let staging_dir = std::path::PathBuf::from(&eff.staging_root);
            if !staging_dir.exists() {
                std::fs::create_dir_all(&staging_dir)
                    .map_err(|e| format!("Failed to create staging directory '{}': {}", staging_dir.display(), e))?;
            }
            let canonical_staging = std::fs::canonicalize(&staging_dir)
                .map_err(|e| format!("Failed to canonicalize staging directory '{}': {}", staging_dir.display(), e))?;

            for file in files {
                let trimmed = file.trim();
                if trimmed.is_empty() {
                    tracing::warn!("Path traversal attempt detected: empty path in staging purge list");
                    return Err("Path traversal attempt detected: empty path in staging purge list".to_string());
                }

                let p = std::path::Path::new(trimmed);
                if p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                    tracing::warn!(
                        "Path traversal attempt detected: path '{}' contains parent directory traversal ('..')",
                        file
                    );
                    return Err(format!(
                        "Path traversal attempt detected: path '{}' contains parent directory traversal ('..')",
                        file
                    ));
                }

                if !p.exists() {
                    continue;
                }

                let canonical_file = std::fs::canonicalize(p)
                    .map_err(|e| format!("Failed to canonicalize path '{}': {}", file, e))?;

                if canonical_file == canonical_staging
                    || !canonical_file.starts_with(&canonical_staging)
                    || !canonical_file.is_file()
                {
                    tracing::warn!(
                        "Path traversal attempt detected: file '{}' resolved to '{}' which is outside staging directory '{}'",
                        file,
                        canonical_file.display(),
                        canonical_staging.display()
                    );
                    return Err(format!(
                        "Path traversal attempt detected: file '{}' is outside staging directory '{}'",
                        file,
                        canonical_staging.display()
                    ));
                }

                std::fs::remove_file(&canonical_file)
                    .map_err(|e| format!("Failed to remove staging file '{}': {}", canonical_file.display(), e))?;
                purged += 1;
            }
        }
    }

    Ok(IntegrityRepairResult {
        purged_staging_files: purged,
        cleaned_database_entries: cleaned_db,
        message: format!("Repaired {} database items and purged {} staging files", cleaned_db, purged),
    })
}

/// Repair detected integrity issues (reset stuck queue items, purge abandoned staging files)
#[tauri::command]
pub async fn repair_integrity_issues(
    state: State<'_, AppState>,
    staging_files_to_purge: Option<Vec<String>>,
) -> Result<IntegrityRepairResult, String> {
    perform_repair_integrity_issues(&state.db, staging_files_to_purge).await
}

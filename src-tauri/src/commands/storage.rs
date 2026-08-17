// Storage Commands - included via include!() in mod.rs

#[derive(Debug, Serialize)]
pub struct StorageFormatCount {
    pub format: String,
    pub size_bytes: i64,
}

#[derive(Debug, Serialize)]
pub struct StorageStats {
    pub used_bytes: i64,
    pub total_bytes: i64,
    pub available_bytes: i64,
    pub path: String,
    pub breakdown: Vec<StorageFormatCount>,
}

#[tauri::command]
pub async fn get_storage_stats(state: State<'_, AppState>) -> Result<StorageStats, String> {
    // 1. Get canonical download path via deterministic resolver
    let effective = resolve_effective_download_paths(&state.db).await
        .map_err(|e| format!("Failed to resolve effective download path: {}", e))?;
    let path = PathBuf::from(&effective.library_root);
    let path_str = path.to_string_lossy().to_string();

    // Ensure directory exists
    if !path.exists() {
        let _ = std::fs::create_dir_all(&path);
    }

    // 2. Get disk stats using sysinfo
    let mut total_bytes = 0;
    let mut available_bytes = 0;

    let disks = Disks::new_with_refreshed_list();
    
    // Find matching disk
    let mut best_match: Option<&sysinfo::Disk> = None;
    let mut best_match_len = 0;

    for disk in &disks {
        let mount_point = disk.mount_point();
        if path.starts_with(mount_point) {
            let m_len = mount_point.to_string_lossy().len();
            if m_len > best_match_len {
                best_match = Some(disk);
                best_match_len = m_len;
            }
        }
    }

    if let Some(disk) = best_match {
        total_bytes = disk.total_space() as i64;
        available_bytes = disk.available_space() as i64;
    }

    // 3. Calculate used space and breakdown using walkdir in spawn_blocking
    let scan_path = path.clone();
    let (used_bytes, breakdown) = tokio::task::spawn_blocking(move || {
        let mut total_used: i64 = 0;
        let mut flac_size: i64 = 0;
        let mut mp3_size: i64 = 0;

        for entry in WalkDir::new(&scan_path).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                if let Ok(metadata) = entry.metadata() {
                    let size = metadata.len() as i64;
                    total_used += size;

                    match entry.path().extension().and_then(|s| s.to_str()) {
                        Some("flac") => flac_size += size,
                        Some("mp3") => mp3_size += size,
                        _ => {}
                    }
                }
            }
        }

        let mut bd = Vec::new();
        if flac_size > 0 {
            bd.push(StorageFormatCount { format: "FLAC".to_string(), size_bytes: flac_size });
        }
        if mp3_size > 0 {
            bd.push(StorageFormatCount { format: "MP3".to_string(), size_bytes: mp3_size });
        }
        
        (total_used, bd)
    })
    .await
    .map_err(|e| e.to_string())?;

    Ok(StorageStats {
        used_bytes,
        total_bytes,
        available_bytes,
        path: path_str,
        breakdown,
    })
}

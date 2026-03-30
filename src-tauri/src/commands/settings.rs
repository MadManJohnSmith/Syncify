// Settings Commands - included via include!() in mod.rs
// 
// Service preferences, sync, quality, folder, lyrics settings



// ==============================================
// SPRINT 1: SERVICE PREFERENCES & SYNC SETTINGS
// ==============================================

use crate::models::{ServicePreference, ServiceSyncSettings, SyncSettings, MetadataPreferences};

/// Get all service preferences ordered by priority
#[tauri::command]
pub async fn get_service_preferences(
    state: State<'_, AppState>,
) -> Result<Vec<ServicePreference>, String> {
    tracing::info!("get_service_preferences called");

    sqlx::query_as::<_, ServicePreference>(
        "SELECT id, service_name, priority, auto_import_enabled FROM service_preferences ORDER BY priority ASC"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Update a service preference's auto-import setting
#[tauri::command]
pub async fn update_service_preference(
    state: State<'_, AppState>,
    service_name: String,
    auto_import_enabled: bool,
) -> Result<ServicePreference, String> {
    tracing::info!(
        "update_service_preference: {} -> {}",
        service_name,
        auto_import_enabled
    );

    sqlx::query(
        "UPDATE service_preferences SET auto_import_enabled = ?, updated_at = CURRENT_TIMESTAMP WHERE service_name = ?"
    )
    .bind(auto_import_enabled)
    .bind(&service_name)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    sqlx::query_as::<_, ServicePreference>(
        "SELECT id, service_name, priority, auto_import_enabled FROM service_preferences WHERE service_name = ?"
    )
    .bind(&service_name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Fetch error: {}", e))
}

/// Reorder service priorities based on the provided order
#[tauri::command]
pub async fn reorder_service_priorities(
    state: State<'_, AppState>,
    service_names: Vec<String>,
) -> Result<Vec<ServicePreference>, String> {
    tracing::info!("reorder_service_priorities: {:?}", service_names);

    for (index, name) in service_names.iter().enumerate() {
        sqlx::query(
            "UPDATE service_preferences SET priority = ?, updated_at = CURRENT_TIMESTAMP WHERE service_name = ?"
        )
        .bind((index + 1) as i32)
        .bind(name)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Reorder error: {}", e))?;
    }

    get_service_preferences(state).await
}

/// Get global sync settings
#[tauri::command]
pub async fn get_sync_settings(state: State<'_, AppState>) -> Result<SyncSettings, String> {
    tracing::info!("get_sync_settings called");

    sqlx::query_as::<_, SyncSettings>(
        "SELECT id, auto_sync_enabled, sync_interval_value, sync_interval_unit, sync_on_startup, 
         background_download, max_concurrent_downloads, rate_limit_delay_ms, 
         pause_on_metered, pause_on_low_battery FROM sync_settings WHERE id = 1"
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Update global sync settings
#[tauri::command]
pub async fn update_sync_settings(
    state: State<'_, AppState>,
    settings: SyncSettings,
) -> Result<SyncSettings, String> {
    tracing::info!("update_sync_settings called");

    sqlx::query(
        "UPDATE sync_settings SET auto_sync_enabled = ?, sync_interval_value = ?, sync_interval_unit = ?,
         sync_on_startup = ?, background_download = ?, max_concurrent_downloads = ?, rate_limit_delay_ms = ?,
         pause_on_metered = ?, pause_on_low_battery = ?,
         updated_at = CURRENT_TIMESTAMP WHERE id = 1"
    )
    .bind(settings.auto_sync_enabled)
    .bind(settings.sync_interval_value)
    .bind(&settings.sync_interval_unit)
    .bind(settings.sync_on_startup)
    .bind(settings.background_download)
    .bind(settings.max_concurrent_downloads)
    .bind(settings.rate_limit_delay_ms)
    .bind(settings.pause_on_metered)
    .bind(settings.pause_on_low_battery)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    get_sync_settings(state).await
}

/// Get per-service sync settings
#[tauri::command]
pub async fn get_service_sync_settings(
    state: State<'_, AppState>,
) -> Result<Vec<ServiceSyncSettings>, String> {
    tracing::info!("get_service_sync_settings called");

    sqlx::query_as::<_, ServiceSyncSettings>(
        "SELECT id, service_name, sync_favorites, sync_playlists, sync_albums, incremental_sync, last_synced 
         FROM service_sync_settings"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Update per-service sync settings
#[tauri::command]
pub async fn update_service_sync_settings(
    state: State<'_, AppState>,
    service_name: String,
    sync_favorites: bool,
    sync_playlists: bool,
    sync_albums: bool,
    incremental_sync: bool,
) -> Result<ServiceSyncSettings, String> {
    tracing::info!("update_service_sync_settings: {}", service_name);

    sqlx::query(
        "UPDATE service_sync_settings SET sync_favorites = ?, sync_playlists = ?, sync_albums = ?, 
         incremental_sync = ?, updated_at = CURRENT_TIMESTAMP WHERE service_name = ?",
    )
    .bind(sync_favorites)
    .bind(sync_playlists)
    .bind(sync_albums)
    .bind(incremental_sync)
    .bind(&service_name)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    sqlx::query_as::<_, ServiceSyncSettings>(
        "SELECT id, service_name, sync_favorites, sync_playlists, sync_albums, incremental_sync, last_synced 
         FROM service_sync_settings WHERE service_name = ?"
    )
    .bind(&service_name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Fetch error: {}", e))
}

// ==============================================
// SPRINT 2: DOWNLOADS + FILE SETTINGS
// ==============================================

use crate::models::{
    AudioProcessingSettings, DuplicateSettings, FolderSettings, QualityPreference,
};

/// Get all quality preferences for all services
#[tauri::command]
pub async fn get_quality_preferences(
    state: State<'_, AppState>,
) -> Result<Vec<QualityPreference>, String> {
    tracing::info!("get_quality_preferences");

    sqlx::query_as::<_, QualityPreference>(
        "SELECT id, service_name, max_quality, preferred_format, fallback_quality, fallback_format 
         FROM quality_preferences ORDER BY service_name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Update quality preference for a service
#[tauri::command]
pub async fn update_quality_preference(
    state: State<'_, AppState>,
    service_name: String,
    max_quality: String,
    preferred_format: String,
    fallback_quality: String,
    fallback_format: String,
) -> Result<QualityPreference, String> {
    tracing::info!("update_quality_preference: service={}, max={}, format={}", service_name, max_quality, preferred_format);

    let result = sqlx::query(
        "UPDATE quality_preferences SET max_quality = ?, preferred_format = ?, 
         fallback_quality = ?, fallback_format = ?, updated_at = datetime('now') 
         WHERE service_name = ?",
    )
    .bind(&max_quality)
    .bind(&preferred_format)
    .bind(&fallback_quality)
    .bind(&fallback_format)
    .bind(&service_name)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    tracing::info!("Rows affected: {}", result.rows_affected());
    
    if result.rows_affected() == 0 {
        tracing::warn!("No rows updated for service: {}", service_name);
    }

    sqlx::query_as::<_, QualityPreference>(
        "SELECT id, service_name, max_quality, preferred_format, fallback_quality, fallback_format 
         FROM quality_preferences WHERE service_name = ?",
    )
    .bind(&service_name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Fetch error: {}", e))
}

/// Get folder settings (singleton)
#[tauri::command]
pub async fn get_folder_settings(state: State<'_, AppState>) -> Result<FolderSettings, String> {
    tracing::info!("get_folder_settings");

    sqlx::query_as::<_, FolderSettings>(
        "SELECT id, base_folder, folder_template, file_template, artist_separator, 
         replace_spaces_with, max_path_length, fallback_action FROM folder_settings WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Update folder settings
#[tauri::command]
pub async fn update_folder_settings(
    state: State<'_, AppState>,
    settings: FolderSettings,
) -> Result<FolderSettings, String> {
    tracing::info!("update_folder_settings");

    sqlx::query(
        "UPDATE folder_settings SET base_folder = ?, folder_template = ?, file_template = ?,
         artist_separator = ?, replace_spaces_with = ?, max_path_length = ?, 
         fallback_action = ?, updated_at = datetime('now') WHERE id = 1",
    )
    .bind(&settings.base_folder)
    .bind(&settings.folder_template)
    .bind(&settings.file_template)
    .bind(&settings.artist_separator)
    .bind(&settings.replace_spaces_with)
    .bind(settings.max_path_length)
    .bind(&settings.fallback_action)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    get_folder_settings(state).await
}

/// Preview folder path for a track (template substitution)
#[tauri::command]
pub async fn preview_folder_path(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<String, String> {
    tracing::info!("preview_folder_path: track_id={}", track_id);

    let settings = get_folder_settings(state.clone()).await?;

    // Get track info
    let track: (String, String, String, i64, String) = sqlx::query_as(
        "SELECT t.title, COALESCE(art.name, 'Unknown Artist') as artist, 
         COALESCE(alb.name, 'Unknown Album') as album, t.track_number, 
         COALESCE(t.file_format, 'flac') as format
         FROM tracks t
         LEFT JOIN artists art ON t.artist_id = art.id
         LEFT JOIN albums alb ON t.album_id = alb.id
         WHERE t.id = ?",
    )
    .bind(track_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Track not found: {}", e))?;

    let (title, artist, album, track_number, format) = track;

    // Simple template substitution
    let folder_path = settings
        .folder_template
        .replace("{AlbumArtist}", &artist)
        .replace("{Artist}", &artist)
        .replace("{Album}", &album)
        .replace("{Title}", &title);

    let file_name = settings
        .file_template
        .replace("{TrackNumber:pad2}", &format!("{:02}", track_number))
        .replace("{TrackNumber}", &track_number.to_string())
        .replace("{Title}", &title)
        .replace("{Format:lower}", &format.to_lowercase())
        .replace("{Format}", &format);

    Ok(format!(
        "{}/{}.{}",
        folder_path,
        file_name,
        format.to_lowercase()
    ))
}

/// Get duplicate settings (singleton)
#[tauri::command]
pub async fn get_duplicate_settings(
    state: State<'_, AppState>,
) -> Result<DuplicateSettings, String> {
    tracing::info!("get_duplicate_settings");

    sqlx::query_as::<_, DuplicateSettings>(
        "SELECT id, enable_detection, prefer_higher_quality, prefer_lossless,
         replace_same_quality_different_source, quality_threshold_kbps,
         delete_duplicates_immediately, move_to_trash FROM duplicate_settings WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Update duplicate settings
#[tauri::command]
pub async fn update_duplicate_settings(
    state: State<'_, AppState>,
    settings: DuplicateSettings,
) -> Result<DuplicateSettings, String> {
    tracing::info!("update_duplicate_settings");

    sqlx::query(
        "UPDATE duplicate_settings SET enable_detection = ?, prefer_higher_quality = ?, prefer_lossless = ?,
         replace_same_quality_different_source = ?, quality_threshold_kbps = ?,
         delete_duplicates_immediately = ?, move_to_trash = ?, updated_at = datetime('now') WHERE id = 1"
    )
    .bind(settings.enable_detection)
    .bind(settings.prefer_higher_quality)
    .bind(settings.prefer_lossless)
    .bind(settings.replace_same_quality_different_source)
    .bind(settings.quality_threshold_kbps)
    .bind(settings.delete_duplicates_immediately)
    .bind(settings.move_to_trash)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    get_duplicate_settings(state).await
}

/// Get audio processing settings (singleton)
#[tauri::command]
pub async fn get_audio_processing_settings(
    state: State<'_, AppState>,
) -> Result<AudioProcessingSettings, String> {
    tracing::info!("get_audio_processing_settings");

    sqlx::query_as::<_, AudioProcessingSettings>(
        "SELECT id, replay_gain_mode, target_loudness_lufs, transcode_enabled, transcode_format,
         transcode_bitrate, keep_original_after_transcode, embed_lyrics, embed_artwork, artwork_max_size 
         FROM audio_processing_settings WHERE id = 1"
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Update audio processing settings
#[tauri::command]
pub async fn update_audio_processing_settings(
    state: State<'_, AppState>,
    settings: AudioProcessingSettings,
) -> Result<AudioProcessingSettings, String> {
    tracing::info!("update_audio_processing_settings");

    sqlx::query(
        "UPDATE audio_processing_settings SET replay_gain_mode = ?, target_loudness_lufs = ?,
         transcode_enabled = ?, transcode_format = ?, transcode_bitrate = ?,
         keep_original_after_transcode = ?, embed_lyrics = ?, embed_artwork = ?, 
         artwork_max_size = ?, updated_at = datetime('now') WHERE id = 1",
    )
    .bind(&settings.replay_gain_mode)
    .bind(settings.target_loudness_lufs)
    .bind(settings.transcode_enabled)
    .bind(&settings.transcode_format)
    .bind(settings.transcode_bitrate)
    .bind(settings.keep_original_after_transcode)
    .bind(settings.embed_lyrics)
    .bind(settings.embed_artwork)
    .bind(settings.artwork_max_size)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    get_audio_processing_settings(state).await
}

// ==============================================
// SPRINT 14: METADATA PREFERENCES
// ==============================================

/// Get metadata preferences
#[tauri::command]
pub async fn get_metadata_preferences(state: State<'_, AppState>) -> Result<MetadataPreferences, String> {
    tracing::info!("get_metadata_preferences called");

    sqlx::query_as::<_, MetadataPreferences>(
        "SELECT * FROM metadata_preferences WHERE id = 1"
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Update metadata preferences
#[tauri::command]
pub async fn update_metadata_preferences(
    state: State<'_, AppState>,
    settings: MetadataPreferences,
) -> Result<MetadataPreferences, String> {
    tracing::info!("update_metadata_preferences called");

    sqlx::query(
        r#"
        UPDATE metadata_preferences 
        SET 
            enable_musicbrainz = ?,
            enable_lastfm = ?,
            enable_acoustid = ?,
            overwrite_on_reimport = ?,
            preserve_custom_tags = ?,
            multi_value_separator = ?,
            write_releasetype = ?,
            write_label = ?,
            write_work_composer = ?,
            write_musicbrainz_ids = ?,
            write_download_source = ?,
            write_download_date = ?,
            write_only_available_on = ?,
            write_not_available_streaming = ?,
            write_quality_score = ?,
            write_lyrics_tags = ?,
            weight_album = ?,
            weight_isrc = ?,
            weight_mb_id = ?,
            weight_cover = ?,
            weight_year = ?,
            weight_genre = ?
        WHERE id = 1
        "#
    )
    .bind(settings.enable_musicbrainz)
    .bind(settings.enable_lastfm)
    .bind(settings.enable_acoustid)
    .bind(settings.overwrite_on_reimport)
    .bind(settings.preserve_custom_tags)
    .bind(&settings.multi_value_separator)
    .bind(settings.write_releasetype)
    .bind(settings.write_label)
    .bind(settings.write_work_composer)
    .bind(settings.write_musicbrainz_ids)
    .bind(settings.write_download_source)
    .bind(settings.write_download_date)
    .bind(settings.write_only_available_on)
    .bind(settings.write_not_available_streaming)
    .bind(settings.write_quality_score)
    .bind(settings.write_lyrics_tags)
    .bind(settings.weight_album)
    .bind(settings.weight_isrc)
    .bind(settings.weight_mb_id)
    .bind(settings.weight_cover)
    .bind(settings.weight_year)
    .bind(settings.weight_genre)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    get_metadata_preferences(state).await
}

// ==============================================
// SPRINT 3: LYRICS TAB + SETTINGS
// ==============================================

use crate::models::{LyricsConfig, LyricsProviderSetting};

/// Get all lyrics provider settings ordered by priority
#[tauri::command]
pub async fn get_lyrics_providers(
    state: State<'_, AppState>,
) -> Result<Vec<LyricsProviderSetting>, String> {
    tracing::info!("get_lyrics_providers");

    sqlx::query_as::<_, LyricsProviderSetting>(
        "SELECT id, provider_id, provider_name, enabled, priority, sync_level 
         FROM lyrics_provider_settings ORDER BY priority ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Update a lyrics provider setting
#[tauri::command]
pub async fn update_lyrics_provider(
    state: State<'_, AppState>,
    provider_id: String,
    enabled: bool,
    priority: i64,
) -> Result<LyricsProviderSetting, String> {
    tracing::info!("update_lyrics_provider: {}", provider_id);

    sqlx::query(
        "UPDATE lyrics_provider_settings SET enabled = ?, priority = ?, updated_at = datetime('now')
         WHERE provider_id = ?"
    )
    .bind(enabled)
    .bind(priority)
    .bind(&provider_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    sqlx::query_as::<_, LyricsProviderSetting>(
        "SELECT id, provider_id, provider_name, enabled, priority, sync_level 
         FROM lyrics_provider_settings WHERE provider_id = ?",
    )
    .bind(&provider_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Fetch error: {}", e))
}

/// Reorder lyrics providers
#[tauri::command]
pub async fn reorder_lyrics_providers(
    state: State<'_, AppState>,
    provider_ids: Vec<String>,
) -> Result<Vec<LyricsProviderSetting>, String> {
    tracing::info!("reorder_lyrics_providers: {:?}", provider_ids);

    for (index, provider_id) in provider_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE lyrics_provider_settings SET priority = ?, updated_at = datetime('now')
             WHERE provider_id = ?",
        )
        .bind((index + 1) as i64)
        .bind(provider_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Update error: {}", e))?;
    }

    get_lyrics_providers(state).await
}

/// Get lyrics configuration (singleton)
#[tauri::command]
pub async fn get_lyrics_config(state: State<'_, AppState>) -> Result<LyricsConfig, String> {
    tracing::info!("get_lyrics_config");

    sqlx::query_as::<_, LyricsConfig>(
        "SELECT id, min_sync_level, preferred_language, storage_format, 
         auto_fetch_on_import, retry_failed, retry_frequency 
         FROM lyrics_config WHERE id = 1",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Update lyrics configuration
#[tauri::command]
pub async fn update_lyrics_config(
    state: State<'_, AppState>,
    config: LyricsConfig,
) -> Result<LyricsConfig, String> {
    tracing::info!("update_lyrics_config");

    sqlx::query(
        "UPDATE lyrics_config SET min_sync_level = ?, preferred_language = ?, storage_format = ?,
         auto_fetch_on_import = ?, retry_failed = ?, retry_frequency = ?, updated_at = datetime('now')
         WHERE id = 1"
    )
    .bind(&config.min_sync_level)
    .bind(&config.preferred_language)
    .bind(&config.storage_format)
    .bind(config.auto_fetch_on_import)
    .bind(config.retry_failed)
    .bind(&config.retry_frequency)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    get_lyrics_config(state).await
}

/// Test a lyrics provider connection (calls Python bridge)
#[tauri::command]
pub async fn test_lyrics_provider(provider_id: String) -> Result<bool, String> {
    tracing::info!("test_lyrics_provider: {}", provider_id);

    let output = std::process::Command::new("python")
        .args(&[
            "scripts/lyrics_bridge.py",
            "test",
            "--provider",
            &provider_id,
        ])
        .output()
        .map_err(|e| format!("Failed to run Python: {}", e))?;

    if !output.status.success() {
        return Ok(false);
    }

    let result: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("Parse error: {}", e))?;

    Ok(result
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false))
}

// ==============================================
// SPRINT 5: ADVANCED SETTINGS & POLISH
// ==============================================

fn default_download_path() -> String {
    if let Some(audio_dir) = dirs::audio_dir() {
        return audio_dir.join("Syncify").to_string_lossy().into_owned();
    }

    if let Some(home_dir) = dirs::home_dir() {
        return home_dir
            .join("Music")
            .join("Syncify")
            .to_string_lossy()
            .into_owned();
    }

    std::path::PathBuf::from("Syncify")
        .to_string_lossy()
        .into_owned()
}

#[tauri::command]
pub async fn get_default_download_path() -> Result<String, String> {
    Ok(default_download_path())
}

/// Save a single string setting
#[tauri::command]
pub async fn save_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    sqlx::query(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))"
    )
    .bind(&key)
    .bind(&value)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Failed to save setting '{}': {}", key, e))?;
    Ok(())
}

/// Get multiple settings by keys mapping them to a Hashmap
#[tauri::command]
pub async fn get_kv_settings(
    state: State<'_, AppState>,
    keys: Vec<String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    if keys.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let placeholders = keys.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query_str = format!(
        "SELECT key, value FROM settings WHERE key IN ({})",
        placeholders
    );
    let mut query = sqlx::query_as::<_, (String, String)>(&query_str);
    for key in &keys {
        query = query.bind(key);
    }
    let rows: Vec<(String, String)> = query
        .fetch_all(&state.db)
        .await
        .map_err(|e| format!("Failed to get settings: {}", e))?;

    let mut result: std::collections::HashMap<String, String> = rows.into_iter().collect();

    let requested_download_path = keys.iter().any(|key| key == "dl_download_path");
    let missing_or_blank_download_path = requested_download_path
        && result
            .get("dl_download_path")
            .map(|value| value.trim().is_empty())
            .unwrap_or(true);

    if missing_or_blank_download_path {
        let default_path = default_download_path();

        sqlx::query(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('dl_download_path', ?, datetime('now'))"
        )
        .bind(&default_path)
        .execute(&state.db)
        .await
        .map_err(|e| format!("Failed to self-heal dl_download_path: {}", e))?;

        result.insert("dl_download_path".to_string(), default_path);
    }

    Ok(result)
}

/// Save multiple settings with a transaction
#[tauri::command]
pub async fn save_settings_batch(
    state: State<'_, AppState>,
    settings: std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let mut tx = state.db.begin()
        .await
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    for (key, value) in &settings {
        sqlx::query(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))"
        )
        .bind(key)
        .bind(value)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to save setting '{}': {}", key, e))?;
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit settings: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod settings_tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Create an in-memory test database with schema
    async fn setup_test_db() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create test database");

        // Create minimal schema for testing
        sqlx::query(
            r#"
            CREATE TABLE services (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                supports_download INTEGER DEFAULT 0,
                max_quality TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create services table");

        sqlx::query(
            r#"
            CREATE TABLE accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
                display_name TEXT,
                email TEXT,
                is_active INTEGER DEFAULT 1,
                credentials_json TEXT,
                last_synced TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(service_id, email)
            )
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create accounts table");

        // Create generic settings table for testing
        sqlx::query(
            r#"
            CREATE TABLE settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create settings table");

        // Seed services
        sqlx::query(
            r#"
            INSERT INTO services (name, supports_download, max_quality) VALUES
                ('spotify', 0, 'lossy'),
                ('qobuz', 1, 'hires'),
                ('tidal', 1, 'hires')
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to seed services");

        // Create service_preferences table for testing
        sqlx::query(
            r#"
            CREATE TABLE service_preferences (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                service_name TEXT NOT NULL UNIQUE,
                priority INTEGER NOT NULL DEFAULT 1,
                auto_import_enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create service_preferences table");

        // Seed service preferences
        sqlx::query(
            r#"
            INSERT INTO service_preferences (service_name, priority, auto_import_enabled) VALUES
                ('spotify', 1, 1),
                ('qobuz', 2, 1),
                ('tidal', 3, 1),
                ('deezer', 4, 0),
                ('soundcloud', 5, 0)
        "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to seed service_preferences");

        pool
    }

    #[tokio::test]
    async fn test_get_service_preferences_returns_all() {
        let pool = setup_test_db().await;

        let prefs: Vec<ServicePreference> = sqlx::query_as(
            "SELECT id, service_name, priority, auto_import_enabled FROM service_preferences ORDER BY priority ASC"
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch service preferences");

        assert_eq!(prefs.len(), 5);
        assert_eq!(prefs[0].service_name, "spotify");
        assert_eq!(prefs[0].priority, 1);
        assert_eq!(prefs[1].service_name, "qobuz");
        assert_eq!(prefs[1].priority, 2);
    }

    #[tokio::test]
    async fn test_get_service_preferences_order_by_priority() {
        let pool = setup_test_db().await;

        let prefs: Vec<ServicePreference> = sqlx::query_as(
            "SELECT id, service_name, priority, auto_import_enabled FROM service_preferences ORDER BY priority ASC"
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch service preferences");

        // Verify they are in priority order
        let mut prev_priority = 0;
        for pref in &prefs {
            assert!(
                pref.priority > prev_priority,
                "Preferences should be in ascending priority order"
            );
            prev_priority = pref.priority;
        }
    }

    #[tokio::test]
    async fn test_update_service_preference_auto_import() {
        let pool = setup_test_db().await;

        // Initial state - spotify has auto_import_enabled = 1
        let before: ServicePreference = sqlx::query_as(
            "SELECT id, service_name, priority, auto_import_enabled FROM service_preferences WHERE service_name = 'spotify'"
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to fetch spotify preference");

        assert_eq!(before.auto_import_enabled, true);

        // Update to disable auto import
        sqlx::query(
            "UPDATE service_preferences SET auto_import_enabled = 0 WHERE service_name = 'spotify'",
        )
        .execute(&pool)
        .await
        .expect("Failed to update preference");

        // Verify update
        let after: ServicePreference = sqlx::query_as(
            "SELECT id, service_name, priority, auto_import_enabled FROM service_preferences WHERE service_name = 'spotify'"
        )
        .fetch_one(&pool)
        .await
        .expect("Failed to fetch updated preference");

        assert_eq!(after.auto_import_enabled, false);
    }

    #[tokio::test]
    async fn test_reorder_service_priorities() {
        let pool = setup_test_db().await;

        // Reorder: move qobuz to priority 1, spotify to 2
        sqlx::query("UPDATE service_preferences SET priority = 1 WHERE service_name = 'qobuz'")
            .execute(&pool)
            .await
            .expect("Failed to update qobuz");
        sqlx::query("UPDATE service_preferences SET priority = 2 WHERE service_name = 'spotify'")
            .execute(&pool)
            .await
            .expect("Failed to update spotify");

        // Verify new order
        let prefs: Vec<ServicePreference> = sqlx::query_as(
            "SELECT id, service_name, priority, auto_import_enabled FROM service_preferences ORDER BY priority ASC"
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch preferences");

        assert_eq!(prefs[0].service_name, "qobuz");
        assert_eq!(prefs[0].priority, 1);
        assert_eq!(prefs[1].service_name, "spotify");
        assert_eq!(prefs[1].priority, 2);
    }

    #[tokio::test]
    async fn test_service_preference_unique_name_constraint() {
        let pool = setup_test_db().await;

        // Try to insert duplicate service name - should fail
        let result = sqlx::query(
            "INSERT INTO service_preferences (service_name, priority, auto_import_enabled) VALUES ('spotify', 99, 1)"
        )
        .execute(&pool)
        .await;

        assert!(result.is_err(), "Should not allow duplicate service_name");
    }

    #[tokio::test]
    async fn test_service_preference_auto_import_disabled_services() {
        let pool = setup_test_db().await;

        // Get services with auto_import disabled
        let disabled: Vec<ServicePreference> = sqlx::query_as(
            "SELECT id, service_name, priority, auto_import_enabled FROM service_preferences WHERE auto_import_enabled = 0"
        )
        .fetch_all(&pool)
        .await
        .expect("Failed to fetch disabled preferences");

        assert_eq!(disabled.len(), 2); // deezer and soundcloud
        let names: Vec<&str> = disabled.iter().map(|p| p.service_name.as_str()).collect();
        assert!(names.contains(&"deezer"));
        assert!(names.contains(&"soundcloud"));
    }

    #[tokio::test]
    async fn test_service_preference_toggle_multiple_updates() {
        let pool = setup_test_db().await;

        // Toggle spotify off
        sqlx::query(
            "UPDATE service_preferences SET auto_import_enabled = 0 WHERE service_name = 'spotify'",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Toggle it back on
        sqlx::query(
            "UPDATE service_preferences SET auto_import_enabled = 1 WHERE service_name = 'spotify'",
        )
        .execute(&pool)
        .await
        .unwrap();

        let pref: ServicePreference = sqlx::query_as(
            "SELECT id, service_name, priority, auto_import_enabled FROM service_preferences WHERE service_name = 'spotify'"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(pref.auto_import_enabled, true);
    }

    #[tokio::test]
    async fn test_save_and_load_setting() {
        let pool = setup_test_db().await;
        // manually insert a setting
        sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('test_key', 'test_value')")
            .execute(&pool).await.unwrap();
        // and fetch it
        let row: (String,) = sqlx::query_as("SELECT value FROM settings WHERE key = 'test_key'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, "test_value");
    }

    #[tokio::test]
    async fn test_save_overwrites() {
        let pool = setup_test_db().await;
        sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('overwrite_key', 'v1')")
            .execute(&pool).await.unwrap();
        // insert with same key should replace
        sqlx::query("INSERT OR REPLACE INTO settings (key, value) VALUES ('overwrite_key', 'v2')")
            .execute(&pool).await.unwrap();
            
        let row: (String,) = sqlx::query_as("SELECT value FROM settings WHERE key = 'overwrite_key'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, "v2");
    }

    #[tokio::test]
    async fn test_get_nonexistent_setting_returns_empty() {
        let pool = setup_test_db().await;
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = 'nonexistent'")
            .fetch_optional(&pool).await.unwrap();
        assert!(row.is_none());
    }
}

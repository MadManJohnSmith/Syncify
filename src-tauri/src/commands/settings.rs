// Settings Commands - included via include!() in mod.rs
// 
// Service preferences, sync, quality, folder, lyrics settings



// ==============================================
// SPRINT 1: SERVICE PREFERENCES & SYNC SETTINGS
// ==============================================

use crate::models::{ServicePreference, ServiceSyncSettings, SyncSettings, MetadataPreferences};

/// Get all service preferences ordered by priority
#[tauri::command]
/// Perform get service preferences ordered by priority
pub async fn perform_get_service_preferences(
    db: &crate::DbPool,
) -> Result<Vec<ServicePreference>, String> {
    sqlx::query_as::<_, ServicePreference>(
        "SELECT id, service_name, priority, auto_import_enabled FROM service_preferences ORDER BY priority ASC"
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Get all service preferences ordered by priority
#[tauri::command]
pub async fn get_service_preferences(
    state: State<'_, AppState>,
) -> Result<Vec<ServicePreference>, String> {
    tracing::info!("get_service_preferences called");
    perform_get_service_preferences(&state.db).await
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

/// Perform reorder service priorities
pub async fn perform_reorder_service_priorities(
    db: &crate::DbPool,
    service_names: Vec<String>,
) -> Result<Vec<ServicePreference>, String> {
    tracing::info!("perform_reorder_service_priorities: {:?}", service_names);

    for (index, name) in service_names.iter().enumerate() {
        sqlx::query(
            "UPDATE service_preferences SET priority = ?, updated_at = CURRENT_TIMESTAMP WHERE service_name = ?"
        )
        .bind((index + 1) as i32)
        .bind(name)
        .execute(db)
        .await
        .map_err(|e| format!("Reorder error: {}", e))?;
    }

    perform_get_service_preferences(db).await
}

/// Reorder service priorities based on the provided order
#[tauri::command]
pub async fn reorder_service_priorities(
    state: State<'_, AppState>,
    service_names: Vec<String>,
) -> Result<Vec<ServicePreference>, String> {
    perform_reorder_service_priorities(&state.db, service_names).await
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

    let max_concurrency = (settings.max_concurrent_downloads as usize).max(1);
    state.worker_state.set_max_concurrent(max_concurrency);

    let _ = sqlx::query("UPDATE advanced_settings SET max_concurrent_downloads = ?, updated_at = datetime('now') WHERE id = 1")
        .bind(max_concurrency as i32)
        .execute(&state.db)
        .await;

    let _ = sqlx::query("INSERT INTO settings (key, value) VALUES ('dl_concurrent_downloads', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(max_concurrency.to_string())
        .execute(&state.db)
        .await;

    get_sync_settings(state).await
}

/// Get per-service sync settings
#[tauri::command]
pub async fn get_service_sync_settings(
    state: State<'_, AppState>,
) -> Result<Vec<ServiceSyncSettings>, String> {
    tracing::info!("get_service_sync_settings called");

    sqlx::query_as::<_, ServiceSyncSettings>(
        r#"SELECT id, service_name, sync_favorites, sync_playlists, sync_albums,
                  IFNULL(sync_favorite_artists, 0) as sync_favorite_artists,
                  IFNULL(sync_purchases, 0) as sync_purchases,
                  IFNULL(sync_library_history, 0) as sync_library_history,
                  IFNULL(sync_include_appearances, 0) as sync_include_appearances,
                  incremental_sync, last_synced 
           FROM service_sync_settings"#
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
        r#"SELECT id, service_name, sync_favorites, sync_playlists, sync_albums,
                  IFNULL(sync_favorite_artists, 0) as sync_favorite_artists,
                  IFNULL(sync_purchases, 0) as sync_purchases,
                  IFNULL(sync_library_history, 0) as sync_library_history,
                  IFNULL(sync_include_appearances, 0) as sync_include_appearances,
                  incremental_sync, last_synced 
           FROM service_sync_settings WHERE service_name = ?"#
    )
    .bind(&service_name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Fetch error: {}", e))
}

/// Helper to get granular import preferences for a service from DB
pub async fn perform_get_service_import_preferences(
    db: &sqlx::SqlitePool,
    service_name: &str,
) -> Result<ImportPreferences, String> {
    let row: Option<(bool, bool, bool, bool, bool, bool, bool, bool)> = sqlx::query_as(
        r#"SELECT IFNULL(sync_favorites, 1),
                  IFNULL(sync_albums, 0),
                  IFNULL(sync_favorite_artists, 0),
                  IFNULL(sync_playlists, 1),
                  IFNULL(sync_purchases, 0),
                  IFNULL(sync_library_history, 0),
                  IFNULL(sync_include_appearances, 0),
                  IFNULL(incremental_sync, 1)
           FROM service_sync_settings
           WHERE service_name = ?"#
    )
    .bind(service_name)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Database error fetching import preferences: {}", e))?;

    match row {
        Some((fav_t, fav_a, fav_art, pl, pur, lib_hist, inc_app, inc_sync)) => Ok(ImportPreferences {
            service_name: service_name.to_string(),
            favorite_tracks: fav_t,
            favorite_albums: fav_a,
            favorite_artists: fav_art,
            playlists: pl,
            purchases: pur,
            library_history: lib_hist,
            include_appearances: inc_app,
            incremental_sync: inc_sync,
            force_retry_unavailable: false,
        }),
        None => Ok(ImportPreferences {
            service_name: service_name.to_string(),
            ..ImportPreferences::default()
        }),
    }
}

/// Helper to update granular import preferences for a service in DB
pub async fn perform_update_service_import_preferences(
    db: &sqlx::SqlitePool,
    prefs: ImportPreferences,
) -> Result<ImportPreferences, String> {
    sqlx::query(
        r#"INSERT INTO service_sync_settings 
           (service_name, sync_favorites, sync_albums, sync_favorite_artists, sync_playlists, sync_purchases, sync_library_history, sync_include_appearances, incremental_sync)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(service_name) DO UPDATE SET
               sync_favorites = excluded.sync_favorites,
               sync_albums = excluded.sync_albums,
               sync_favorite_artists = excluded.sync_favorite_artists,
               sync_playlists = excluded.sync_playlists,
               sync_purchases = excluded.sync_purchases,
               sync_library_history = excluded.sync_library_history,
               sync_include_appearances = excluded.sync_include_appearances,
               incremental_sync = excluded.incremental_sync"#
    )
    .bind(&prefs.service_name)
    .bind(prefs.favorite_tracks)
    .bind(prefs.favorite_albums)
    .bind(prefs.favorite_artists)
    .bind(prefs.playlists)
    .bind(prefs.purchases)
    .bind(prefs.library_history)
    .bind(prefs.include_appearances)
    .bind(prefs.incremental_sync)
    .execute(db)
    .await
    .map_err(|e| format!("Database error updating import preferences: {}", e))?;

    perform_get_service_import_preferences(db, &prefs.service_name).await
}

/// Get granular import preferences for a service
#[tauri::command]
pub async fn get_service_import_preferences(
    state: State<'_, AppState>,
    service: String,
) -> Result<ImportPreferences, String> {
    perform_get_service_import_preferences(&state.db, &service).await
}

/// Update granular import preferences for a service
#[tauri::command]
pub async fn update_service_import_preferences(
    state: State<'_, AppState>,
    preferences: ImportPreferences,
) -> Result<ImportPreferences, String> {
    perform_update_service_import_preferences(&state.db, preferences).await
}

// ==============================================
// SPRINT 2: DOWNLOADS + FILE SETTINGS
// ==============================================

use crate::models::{
    AudioProcessingSettings, DuplicateSettings, FolderSettings, QualityPreference,
};

/// Perform get all quality preferences for all services
pub async fn perform_get_quality_preferences(
    db: &crate::DbPool,
) -> Result<Vec<QualityPreference>, String> {
    tracing::info!("get_quality_preferences");

    sqlx::query_as::<_, QualityPreference>(
        "SELECT id, service_name, max_quality, preferred_format, fallback_quality, fallback_format 
         FROM quality_preferences ORDER BY service_name",
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Get all quality preferences for all services
#[tauri::command]
pub async fn get_quality_preferences(
    state: State<'_, AppState>,
) -> Result<Vec<QualityPreference>, String> {
    perform_get_quality_preferences(&state.db).await
}

/// Perform update quality preference for a service
pub async fn perform_update_quality_preference(
    db: &crate::DbPool,
    service_name: String,
    max_quality: String,
    preferred_format: String,
    fallback_quality: String,
    fallback_format: String,
) -> Result<QualityPreference, String> {
    tracing::info!("update_quality_preference: service={}, max={}, format={}", service_name, max_quality, preferred_format);

    sqlx::query(
        r#"INSERT INTO quality_preferences (service_name, max_quality, preferred_format, fallback_quality, fallback_format, updated_at)
           VALUES (?, ?, ?, ?, ?, datetime('now'))
           ON CONFLICT(service_name) DO UPDATE SET
               max_quality = excluded.max_quality,
               preferred_format = excluded.preferred_format,
               fallback_quality = excluded.fallback_quality,
               fallback_format = excluded.fallback_format,
               updated_at = excluded.updated_at"#
    )
    .bind(&service_name)
    .bind(&max_quality)
    .bind(&preferred_format)
    .bind(&fallback_quality)
    .bind(&fallback_format)
    .execute(db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    sqlx::query_as::<_, QualityPreference>(
        "SELECT id, service_name, max_quality, preferred_format, fallback_quality, fallback_format 
         FROM quality_preferences WHERE service_name = ?",
    )
    .bind(&service_name)
    .fetch_one(db)
    .await
    .map_err(|e| format!("Fetch error: {}", e))
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
    perform_update_quality_preference(
        &state.db,
        service_name,
        max_quality,
        preferred_format,
        fallback_quality,
        fallback_format,
    )
    .await
}

/// Perform get folder settings (singleton)
pub async fn perform_get_folder_settings(db: &crate::DbPool) -> Result<FolderSettings, String> {
    tracing::info!("get_folder_settings");

    sqlx::query_as::<_, FolderSettings>(
        "SELECT id, base_folder, folder_template, file_template, artist_separator, 
         replace_spaces_with, max_path_length, fallback_action FROM folder_settings WHERE id = 1",
    )
    .fetch_one(db)
    .await
    .map_err(|e| format!("Database error: {}", e))
}

/// Get folder settings (singleton)
#[tauri::command]
pub async fn get_folder_settings(state: State<'_, AppState>) -> Result<FolderSettings, String> {
    perform_get_folder_settings(&state.db).await
}

/// Perform update folder settings
pub async fn perform_update_folder_settings(
    db: &crate::DbPool,
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
    .execute(db)
    .await
    .map_err(|e| format!("Update error: {}", e))?;

    if !settings.base_folder.trim().is_empty() {
        let trimmed = settings.base_folder.trim();
        for key in &["dl_download_path", "download_dir", "download_path"] {
            let _ = sqlx::query("INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now')) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at")
                .bind(key)
                .bind(trimmed)
                .execute(db)
                .await;
        }
    }

    perform_get_folder_settings(db).await
}

/// Update folder settings
#[tauri::command]
pub async fn update_folder_settings(
    state: State<'_, AppState>,
    settings: FolderSettings,
) -> Result<FolderSettings, String> {
    perform_update_folder_settings(&state.db, settings).await
}

/// Preview folder path for a track (template substitution using LibraryLayout)
#[tauri::command]
pub async fn preview_folder_path(
    state: State<'_, AppState>,
    track_id: i64,
) -> Result<String, String> {
    tracing::info!("preview_folder_path: track_id={}", track_id);

    let settings = get_folder_settings(state.clone()).await?;

    // Get track info
    let track: (String, String, String, Option<String>, Option<i32>, i64, String) = sqlx::query_as(
        "SELECT t.title, COALESCE(art.name, 'Unknown Artist') as artist, 
         COALESCE(alb.name, 'Unknown Album') as album, alb.release_date,
         t.disc_number, t.track_number, 
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

    let (title, artist, album, rel_date, disc_number, track_number, format) = track;
    let rel_year = rel_date.as_deref().and_then(|d| d.split('-').next().and_then(|y| y.parse::<i32>().ok()));

    let template_config = syncify_core_domain::FolderFileTemplateConfig {
        folder_template: settings.folder_template,
        file_template: settings.file_template,
        artist_separator: settings.artist_separator,
        replace_spaces_with: settings.replace_spaces_with,
        max_path_length: settings.max_path_length as usize,
    };

    let layout = syncify_core_domain::LibraryLayout::with_config(
        std::path::Path::new(&settings.base_folder),
        template_config,
    );

    let layout_ctx = syncify_core_domain::TrackLayoutContext {
        artist: &artist,
        album_artist: Some(&artist),
        album: &album,
        title: &title,
        year: rel_year,
        original_date: rel_date.as_deref(),
        track_number: track_number as u32,
        track_total: None,
        disc_number: disc_number.unwrap_or(1) as u32,
        total_discs: 1,
        format: &format,
        bit_depth: None,
        sample_rate: None,
    };

    let resolved = layout.resolve_track_path(&layout_ctx);
    Ok(resolved.to_string_lossy().to_string())
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
    // Deterministic first-run default: prefer the OS audio/home directories, but fall
    // back to a writable location instead of reporting an unusable root on systems
    // where those directories cannot be created or written (locked-down profiles,
    // containers, CI). A default library root MUST always validate as usable.
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Some(audio_dir) = dirs::audio_dir() {
        candidates.push(audio_dir.join("Syncify"));
    }
    if let Some(home_dir) = dirs::home_dir() {
        candidates.push(home_dir.join("Music").join("Syncify"));
    }
    candidates.push(std::env::temp_dir().join("Syncify"));

    for candidate in candidates {
        if ensure_writable_dir(&candidate) {
            return candidate.to_string_lossy().into_owned();
        }
    }

    std::env::temp_dir().join("Syncify").to_string_lossy().into_owned()
}

/// Creates `dir` (including parents) and verifies it accepts writes via a probe file.
fn ensure_writable_dir(dir: &std::path::Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe_file = dir.join(format!(
        ".syncify_probe_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    match std::fs::write(&probe_file, b"probe") {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe_file);
            true
        }
        Err(_) => false,
    }
}

#[tauri::command]
pub async fn get_default_download_path() -> Result<String, String> {
    Ok(default_download_path())
}

fn default_temp_path() -> String {
    if let Some(cache_dir) = dirs::cache_dir() {
        return cache_dir.join("Syncify").join(".staging").to_string_lossy().into_owned();
    }
    if let Some(local_data) = dirs::data_local_dir() {
        return local_data.join("Syncify").join(".staging").to_string_lossy().into_owned();
    }
    std::env::temp_dir().join("Syncify").join(".staging").to_string_lossy().into_owned()
}

#[tauri::command]
pub async fn get_default_temp_path() -> Result<String, String> {
    Ok(default_temp_path())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathValidationResult {
    pub valid: bool,
    pub exists: bool,
    pub is_dir: bool,
    pub is_writable: bool,
    pub available_bytes: u64,
    pub drive_mounted: bool,
    pub canonical_path: String,
    pub error_message: Option<String>,
}

#[tauri::command]
pub async fn validate_directory_path(path: String) -> Result<PathValidationResult, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Ok(PathValidationResult {
            valid: false,
            exists: false,
            is_dir: false,
            is_writable: false,
            available_bytes: 0,
            drive_mounted: false,
            canonical_path: String::new(),
            error_message: Some("Path cannot be empty".to_string()),
        });
    }

    let p = std::path::Path::new(trimmed);
    
    // Check drive / root existence
    let drive_mounted = if let Some(prefix) = p.components().next() {
        match prefix {
            std::path::Component::Prefix(p_info) => {
                let prefix_str = p_info.as_os_str().to_string_lossy();
                let root_path = format!("{}\\", prefix_str);
                std::path::Path::new(&root_path).exists()
            }
            std::path::Component::RootDir => true,
            _ => true,
        }
    } else {
        false
    };

    if !drive_mounted {
        return Ok(PathValidationResult {
            valid: false,
            exists: false,
            is_dir: false,
            is_writable: false,
            available_bytes: 0,
            drive_mounted: false,
            canonical_path: trimmed.to_string(),
            error_message: Some(format!("Drive or volume for path '{}' is not mounted or accessible", trimmed)),
        });
    }

    let exists = p.exists();
    let is_dir = if exists { p.is_dir() } else { false };

    let mut is_writable = false;
    let mut write_err = None;

    if exists {
        if !is_dir {
            return Ok(PathValidationResult {
                valid: false,
                exists: true,
                is_dir: false,
                is_writable: false,
                available_bytes: 0,
                drive_mounted: true,
                canonical_path: trimmed.to_string(),
                error_message: Some("Specified path exists but is a file, not a directory".to_string()),
            });
        }
        
        let probe_file = p.join(format!(".syncify_probe_{}.tmp", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));
        match std::fs::write(&probe_file, b"probe") {
            Ok(_) => {
                let _ = std::fs::remove_file(&probe_file);
                is_writable = true;
            }
            Err(e) => {
                write_err = Some(format!("Directory is not writable: {}", e));
            }
        }
    } else {
        match std::fs::create_dir_all(p) {
            Ok(_) => {
                let probe_file = p.join(format!(".syncify_probe_{}.tmp", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));
                match std::fs::write(&probe_file, b"probe") {
                    Ok(_) => {
                        let _ = std::fs::remove_file(&probe_file);
                        is_writable = true;
                    }
                    Err(e) => {
                        write_err = Some(format!("Created directory but cannot write files: {}", e));
                    }
                }
            }
            Err(e) => {
                write_err = Some(format!("Cannot create directory: {}", e));
            }
        }
    }

    let available_bytes = {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        let mut matched_avail = 0u64;
        let mut max_prefix_len = 0;
        for disk in disks.iter() {
            let mount = disk.mount_point();
            if p.starts_with(mount) {
                let len = mount.to_string_lossy().len();
                if len >= max_prefix_len {
                    max_prefix_len = len;
                    matched_avail = disk.available_space();
                }
            }
        }
        matched_avail
    };

    let valid = is_writable && write_err.is_none();
    let canonical = p.to_string_lossy().to_string();

    Ok(PathValidationResult {
        valid,
        exists: p.exists(),
        is_dir: p.is_dir(),
        is_writable,
        available_bytes,
        drive_mounted: true,
        canonical_path: canonical,
        error_message: write_err,
    })
}

/// Effective download and staging paths descriptor for Sprint S124B
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectiveDownloadPaths {
    pub library_root: String,
    pub staging_root: String,
    pub path_status: String,
    pub free_space_bytes: u64,
    pub is_writable: bool,
    pub drive_mounted: bool,
    pub exists: bool,
    pub error_message: Option<String>,
}

/// Deterministic resolution for effective download library root and staging paths
pub async fn resolve_effective_download_paths(db: &crate::DbPool) -> Result<EffectiveDownloadPaths, String> {
    // 1. Canonical source: folder_settings.base_folder
    let base_folder_opt: Option<String> = sqlx::query_scalar(
        "SELECT base_folder FROM folder_settings WHERE id = 1 AND base_folder IS NOT NULL AND TRIM(base_folder) != ''"
    )
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Database error fetching folder_settings: {}", e))?;

    let canonical_root = match base_folder_opt {
        Some(ref s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => {
            // 2. Compatibility fallback: settings table keys (dl_download_path > download_dir > download_path)
            let setting_path: Option<String> = sqlx::query_scalar(
                "SELECT value FROM settings WHERE key IN ('dl_download_path', 'download_dir', 'download_path') AND value IS NOT NULL AND TRIM(value) != '' ORDER BY CASE key WHEN 'dl_download_path' THEN 1 WHEN 'download_dir' THEN 2 ELSE 3 END LIMIT 1"
            )
            .fetch_optional(db)
            .await
            .unwrap_or(None);

            match setting_path {
                Some(ref s) if !s.trim().is_empty() => s.trim().to_string(),
                _ => default_download_path(),
            }
        }
    };

    let validation = validate_directory_path(canonical_root.clone()).await?;

    let staging_root = std::path::Path::new(&canonical_root)
        .join(".staging")
        .to_string_lossy()
        .into_owned();

    let path_status = if !validation.drive_mounted {
        "unmounted".to_string()
    } else if !validation.valid {
        "invalid".to_string()
    } else {
        "valid".to_string()
    };

    Ok(EffectiveDownloadPaths {
        library_root: canonical_root,
        staging_root,
        path_status,
        free_space_bytes: validation.available_bytes,
        is_writable: validation.is_writable,
        drive_mounted: validation.drive_mounted,
        exists: validation.exists,
        error_message: validation.error_message,
    })
}

/// Perform get effective download paths
pub async fn perform_get_effective_download_paths(db: &crate::DbPool) -> Result<EffectiveDownloadPaths, String> {
    resolve_effective_download_paths(db).await
}

/// Expose single effective download paths query for UI & backend commands
#[tauri::command]
pub async fn get_effective_download_paths(state: State<'_, AppState>) -> Result<EffectiveDownloadPaths, String> {
    perform_get_effective_download_paths(&state.db).await
}

/// Perform save a single setting and keep canonical folder_settings in sync
pub async fn perform_save_setting(
    db: &crate::DbPool,
    key: String,
    value: String,
) -> Result<(), String> {
    let is_dl_path_key = key == "dl_download_path" || key == "download_dir" || key == "download_path";
    let trimmed_val = value.trim();

    if is_dl_path_key {
        if trimmed_val.is_empty() {
            // Guard: do not overwrite a valid configured path with an empty string from a stale key
            let current_base: Option<String> = sqlx::query_scalar(
                "SELECT base_folder FROM folder_settings WHERE id = 1 AND base_folder IS NOT NULL AND TRIM(base_folder) != ''"
            )
            .fetch_optional(db)
            .await
            .unwrap_or(None);
            if current_base.is_some() {
                return Ok(());
            }
        } else {
            let _ = sqlx::query("UPDATE folder_settings SET base_folder = ?, updated_at = datetime('now') WHERE id = 1")
                .bind(trimmed_val)
                .execute(db)
                .await;
            for k in &["dl_download_path", "download_dir", "download_path"] {
                let _ = sqlx::query(
                    "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now')) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at"
                )
                .bind(k)
                .bind(trimmed_val)
                .execute(db)
                .await;
            }
            return Ok(());
        }
    }

    sqlx::query(
        "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))"
    )
    .bind(&key)
    .bind(&value)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to save setting '{}': {}", key, e))?;
    Ok(())
}

/// Save a single string setting
#[tauri::command]
pub async fn save_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    perform_save_setting(&state.db, key, value).await
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

    let requested_download_path = keys.iter().any(|key| key == "dl_download_path" || key == "download_dir" || key == "download_path");
    let missing_or_blank_download_path = requested_download_path
        && result
            .get("dl_download_path")
            .or_else(|| result.get("download_dir"))
            .or_else(|| result.get("download_path"))
            .map(|value| value.trim().is_empty())
            .unwrap_or(true);

    if missing_or_blank_download_path {
        let eff = resolve_effective_download_paths(&state.db).await
            .map(|e| e.library_root)
            .unwrap_or_else(|_| default_download_path());

        for k in &["dl_download_path", "download_dir", "download_path"] {
            let _ = sqlx::query(
                "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))"
            )
            .bind(k)
            .bind(&eff)
            .execute(&state.db)
            .await;
        }

        result.insert("dl_download_path".to_string(), eff);
    }

    Ok(result)
}

/// Perform save multiple settings with a transaction
pub async fn perform_save_settings_batch(
    db: &crate::DbPool,
    settings: std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let mut tx = db.begin()
        .await
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    let mut new_dl_path: Option<String> = None;
    for (k, v) in &settings {
        if (k == "dl_download_path" || k == "download_dir" || k == "download_path") && !v.trim().is_empty() {
            new_dl_path = Some(v.trim().to_string());
        }
    }

    if let Some(ref path) = new_dl_path {
        sqlx::query(
            "UPDATE folder_settings SET base_folder = ?, updated_at = datetime('now') WHERE id = 1"
        )
        .bind(path)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to sync folder_settings: {}", e))?;
    }

    for (key, value) in &settings {
        let is_dl_key = key == "dl_download_path" || key == "download_dir" || key == "download_path";
        if is_dl_key && value.trim().is_empty() && new_dl_path.is_none() {
            // Guard against wiping configured path
            continue;
        }
        let val_to_write = if is_dl_key {
            new_dl_path.as_deref().unwrap_or(value)
        } else {
            value
        };

        sqlx::query(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))"
        )
        .bind(key)
        .bind(val_to_write)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to save setting '{}': {}", key, e))?;
    }

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit settings: {}", e))?;

    Ok(())
}

/// Save multiple settings with a transaction
#[tauri::command]
pub async fn save_settings_batch(
    state: State<'_, AppState>,
    settings: std::collections::HashMap<String, String>,
) -> Result<(), String> {
    perform_save_settings_batch(&state.db, settings).await
}

// ==============================================
// SPRINT 120: UNIFIED DOWNLOADS & SIDECAR SETTINGS
// ==============================================

/// Unified DTO for downloads, folder templates, concurrency, sidecar flags, and effective paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadSettingsDto {
    pub download_path: String,
    #[serde(default)]
    pub temporary_root: Option<String>,
    pub folder_template: String,
    pub file_template: String,
    pub artist_separator: String,
    pub replace_spaces_with: Option<String>,
    pub max_path_length: i64,
    pub fallback_action: String,
    pub max_concurrent_downloads: i64,
    pub retry_failed: bool,
    pub retry_count: i64,
    pub retry_delay_ms: i64,
    pub auto_download_favorites: bool,
    pub organize_by_artist: bool,
    pub organize_by_album: bool,
    pub generate_lyrics_lrc: bool,
    pub generate_cover_art: bool,
    pub generate_animated_cover: bool,
    pub generate_booklet: bool,
    pub generate_artist_sidecars: bool,
    #[serde(default)]
    pub library_root: Option<String>,
    #[serde(default)]
    pub staging_root: Option<String>,
    #[serde(default)]
    pub path_status: Option<String>,
    #[serde(default)]
    pub free_space_bytes: Option<u64>,
}

/// Perform get unified download and file structure settings
pub async fn perform_get_download_settings(state: &AppState) -> Result<DownloadSettingsDto, String> {
    tracing::info!("get_download_settings called");

    let folder: FolderSettings = perform_get_folder_settings(&state.db).await?;
    let sync: SyncSettings = sqlx::query_as::<_, SyncSettings>(
        "SELECT id, auto_sync_enabled, sync_interval_value, sync_interval_unit, sync_on_startup, 
         background_download, max_concurrent_downloads, rate_limit_delay_ms, 
         pause_on_metered, pause_on_low_battery FROM sync_settings WHERE id = 1"
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("Database error: {}", e))?;

    let effective = resolve_effective_download_paths(&state.db).await?;

    let kv_rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings WHERE key LIKE 'dl_%' OR key IN ('download_dir', 'temp_dir')")
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let mut dl_map = std::collections::HashMap::new();
    for (k, v) in kv_rows {
        dl_map.insert(k, v);
    }

    let download_path = effective.library_root.clone();
    let temporary_root = dl_map.get("dl_temp_dir").or_else(|| dl_map.get("temp_dir")).cloned().or_else(|| Some(effective.staging_root.clone()));
    let retry_failed = dl_map.get("dl_retry_failed").map(|v| v == "true" || v == "1").unwrap_or(true);
    let retry_count = dl_map.get("dl_retry_count").and_then(|v| v.parse().ok()).unwrap_or(3);
    let retry_delay_ms = dl_map.get("dl_retry_delay").and_then(|v| v.parse().ok()).unwrap_or(sync.rate_limit_delay_ms as i64);
    let auto_download_favorites = dl_map.get("dl_auto_download_favorites").map(|v| v == "true" || v == "1").unwrap_or(false);
    let organize_by_artist = dl_map.get("dl_create_artist_folder").map(|v| v == "true" || v == "1").unwrap_or(true);
    let organize_by_album = dl_map.get("dl_create_album_folder").map(|v| v == "true" || v == "1").unwrap_or(true);

    let generate_lyrics_lrc = dl_map.get("dl_generate_lyrics_lrc").map(|v| v == "true" || v == "1").unwrap_or(true);
    let generate_cover_art = dl_map.get("dl_generate_cover_art").map(|v| v == "true" || v == "1").unwrap_or(true);
    let generate_animated_cover = dl_map.get("dl_generate_animated_cover").map(|v| v == "true" || v == "1").unwrap_or(true);
    let generate_booklet = dl_map.get("dl_generate_booklet").map(|v| v == "true" || v == "1").unwrap_or(true);
    let generate_artist_sidecars = dl_map.get("dl_generate_artist_sidecars").map(|v| v == "true" || v == "1").unwrap_or(true);

    let max_concurrent = state.worker_state.max_concurrent() as i64;

    Ok(DownloadSettingsDto {
        download_path,
        temporary_root,
        folder_template: folder.folder_template,
        file_template: folder.file_template,
        artist_separator: folder.artist_separator,
        replace_spaces_with: folder.replace_spaces_with,
        max_path_length: folder.max_path_length,
        fallback_action: folder.fallback_action,
        max_concurrent_downloads: max_concurrent,
        retry_failed,
        retry_count,
        retry_delay_ms,
        auto_download_favorites,
        organize_by_artist,
        organize_by_album,
        generate_lyrics_lrc,
        generate_cover_art,
        generate_animated_cover,
        generate_booklet,
        generate_artist_sidecars,
        library_root: Some(effective.library_root),
        staging_root: Some(effective.staging_root),
        path_status: Some(effective.path_status),
        free_space_bytes: Some(effective.free_space_bytes),
    })
}

/// Get unified download and file structure settings
#[tauri::command]
pub async fn get_download_settings(state: State<'_, AppState>) -> Result<DownloadSettingsDto, String> {
    perform_get_download_settings(&state).await
}

/// Perform save unified download and file structure settings
pub async fn perform_save_download_settings(
    state: &AppState,
    settings: DownloadSettingsDto,
) -> Result<DownloadSettingsDto, String> {
    tracing::info!("save_download_settings called");

    // 1. Update folder_settings table (canonical library_root)
    sqlx::query(
        "UPDATE folder_settings SET base_folder = ?, folder_template = ?, file_template = ?,
         artist_separator = ?, replace_spaces_with = ?, max_path_length = ?, 
         fallback_action = ?, updated_at = datetime('now') WHERE id = 1"
    )
    .bind(&settings.download_path)
    .bind(&settings.folder_template)
    .bind(&settings.file_template)
    .bind(&settings.artist_separator)
    .bind(&settings.replace_spaces_with)
    .bind(settings.max_path_length)
    .bind(&settings.fallback_action)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Database error updating folder_settings: {}", e))?;

    // 2. Update sync_settings, advanced_settings, and worker state
    sqlx::query(
        "UPDATE sync_settings SET max_concurrent_downloads = ?, rate_limit_delay_ms = ?, updated_at = CURRENT_TIMESTAMP WHERE id = 1"
    )
    .bind(settings.max_concurrent_downloads as i32)
    .bind(settings.retry_delay_ms as i32)
    .execute(&state.db)
    .await
    .map_err(|e| format!("Database error updating sync_settings: {}", e))?;

    let _ = sqlx::query(
        "UPDATE advanced_settings SET max_concurrent_downloads = ?, updated_at = datetime('now') WHERE id = 1"
    )
    .bind(settings.max_concurrent_downloads as i32)
    .execute(&state.db)
    .await;

    state.worker_state.set_max_concurrent(settings.max_concurrent_downloads as usize);

    // 3. Update settings key-value table and keep all legacy keys synchronized
    let mut kv_pairs = vec![
        ("dl_download_path".to_string(), settings.download_path.clone()),
        ("download_dir".to_string(), settings.download_path.clone()),
        ("download_path".to_string(), settings.download_path.clone()),
        ("dl_concurrent_downloads".to_string(), settings.max_concurrent_downloads.to_string()),
        ("dl_retry_failed".to_string(), settings.retry_failed.to_string()),
        ("dl_retry_count".to_string(), settings.retry_count.to_string()),
        ("dl_retry_delay".to_string(), settings.retry_delay_ms.to_string()),
        ("dl_create_artist_folder".to_string(), settings.organize_by_artist.to_string()),
        ("dl_create_album_folder".to_string(), settings.organize_by_album.to_string()),
        ("dl_auto_download_favorites".to_string(), settings.auto_download_favorites.to_string()),
        ("dl_generate_lyrics_lrc".to_string(), settings.generate_lyrics_lrc.to_string()),
        ("dl_generate_cover_art".to_string(), settings.generate_cover_art.to_string()),
        ("dl_generate_animated_cover".to_string(), settings.generate_animated_cover.to_string()),
        ("dl_generate_booklet".to_string(), settings.generate_booklet.to_string()),
        ("dl_generate_artist_sidecars".to_string(), settings.generate_artist_sidecars.to_string()),
    ];

    if let Some(ref tr) = settings.temporary_root {
        kv_pairs.push(("dl_temp_dir".to_string(), tr.clone()));
        kv_pairs.push(("temp_dir".to_string(), tr.clone()));
    }

    for (k, v) in kv_pairs {
        sqlx::query("INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now')) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at")
            .bind(&k)
            .bind(&v)
            .execute(&state.db)
            .await
            .map_err(|e| format!("Database error saving setting '{}': {}", k, e))?;
    }

    perform_get_download_settings(state).await
}

/// Save unified download and file structure settings
#[tauri::command]
pub async fn save_download_settings(
    state: State<'_, AppState>,
    settings: DownloadSettingsDto,
) -> Result<DownloadSettingsDto, String> {
    perform_save_download_settings(&state, settings).await
}

/// Perform update fallback action policy for layout and downloads
pub async fn perform_update_fallback_action(
    db: &crate::DbPool,
    fallback_action: String,
) -> Result<String, String> {
    tracing::info!("update_fallback_action: {}", fallback_action);
    sqlx::query("UPDATE folder_settings SET fallback_action = ?, updated_at = datetime('now') WHERE id = 1")
        .bind(&fallback_action)
        .execute(db)
        .await
        .map_err(|e| format!("Database error: {}", e))?;
    Ok(fallback_action)
}

/// Update fallback action policy for layout and downloads
#[tauri::command]
pub async fn update_fallback_action(
    state: State<'_, AppState>,
    fallback_action: String,
) -> Result<String, String> {
    perform_update_fallback_action(&state.db, fallback_action).await
}

/// Sidecar settings DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarSettingsDto {
    pub generate_lyrics_lrc: bool,
    pub generate_cover_art: bool,
    pub generate_animated_cover: bool,
    pub generate_booklet: bool,
    pub generate_artist_sidecars: bool,
}

/// Perform get sidecar generation flags
pub async fn perform_get_sidecar_settings(db: &crate::DbPool) -> Result<SidecarSettingsDto, String> {
    tracing::info!("get_sidecar_settings called");
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT key, value FROM settings WHERE key LIKE 'dl_generate_%'"
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut dto = SidecarSettingsDto {
        generate_lyrics_lrc: true,
        generate_cover_art: true,
        generate_animated_cover: true,
        generate_booklet: true,
        generate_artist_sidecars: true,
    };

    for (k, v) in rows {
        match k.as_str() {
            "dl_generate_lyrics_lrc" => dto.generate_lyrics_lrc = v == "true" || v == "1",
            "dl_generate_cover_art" => dto.generate_cover_art = v == "true" || v == "1",
            "dl_generate_animated_cover" => dto.generate_animated_cover = v == "true" || v == "1",
            "dl_generate_booklet" => dto.generate_booklet = v == "true" || v == "1",
            "dl_generate_artist_sidecars" => dto.generate_artist_sidecars = v == "true" || v == "1",
            _ => {}
        }
    }

    Ok(dto)
}

/// Get sidecar generation flags
#[tauri::command]
pub async fn get_sidecar_settings(state: State<'_, AppState>) -> Result<SidecarSettingsDto, String> {
    perform_get_sidecar_settings(&state.db).await
}

/// Perform update sidecar generation flags
pub async fn perform_update_sidecar_settings(
    db: &crate::DbPool,
    settings: SidecarSettingsDto,
) -> Result<SidecarSettingsDto, String> {
    tracing::info!("update_sidecar_settings: {:?}", settings);

    let kvs = vec![
        ("dl_generate_lyrics_lrc", settings.generate_lyrics_lrc.to_string()),
        ("dl_generate_cover_art", settings.generate_cover_art.to_string()),
        ("dl_generate_animated_cover", settings.generate_animated_cover.to_string()),
        ("dl_generate_booklet", settings.generate_booklet.to_string()),
        ("dl_generate_artist_sidecars", settings.generate_artist_sidecars.to_string()),
    ];

    for (k, v) in kvs {
        sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
            .bind(k)
            .bind(v)
            .execute(db)
            .await
            .map_err(|e| format!("Database error: {}", e))?;
    }

    perform_get_sidecar_settings(db).await
}

/// Update sidecar generation flags
#[tauri::command]
pub async fn update_sidecar_settings(
    state: State<'_, AppState>,
    settings: SidecarSettingsDto,
) -> Result<SidecarSettingsDto, String> {
    perform_update_sidecar_settings(&state.db, settings).await
}

// ==============================================
// SPRINT 148: CANONICAL EFFECTIVE DOWNLOAD PREFERENCES
// ==============================================

/// Resolve all effective download and scheduling preferences from canonical sources
pub async fn resolve_effective_download_preferences(
    db: &crate::DbPool,
    worker_state: &crate::worker::DownloadWorkerState,
) -> Result<EffectiveDownloadPreferences, String> {
    tracing::info!("resolve_effective_download_preferences called");

    // 1. Effective paths
    let effective_paths = resolve_effective_download_paths(db).await?;

    // 2. Folder settings
    let folder: FolderSettings = perform_get_folder_settings(db).await
        .unwrap_or_else(|_| FolderSettings {
            id: 1,
            base_folder: effective_paths.library_root.clone(),
            folder_template: "{AlbumArtist}/{Album}".to_string(),
            file_template: "{TrackNumber:pad2} - {Title}".to_string(),
            artist_separator: ", ".to_string(),
            replace_spaces_with: None,
            max_path_length: 255,
            fallback_action: "try_next".to_string(),
        });

    // 3. Sync settings
    let sync: SyncSettings = sqlx::query_as::<_, SyncSettings>(
        "SELECT id, auto_sync_enabled, sync_interval_value, sync_interval_unit, sync_on_startup, 
         background_download, max_concurrent_downloads, rate_limit_delay_ms, 
         pause_on_metered, pause_on_low_battery FROM sync_settings WHERE id = 1"
    )
    .fetch_optional(db)
    .await
    .unwrap_or(None)
    .unwrap_or_else(|| SyncSettings {
        id: 1,
        auto_sync_enabled: true,
        sync_interval_value: 1,
        sync_interval_unit: "hours".to_string(),
        sync_on_startup: true,
        background_download: true,
        max_concurrent_downloads: 3,
        rate_limit_delay_ms: 500,
        pause_on_metered: true,
        pause_on_low_battery: true,
    });

    // 4. Advanced settings
    #[derive(sqlx::FromRow)]
    struct AdvRow {
        max_retries: Option<i32>,
        retry_delay_seconds: Option<i32>,
    }
    let adv: Option<AdvRow> = sqlx::query_as(
        "SELECT max_retries, retry_delay_seconds FROM advanced_settings WHERE id = 1"
    )
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let max_retries = adv.as_ref().and_then(|a| a.max_retries).unwrap_or(3).max(0) as u32;
    let retry_delay_seconds = adv.as_ref().and_then(|a| a.retry_delay_seconds).unwrap_or(5).max(0) as u32;

    // 5. Quality preferences
    let service_qualities = perform_get_quality_preferences(db).await.unwrap_or_default();

    // 6. Service preferences (priority order)
    let service_prefs: Vec<ServicePreference> = sqlx::query_as(
        "SELECT id, service_name, priority, auto_import_enabled FROM service_preferences ORDER BY priority ASC"
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let service_priority_order: Vec<String> = service_prefs.into_iter().map(|p| p.service_name).collect();

    // 7. Preferred download service: first downloadable service according to priority order
    let preferred_download_service: Option<String> = sqlx::query_scalar(
        r#"SELECT sp.service_name 
           FROM service_preferences sp 
           JOIN services s ON s.name = sp.service_name 
           WHERE s.supports_download = 1 
           ORDER BY sp.priority ASC 
           LIMIT 1"#
    )
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    // 8. Global quality & format derived from top downloadable service or default
    let (max_quality, preferred_format) = if let Some(ref pref_svc) = preferred_download_service {
        service_qualities.iter()
            .find(|q| q.service_name.eq_ignore_ascii_case(pref_svc))
            .map(|q| (q.max_quality.clone(), q.preferred_format.clone()))
            .unwrap_or_else(|| ("hires".to_string(), "flac".to_string()))
    } else {
        ("hires".to_string(), "flac".to_string())
    };

    // 9. Sidecar flags & KV settings
    let kv_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT key, value FROM settings WHERE key LIKE 'dl_%'"
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let kv_map: std::collections::HashMap<String, String> = kv_rows.into_iter().collect();

    let auto_download_favorites = kv_map.get("dl_auto_download_favorites").map(|v| v == "true" || v == "1").unwrap_or(false);
    let generate_lyrics_lrc = kv_map.get("dl_generate_lyrics_lrc").map(|v| v == "true" || v == "1").unwrap_or(true);
    let generate_cover_art = kv_map.get("dl_generate_cover_art").map(|v| v == "true" || v == "1").unwrap_or(true);
    let generate_animated_cover = kv_map.get("dl_generate_animated_cover").map(|v| v == "true" || v == "1").unwrap_or(true);
    let generate_booklet = kv_map.get("dl_generate_booklet").map(|v| v == "true" || v == "1").unwrap_or(true);
    let generate_artist_sidecars = kv_map.get("dl_generate_artist_sidecars").map(|v| v == "true" || v == "1").unwrap_or(true);

    let max_concurrent_downloads = worker_state.max_concurrent().max(1) as u32;

    let fallback_action = folder.fallback_action.clone();
    let allow_downgrade = fallback_action != "skip";
    let strict_quality = fallback_action == "skip";

    Ok(EffectiveDownloadPreferences {
        download_path: effective_paths.library_root,
        staging_path: effective_paths.staging_root,
        path_status: effective_paths.path_status,
        free_space_bytes: effective_paths.free_space_bytes,
        max_quality,
        preferred_format,
        fallback_action,
        allow_downgrade,
        strict_quality,
        preferred_download_service,
        service_priority_order,
        service_qualities,
        max_concurrent_downloads,
        rate_limit_delay_ms: sync.rate_limit_delay_ms.max(0) as u32,
        max_retries,
        retry_delay_seconds,
        auto_download_favorites,
        generate_lyrics_lrc,
        generate_cover_art,
        generate_animated_cover,
        generate_booklet,
        generate_artist_sidecars,
        auto_sync_enabled: sync.auto_sync_enabled,
        sync_interval_value: sync.sync_interval_value.max(1) as u32,
        sync_interval_unit: sync.sync_interval_unit,
        sync_on_startup: sync.sync_on_startup,
        background_download: sync.background_download,
        pause_on_metered: sync.pause_on_metered,
        pause_on_low_battery: sync.pause_on_low_battery,
        folder_template: folder.folder_template,
        file_template: folder.file_template,
        artist_separator: folder.artist_separator,
        replace_spaces_with: folder.replace_spaces_with,
        max_path_length: folder.max_path_length.max(64) as u32,
    })
}

/// Perform get effective download preferences
pub async fn perform_get_effective_download_preferences(
    state: &AppState,
) -> Result<EffectiveDownloadPreferences, String> {
    resolve_effective_download_preferences(&state.db, &state.worker_state).await
}

/// Perform save effective download preferences atomically
pub async fn perform_save_effective_download_preferences(
    state: &AppState,
    prefs: EffectiveDownloadPreferences,
) -> Result<EffectiveDownloadPreferences, String> {
    tracing::info!("save_effective_download_preferences called: {:?}", prefs);

    // 1. Strict validation of download path
    let trimmed_path = prefs.download_path.trim();
    if trimmed_path.is_empty() {
        return Err("Download directory path cannot be empty".to_string());
    }

    let validation = validate_directory_path(trimmed_path.to_string()).await?;
    if !validation.drive_mounted {
        return Err(format!("Drive for path '{}' is not mounted or accessible", trimmed_path));
    }
    if !validation.valid || !validation.is_writable {
        return Err(format!(
            "Directory path '{}' is invalid or not writable: {}",
            trimmed_path,
            validation.error_message.unwrap_or_else(|| "Permission denied".to_string())
        ));
    }

    let canonical_path = validation.canonical_path;

    // 2. Begin atomic SQLite transaction
    let mut tx = state.db.begin()
        .await
        .map_err(|e| format!("Failed to begin transaction: {}", e))?;

    // 3. Update folder_settings
    let fallback_act = if prefs.fallback_action.is_empty() {
        if prefs.allow_downgrade { "try_next".to_string() } else { "skip".to_string() }
    } else {
        prefs.fallback_action.clone()
    };

    sqlx::query(
        r#"UPDATE folder_settings 
           SET base_folder = ?, folder_template = ?, file_template = ?,
               artist_separator = ?, replace_spaces_with = ?, max_path_length = ?, 
               fallback_action = ?, updated_at = datetime('now') 
           WHERE id = 1"#
    )
    .bind(&canonical_path)
    .bind(&prefs.folder_template)
    .bind(&prefs.file_template)
    .bind(&prefs.artist_separator)
    .bind(&prefs.replace_spaces_with)
    .bind(prefs.max_path_length as i64)
    .bind(&fallback_act)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to update folder_settings: {}", e))?;

    // 4. Update sync_settings
    sqlx::query(
        r#"UPDATE sync_settings 
           SET auto_sync_enabled = ?, sync_interval_value = ?, sync_interval_unit = ?,
               sync_on_startup = ?, background_download = ?, max_concurrent_downloads = ?, 
               rate_limit_delay_ms = ?, pause_on_metered = ?, pause_on_low_battery = ?,
               updated_at = CURRENT_TIMESTAMP 
           WHERE id = 1"#
    )
    .bind(prefs.auto_sync_enabled)
    .bind(prefs.sync_interval_value as i32)
    .bind(&prefs.sync_interval_unit)
    .bind(prefs.sync_on_startup)
    .bind(prefs.background_download)
    .bind(prefs.max_concurrent_downloads as i32)
    .bind(prefs.rate_limit_delay_ms as i32)
    .bind(prefs.pause_on_metered)
    .bind(prefs.pause_on_low_battery)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("Failed to update sync_settings: {}", e))?;

    // 5. Update advanced_settings
    let _ = sqlx::query(
        r#"UPDATE advanced_settings 
           SET max_concurrent_downloads = ?,
               max_retries = ?, retry_delay_seconds = ?, updated_at = datetime('now') 
           WHERE id = 1"#
    )
    .bind(prefs.max_concurrent_downloads as i32)
    .bind(prefs.max_retries as i32)
    .bind(prefs.retry_delay_seconds as i32)
    .execute(&mut *tx)
    .await;

    // 6. Update service priority order if provided
    for (idx, svc_name) in prefs.service_priority_order.iter().enumerate() {
        let _ = sqlx::query(
            "UPDATE service_preferences SET priority = ?, updated_at = CURRENT_TIMESTAMP WHERE service_name = ?"
        )
        .bind((idx + 1) as i32)
        .bind(svc_name)
        .execute(&mut *tx)
        .await;
    }

    // 7. Update service qualities via UPSERT
    for q in &prefs.service_qualities {
        let _ = sqlx::query(
            r#"INSERT INTO quality_preferences (service_name, max_quality, preferred_format, fallback_quality, fallback_format, updated_at)
               VALUES (?, ?, ?, ?, ?, datetime('now'))
               ON CONFLICT(service_name) DO UPDATE SET
                   max_quality = excluded.max_quality,
                   preferred_format = excluded.preferred_format,
                   fallback_quality = excluded.fallback_quality,
                   fallback_format = excluded.fallback_format,
                   updated_at = excluded.updated_at"#
        )
        .bind(&q.service_name)
        .bind(&q.max_quality)
        .bind(&q.preferred_format)
        .bind(&q.fallback_quality)
        .bind(&q.fallback_format)
        .execute(&mut *tx)
        .await;
    }

    // 8. Update settings key-value table
    let kv_pairs = vec![
        ("dl_download_path".to_string(), canonical_path.clone()),
        ("download_dir".to_string(), canonical_path.clone()),
        ("download_path".to_string(), canonical_path.clone()),
        ("dl_concurrent_downloads".to_string(), prefs.max_concurrent_downloads.to_string()),
        ("dl_retry_failed".to_string(), (prefs.max_retries > 0).to_string()),
        ("dl_retry_count".to_string(), prefs.max_retries.to_string()),
        ("dl_retry_delay".to_string(), (prefs.retry_delay_seconds * 1000).to_string()),
        ("dl_auto_download_favorites".to_string(), prefs.auto_download_favorites.to_string()),
        ("dl_generate_lyrics_lrc".to_string(), prefs.generate_lyrics_lrc.to_string()),
        ("dl_generate_cover_art".to_string(), prefs.generate_cover_art.to_string()),
        ("dl_generate_animated_cover".to_string(), prefs.generate_animated_cover.to_string()),
        ("dl_generate_booklet".to_string(), prefs.generate_booklet.to_string()),
        ("dl_generate_artist_sidecars".to_string(), prefs.generate_artist_sidecars.to_string()),
        ("dl_allow_downgrade".to_string(), (fallback_act != "skip").to_string()),
    ];

    for (k, v) in kv_pairs {
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now')) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at"
        )
        .bind(&k)
        .bind(&v)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Failed to save setting '{}': {}", k, e))?;
    }

    // Commit transaction
    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit preferences transaction: {}", e))?;

    // 9. Update runtime worker state
    let concurrency = (prefs.max_concurrent_downloads as usize).max(1);
    state.worker_state.set_max_concurrent(concurrency);

    resolve_effective_download_preferences(&state.db, &state.worker_state).await
}

/// Get canonical effective download preferences
#[tauri::command]
pub async fn get_effective_download_preferences(
    state: State<'_, AppState>,
) -> Result<EffectiveDownloadPreferences, String> {
    perform_get_effective_download_preferences(&state).await
}

/// Save canonical effective download preferences atomically
#[tauri::command]
pub async fn save_effective_download_preferences(
    state: State<'_, AppState>,
    preferences: EffectiveDownloadPreferences,
) -> Result<EffectiveDownloadPreferences, String> {
    perform_save_effective_download_preferences(&state, preferences).await
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

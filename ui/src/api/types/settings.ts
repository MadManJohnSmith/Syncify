// Barrel split from api/types.ts; tipos espejo de los comandos Rust.
// ==============================================
// SETTINGS TYPES
// ==============================================

export interface AppSettings {
    download_path: string;
    preferred_quality: string;
}

export type PathStatus = 'valid' | 'missing' | 'not_writable' | 'unavailable';

export interface DownloadSettingsDto {
    library_root: string;
    staging_root: string;
    path_status: PathStatus;
    free_space_bytes: number | null;
}

// ==============================================
// SPRINT 1: SERVICE PREFERENCES & SYNC SETTINGS
// ==============================================

export interface ServicePreference {
    id: number;
    service_name: string;
    priority: number;
    auto_import_enabled: boolean;
}

export interface SyncSettings {
    id: number;
    auto_sync_enabled: boolean;
    sync_interval_value: number;
    sync_interval_unit: 'minutes' | 'hours' | 'days';
    sync_on_startup: boolean;
    background_download: boolean;
    max_concurrent_downloads: number;
    rate_limit_delay_ms: number;
    pause_on_metered: boolean;
    pause_on_low_battery: boolean;
}

// #197 (SB02): `incremental_sync` retirado del contrato; la columna
// (DEFAULT 1) permanece en BD sin lectores.
export interface ServiceSyncSettings {
    id: number;
    service_name: string;
    sync_favorites: boolean;
    sync_playlists: boolean;
    sync_albums: boolean;
    last_synced: string | null;
}

// ==============================================
// SPRINT 2: DOWNLOADS + FILE SETTINGS
// ==============================================

export interface QualityPreference {
    id: number;
    service_name: string;
    max_quality: string;
    preferred_format: string;
    fallback_quality: string;
    fallback_format: string;
}

export interface EffectiveDownloadPreferences {
    downloadPath: string;
    stagingPath: string;
    pathStatus: PathStatus;
    freeSpaceBytes: number | null;
    maxQuality: string;
    preferredFormat: string;
    fallbackAction: string;
    allowDowngrade: boolean;
    strictQuality: boolean;
    preferredDownloadService: string | null;
    servicePriorityOrder: string[];
    serviceQualities: QualityPreference[];
    maxConcurrentDownloads: number;
    rateLimitDelayMs: number;
    maxRetries: number;
    retryDelaySeconds: number;
    generateLyricsLrc: boolean;
    generateCoverArt: boolean;
    generateAnimatedCover: boolean;
    generateBooklet: boolean;
    generateArtistSidecars: boolean;
    autoSyncEnabled: boolean;
    syncIntervalValue: number;
    syncIntervalUnit: 'minutes' | 'hours' | 'days';
    syncOnStartup: boolean;
    backgroundDownload: boolean;
    pauseOnMetered: boolean;
    pauseOnLowBattery: boolean;
    folderTemplate: string;
    fileTemplate: string;
    artistSeparator: string;
    replaceSpacesWith: string | null;
    maxPathLength: number;
}

export interface FolderSettings {
    id: number;
    base_folder: string;
    folder_template: string;
    file_template: string;
    artist_separator: string;
    replace_spaces_with: string | null;
    max_path_length: number;
    fallback_action: string;
}

export interface AudioProcessingSettings {
    id: number;
    replay_gain_mode: string;
    target_loudness_lufs: number;
    transcode_enabled: boolean;
    transcode_format: string;
    transcode_bitrate: number;
    keep_original_after_transcode: boolean;
    embed_lyrics: boolean;
    embed_artwork: boolean;
    artwork_max_size: number;
}

// ==============================================
// SPRINT 5: ADVANCED SETTINGS & POLISH
// ==============================================

export interface AdvancedSettings {
    id: number;
    // Logging settings
    log_level: string;
    log_to_file: boolean;
    log_file_max_size_mb: number;
    log_file_retention_days: number;
    // Worker settings
    max_concurrent_downloads: number;
    max_concurrent_imports: number;
    worker_timeout_seconds: number;
    // Cache settings  
    cache_enabled: boolean;
    cache_max_size_mb: number;
    cache_ttl_hours: number;
    // Matching settings
    fuzzy_match_threshold: number;
    use_acoustic_fingerprinting: boolean;
    prefer_exact_matches: boolean;
    // Network settings
    request_timeout_seconds: number;
    max_retries: number;
    retry_delay_seconds: number;
    use_proxy: boolean;
    proxy_url: string | null;
    // Debug settings
    debug_mode: boolean;
    verbose_api_logging: boolean;
}

export interface MetadataPreferences {
    id: number;
    enable_musicbrainz: boolean;
    enable_lastfm: boolean;
    enable_acoustid: boolean;
    overwrite_on_reimport: boolean;
    preserve_custom_tags: boolean;
    multi_value_separator: string;
    write_releasetype: boolean;
    write_label: boolean;
    write_work_composer: boolean;
    write_musicbrainz_ids: boolean;
    write_download_source: boolean;
    write_download_date: boolean;
    write_only_available_on: boolean;
    write_not_available_streaming: boolean;
    write_quality_score: boolean;
    write_lyrics_tags: boolean;
    weight_album: number;
    weight_isrc: number;
    weight_mb_id: number;
    weight_cover: number;
    weight_year: number;
    weight_genre: number;
}

export interface DiagnosticResult {
    check_name: string;
    status: string;
    message: string;
    duration_ms: number;
}

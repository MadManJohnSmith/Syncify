/**
 * API Type Definitions
 * 
 * TypeScript interfaces matching Rust command response types.
 */

// ==============================================
// LIBRARY TYPES
// ==============================================

export interface LibraryTrack {
    id: number;
    title: string;
    artist_name: string | null;
    artist_id: number | null;
    album_name: string | null;
    album_id: number | null;
    duration_ms: number | null;
    isrc: string | null;
    services: string | null;         // Comma-separated service names
    quality: string | null;          // e.g. "24/96", "16/44.1", "320kbps"
    download_status: string | null;  // "downloaded", "queued", "not_downloaded"
    metadata_score: number | null;   // 0-100 based on field completeness
    lyrics_type: string | null;      // "synced", "timed", "plain", "none"
    cover_art_url: string | null;    // Album artwork URL
    spotify_track_id?: string | null; // External Spotify ID
    // Extended metadata fields
    track_number: number | null;
    disc_number: number | null;
    genre: string | null;
    bpm: number | null;
    musical_key: string | null;
    release_year: number | null;
    explicit: boolean | null;
    file_path: string | null;
    musicbrainz_id: string | null;
}

export interface LibraryPage {
    tracks: LibraryTrack[];
    total: number;
    offset: number;
    limit: number;
    has_more: boolean;
}

export interface SearchResult {
    tracks: LibraryTrack[];
    total: number;
    offset: number;
    limit: number;
    has_more: boolean;
}

export interface LibraryStats {
    total_tracks: number;
    total_artists: number;
    total_albums: number;
    total_downloads: number;
    queued_downloads: number;
    active_downloads: number;
    library_entries: number;
    playlists: number;
    services_with_data: number;
}

export interface TopArtist {
    name: string;
    track_count: number;
}

export interface TopGenre {
    name: string;
    count: number;
}

export interface QualityBucket {
    label: string;
    count: number;
}

export interface Playlist {
    id: number;
    name: string;
    description: string | null;
    owner_name: string | null;
    track_count: number;
    image_url: string | null;
    service_name: string | null;
}

// ==============================================
// DOWNLOAD TYPES
// ==============================================

export interface DownloadItem {
    id: number;
    title: string;
    artist_name: string;
    status: 'queued' | 'downloading' | 'completed' | 'failed' | 'paused';
    progress_percent: number;
}

export interface QueueItem {
    id: number;
    track_id: number;
    title: string | null;      // Changed from track_title?: string
    artist: string | null;     // Changed from track_artist?: string
    service: string;
    quality: string;
    status: string;
    priority: number;
    progress_percent: number;
    error_message: string | null;
    created_at: string;
    started_at: string | null;
    completed_at: string | null;
}

export interface QueueStats {
    total: number;
    queued: number;
    downloading: number;
    completed: number;
    failed: number;
    paused: number;
}

export interface WorkerStatus {
    is_running: boolean;
    is_paused: boolean;
    current_downloads: number;
    max_concurrent: number;
    total_processed: number;
    total_failed: number;
}

// ==============================================
// SERVICE/ACCOUNT TYPES
// ==============================================

export interface Service {
    id: number;
    name: string;
    supports_download: number;
    max_quality: string | null;
    // Derived/optional fields for UI
    display_name?: string;
}

export interface Account {
    id: number;
    service_id: number;
    service_name: string;
    display_name: string;
    email: string | null;
    is_active: boolean;
    last_synced: string | null;
    created_at: string;
}

export interface ServiceStatus {
    name: string;
    connected: boolean;
    account_email: string | null;
    library_count: number;
    favorites_count: number;
    playlists_count: number;
    last_synced: string | null;
    credentials_invalid: boolean;
}

export interface SessionStatus {
    service: string;
    connected: boolean;
    valid: boolean;
    message: string;
    user_info: string | null;
}

// ==============================================
// AUTH TYPES
// ==============================================

export interface AuthResult {
    success: boolean;
    data: Record<string, unknown> | null;
    error: string | null;
}

export interface ImportResult {
    imported: number;
    skipped: number;
    errors: string[];
}

export interface UrlParseResult {
    service: string;
    content_type: string;
    id: string;
    url: string;
}

// ==============================================
// LYRICS TYPES
// ==============================================

export interface LyricsData {
    synced_lyrics: string | null;
    plain_lyrics: string | null;
    word_synced: boolean;
    instrumental: boolean | null;
    source: string | null;
}

export interface LyricsResult {
    success: boolean;
    data: LyricsData | null;
    error: string | null;
}

export interface LyricsStats {
    total_tracks: number;
    with_lyrics: number;
    synced_lyrics: number;
    embedded_lyrics: number;
}

// ==============================================
// METADATA TYPES
// ==============================================

export interface MetadataStats {
    total_tracks: number;
    with_isrc: number;
    with_musicbrainz_id: number;
    with_album: number;
    with_year: number;
    with_genre: number;
    with_art: number;
    average_completeness: number;
}

export interface MetadataResult {
    success: boolean;
    data: Record<string, unknown> | null;
    error: string | null;
}

// ==============================================
// FINGERPRINT TYPES
// ==============================================

export interface FingerprintResult {
    success: boolean;
    data: Record<string, unknown> | null;
    error: string | null;
}

// ==============================================
// BRIDGE RESULT (Generic)
// ==============================================

export interface BridgeResult {
    success: boolean;
    data: Record<string, unknown> | null;
    error: string | null;
}

// ==============================================
// PLAYLIST TYPES
// ==============================================

export interface Playlist {
    id: number;
    name: string;
    description: string | null;
    track_count: number;
    source_service: string | null;
    source_id: string | null;
    created_at: string;
    updated_at: string;
}

export interface PlaylistTrack {
    id: number;
    track_id: number;
    position: number;
    title: string;
    artist_name: string;
    album_name: string | null;
}

// ==============================================
// SETTINGS TYPES
// ==============================================

export interface AppSettings {
    download_path: string;
    preferred_quality: string;
    auto_download_favorites: boolean;
}

// ==============================================
// HEALTH CHECK TYPES
// ==============================================

export interface HealthCheck {
    database_ok: boolean;
    python_ok: boolean;
    ffmpeg_available: boolean;
    chromaprint_available: boolean;
    services_configured: string[];
    errors: string[];
}

// ==============================================
// PROGRESS EVENT TYPES
// ==============================================

export interface ProgressEvent {
    operation: string;
    id: string;
    status: 'started' | 'progress' | 'completed' | 'failed';
    current: number;
    total: number;
    percentage: number;
    message: string | null;
    data: Record<string, unknown> | null;
}

// ==============================================
// DEPENDENCY TYPES
// ==============================================

export interface DependencyStatus {
    name: string;
    available: boolean;
    version: string | null;
    required: boolean;
}

export interface DependencyCheckResult {
    all_available: boolean;
    dependencies: DependencyStatus[];
}

// ==============================================
// PLAYLIST TYPES
// ==============================================

export interface Playlist {
    id: number;
    account_id: number;
    service_playlist_id: string | null;
    name: string;
    description: string | null;
    is_public: boolean;
    track_count: number;
    last_synced: string | null;
    created_at: string;
}

// PlaylistTrack interface already exists at line ~195

// ==============================================
// LYRICS TYPES
// ==============================================

export interface Lyrics {
    id: number;
    track_id: number;
    format: 'ttml' | 'lrc' | 'plain';
    sync_level: 'syllable' | 'word' | 'line' | 'none' | null;
    source: string | null;
    content: string;
    language: string | null;
    embedded_in_file: boolean;
    created_at: string;
}

export interface LyricsSearchResult {
    id: string;
    source: string;
    title: string;
    artist: string;
    sync_level: string;
    confidence: number;
}

// ==============================================
// METADATA TYPES
// ==============================================

export interface Track {
    id: number;
    title: string;
    album_id: number | null;
    duration_ms: number | null;
    track_number: number | null;
    disc_number: number | null;
    isrc: string | null;
    musicbrainz_id: string | null;
    explicit: boolean;
    created_at: string;
    // Joined info
    artist_name?: string;
    album_name?: string;
}

export interface MetadataMatch {
    recording_id: string;
    title: string;
    artist: string;
    album: string | null;
    release_date: string | null;
    score: number;
    source: 'musicbrainz' | 'acoustid';
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

export interface ServiceSyncSettings {
    id: number;
    service_name: string;
    sync_favorites: boolean;
    sync_playlists: boolean;
    sync_albums: boolean;
    incremental_sync: boolean;
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

export interface DuplicateSettings {
    id: number;
    enable_detection: boolean;
    prefer_higher_quality: boolean;
    prefer_lossless: boolean;
    replace_same_quality_different_source: boolean;
    quality_threshold_kbps: number;
    delete_duplicates_immediately: boolean;
    move_to_trash: boolean;
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
// SPRINT 3: LYRICS TAB + SETTINGS
// ==============================================

export interface LyricsProviderSetting {
    id: number;
    provider_id: string;
    provider_name: string;
    enabled: boolean;
    priority: number;
    sync_level: string;
}

export interface LyricsConfig {
    id: number;
    min_sync_level: string;
    preferred_language: string;
    storage_format: string;
    auto_fetch_on_import: boolean;
    retry_failed: boolean;
    retry_frequency: string;
}

// ==============================================
// SPRINT 4: DASHBOARD + LIBRARY DETAIL VIEWS
// ==============================================

export interface LibrarySnapshot {
    id: number;
    snapshot_date: string;
    total_tracks: number;
    total_albums: number;
    total_artists: number;
    total_size_bytes: number;
    tracks_with_lyrics: number;
    tracks_lossless: number;
    tracks_hires: number;
    metadata_excellent: number;
    metadata_good: number;
    metadata_needs_work: number;
    downloaded_tracks: number;
}

export interface ServiceHealthInfo {
    id: number;
    service_name: string;
    is_connected: boolean;
    token_valid: boolean;
    token_expires_at: string | null;
    last_checked: string;
    error_message: string | null;
    rate_limit_remaining: number | null;
    rate_limit_reset_at: string | null;
}

export interface ArtistDetail {
    id: number;
    name: string;
    bio: string | null;
    image_url: string | null;
    album_count: number;
    track_count: number;
    albums: ArtistAlbum[];
    top_tracks: ArtistTrack[];
}

export interface ArtistAlbum {
    id: number;
    title: string;
    cover_url: string | null;
    release_year: number | null;
    track_count: number;
}

export interface ArtistTrack {
    id: number;
    title: string;
    album: string | null;
    duration_ms: number | null;
}

export interface AlbumDetail {
    id: number;
    title: string;
    artist_name: string | null;
    release_year: number | null;
    cover_art_url: string | null;
    track_count: number;
    total_duration_ms: number;
    genre: string | null;
    tracks: AlbumTrack[];
}

export interface AlbumTrack {
    id: number;
    title: string;
    artist_name: string | null;
    duration_ms: number | null;
    track_number: number | null;
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

export interface CacheStats {
    id: number;
    cache_type: string;
    size_bytes: number;
    item_count: number;
    hit_count: number;
    miss_count: number;
    last_updated: string;
}

export interface DiagnosticResult {
    check_name: string;
    status: string;
    message: string;
    duration_ms: number;
}

export interface LibrarySnapshot {
    id: number;
    snapshot_date: string;
    total_tracks: number;
    total_albums: number;
    total_artists: number;
    total_size_bytes: number;
    tracks_with_lyrics: number;
    tracks_lossless: number;
    tracks_hires: number;
    metadata_excellent: number;
    metadata_good: number;
    metadata_needs_work: number;
    downloaded_tracks: number;
}

// ==============================================
// SPRINT 6: MIGRATION TYPES
// ==============================================

export interface MigrationJob {
    id: string;
    source_service: string;
    destination_service: string;
    source_playlist_ids: string | null;
    options: string;
    status: string;
    total_items: number;
    completed_items: number;
    failed_items: number;
    skipped_items: number;
    started_at: string | null;
    completed_at: string | null;
    error_message: string | null;
    created_at: string;
}

export interface MigrationItem {
    id: number;
    job_id: string;
    source_track_id: string;
    source_track_title: string;
    source_track_artist: string;
    source_track_album: string | null;
    source_playlist_id: string | null;
    source_playlist_name: string | null;
    destination_track_id: string | null;
    match_confidence: number | null;
    match_method: string | null;
    status: string;
    error_message: string | null;
    processed_at: string | null;
    created_at: string;
}

export interface MigrationTemplate {
    id: number;
    name: string;
    description: string | null;
    source_service: string;
    destination_service: string;
    options: string;
    created_at: string;
    updated_at: string;
}

export interface MigrationOptions {
    match_threshold: number;
    skip_unmatched: boolean;
    create_playlists: boolean;
    merge_existing: boolean;
    download_matched: boolean;
}

export interface MigrationPreviewResult {
    total_tracks: number;
    matched_tracks: number;
    unmatched_tracks: number;
    playlists: PlaylistPreview[];
}

export interface PlaylistPreview {
    id: string;
    name: string;
    track_count: number;
    matched_count: number;
}

export interface MigrationProgress {
    job_id: string;
    current_item: number;
    total_items: number;
    current_track: string;
    status: string;
    completed_count: number;
    failed_count: number;
    skipped_count: number;
}

export interface DestinationTrackMatch {
    track_id: string;
    title: string;
    artist: string;
    album: string | null;
    duration_ms: number;
    quality: string | null;
    confidence: number;
}


// Barrel split from api/types.ts; tipos espejo de los comandos Rust.
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

/**
 * Wire contract of the Rust `LyricsSearchResult` (commands/lyrics.rs).
 * One entry per provider response returned by `search_lyrics`.
 */
export interface LyricsSearchResult {
    source: string;
    title: string;
    artist: string;
    album: string | null;
    duration_ms: number | null;
    synced_lyrics: string | null;   // LRC payload when the provider has timed lines
    plain_lyrics: string | null;    // Unsynced text fallback
    sync_type: string;              // 'LINE_SYNCED' | 'WORD_SYNCED' | 'NOT_SYNCED'
    instrumental: boolean;
}

/**
 * Wire contract of the Rust `BatchLyricsResult` (commands/lyrics.rs).
 * `cancelled` is the terminal state of the run, so a cancelled run is never
 * mistaken for a completed one.
 */
export interface BatchLyricsResult {
    fetched: number;
    failed: number;
    skipped: number;
    cancelled: boolean;
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

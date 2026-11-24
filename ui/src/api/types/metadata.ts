// Barrel split from api/types.ts; tipos espejo de los comandos Rust.
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
// INCREMENTAL ENRICHMENT TYPES (S144)
// ==============================================

export type EnrichmentMode = 'incomplete_only' | 'revalidate_all' | 'selection';

export type TrackEnrichmentStatus =
    | 'queued'
    | 'resolving'
    | 'enriching'
    | 'persisted'
    | 'skipped_complete'
    | 'skipped_precedence'
    | 'partial'
    | 'failed'
    | 'cancelled';

export type JobStatus = 'queued' | 'running' | 'completed' | 'cancelled' | 'failed';

export interface EnrichmentPreview {
    totalTracks: number;
    totalEligible: number;
    totalComplete: number;
    totalSkippedPrecedence: number;
    availableSources: string[];
    mode: EnrichmentMode;
}

export interface TrackEnrichmentReportItem {
    trackId: number;
    trackTitle: string;
    artistName: string;
    requestedFields: string[];
    modifiedFields: string[];
    previousProvenance: Record<string, string>;
    newProvenance: Record<string, string>;
    provider: string;
    confidence: number;
    durationMs: number;
    error?: string | null;
    status: TrackEnrichmentStatus;
}

export interface EnrichmentJobSummary {
    jobId: string;
    mode: EnrichmentMode;
    status: JobStatus;
    totalTracks: number;
    processedTracks: number;
    modifiedTracks: number;
    skippedCompleteTracks: number;
    skippedPrecedenceTracks: number;
    failedTracks: number;
    currentTrack?: string | null;
    currentPhase?: string | null;
    items: TrackEnrichmentReportItem[];
    availableSources: string[];
    startedAt: string;
    completedAt?: string | null;
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

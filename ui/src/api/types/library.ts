// Barrel split from api/types.ts; tipos espejo de los comandos Rust.
/**
 * API Type Definitions
 * 
 * TypeScript interfaces matching Rust command response types.
 */

// ==============================================
// LIBRARY TYPES
// ==============================================

export type SourceAvailabilityStatus = 'available' | 'stale_404' | 'region_unavailable' | 'requires_auth' | 'unknown_unchecked';

export type LyricsType = 'synced' | 'timed' | 'plain' | 'unsynced' | 'none';

export interface TrackSourceAvailability {
    id: number;
    trackId: number;
    serviceId: number;
    serviceName: string;
    serviceTrackId: string;
    format: string | null;
    bitDepth: number | null;
    sampleRate: number | null;
    qualityScore: number | null;
    available: number;
    availabilityStatus: SourceAvailabilityStatus;
    availabilityReason: string | null;
    lastChecked: string | null;
}

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
    imported_from?: string | null;   // Historical import provenance (e.g. "Spotify", "Qobuz")
    downloaded_from?: string | null; // Effective download provider (e.g. "Tidal")
    available_services?: string | null; // Verified available services
    availability_summary?: string | null; // Summary string of availability states
    sources?: TrackSourceAvailability[]; // Detailed per-source availability
    quality: string | null;          // e.g. "24/96", "16/44.1", "320kbps"
    quality_bucket?: string | null;  // FE-10: 'Hi-Res (24-bit+)' | 'CD Quality' | 'Lossy' (get_library only)
    download_status: string | null;  // "downloaded" | "queued" | "downloading" | "failed" | "not_downloaded"
    download_quality?: string | null; // L14: calidad efectiva del fichero, p. ej. "FLAC 24/96" (query_library)
    metadata_score: number | null;   // 0-100 based on field completeness
    lyrics_type: LyricsType | null;  // "synced", "timed", "plain", "unsynced", "none"
    cover_art_url: string | null;    // Album artwork URL
    spotify_track_id?: string | null; // External Spotify ID
    // PL02 (#155): posición de la ocurrencia dentro de la playlist. Distingue
    // las repeticiones de la misma pista; las consultas de biblioteca no la
    // proyectan (queda undefined).
    position?: number | null;
    // Extended metadata fields
    track_number: number | null;
    disc_number: number | null;
    genre: string | null;
    bpm: number | null;
    musical_key: string | null;
    release_year: number | null;
    explicit: boolean | null;
    is_favorite?: boolean | null;
    favorite_at?: string | null;
    // Route 2 (2026-10-18): identity of the favorited release row in the
    // expanded favorites list (get_favorite_tracks only).
    edition_item_id?: string | null;
    file_path: string | null;
    musicbrainz_id: string | null;
    display_title?: string | null;
    source_title?: string | null;
    file_disambiguator?: string | null;
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

// ==========================================================================
// Consulta de biblioteca v2 (L01, issue #98): filtros, orden y contadores
// se resuelven EN SERVIDOR. Tipos espejo de `commands/library_query.rs`.
// ==========================================================================

export type LibrarySortKey =
    | 'dateAdded'
    | 'title'
    | 'artist'
    | 'album'
    | 'duration'
    | 'quality'
    | 'metadataScore';

export type SortDirection = 'asc' | 'desc';

export type QuarantineMode = 'exclude' | 'only' | 'include';

export type CollectionFilter = 'favorite' | 'playlist' | 'album' | 'artist';

/** Los cinco estados del badge de descarga (L14, #109). */
export type DownloadStatusFilter = 'downloaded' | 'queued' | 'downloading' | 'failed' | 'not_downloaded';

/** Etiquetas del bucket de calidad, idénticas al deep-link FE-10 del dashboard. */
export type QualityBucketFilter = 'hi_res' | 'cd_quality' | 'lossy';

export type DuplicatesFilter = 'only' | 'exclude';

/**
 * Consulta de biblioteca: AND entre dimensiones, OR dentro de cada lista.
 * Cuenta y servicio se evalúan sobre la misma pertenencia cuando ambos
 * vienen. `album`/`artist` en `collections`, `duplicates` y cuarentena
 * `only`/`include` son contrato pendiente (L00/#97, L10/#73, L09/#72): el
 * servidor responde con error explícito, no con semántica inventada.
 */
export interface LibraryQuery {
    /** Texto libre (título, artista, álbum). Se combina con los filtros. */
    text: string | null;
    serviceIds: number[];
    accountIds: number[];
    collections: CollectionFilter[];
    favoritesOnly: boolean;
    downloadStatuses: DownloadStatusFilter[];
    qualityBuckets: QualityBucketFilter[];
    inPlaylists: boolean | null;
    duplicates: DuplicatesFilter | null;
    quarantine: QuarantineMode;
    genre: string[];
    sort: LibrarySortKey;
    direction: SortDirection;
    offset: number;
    /** Default 100, máximo 200 (recortado en servidor). */
    limit: number | null;
}

export interface LibraryServiceCount {
    serviceId: number;
    serviceName: string;
    count: number;
}

/** Pertenencias por cuenta: puede superar el total de pistas distintas. */
export interface LibraryAccountCount {
    accountId: number;
    displayName: string | null;
    serviceName: string;
    count: number;
}

/**
 * Contadores de cabecera. Cada contador se calcula sobre el filtro actual
 * SIN su propia dimensión, para que el chip coincida con el total al
 * aplicarlo. Solo en la primera página (`offset == 0`).
 */
export interface LibraryQueryCounts {
    total: number;
    favorites: number;
    downloaded: number;
    inPlaylists: number;
    /** Álbumes distintos con al menos una pista viva en el filtro (L07). */
    albums: number;
    /** Artistas distintos con al menos una pista viva en el filtro (L08). */
    artists: number;
    byService: LibraryServiceCount[];
    byAccount: LibraryAccountCount[];
}

export interface LibraryQueryResult extends LibraryPage {
    /** `null` al paginar: la vista conserva los de la primera página. */
    counts: LibraryQueryCounts | null;
}

// ==========================================================================
// Pestaña de álbumes (L07, issue #104). Espejo de
// `commands/library_albums.rs`.
// ==========================================================================

export type LibraryAlbumSortKey = 'dateAdded' | 'title' | 'artist' | 'year';

/**
 * Cobertura de descarga del álbum: todas las pistas, algunas o ninguna.
 * La cobertura se calcula sobre las pistas vivas del filtro.
 */
export type AlbumCoverageFilter = 'complete' | 'partial' | 'none';

export interface LibraryAlbumQuery {
    text: string | null;
    serviceIds: number[];
    accountIds: number[];
    coverage: AlbumCoverageFilter[];
    sort: LibraryAlbumSortKey;
    direction: SortDirection;
    offset: number;
    /** Default 100, máximo 200 (recortado en servidor). */
    limit: number | null;
}

export interface LibraryAlbumAccountBadge {
    accountId: number;
    displayName: string | null;
    serviceName: string;
}

export interface LibraryAlbumServiceBadge {
    serviceId: number;
    serviceName: string;
}

export interface LibraryAlbumRow {
    albumId: number;
    title: string;
    artistName: string | null;
    releaseYear: number | null;
    /** Pistas distintas con pertenencia viva en el filtro. */
    trackCount: number;
    downloadedCount: number;
    /** L14 (#109): los otros estados del mismo badge de descarga. */
    queuedCount: number;
    downloadingCount: number;
    failedCount: number;
    notDownloadedCount: number;
    /** `downloadedCount / trackCount`; `null` si el total es cero. */
    coverage: number | null;
    accounts: LibraryAlbumAccountBadge[];
    services: LibraryAlbumServiceBadge[];
    dateAdded: string | null;
    coverArtUrl: string | null;
}

export interface LibraryAlbumsResult {
    albums: LibraryAlbumRow[];
    /** Álbumes distintos con al menos una pista viva en el filtro. */
    total: number;
    offset: number;
    limit: number;
    hasMore: boolean;
}

// ==========================================================================
// Pestaña de artistas (L08, issue #105). Espejo de
// `commands/library_artists.rs`.
// ==========================================================================

export type LibraryArtistSortKey = 'name' | 'tracks' | 'dateAdded';

export interface LibraryArtistQuery {
    text: string | null;
    serviceIds: number[];
    accountIds: number[];
    sort: LibraryArtistSortKey;
    direction: SortDirection;
    offset: number;
    /** Default 100, máximo 200 (recortado en servidor). */
    limit: number | null;
}

export interface LibraryArtistAccountBadge {
    accountId: number;
    displayName: string | null;
    serviceName: string;
}

export interface LibraryArtistServiceBadge {
    serviceId: number;
    serviceName: string;
}

export interface LibraryArtistRow {
    artistId: number;
    name: string;
    imageUrl: string | null;
    /** Pistas distintas con pertenencia viva en el filtro. */
    trackCount: number;
    /** Álbumes distintos de esas pistas. */
    albumCount: number;
    /** Pistas vivas del artista con al menos otro artista. */
    collaborationCount: number;
    accounts: LibraryArtistAccountBadge[];
    services: LibraryArtistServiceBadge[];
    dateAdded: string | null;
}

export interface LibraryArtistsResult {
    artists: LibraryArtistRow[];
    /** Artistas distintos con al menos una pista viva en el filtro. */
    total: number;
    offset: number;
    limit: number;
    hasMore: boolean;
}

/** Fuente de una pista del detalle de artista (cross-service visible). */
export interface ArtistTrackSource {
    serviceId: number;
    serviceName: string;
}

/** Pista viva del detalle: colaboradores y fuentes por servicio. */
export interface ArtistTrackRow {
    id: number;
    title: string;
    album: string | null;
    albumId: number | null;
    durationMs: number | null;
    collaborators: string[];
    sources: ArtistTrackSource[];
}

export interface ArtistTracksQuery {
    artistId: number;
    offset: number;
    /** Default 100, máximo 200 (recortado en servidor). */
    limit: number | null;
}

export interface ArtistTracksResult {
    tracks: ArtistTrackRow[];
    /** Pistas distintas vivas del artista (independiente de la página). */
    total: number;
    offset: number;
    limit: number;
    hasMore: boolean;
}

// ==========================================================================
// Acciones por lotes (L05, issue #102). Espejo de
// `commands/library_actions.rs`.
// ==========================================================================

/**
 * Selección con dos modos explícitos. `ids` es literal; `filter` es «todo el
 * filtro» (el servidor ignora offset/limit) menos `excludedIds`.
 */
export type LibrarySelection =
    | { mode: 'ids'; ids: number[] }
    | { mode: 'filter'; query: LibraryQuery; excludedIds: number[] };

/**
 * Acciones por lotes. Las destructivas (`delete_audio`, `restore`,
 * `quarantine_touch`) existen solo en el contrato del servidor para rechazarlas:
 * son por pista y con confirmación, nunca por lote.
 */
export type LibraryBatchAction =
    | {
          action: 'queue_download';
          priority?: number | null;
          qualityPreference?: string | null;
          allowFallback?: boolean | null;
      }
    | { action: 'set_favorite'; favorite: boolean; accountId?: number | null }
    | { action: 'add_to_playlist'; playlistId: number };

export interface LibraryActionOmission {
    /** Clave estable (`already_downloaded`, `already_queued`, ...). */
    reason: string;
    message: string;
    count: number;
}

export interface LibraryActionPreview {
    action: string;
    label: string;
    selectionMode: string;
    total: number;
    eligible: number;
    omitted: number;
    omissions: LibraryActionOmission[];
}

export interface LibraryActionBlockError {
    block: number;
    message: string;
    tracks: number;
}

export interface LibraryActionSummary {
    action: string;
    label: string;
    selectionMode: string;
    total: number;
    processed: number;
    eligible: number;
    succeeded: number;
    failed: number;
    skipped: number;
    omissions: LibraryActionOmission[];
    blocks: number;
    blocksFailed: number;
    errors: LibraryActionBlockError[];
    /** `true` si el servidor recalculó porque la biblioteca cambió. */
    recalculated: boolean;
}

export interface LibraryActionProgress {
    action: string;
    total: number;
    processed: number;
    succeeded: number;
    failed: number;
    skipped: number;
    blocksDone: number;
    blocksTotal: number;
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
    albums: ArtistAlbum[];
    /** Primera página de la misma consulta que pagina el detalle (L08). */
    top_tracks: ArtistTrackRow[];
    /**
     * Pistas vivas distintas del artista (todas las páginas). Único contador
     * de pistas del detalle: el viejo `track_count` (filas de `track_artists`,
     * con roles duplicados y pistas no vivas) se retiró en #336.
     */
    tracks_total?: number;
}

export interface ArtistAlbum {
    id: number;
    title: string;
    cover_url: string | null;
    release_year: number | null;
    track_count: number;
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
    disc_number?: number | null;
}

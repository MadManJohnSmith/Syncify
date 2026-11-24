// Barrel split from api/types.ts; tipos espejo de los comandos Rust.
export interface Playlist {
    id: number;
    name: string;
    description: string | null;
    track_count: number;
    owner_name?: string | null;
    image_url?: string | null;
    service_name?: string | null;
    source_service?: string | null;
    source_id?: string | null;
    account_id?: number | null;
    service_playlist_id?: string | null;
    is_public?: boolean;
    last_synced?: string | null;
    created_at?: string | null;
    updated_at?: string | null;
    tracks?: PlaylistTrack[];
    is_smart?: boolean;
    rules_json?: string | null;
}

// ==============================================
// PLAYLIST TYPES
// ==============================================

export interface PlaylistTrack {
    id: number;
    track_id: number;
    position: number;
    title: string;
    artist_name: string;
    album_name: string | null;
}

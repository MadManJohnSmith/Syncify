import type { Playlist } from '@/api/types';

export interface PlaylistMosaicTile {
    src: string;
    itemId: number;
}

export interface PlaylistCoverTrack {
    id: number;
    position?: number | null;
    cover_art_url?: string | null;
}

export interface PlaylistMosaicCacheEntry {
    signature: string;
    tiles: PlaylistMosaicTile[];
}

export type PlaylistMosaicCache = Map<number, PlaylistMosaicCacheEntry>;

function membershipSignature(tracks: readonly PlaylistCoverTrack[]): string {
    return JSON.stringify(tracks.map(track => [
        track.id,
        track.position ?? null,
        track.cover_art_url ?? null,
    ]));
}

/** Reutiliza el mosaico hasta que cambia el orden o la pertenencia efectiva. */
export function reconcilePlaylistMosaic(
    cache: PlaylistMosaicCache,
    playlistId: number,
    tracks: readonly PlaylistCoverTrack[],
): PlaylistMosaicTile[] {
    const signature = membershipSignature(tracks);
    const cached = cache.get(playlistId);
    if (cached?.signature === signature) return cached.tiles;

    const tiles = buildPlaylistMosaic(tracks);
    cache.set(playlistId, { signature, tiles });
    return tiles;
}

/** Primeras cuatro portadas distintas, en el orden canónico de la playlist. */
export function buildPlaylistMosaic(
    tracks: readonly PlaylistCoverTrack[],
): PlaylistMosaicTile[] {
    const seen = new Set<string>();
    const tiles: PlaylistMosaicTile[] = [];

    for (const track of tracks) {
        const src = track.cover_art_url?.trim();
        if (!src || seen.has(src)) continue;
        seen.add(src);
        tiles.push({ src, itemId: track.id });
        if (tiles.length === 4) break;
    }

    return tiles;
}

export type PlaylistCategory = 'local' | 'service' | 'smart';

export interface PlaylistPresentation {
    /** Imagen persistida del proveedor; el fallback de `get_playlists` no cuenta. */
    providerImageUrl?: string | null;
    tiles?: PlaylistMosaicTile[];
}

export interface PlaylistListItem extends Playlist {
    category: PlaylistCategory;
    serviceKey: string | null;
    coverImageUrl: string | null;
    mosaicTiles: PlaylistMosaicTile[];
}

export interface PlaylistServiceGroup {
    id: string;
    name: string;
    playlists: PlaylistListItem[];
}

export interface PlaylistCatalog {
    local: PlaylistListItem[];
    services: PlaylistServiceGroup[];
    smart: PlaylistListItem[];
}

const SERVICE_NAMES: Record<string, string> = {
    spotify: 'Spotify',
    qobuz: 'Qobuz',
    tidal: 'Tidal',
    deezer: 'Deezer',
    apple_music: 'Apple Music',
    soundcloud: 'SoundCloud',
};

function normalizeService(value: string | null | undefined): string | null {
    const normalized = value?.trim().toLowerCase().replace(/[\s-]+/g, '_');
    return normalized && normalized !== 'local' ? normalized : null;
}

export function getPlaylistServiceKey(playlist: Playlist): string | null {
    return normalizeService(playlist.source_service) ?? normalizeService(playlist.service_name);
}

export function formatPlaylistServiceName(key: string): string {
    return SERVICE_NAMES[key] ?? key.split('_').map(part => part.charAt(0).toUpperCase() + part.slice(1)).join(' ');
}

export function aggregatePlaylists(
    playlists: readonly Playlist[],
    presentation: Readonly<Record<number, PlaylistPresentation>>,
): PlaylistCatalog {
    const local: PlaylistListItem[] = [];
    const smart: PlaylistListItem[] = [];
    const services = new Map<string, PlaylistListItem[]>();

    for (const playlist of playlists) {
        const serviceKey = playlist.is_smart ? null : getPlaylistServiceKey(playlist);
        const category: PlaylistCategory = playlist.is_smart ? 'smart' : serviceKey ? 'service' : 'local';
        const artwork = presentation[playlist.id];
        const providerImageUrl = category === 'service' ? (artwork?.providerImageUrl?.trim() || null) : null;
        const item: PlaylistListItem = {
            ...playlist,
            category,
            serviceKey,
            coverImageUrl: providerImageUrl,
            mosaicTiles: artwork?.tiles ?? [],
        };

        if (category === 'smart') {
            smart.push(item);
        } else if (serviceKey) {
            const group = services.get(serviceKey) ?? [];
            group.push(item);
            services.set(serviceKey, group);
        } else {
            local.push(item);
        }
    }

    return {
        local,
        services: Array.from(services, ([id, groupPlaylists]) => ({
            id,
            name: formatPlaylistServiceName(id),
            playlists: groupPlaylists,
        })).sort((a, b) => a.name.localeCompare(b.name)),
        smart,
    };
}

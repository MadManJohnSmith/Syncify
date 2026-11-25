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

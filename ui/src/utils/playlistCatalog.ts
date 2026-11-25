export interface PlaylistMosaicTile {
    src: string;
    itemId: number;
}

export interface PlaylistCoverTrack {
    id: number;
    position?: number | null;
    cover_art_url?: string | null;
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

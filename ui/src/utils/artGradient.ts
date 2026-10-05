/**
 * Deterministic fallback artwork for rows that have no cover URL.
 *
 * Shared by every view so a track looks the same in Library, Playlists,
 * Downloads, Search, Now Playing and the detail views: the gradient is derived
 * from the track id instead of being random, so the same track keeps its color
 * across screens and across reloads.
 */

const GRADIENTS = [
    'bg-gradient-to-br from-purple-500 to-pink-500',
    'bg-gradient-to-br from-blue-500 to-cyan-500',
    'bg-gradient-to-br from-green-500 to-emerald-500',
    'bg-gradient-to-br from-orange-500 to-red-500',
    'bg-gradient-to-br from-yellow-500 to-amber-500',
    'bg-gradient-to-br from-indigo-500 to-purple-500',
    'bg-gradient-to-br from-rose-500 to-pink-500',
    'bg-gradient-to-br from-teal-500 to-green-500',
];

/**
 * Gradiente estable para un id. Los ids no numéricos caen en el primero: un id
 * ausente es un hueco de datos, no una razón para renderizar `undefined`.
 */
export function getArtGradient(id: number | string | null | undefined): string {
    const numericId = typeof id === 'number' ? id : Number.parseInt(String(id ?? ''), 10);
    const index = Number.isFinite(numericId) && numericId >= 0 ? Math.trunc(numericId) % GRADIENTS.length : 0;
    return GRADIENTS[index];
}
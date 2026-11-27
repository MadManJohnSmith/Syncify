/**
 * P10b (#409): envoltorios IPC de la búsqueda de catálogo.
 *
 * El gateway de #402 vive en Rust y reparte la búsqueda entre proveedores; la
 * vista solo consume estos envoltorios. `catalog_album_tracks` y
 * `catalog_artist_albums` son fichas acotadas: piden al gateway la página de
 * búsqueda y el backend la filtra por el álbum o el artista abierto. Navegar
 * el catálogo completo sigue fuera de alcance (ADR-0010).
 */

import { invokeCommand } from './tauri';
import type {
    CatalogAlbum,
    CatalogSearchResponse,
    CatalogSection,
    CatalogTrack,
} from './types/catalogSearch';

export type {
    CatalogAlbum,
    CatalogArtist,
    CatalogFailureKind,
    CatalogPage,
    CatalogProviderSearch,
    CatalogSearchFailure,
    CatalogSearchResponse,
    CatalogSection,
    CatalogTrack,
} from './types/catalogSearch';

/** Tamaño de página por defecto del gateway (`DEFAULT_CATALOG_PAGE_SIZE`). */
export const CATALOG_PAGE_SIZE = 20;

/**
 * Busca en todos los proveedores registrados. Un proveedor caído viaja en su
 * entrada con un fallo por sección: no lanza ni oculta a los sanos.
 */
export function catalogSearchProviders(
    query: string,
    offset = 0,
    limit = CATALOG_PAGE_SIZE,
): Promise<CatalogSearchResponse> {
    return invokeCommand<CatalogSearchResponse>('catalog_search', { query, offset, limit });
}

/** Pistas de un álbum del catálogo, filtradas por el título abierto. */
export function catalogAlbumTracks(
    provider: string,
    title: string,
    artist?: string | null,
): Promise<CatalogSection<CatalogTrack>> {
    return invokeCommand<CatalogSection<CatalogTrack>>('catalog_album_tracks', {
        provider,
        title,
        artist: artist ?? undefined,
    });
}

/** Álbumes de un artista del catálogo, filtrados por el nombre abierto. */
export function catalogArtistAlbums(
    provider: string,
    name: string,
): Promise<CatalogSection<CatalogAlbum>> {
    return invokeCommand<CatalogSection<CatalogAlbum>>('catalog_artist_albums', {
        provider,
        name,
    });
}

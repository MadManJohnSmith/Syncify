/**
 * P10b (#409): tipos del gateway de búsqueda de catálogo (#402).
 *
 * Reflejan el payload snake_case de `services::catalog_search` tal como sale
 * por IPC. Las secciones son excluyentes: una página de resultados O un fallo
 * del proveedor, nunca ambos — así la vista no convierte «no pude consultar»
 * en «no hay resultados».
 */

export type CatalogFailureKind =
    | 'auth_expired'
    | 'rate_limited'
    | 'retryable'
    | 'permanent'
    | 'unsupported'
    | 'no_account';

/** Fallo ya atribuido a un proveedor concreto. */
export interface CatalogSearchFailure {
    provider: string;
    kind: CatalogFailureKind;
    message: string;
    /** Solo en `rate_limited`: segundos que el proveedor pidió esperar. */
    retry_after_sec?: number;
}

export interface CatalogPage<T> {
    items: T[];
    /** `null` cuando el proveedor no publica un total; nunca se inventa. */
    total: number | null;
    offset: number;
    limit: number;
}

/** Página o fallo de una de las tres secciones de un proveedor. */
export interface CatalogSection<T> {
    page?: CatalogPage<T>;
    error?: CatalogSearchFailure;
}

export interface CatalogTrack {
    /** Id del ítem en el proveedor; no es un `tracks.id` de biblioteca. */
    id: string;
    title: string;
    artist: string;
    album: string | null;
    isrc: string | null;
    duration_ms: number;
    bit_depth: number | null;
    sample_rate: number | null;
}

export interface CatalogAlbum {
    id: string;
    title: string;
    artist: string | null;
    track_count: number | null;
    /** Marca temporal Unix que publica Qobuz; `null` si no la trae. */
    released_at: number | null;
    cover_url: string | null;
}

export interface CatalogArtist {
    id: string;
    name: string;
    image_url: string | null;
    album_count: number | null;
}

/** Resultado de un proveedor para una búsqueda. */
export interface CatalogProviderSearch {
    provider: string;
    /** Id de `services`; la entrada efímera de cola lo necesita. */
    service_id: number | null;
    account_id: number | null;
    tracks: CatalogSection<CatalogTrack>;
    albums: CatalogSection<CatalogAlbum>;
    artists: CatalogSection<CatalogArtist>;
}

export interface CatalogSearchRequestPage {
    offset: number;
    limit: number;
}

export interface CatalogSearchResponse {
    query: string;
    page: CatalogSearchRequestPage;
    /** Todos los proveedores registrados, incluidos los que fallaron. */
    providers: CatalogProviderSearch[];
}

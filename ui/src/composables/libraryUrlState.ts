/**
 * libraryUrlState.ts — L03 (#100, spec #96): el estado de la vista de
 * biblioteca vive en la URL (deep-linking) y el armazón de pestañas comparte
 * los filtros compatibles.
 *
 * Módulo PURO (sin invoke, sin refs): la vista traduce su estado a query de
 * URL con `buildLibraryUrlQuery` y lo restaura con `parseLibraryUrlQuery`.
 * Ninguna vista llama a `invoke`: la consulta sigue entrando solo por
 * `ui/src/api/library.ts`.
 *
 * Dimensiones EN URL hoy (las que la vista ya dirige):
 *   q       — texto (búsqueda FTS de L02)
 *   sort    — clave de orden
 *   dir     — dirección (solo afecta a la clave primaria)
 *   dl      — estados de descarga (coma: downloaded, queued, not_downloaded)
 *   quality — buckets de calidad (coma: hi_res, cd_quality, lossy)
 *   fav     — solo favoritos («1»)
 *
 * Dimensiones FUERA de URL a propósito (no se simula): servicios, cuentas,
 * colecciones (más allá de favoritos), «en playlist», cuarentena y género.
 * No tienen control en la vista todavía —llegan con L00/#97, L09/#72 y
 * L11/#77— y el servidor responde con error explícito si se envían las
 * pendientes; escribir el parámetro sin semántica sería inventar estado.
 * Sus parámetros, si alguien los enlaza a mano, se recogen en
 * `unsupported` y NO se aplican.
 *
 * Compatibilidad FE-10: los deep-links del dashboard
 * `/library?filter=quality&quality=<etiqueta>` y `/library?filter=duplicates`
 * se siguen parseando (la etiqueta de calidad se mapea a bucket; la de
 * duplicados se reporta como unsupported, como hasta ahora).
 *
 * El deep-link `?track=<id>` es de L04 (panel): esta capa lo ignora y la
 * vista lo consume aparte, sin duplicar esa lógica.
 */
import type {
    DownloadStatusFilter,
    LibrarySortKey,
    QualityBucketFilter,
    SortDirection,
} from '@/api/types';

/** Claves de query gestionadas por esta capa (las demás no se tocan). */
export const LIBRARY_URL_KEYS = {
    text: 'q',
    sort: 'sort',
    direction: 'dir',
    downloadStatuses: 'dl',
    qualityBuckets: 'quality',
    favoritesOnly: 'fav',
} as const;

export type LibraryUrlKey = (typeof LIBRARY_URL_KEYS)[keyof typeof LIBRARY_URL_KEYS];

/** Estado de la vista que se refleja en la URL (solo lo que hoy existe). */
export interface LibraryUrlState {
    text: string;
    sort: LibrarySortKey;
    direction: SortDirection;
    downloadStatuses: DownloadStatusFilter[];
    qualityBuckets: QualityBucketFilter[];
    favoritesOnly: boolean;
}

export const LIBRARY_URL_DEFAULTS: LibraryUrlState = {
    text: '',
    sort: 'dateAdded',
    direction: 'desc',
    downloadStatuses: [],
    qualityBuckets: [],
    favoritesOnly: false,
};

const DOWNLOAD_STATUS_VALUES: readonly DownloadStatusFilter[] = [
    'downloaded',
    'queued',
    'not_downloaded',
];

const QUALITY_BUCKET_VALUES: readonly QualityBucketFilter[] = [
    'hi_res',
    'cd_quality',
    'lossy',
];

const SORT_VALUES: readonly LibrarySortKey[] = [
    'dateAdded',
    'title',
    'artist',
    'album',
    'duration',
    'quality',
    'metadataScore',
];

/** Etiquetas FE-10 del dashboard ('Hi-Res (24-bit+)' etc.) → bucket del contrato. */
const LEGACY_QUALITY_LABELS: Record<string, QualityBucketFilter> = {
    'Hi-Res (24-bit+)': 'hi_res',
    'CD Quality': 'cd_quality',
    Lossy: 'lossy',
};

/** Bucket → etiqueta FE-10, para que la barra de filtros activos siga igual. */
export const QUALITY_BUCKET_LABELS: Record<QualityBucketFilter, string> = {
    hi_res: 'Hi-Res (24-bit+)',
    cd_quality: 'CD Quality',
    lossy: 'Lossy',
};

/**
 * Parámetros de dimensiones pendientes: si llegan por deep-link se listan en
 * `unsupported` y no se aplican (el contrato del servidor los rechaza con
 * error explícito; no hay semántica que inventar todavía).
 */
const PENDING_DIMENSION_KEYS = [
    'services',
    'accounts',
    'collections',
    'inPlaylist',
    'in_playlist',
    'pl',
    'quarantine',
    'genre',
    'duplicates',
] as const;

export interface ParsedLibraryUrl {
    state: LibraryUrlState;
    /** Parámetros de dimensiones pendientes que llegaron y NO se aplicaron. */
    unsupported: string[];
    /** FE-10: etiqueta de calidad tal cual llegó, para mostrarla en la barra. */
    legacyQualityLabel: string | null;
}

type QueryValue = string | null | undefined | (string | null)[];

function firstString(value: QueryValue): string {
    if (Array.isArray(value)) {
        return typeof value[0] === 'string' ? value[0] : '';
    }
    return typeof value === 'string' ? value : '';
}

function parseList(value: QueryValue, allowed: readonly string[]): string[] {
    const raw = firstString(value);
    const out: string[] = [];
    for (const token of raw.split(',')) {
        if (allowed.includes(token) && !out.includes(token)) {
            out.push(token);
        }
    }
    return out;
}

function parseQualityToken(value: QueryValue): { buckets: QualityBucketFilter[]; legacyLabel: string | null } {
    const raw = firstString(value);
    const buckets: QualityBucketFilter[] = [];
    let legacyLabel: string | null = null;
    for (const token of raw.split(',')) {
        if (!token) continue;
        if (QUALITY_BUCKET_VALUES.includes(token as QualityBucketFilter)) {
            if (!buckets.includes(token as QualityBucketFilter)) {
                buckets.push(token as QualityBucketFilter);
            }
            continue;
        }
        const bucket = LEGACY_QUALITY_LABELS[token];
        if (bucket) {
            if (!buckets.includes(bucket)) buckets.push(bucket);
            legacyLabel = token;
        }
        // Un token de calidad desconocido no activa nada: sin semántica no
        // se inventa bucket.
    }
    return { buckets, legacyLabel };
}

/**
 * Parsea el query de la ruta al estado de la vista. Valores inválidos se
 * caen al default (un enlace a mano no rompe la vista); las dimensiones
 * pendientes se reportan sin aplicarse.
 */
export function parseLibraryUrlQuery(query: Record<string, unknown> | null | undefined): ParsedLibraryUrl {
    const unsupported: string[] = [];
    if (query) {
        for (const key of PENDING_DIMENSION_KEYS) {
            if (query[key] !== undefined) unsupported.push(key);
        }
    }

    const { buckets: legacyBuckets, legacyLabel } = parseQualityToken(query?.[LIBRARY_URL_KEYS.qualityBuckets] as QueryValue);

    // Compatibilidad FE-10: `?filter=<pill-id>` con ids de píldora vigentes.
    // `filter=quality` ya se resolvió arriba (calidad comparte la clave
    // `quality`); los ids de píldoras deshabilitadas se reportan.
    const legacyFilter = firstString(query?.filter as QueryValue);
    if (['duplicates', 'multiSource'].includes(legacyFilter)) {
        if (!unsupported.includes('filter')) unsupported.push('filter');
    }

    const downloadRaw = parseList(query?.[LIBRARY_URL_KEYS.downloadStatuses] as QueryValue, DOWNLOAD_STATUS_VALUES);
    if (legacyFilter === 'downloaded' && !downloadRaw.includes('downloaded')) {
        downloadRaw.push('downloaded');
    }
    if (legacyFilter === 'notDownloaded') {
        for (const status of ['queued', 'not_downloaded'] as const) {
            if (!downloadRaw.includes(status)) downloadRaw.push(status);
        }
    }

    let qualityBuckets = legacyBuckets;
    if (legacyFilter === 'flac' && !qualityBuckets.includes('hi_res')) {
        qualityBuckets = ['hi_res', 'cd_quality', ...qualityBuckets];
    }
    if (legacyFilter === 'mp3' && !qualityBuckets.includes('lossy')) {
        qualityBuckets = [...qualityBuckets, 'lossy'];
    }

    const sortRaw = firstString(query?.[LIBRARY_URL_KEYS.sort] as QueryValue);
    const sort = SORT_VALUES.includes(sortRaw as LibrarySortKey) ? (sortRaw as LibrarySortKey) : LIBRARY_URL_DEFAULTS.sort;

    const dirRaw = firstString(query?.[LIBRARY_URL_KEYS.direction] as QueryValue);
    const direction: SortDirection = dirRaw === 'asc' ? 'asc' : 'desc';

    const favRaw = firstString(query?.[LIBRARY_URL_KEYS.favoritesOnly] as QueryValue);
    const legacyFavorites = legacyFilter === 'favorites';

    return {
        state: {
            text: firstString(query?.[LIBRARY_URL_KEYS.text] as QueryValue),
            sort,
            direction,
            // Orden canónico (el de la vista) para un round-trip estable.
            downloadStatuses: DOWNLOAD_STATUS_VALUES.filter((s) => downloadRaw.includes(s)),
            // Orden canónico del contrato para que el round-trip
            // URL → vista → URL sea estable.
            qualityBuckets: QUALITY_BUCKET_VALUES.filter((b) => qualityBuckets.includes(b)),
            favoritesOnly: favRaw === '1' || favRaw === 'true' || legacyFavorites,
        },
        unsupported,
        legacyQualityLabel: legacyLabel,
    };
}

/**
 * Estado → query de URL. Los valores por defecto NO se escriben: un enlace
 * limpio `/library` es el estado vacío, y una dimensión que vuelve a su
 * default desaparece de la URL.
 */
export function buildLibraryUrlQuery(state: LibraryUrlState): Record<string, string> {
    const out: Record<string, string> = {};
    if (state.text) out[LIBRARY_URL_KEYS.text] = state.text;
    if (state.sort !== LIBRARY_URL_DEFAULTS.sort) out[LIBRARY_URL_KEYS.sort] = state.sort;
    if (state.direction !== LIBRARY_URL_DEFAULTS.direction) out[LIBRARY_URL_KEYS.direction] = state.direction;
    if (state.downloadStatuses.length) out[LIBRARY_URL_KEYS.downloadStatuses] = state.downloadStatuses.join(',');
    if (state.qualityBuckets.length) out[LIBRARY_URL_KEYS.qualityBuckets] = state.qualityBuckets.join(',');
    if (state.favoritesOnly) out[LIBRARY_URL_KEYS.favoritesOnly] = '1';
    return out;
}

// ==========================================================================
// Armazón de pestañas (spec #96: «Cabecera, pestañas y navegación»)
// ==========================================================================

export type LibraryTabId = 'tracks' | 'albums' | 'artists' | 'duplicates';

export interface LibraryTabMeta {
    id: LibraryTabId;
    path: string;
    label: string;
    /** Qué carril completa la pestaña (el armazón no simula contenido). */
    completesWith: string;
}

export const LIBRARY_TABS: readonly LibraryTabMeta[] = [
    { id: 'tracks', path: '/library', label: 'Tracks', completesWith: '' },
    { id: 'albums', path: '/library/albums', label: 'Albums', completesWith: 'L07 (#104)' },
    { id: 'artists', path: '/library/artists', label: 'Artists', completesWith: 'L08 (#105)' },
    { id: 'duplicates', path: '/library/duplicates', label: 'Duplicates', completesWith: 'L10 (#73)' },
];

/**
 * Query que sobrevive al cambio de pestaña: SOLO los filtros compatibles.
 * Las pestañas comparten texto y orden (query_library_albums/artists
 * comparten texto, filtros y paginación con query_library); las dimensiones
 * de pista —estado de descarga, calidad, favoritos y deep-links legacy—
 * no aplican a listas de álbumes, artistas o clústeres y se descartan.
 * Las pendientes (cuarentena, duplicados, servicios, cuentas) no llegan
 * nunca a la URL, así que no hay nada que descartar de ellas.
 */
export function libraryTabQuery(target: LibraryTabId, query: Record<string, unknown> | null | undefined): Record<string, string> {
    void target;
    const { state } = parseLibraryUrlQuery(query);
    const kept: Record<string, string> = {};
    if (state.text) kept[LIBRARY_URL_KEYS.text] = state.text;
    if (state.sort !== LIBRARY_URL_DEFAULTS.sort) kept[LIBRARY_URL_KEYS.sort] = state.sort;
    if (state.direction !== LIBRARY_URL_DEFAULTS.direction) kept[LIBRARY_URL_KEYS.direction] = state.direction;
    return kept;
}

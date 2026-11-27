/**
 * Etiqueta humana de un fallo del gateway de catálogo (P10b, #409).
 *
 * La taxonomía es la de T02 (#66): la UI no inventa estados nuevos, solo
 * traduce los que el backend ya clasificó.
 */
import type { CatalogFailureKind } from '@/api/types/catalogSearch'

export function catalogFailureLabel(kind: CatalogFailureKind): string {
    switch (kind) {
        case 'auth_expired':
            return 'Session expired'
        case 'rate_limited':
            return 'Rate limited'
        case 'retryable':
            return 'Temporary failure'
        case 'permanent':
            return 'Provider rejected the search'
        case 'unsupported':
            return 'Search not supported'
        case 'no_account':
            return 'No account'
        default:
            return 'Provider failure'
    }
}

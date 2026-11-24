// Barrel split from api/types.ts; tipos espejo de los comandos Rust.
// ==============================================
// SERVICE/ACCOUNT TYPES
// ==============================================

export interface Service {
    id: number;
    name: string;
    supports_download: number;
    max_quality: string | null;
    // Derived/optional fields for UI
    display_name?: string;
}

/**
 * Honest, secret-free summary of WHAT KIND of credential an account stores.
 *
 * `kind` and the booleans are the only data sent over IPC — never the password,
 * cookie or token itself. This is what lets the UI tell the truth about an
 * imported cookie-only account ("browser session cookie; reconnect to renew")
 * without ever revealing the credential or claiming a password is recoverable.
 */
export type StoredCredentialKind =
  | 'password'
  | 'cookie'
  | 'token'
  | 'oauth'
  | 'mixed'
  | 'none'

/**
 * Masked stored-credential state: what is stored and whether it can renew
 * itself. No field ever holds credential material.
 */
export interface StoredCredentialState {
  /** Which kind of credential the account holds. */
  kind: StoredCredentialKind
  /** True only when a username/email + password pair is stored. */
  has_password: boolean
  /** True when a browser/provider session cookie or session id is stored. */
  has_cookie: boolean
  /** True when any token-shaped secret (API token, OAuth token, cookie token) is stored. */
  has_token: boolean
  /** False when nothing usable is stored at all. */
  has_credential: boolean
  /** True when the credential can renew itself (OAuth refresh token). */
  auto_refreshable: boolean
}

/**
 * Ciclo de vida explícito de cuenta (issue #85, ADR-0007 decisión 3).
 * Eliminar no es un estado: la fila desaparece.
 */
export type AccountLifecycleState = 'enabled' | 'disabled' | 'archived';

export interface Account {
    id: number;
    service_id: number;
    service_name: string;
    display_name: string | null;
    email: string | null;
    is_active: boolean;
    /** Ciclo de vida; ausente en payloads legacy previos a la migración 0107. */
    state?: AccountLifecycleState;
    last_synced: string | null;
    created_at: string | null;
    credentials_invalid?: boolean;
    invalid_reason?: string | null;
    last_auth_error?: string | null;
    /** Masked stored-credential state (never the credential itself). */
    credential_state?: StoredCredentialState | null;
    /**
     * Streaming entitlement of this account: true confirmed, false
     * authenticated but without streaming (Qobuz serves a 30-second preview to
     * those), null when unknown.
     */
    streaming_entitled?: boolean | null;
}

export interface ServiceStatus {
    name: string;
    connected: boolean;
    account_email: string | null;
    library_count: number;
    favorites_count: number;
    playlists_count: number;
    last_synced: string | null;
    credentials_invalid: boolean;
    invalid_reason?: string | null;
    last_auth_error?: string | null;
}

export interface SessionStatus {
    service: string;
    connected: boolean;
    valid: boolean;
    message: string;
    user_info: string | null;
}

// ==============================================
// AUTH TYPES
// ==============================================

export interface AuthResult {
    success: boolean;
    data: (Record<string, unknown> & { account_id?: number }) | null;
    error: string | null;
}

export interface ImportResult {
    imported: number;
    skipped: number;
    errors: string[];
}

export interface UrlParseResult {
    service: string;
    content_type: string;
    id: string;
    url: string;
    queue_id?: number | null;
    track_id?: number | null;
    title?: string | null;
    artist?: string | null;
    status?: string | null;
}

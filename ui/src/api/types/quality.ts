// Barrel split from api/types.ts; tipos espejo de los comandos Rust.
// ==============================================
// QUALITY POLICY & DECISION TYPES
// ==============================================

export type QualityDecisionKind =
    | 'ReadyExactQuality'
    | 'ReadyProviderFallbackExactQuality'
    | 'ReadyQualityFallback'
    | 'CompletedExactQuality'
    | 'CompletedWithProviderFallback'
    | 'CompletedWithQualityFallback'
    | 'CompletedWithQualityShortfall'
    | 'RejectedQuality'
    | 'NoDownloadProvider'
    | 'UnavailableFromProvider'
    | 'EntitlementDenied'
    | 'AuthInvalid'
    | 'RateLimited'
    | 'TemporaryFailure';

export interface QualityDecision {
    requested_quality: string;
    provider_available_quality?: string | null;
    effective_quality: string;
    requested_format: string;
    effective_format: string;
    strict_quality: boolean;
    allow_lossy_fallback: boolean;
    provider_fallback_used: boolean;
    quality_fallback_used: boolean;
    decision: QualityDecisionKind;
    reason?: string | null;
    retryable: boolean;
    user_message: string;
}

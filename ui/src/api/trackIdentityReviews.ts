/**
 * Track Identity Reviews API (#383)
 *
 * Wraps the two IPC commands that expose `track_identity_reviews` (0115):
 * pending reviews with the frozen evidence of both sides, and the human
 * resolution (`merged`, `separate` or `dismissed`). Views must call these
 * wrappers, never `invoke` directly.
 *
 * The backend is the source of truth for validation: a review that is no
 * longer pending is rejected there, and `merged` only accepts a track that is
 * one of the frozen candidates.
 */

import { asArray, asNumber, asRecord, asString, optionalNumber, optionalString } from './normalize';
import { invokeCommand } from './tauri';

export type IdentityReviewResolution = 'merged' | 'separate' | 'dismissed';

export interface IdentityReviewCandidate {
    track_id: number;
    title: string | null;
    artist: string | null;
    album: string | null;
    duration_ms: number | null;
    isrc: string | null;
    duration_delta_ms: number | null;
}

export interface TrackIdentityReview {
    id: number;
    service_id: number;
    service_name: string;
    account_id: number | null;
    account_name: string | null;
    service_track_id: string;
    reason: string;
    status: string;
    incoming_title: string | null;
    incoming_artist: string | null;
    incoming_album: string | null;
    incoming_duration_ms: number | null;
    incoming_isrc: string | null;
    linked_track_id: number | null;
    candidates: IdentityReviewCandidate[];
    created_at: string;
    updated_at: string;
}

export interface IdentityMergeSummary {
    sources_moved: number;
    library_entries_moved: number;
    memberships_moved: number;
    playlist_entries_moved: number;
    edition_links_moved: number;
}

export interface IdentityReviewResolutionResult {
    review_id: number;
    status: string;
    merged_track_id: number | null;
    merge: IdentityMergeSummary | null;
}

function normalizeCandidate(raw: unknown): IdentityReviewCandidate | null {
    const rec = asRecord(raw);
    if (!rec) return null;
    const trackId = asNumber(rec.track_id, Number.NaN);
    if (!Number.isFinite(trackId)) return null;
    return {
        track_id: trackId,
        title: optionalString(rec.title),
        artist: optionalString(rec.artist),
        album: optionalString(rec.album),
        duration_ms: optionalNumber(rec.duration_ms) ?? null,
        isrc: optionalString(rec.isrc),
        duration_delta_ms: optionalNumber(rec.duration_delta_ms) ?? null,
    };
}

function normalizeReview(raw: unknown): TrackIdentityReview | null {
    const rec = asRecord(raw);
    if (!rec) return null;
    const id = asNumber(rec.id, Number.NaN);
    if (!Number.isFinite(id)) return null;
    return {
        id,
        service_id: asNumber(rec.service_id),
        service_name: asString(rec.service_name),
        account_id: optionalNumber(rec.account_id) ?? null,
        account_name: optionalString(rec.account_name),
        service_track_id: asString(rec.service_track_id),
        reason: asString(rec.reason),
        status: asString(rec.status, 'pending'),
        incoming_title: optionalString(rec.incoming_title),
        incoming_artist: optionalString(rec.incoming_artist),
        incoming_album: optionalString(rec.incoming_album),
        incoming_duration_ms: optionalNumber(rec.incoming_duration_ms) ?? null,
        incoming_isrc: optionalString(rec.incoming_isrc),
        linked_track_id: optionalNumber(rec.linked_track_id) ?? null,
        candidates: asArray<unknown>(rec.candidates)
            .map(normalizeCandidate)
            .filter((candidate): candidate is IdentityReviewCandidate => candidate !== null),
        created_at: asString(rec.created_at),
        updated_at: asString(rec.updated_at),
    };
}

function normalizeMergeSummary(raw: unknown): IdentityMergeSummary | null {
    const rec = asRecord(raw);
    if (!rec) return null;
    return {
        sources_moved: asNumber(rec.sources_moved),
        library_entries_moved: asNumber(rec.library_entries_moved),
        memberships_moved: asNumber(rec.memberships_moved),
        playlist_entries_moved: asNumber(rec.playlist_entries_moved),
        edition_links_moved: asNumber(rec.edition_links_moved),
    };
}

function normalizeResolution(raw: unknown): IdentityReviewResolutionResult {
    const rec = asRecord(raw);
    return {
        review_id: asNumber(rec?.review_id),
        status: asString(rec?.status),
        merged_track_id: optionalNumber(rec?.merged_track_id) ?? null,
        merge: normalizeMergeSummary(rec?.merge),
    };
}

/** Pending identity reviews with incoming and local evidence. */
export async function getPendingTrackIdentityReviews(): Promise<TrackIdentityReview[]> {
    const raw = await invokeCommand<unknown>('list_track_identity_reviews');
    return asArray<unknown>(raw)
        .map(normalizeReview)
        .filter((review): review is TrackIdentityReview => review !== null);
}

/**
 * Close a review. `trackId` is required for `merged` and must be one of the
 * review's candidates; `separate` and `dismissed` never touch library rows.
 */
export async function resolveTrackIdentityReview(
    reviewId: number,
    resolution: IdentityReviewResolution,
    trackId?: number | null,
): Promise<IdentityReviewResolutionResult> {
    if (resolution === 'merged' && (trackId === undefined || trackId === null)) {
        throw new Error('A merge needs the canonical track to keep');
    }
    const raw = await invokeCommand<unknown>('resolve_track_identity_review', {
        reviewId,
        resolution,
        trackId: trackId ?? null,
    });
    return normalizeResolution(raw);
}

import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
    getPendingTrackIdentityReviews,
    resolveTrackIdentityReview,
} from '@/api/trackIdentityReviews'

vi.mock('@/api/tauri', () => ({
    invokeCommand: vi.fn(),
}))

import { invokeCommand } from '@/api/tauri'

const rawReview = {
    id: 7,
    service_id: 2,
    service_name: 'qobuz',
    account_id: 3,
    account_name: 'Main account',
    service_track_id: 'qobuz-item-7',
    reason: 'fingerprint_multiple_candidates',
    status: 'pending',
    incoming_title: 'Same Song',
    incoming_artist: 'Twice',
    incoming_album: 'Album',
    incoming_duration_ms: 200400,
    incoming_isrc: null,
    linked_track_id: 42,
    candidates: [
        {
            track_id: 11,
            title: 'Same Song',
            artist: 'Twice',
            album: 'Album',
            duration_ms: 200000,
            isrc: 'USAAA0000001',
            duration_delta_ms: 400,
        },
        {
            track_id: 12,
            title: 'Same Song',
            artist: 'Twice',
            album: null,
            duration_ms: null,
            isrc: null,
            duration_delta_ms: null,
        },
    ],
    created_at: '2026-11-23 10:00:00',
    updated_at: '2026-11-23 10:00:00',
}

describe('trackIdentityReviews API (#383)', () => {
    beforeEach(() => {
        vi.clearAllMocks()
    })

    it('lists pending reviews through the registered command and normalizes the payload', async () => {
        vi.mocked(invokeCommand).mockResolvedValueOnce([
            rawReview,
            { id: 8, service_name: 'tidal', candidates: 'not-an-array' },
            null,
            { service_name: 'no-id' },
        ])

        const reviews = await getPendingTrackIdentityReviews()

        expect(invokeCommand).toHaveBeenCalledWith('list_track_identity_reviews')
        expect(reviews).toHaveLength(2)
        expect(reviews[0]).toMatchObject({
            id: 7,
            service_name: 'qobuz',
            account_name: 'Main account',
            incoming_duration_ms: 200400,
            linked_track_id: 42,
        })
        expect(reviews[0].candidates).toHaveLength(2)
        expect(reviews[0].candidates[1]).toMatchObject({
            track_id: 12,
            album: null,
            duration_delta_ms: null,
        })
        expect(reviews[1]).toMatchObject({ id: 8, candidates: [] })
    })

    it('returns an empty list when the backend omits the array', async () => {
        vi.mocked(invokeCommand).mockResolvedValueOnce({ reviews: [] })
        await expect(getPendingTrackIdentityReviews()).resolves.toEqual([])
    })

    it('sends the merge as camelCase args with the canonical track', async () => {
        vi.mocked(invokeCommand).mockResolvedValueOnce({
            review_id: 7,
            status: 'resolved_merged',
            merged_track_id: 11,
            merge: {
                sources_moved: 1,
                library_entries_moved: 1,
                memberships_moved: 1,
                playlist_entries_moved: 2,
                edition_links_moved: 1,
            },
        })

        const result = await resolveTrackIdentityReview(7, 'merged', 11)

        expect(invokeCommand).toHaveBeenCalledWith('resolve_track_identity_review', {
            reviewId: 7,
            resolution: 'merged',
            trackId: 11,
        })
        expect(result).toMatchObject({
            review_id: 7,
            status: 'resolved_merged',
            merged_track_id: 11,
            merge: { sources_moved: 1, playlist_entries_moved: 2 },
        })
    })

    it('rejects a merge without a canonical track before invoking the backend', async () => {
        await expect(resolveTrackIdentityReview(7, 'merged')).rejects.toThrow(
            'A merge needs the canonical track to keep',
        )
        expect(invokeCommand).not.toHaveBeenCalled()
    })

    it('sends separate and dismissed without a track id', async () => {
        vi.mocked(invokeCommand).mockResolvedValue({
            review_id: 7,
            status: 'resolved_separate',
            merged_track_id: null,
            merge: null,
        })

        const result = await resolveTrackIdentityReview(7, 'separate')

        expect(invokeCommand).toHaveBeenCalledWith('resolve_track_identity_review', {
            reviewId: 7,
            resolution: 'separate',
            trackId: null,
        })
        expect(result.merged_track_id).toBeNull()
        expect(result.merge).toBeNull()
    })
})

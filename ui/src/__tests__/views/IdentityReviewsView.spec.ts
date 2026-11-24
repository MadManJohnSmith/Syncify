/**
 * IdentityReviewsView.spec.ts — T08b (#383).
 *
 * Contract:
 * - the view loads pending reviews through `list_track_identity_reviews`;
 * - it renders the provider item and the local candidates side by side;
 * - merge is a two-step action and only sends the selected candidate;
 * - separate/dismiss close the review without any track id;
 * - loading, error/retry and empty states are honest;
 * - nothing is written until an explicit button is pressed.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import IdentityReviewsView from '@/views/IdentityReviewsView.vue'
import { mockInvoke, resetMocks } from '../setup'

interface ReviewFixture {
    id: number
    incoming_title: string
    incoming_artist: string
    service_track_id: string
    linked_track_id: number | null
    candidates: Array<Record<string, unknown>>
    [key: string]: unknown
}

function review(overrides: Partial<ReviewFixture> = {}): ReviewFixture & Record<string, unknown> {
    return {
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
                duration_ms: 200900,
                isrc: null,
                duration_delta_ms: 500,
            },
        ],
        created_at: '2026-11-23 10:00:00',
        updated_at: '2026-11-23 10:00:00',
        ...overrides,
    }
}

const mergedResult = {
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
}

async function mountView() {
    const wrapper = mount(IdentityReviewsView)
    await flushPromises()
    await flushPromises()
    return wrapper
}

describe('IdentityReviewsView (#383)', () => {
    beforeEach(() => {
        resetMocks()
        vi.clearAllMocks()
    })

    it('shows a loading state until the queue arrives', async () => {
        mockInvoke((command) => {
            if (command === 'list_track_identity_reviews') return [review()]
            return null
        })

        const wrapper = mount(IdentityReviewsView)
        // `load()` sets the flag synchronously before awaiting the IPC.
        expect(wrapper.find('[data-testid="identity-reviews-loading"]').exists()).toBe(true)

        await flushPromises()
        await flushPromises()
        expect(wrapper.find('[data-testid="identity-reviews-loading"]').exists()).toBe(false)
        expect(wrapper.find('[data-testid="identity-review-detail"]').exists()).toBe(true)
    })

    it('shows the empty state when there is nothing to review', async () => {
        mockInvoke(() => [])
        const wrapper = await mountView()
        expect(wrapper.find('[data-testid="identity-reviews-empty"]').exists()).toBe(true)
        expect(wrapper.text()).toContain('No ambiguous identities')
    })

    it('shows the backend error and retries from the error state', async () => {
        let calls = 0
        mockInvoke((command) => {
            if (command !== 'list_track_identity_reviews') return null
            calls += 1
            if (calls === 1) throw new Error('database is locked')
            return []
        })

        const wrapper = await mountView()
        expect(wrapper.find('[data-testid="identity-reviews-error"]').exists()).toBe(true)
        expect(wrapper.text()).toContain('database is locked')

        await wrapper.find('[data-testid="identity-reviews-retry"]').trigger('click')
        await flushPromises()
        await flushPromises()
        expect(wrapper.find('[data-testid="identity-reviews-empty"]').exists()).toBe(true)
    })

    it('renders both sides and merges only after selecting and confirming a candidate', async () => {
        let listCalls = 0
        const invocations: Array<{ command: string; args?: Record<string, unknown> }> = []
        mockInvoke((command, args) => {
            invocations.push({ command, args })
            if (command === 'list_track_identity_reviews') {
                listCalls += 1
                return listCalls === 1 ? [review()] : []
            }
            if (command === 'resolve_track_identity_review') return mergedResult
            return null
        })

        const wrapper = await mountView()

        expect(wrapper.find('[data-testid="identity-review-item-7"]').exists()).toBe(true)
        expect(wrapper.text()).toContain('Provider item')
        expect(wrapper.text()).toContain('Local candidates')
        expect(wrapper.text()).toContain('track #11')
        expect(wrapper.text()).toContain('track #12')
        expect(wrapper.text()).toContain('±0.4 s')

        const merge = wrapper.find('[data-testid="identity-merge"]')
        expect(merge.attributes('disabled')).toBeDefined()

        await wrapper.find('[data-testid="identity-candidate-11"] input').setValue()
        expect(merge.attributes('disabled')).toBeUndefined()

        await merge.trigger('click')
        expect(wrapper.find('[data-testid="identity-merge-confirmation"]').exists()).toBe(true)
        expect(
            invocations.some((invocation) => invocation.command === 'resolve_track_identity_review'),
        ).toBe(false)

        await wrapper.find('[data-testid="identity-confirm-merge"]').trigger('click')
        await flushPromises()
        await flushPromises()

        const resolution = invocations.find(
            (invocation) => invocation.command === 'resolve_track_identity_review',
        )
        expect(resolution?.args).toEqual({
            reviewId: 7,
            resolution: 'merged',
            trackId: 11,
        })
        expect(wrapper.find('[data-testid="identity-resolution-notice"]').text()).toContain(
            'Merged “Same Song” into track #11',
        )
        expect(wrapper.find('[data-testid="identity-reviews-empty"]').exists()).toBe(true)
    })

    it('closes as separate without selecting a candidate or touching a track', async () => {
        let listCalls = 0
        const invocations: Array<{ command: string; args?: Record<string, unknown> }> = []
        mockInvoke((command, args) => {
            invocations.push({ command, args })
            if (command === 'list_track_identity_reviews') {
                listCalls += 1
                return listCalls === 1 ? [review()] : []
            }
            if (command === 'resolve_track_identity_review') {
                return {
                    review_id: 7,
                    status: 'resolved_separate',
                    merged_track_id: null,
                    merge: null,
                }
            }
            return null
        })

        const wrapper = await mountView()
        await wrapper.find('[data-testid="identity-resolve-separate"]').trigger('click')
        await flushPromises()
        await flushPromises()

        const resolution = invocations.find(
            (invocation) => invocation.command === 'resolve_track_identity_review',
        )
        expect(resolution?.args).toEqual({
            reviewId: 7,
            resolution: 'separate',
            trackId: null,
        })
        expect(wrapper.find('[data-testid="identity-resolution-notice"]').text()).toContain(
            'No track was changed',
        )
    })

    it('dismisses the evidence and keeps the resolution error visible if the backend refuses', async () => {
        mockInvoke((command) => {
            if (command === 'list_track_identity_reviews') return [review()]
            if (command === 'resolve_track_identity_review') {
                throw new Error('identity review 7 is already resolved_merged and is not reopened')
            }
            return null
        })

        const wrapper = await mountView()
        await wrapper.find('[data-testid="identity-resolve-dismissed"]').trigger('click')
        await flushPromises()
        await flushPromises()

        expect(wrapper.find('[data-testid="identity-resolution-error"]').text()).toContain(
            'already resolved_merged',
        )
        expect(wrapper.find('[data-testid="identity-review-detail"]').exists()).toBe(true)
    })

    it('lets another review be selected without inheriting the previous candidate', async () => {
        const second = review({
            id: 8,
            incoming_title: 'Other Song',
            service_track_id: 'qobuz-item-8',
            candidates: [
                {
                    track_id: 21,
                    title: 'Other Song',
                    artist: 'Twice',
                    album: null,
                    duration_ms: 180000,
                    isrc: null,
                    duration_delta_ms: null,
                },
            ],
        })
        mockInvoke((command) => {
            if (command === 'list_track_identity_reviews') return [review(), second]
            return null
        })

        const wrapper = await mountView()
        await wrapper.find('[data-testid="identity-candidate-11"] input').setValue()
        await wrapper.find('[data-testid="identity-review-item-8"]').trigger('click')
        await flushPromises()

        expect(wrapper.find('[data-testid="identity-merge"]').attributes('disabled')).toBeDefined()
        expect(wrapper.text()).toContain('Other Song')
    })
})

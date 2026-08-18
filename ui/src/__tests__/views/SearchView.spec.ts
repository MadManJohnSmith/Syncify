/**
 * SearchView.spec.ts
 * Tests for SearchView.vue download actions and IPC payloads
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import SearchView from '@/views/SearchView.vue';
import { mockInvoke, resetMocks } from '../setup';

vi.mock('vue-router', () => ({
    useRouter: () => ({
        push: vi.fn(),
    }),
}));

describe('SearchView', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    const mockSearchResults = {
        tracks: [
            { id: 501, title: 'Search Hit Track', artist_name: 'Search Hit Artist', album_name: 'Search Hit Album', duration_ms: 210000, isrc: 'US1234567890' }
        ],
        total: 1,
        offset: 0,
        limit: 50,
        has_more: false
    };

    it('searches and allows downloading a search result track with add_to_queue', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'search_tracks') return mockSearchResults;
            if (cmd === 'add_to_queue') return 1;
            return null;
        });

        const wrapper = mount(SearchView);
        await flushPromises();

        // Type search query
        const input = wrapper.find('input[type="text"]');
        await input.setValue('Search Hit');
        // Wait for debounce timer (500ms)
        await new Promise(r => setTimeout(r, 600));
        await flushPromises();

        expect(wrapper.text()).toContain('Search Hit Track');

        // Click download track button
        const downloadBtn = wrapper.find('button[title="Download Track"]');
        expect(downloadBtn.exists()).toBe(true);
        await downloadBtn.trigger('click');
        await flushPromises();

        const addCall = invokeCalls.find(c => c.cmd === 'add_to_queue');
        expect(addCall).toBeDefined();
        expect(addCall?.args).toEqual({
            trackId: 501,
            targetTitle: 'Search Hit Track',
            targetArtist: 'Search Hit Artist',
            targetAlbum: 'Search Hit Album',
            targetIsrc: 'US1234567890',
            allowFallback: true,
        });
    });

    it('handles SourceIdentityMissing error on search download gracefully', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'search_tracks') return mockSearchResults;
            if (cmd === 'add_to_queue') {
                throw new Error('SourceIdentityMissing: track 501 has no source');
            }
            return null;
        });

        const wrapper = mount(SearchView);
        await flushPromises();

        const input = wrapper.find('input[type="text"]');
        await input.setValue('Search Hit');
        await new Promise(r => setTimeout(r, 600));
        await flushPromises();

        const downloadBtn = wrapper.find('button[title="Download Track"]');
        await downloadBtn.trigger('click');
        await flushPromises();

        expect(wrapper.exists()).toBe(true);
    });
});

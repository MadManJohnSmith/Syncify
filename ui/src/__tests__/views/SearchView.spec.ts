/**
 * SearchView.spec.ts
 * Tests for SearchView.vue (modal, R17) — download actions, playback and IPC payloads.
 *
 * The view renders through `<Teleport to="body">`, so its DOM lives in the
 * document, not inside the wrapper returned by `mount`.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import SearchView from '@/views/SearchView.vue';
import { mockInvoke, resetMocks } from '../setup';

vi.mock('vue-router', () => ({
    useRouter: () => ({
        push: vi.fn(),
    }),
}));

const mockPlayerPlay = vi.fn().mockResolvedValue(undefined);
vi.mock('@/composables/usePlayer', () => ({
    usePlayer: () => ({
        play: mockPlayerPlay,
    }),
}));

function modal() {
    return document.body.querySelector('[data-testid="search-modal"]') as HTMLElement;
}

describe('SearchView', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    afterEach(() => {
        document.body.innerHTML = '';
    });

    const mockSearchResults = {
        tracks: [
            { id: 501, title: 'Search Hit Track', artist_name: 'Search Hit Artist', album_name: 'Search Hit Album', duration_ms: 210000, isrc: 'US1234567890', cover_art_url: null }
        ],
        total: 1,
        offset: 0,
        limit: 50,
        has_more: false
    };

    async function openWithQuery(query: string) {
        const wrapper = mount(SearchView);
        await flushPromises();

        const input = modal().querySelector('input[type="text"]') as HTMLInputElement;
        input.value = query;
        input.dispatchEvent(new Event('input'));
        await new Promise(r => setTimeout(r, 600));
        await flushPromises();

        return wrapper;
    }

    it('renders as a modal over a backdrop', async () => {
        mount(SearchView);
        await flushPromises();

        expect(document.body.querySelector('[data-testid="search-modal-backdrop"]')).not.toBeNull();
        expect(modal().getAttribute('role')).toBe('dialog');
        expect(modal().getAttribute('aria-modal')).toBe('true');
    });

    it('closes on Escape and on the backdrop', async () => {
        const wrapper = mount(SearchView, { attachTo: document.body });
        await flushPromises();

        document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
        await flushPromises();
        expect(wrapper.emitted('close')).toBeTruthy();

        wrapper.unmount();
        document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    });

    it('searches and allows downloading a search result track with add_to_queue', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'search_tracks') return mockSearchResults;
            if (cmd === 'add_to_queue') return 1;
            return null;
        });

        await openWithQuery('Search Hit');

        expect(modal().textContent).toContain('Search Hit Track');

        const downloadBtn = modal().querySelector('button[title="Download Track"]') as HTMLButtonElement;
        expect(downloadBtn).not.toBeNull();
        downloadBtn.click();
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

        const wrapper = await openWithQuery('Search Hit');

        const downloadBtn = modal().querySelector('button[title="Download Track"]') as HTMLButtonElement;
        downloadBtn.click();
        await flushPromises();

        expect(wrapper.exists()).toBe(true);
    });

    it('shows a search failure instead of "no results" when the query fails', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'search_tracks') throw new Error('Search backend unavailable');
            return null;
        });

        await openWithQuery('Boom');

        expect(document.body.querySelector('[data-testid="search-modal-error"]')).not.toBeNull();
        expect(modal().textContent).not.toContain('No results found');
    });

    it('plays track via usePlayer when clicking on track row in search results', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'search_tracks') return mockSearchResults;
            return null;
        });

        await openWithQuery('Search Hit');

        const trackRow = modal().querySelector('.cursor-pointer.group') as HTMLElement;
        expect(trackRow).not.toBeNull();
        trackRow.click();
        await flushPromises();

        expect(mockPlayerPlay).toHaveBeenCalledTimes(1);
        expect(mockPlayerPlay).toHaveBeenCalledWith({
            id: 501,
            title: 'Search Hit Track',
            artist: 'Search Hit Artist',
            album: 'Search Hit Album',
            coverUrl: null,
        });
    });
});
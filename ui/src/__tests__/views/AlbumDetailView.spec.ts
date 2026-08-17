/**
 * AlbumDetailView.spec.ts
 * Tests for AlbumDetailView.vue download actions and IPC payloads
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import AlbumDetailView from '@/views/AlbumDetailView.vue';
import { mockInvoke, resetMocks } from '../setup';

vi.mock('vue-router', () => ({
    useRoute: () => ({
        params: { id: '77' },
        query: {},
    }),
    useRouter: () => ({
        push: vi.fn(),
        back: vi.fn(),
    }),
}));

describe('AlbumDetailView', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
    });

    const mockAlbum = {
        id: 77,
        title: 'Dark Side of the Moon',
        artist_name: 'Pink Floyd',
        artist_id: 12,
        release_year: 1973,
        track_count: 2,
        duration_ms: 500000,
        cover_art_url: null,
        tracks: [
            { id: 701, title: 'Speak to Me', duration_ms: 90000, track_number: 1, artist_name: 'Pink Floyd' },
            { id: 702, title: 'Breathe', duration_ms: 163000, track_number: 2, artist_name: 'Pink Floyd' }
        ]
    };

    it('renders album information', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_album') return mockAlbum;
            return null;
        });

        const wrapper = mount(AlbumDetailView);
        await flushPromises();

        expect(wrapper.text()).toContain('Dark Side of the Moon');
        expect(wrapper.text()).toContain('Pink Floyd');
    });

    it('enqueues album download with add_batch_to_queue', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_album') return mockAlbum;
            if (cmd === 'add_batch_to_queue') return { added: 2, skipped: 0 };
            return null;
        });

        const wrapper = mount(AlbumDetailView);
        await flushPromises();

        const downloadBtn = wrapper.findAll('button').find(b => b.text().includes('Download All'));
        expect(downloadBtn).toBeDefined();
        await downloadBtn!.trigger('click');
        await flushPromises();

        const batchCall = invokeCalls.find(c => c.cmd === 'add_batch_to_queue');
        expect(batchCall).toBeDefined();
        expect(batchCall?.args).toEqual({
            trackIds: [701, 702],
            allowFallback: false,
        });
    });

    it('enqueues single track download with add_to_queue', async () => {
        const invokeCalls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            invokeCalls.push({ cmd, args });
            if (cmd === 'get_album') return mockAlbum;
            if (cmd === 'add_to_queue') return 1;
            return null;
        });

        const wrapper = mount(AlbumDetailView);
        await flushPromises();

        const trackDownloadBtn = wrapper.find('button[title="Download Track"]');
        expect(trackDownloadBtn.exists()).toBe(true);
        await trackDownloadBtn.trigger('click');
        await flushPromises();

        const addCall = invokeCalls.find(c => c.cmd === 'add_to_queue');
        expect(addCall).toBeDefined();
        expect(addCall?.args).toEqual({
            trackId: 701,
            targetTitle: 'Speak to Me',
            targetArtist: 'Pink Floyd',
            targetAlbum: 'Dark Side of the Moon',
            allowFallback: false,
        });
    });

    it('handles SourceIdentityMissing error on album download gracefully', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_album') return mockAlbum;
            if (cmd === 'add_batch_to_queue') {
                throw new Error('SourceIdentityMissing: album tracks have no streaming source');
            }
            return null;
        });

        const wrapper = mount(AlbumDetailView);
        await flushPromises();

        const downloadBtn = wrapper.findAll('button').find(b => b.text().includes('Download All'));
        expect(downloadBtn).toBeDefined();
        await downloadBtn!.trigger('click');
        await flushPromises();

        expect(wrapper.exists()).toBe(true);
    });
});

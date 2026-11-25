import { beforeEach, describe, expect, it } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import PlaylistView from '@/views/PlaylistView.vue';
import { mockInvoke, resetMocks } from '../setup';

function page(tracks: any[], total = tracks.length) {
    return { tracks, total, offset: 0, limit: 100, has_more: false };
}

describe('PlaylistView — catálogo de playlists', () => {
    beforeEach(() => {
        resetMocks();
        document.body.innerHTML = '';
    });

    it('separa locales, importadas por servicio y smart aunque una smart tenga servicio', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_playlists') {
                return [
                    { id: 1, name: 'Local', track_count: 0, image_url: null, service_name: null },
                    { id: 2, name: 'Remota', track_count: 0, image_url: null, service_name: 'Spotify' },
                    { id: 3, name: 'Inteligente', track_count: 0, image_url: null, service_name: 'Qobuz', is_smart: true },
                ];
            }
            if (cmd === 'get_local_playlist_tracks') return page([]);
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();

        expect(wrapper.text()).toContain('My Playlists (1)');
        expect(wrapper.text()).toContain('Imported from Spotify (1)');
        expect(wrapper.text()).toContain('Smart Playlists (1)');
        expect(wrapper.findAll('.playlist-item').map(item => item.text())).toEqual([
            expect.stringContaining('Favorites'),
            expect.stringContaining('Local'),
            expect.stringContaining('Remota'),
            expect.stringContaining('Inteligente'),
        ]);
    });

    it('usa la imagen del proveedor en una playlist importada', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_playlists') {
                return [{
                    id: 2,
                    name: 'Remota',
                    track_count: 0,
                    image_url: 'https://cdn.test/provider.jpg',
                    service_name: 'spotify',
                }];
            }
            if (cmd === 'get_local_playlist_tracks') return page([]);
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();

        const item = wrapper.findAll('.playlist-item').find(row => row.text().includes('Remota'))!;
        expect(item.find('[data-testid="track-cover-img"]').attributes('src')).toBe('https://cdn.test/provider.jpg');
    });

    it('reconcilia track_count con las filas recibidas y cuenta ocurrencias repetidas', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_playlists') {
                return [{ id: 1, name: 'Repetida', track_count: 99, image_url: null, service_name: null }];
            }
            if (cmd === 'get_local_playlist_tracks') {
                return page([
                    { id: 7, position: 1, title: 'Misma', duration_ms: 1000 },
                    { id: 7, position: 2, title: 'Misma', duration_ms: 1000 },
                ], 2);
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();
        const item = wrapper.findAll('.playlist-item').find(row => row.text().includes('Repetida'))!;
        await item.trigger('click');
        await flushPromises();

        expect(wrapper.findAll('.track-row')).toHaveLength(2);
        expect(item.text()).toContain('2 tracks');
        expect(wrapper.find('.playlist-header').text()).toContain('2 tracks');
    });
});

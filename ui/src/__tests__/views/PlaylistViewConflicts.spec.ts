/**
 * PlaylistViewConflicts.spec.ts
 *
 * #410: la primera edición local de una playlist remota pregunta entre
 * `editable` y `gana_local`; la elección persiste y los conflictos en
 * `needs_review` se ven y se resuelven sin pisar en silencio.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import PlaylistView from '@/views/PlaylistView.vue';
import { mockInvoke, resetMocks } from '../setup';

const PLAYLIST = {
    id: 1,
    name: 'Imported Mix',
    track_count: 2,
    account_id: 1,
    service_name: 'spotify',
    is_smart: false,
};

const PAGE = {
    tracks: [
        { id: 301, title: 'Track 1', artist_name: 'Artist 1', album_name: 'Album 1', duration_ms: 180000, position: 1 },
        { id: 302, title: 'Track 2', artist_name: 'Artist 2', album_name: 'Album 2', duration_ms: 200000, position: 2 },
    ],
    total: 2,
    offset: 0,
    limit: 100,
    has_more: false,
};

const NAME_REVIEW = {
    id: 11,
    playlist_id: 1,
    playlist_name: 'Imported Mix',
    service_name: 'spotify',
    conflict_kind: 'name',
    local_value: 'Mi nombre',
    provider_value: 'Nombre del proveedor',
    status: 'needs_review',
    created_at: '2026-11-27 10:00:00',
    updated_at: '2026-11-27 10:00:00',
    resolved_at: null,
    local_track_ids: null,
    provider_track_ids: null,
};

function state(mode: string, pending: unknown[] = []) {
    return {
        playlist_id: 1,
        conflict_mode: mode,
        service_name: 'spotify',
        is_remote: true,
        requires_mode_choice: mode === 'gana_proveedor',
        pending_reviews: pending,
    };
}

function bodyElement(testId: string): HTMLElement | null {
    return document.body.querySelector(`[data-testid="${testId}"]`);
}

async function selectImportedPlaylist(wrapper: any) {
    await flushPromises();
    const item = wrapper.findAll('.group').find((el: any) => el.text().includes('Imported Mix'));
    expect(item).toBeDefined();
    await item!.trigger('click');
    await flushPromises();
}

describe('PlaylistView conflict mode (#410)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        document.body.innerHTML = '';
    });

    it('asks before the first local edit and persists the chosen mode', async () => {
        let mode = 'gana_proveedor';
        const calls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            calls.push({ cmd, args });
            if (cmd === 'get_playlists') return [PLAYLIST];
            if (cmd === 'get_local_playlist_tracks') return PAGE;
            if (cmd === 'get_playlist') return PLAYLIST;
            if (cmd === 'get_playlist_conflict_state') return state(mode);
            if (cmd === 'list_playlist_conflict_reviews') return [];
            if (cmd === 'set_playlist_conflict_mode') {
                mode = (args as any).mode;
                return state(mode);
            }
            if (cmd === 'update_playlist') {
                return { ...PLAYLIST, name: (args as any).name };
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await selectImportedPlaylist(wrapper);

        const nameHeading = wrapper.find('h1');
        expect(nameHeading.text()).toBe('Imported Mix');
        await nameHeading.trigger('click');
        await flushPromises();

        const nameInput = wrapper.find('input.text-2xl');
        expect(nameInput.exists()).toBe(true);
        await nameInput.setValue('Mi nombre local');
        await nameInput.trigger('blur');
        await flushPromises();

        // The edit did not reach the backend before the choice.
        expect(calls.find(call => call.cmd === 'update_playlist')).toBeUndefined();
        expect(bodyElement('playlist-conflict-mode-dialog')).not.toBeNull();

        bodyElement('conflict-mode-editable')!.click();
        await flushPromises();

        const setCall = calls.find(call => call.cmd === 'set_playlist_conflict_mode');
        expect(setCall?.args).toEqual({ playlistId: 1, mode: 'editable' });
        const updateCall = calls.find(call => call.cmd === 'update_playlist');
        expect(updateCall?.args).toEqual({ id: 1, name: 'Mi nombre local' });
        expect(bodyElement('playlist-conflict-mode-dialog')).toBeNull();

        // The persisted mode stops the warning on the next edit, even in a new mount.
        await wrapper.find('h1').trigger('click');
        await flushPromises();
        const secondInput = wrapper.find('input.text-2xl');
        await secondInput.setValue('Otro nombre');
        await secondInput.trigger('blur');
        await flushPromises();
        expect(bodyElement('playlist-conflict-mode-dialog')).toBeNull();
        expect(calls.filter(call => call.cmd === 'update_playlist')).toHaveLength(2);
    });

    it('cancelling the first-edit choice aborts the edit instead of saving silently', async () => {
        const calls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            calls.push({ cmd, args });
            if (cmd === 'get_playlists') return [PLAYLIST];
            if (cmd === 'get_local_playlist_tracks') return PAGE;
            if (cmd === 'get_playlist') return PLAYLIST;
            if (cmd === 'get_playlist_conflict_state') return state('gana_proveedor');
            if (cmd === 'list_playlist_conflict_reviews') return [];
            return null;
        });

        const wrapper = mount(PlaylistView);
        await selectImportedPlaylist(wrapper);

        await wrapper.find('h1').trigger('click');
        await flushPromises();
        const nameInput = wrapper.find('input.text-2xl');
        await nameInput.setValue('No se guarda');
        await nameInput.trigger('blur');
        await flushPromises();

        bodyElement('conflict-mode-cancel')!.click();
        await flushPromises();

        expect(bodyElement('playlist-conflict-mode-dialog')).toBeNull();
        expect(calls.find(call => call.cmd === 'update_playlist')).toBeUndefined();
        expect(calls.find(call => call.cmd === 'set_playlist_conflict_mode')).toBeUndefined();
        expect(wrapper.find('h1').text()).toBe('Imported Mix');
    });
});

describe('PlaylistView conflict reviews (#410)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        document.body.innerHTML = '';
    });

    it('lists needs_review conflicts globally and resolves them explicitly', async () => {
        let pending = [NAME_REVIEW];
        const calls: { cmd: string; args: any }[] = [];
        mockInvoke((cmd, args) => {
            calls.push({ cmd, args });
            if (cmd === 'get_playlists') return [PLAYLIST];
            if (cmd === 'get_local_playlist_tracks') return PAGE;
            if (cmd === 'get_playlist') return PLAYLIST;
            if (cmd === 'get_playlist_conflict_state') return state('editable', pending);
            if (cmd === 'list_playlist_conflict_reviews') return pending;
            if (cmd === 'resolve_playlist_conflict_review') {
                pending = [];
                return { ...NAME_REVIEW, status: 'resolved_provider', resolved_at: 'now' };
            }
            return null;
        });

        const wrapper = mount(PlaylistView);
        await flushPromises();

        const openButton = wrapper.find('[data-testid="playlist-conflicts-button"]');
        expect(openButton.exists()).toBe(true);
        expect(openButton.text()).toContain('Playlist conflicts (1)');
        await openButton.trigger('click');
        await flushPromises();

        const dialog = document.body.querySelector('[data-testid="playlist-conflict-reviews-dialog"]');
        expect(dialog).not.toBeNull();
        expect(document.body.textContent).toContain('Mi nombre');
        expect(document.body.textContent).toContain('Nombre del proveedor');

        bodyElement('conflict-use-provider-11')!.click();
        await flushPromises();

        const resolveCall = calls.find(call => call.cmd === 'resolve_playlist_conflict_review');
        expect(resolveCall?.args).toEqual({ reviewId: 11, resolution: 'provider' });
        expect(document.body.querySelector('[data-testid="playlist-conflict-reviews-dialog"]')).toBeNull();
        expect(wrapper.find('[data-testid="playlist-conflicts-button"]').exists()).toBe(false);
    });

    it('shows a banner for the selected playlist without overwriting its local values', async () => {
        mockInvoke((cmd) => {
            if (cmd === 'get_playlists') return [PLAYLIST];
            if (cmd === 'get_local_playlist_tracks') return PAGE;
            if (cmd === 'get_playlist') return PLAYLIST;
            if (cmd === 'get_playlist_conflict_state') return state('editable', [NAME_REVIEW]);
            if (cmd === 'list_playlist_conflict_reviews') return [NAME_REVIEW];
            return null;
        });

        const wrapper = mount(PlaylistView);
        await selectImportedPlaylist(wrapper);

        const banner = wrapper.find('[data-testid="playlist-conflict-banner"]');
        expect(banner.exists()).toBe(true);
        expect(banner.text()).toContain('1 conflict need review');
        expect(banner.text()).toContain('Your local values were preserved.');
        // The local name is still the one in the playlist header.
        expect(wrapper.find('h1').text()).toBe('Imported Mix');
    });
});

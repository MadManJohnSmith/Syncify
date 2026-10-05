/**
 * PlaylistViewStates.spec.ts — regresión de auditoría 5, 24, 26 y 46.
 *
 * 5:  la vista distingue «cargando» y «falló» de «esta vacía».
 * 24: Favoritos se lee de la biblioteca (id sintético -1), no de `playlists`.
 * 26: el botón de opciones abre un menú; no borra nada por su cuenta.
 * 46: `/playlists?create=1` abre el formulario de creación y limpia el query.
 * R13: el borrado pide la confirmación unificada de la app, no window.confirm.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import PlaylistView from '@/views/PlaylistView.vue';
import { mockInvoke, resetMocks } from '../setup';
import { useDialog } from '@/composables/useToast';

const mockPlayerPlay = vi.fn().mockResolvedValue(undefined);
const mockPlayerPlayNext = vi.fn().mockResolvedValue('queued');
vi.mock('@/composables/usePlayer', () => ({
    usePlayer: () => ({
        play: mockPlayerPlay,
        playNext: mockPlayerPlayNext,
    }),
}));

// Estado hoisted: la factoría del mock se evalúa al importar la vista, antes
// de que el cuerpo del módulo declare sus variables, así que vive aquí.
const routerState = vi.hoisted(() => ({
    query: {} as Record<string, unknown>,
    replace: undefined as undefined | ((query: unknown) => Promise<void>),
}));

vi.mock('vue-router', () => ({
    useRouter: () => ({
        push: vi.fn(),
        back: vi.fn(),
        replace: (query: unknown) =>
            routerState.replace ? routerState.replace(query) : Promise.resolve(),
    }),
    useRoute: () => ({ query: routerState.query, params: {} }),
}));

const mockPlaylists = [
    { id: 1, name: 'Chill Vibes', track_count: 2, is_favorite: false, account_id: 1, is_smart: false },
];

const mockTracks = [
    { id: 301, title: 'Chill Track 1', artist_name: 'Artist 1', album_name: 'Album 1', duration_ms: 180000, cover_art_url: 'http://img1.jpg' },
    { id: 302, title: 'Chill Track 2', artist_name: 'Artist 2', album_name: 'Album 2', duration_ms: 200000, cover_art_url: null },
];

function page(tracks: any[], total = tracks.length) {
    return { tracks, total, offset: 0, limit: 500, has_more: false };
}

async function mountView(invoke: (cmd: string, args?: any) => unknown) {
    mockInvoke(invoke as any);
    const wrapper = mount(PlaylistView, { attachTo: document.body });
    await flushPromises();
    return wrapper;
}

async function openPlaylist(name: string, wrapper: any) {
    const item = wrapper.findAll('.playlist-item').find((el: any) => el.text().includes(name));
    expect(item).toBeDefined();
    await item!.trigger('click');
    await flushPromises();
}

describe('PlaylistView — estados de carga y error (auditoría 5)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        routerState.query = {};
        document.body.innerHTML = '';
    });

    afterEach(() => {
        document.body.innerHTML = '';
    });

    it('muestra el estado vacío solo cuando la carga terminó bien', async () => {
        const wrapper = await mountView((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return page(mockTracks);
            return null;
        });

        await openPlaylist('Chill Vibes', wrapper);

        expect(wrapper.find('[data-testid="playlist-tracks-empty"]').exists()).toBe(false);
        expect(wrapper.find('[data-testid="playlist-tracks-error"]').exists()).toBe(false);
        expect(wrapper.findAll('.track-row').length).toBe(2);
    });

    it('una playlist realmente vacía muestra el estado vacío', async () => {
        const wrapper = await mountView((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return page([]);
            return null;
        });

        await openPlaylist('Chill Vibes', wrapper);

        expect(wrapper.find('[data-testid="playlist-tracks-empty"]').exists()).toBe(true);
    });

    it('un fallo de carga muestra un error reintentable, no una lista vacía', async () => {
        let fail = true;
        const wrapper = await mountView((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') {
                if (fail) throw new Error('database is locked');
                return page(mockTracks);
            }
            return null;
        });

        await openPlaylist('Chill Vibes', wrapper);

        expect(wrapper.find('[data-testid="playlist-tracks-error"]').exists()).toBe(true);
        expect(wrapper.find('[data-testid="playlist-tracks-empty"]').exists()).toBe(false);
        expect(wrapper.find('[data-testid="playlist-tracks-error"]').text()).toContain('database is locked');

        // El reintento del propio estado de error recupera la lista.
        fail = false;
        await wrapper.find('[data-testid="playlist-tracks-error"] button').trigger('click');
        await flushPromises();

        expect(wrapper.find('[data-testid="playlist-tracks-error"]').exists()).toBe(false);
        expect(wrapper.findAll('.track-row').length).toBe(2);
    });
});

describe('PlaylistView — Favoritos reales (auditoría 24)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        routerState.query = {};
        document.body.innerHTML = '';
    });

    afterEach(() => {
        document.body.innerHTML = '';
    });

    it('lee Favoritos de la biblioteca y no de get_local_playlist_tracks(-1)', async () => {
        const calls: { cmd: string; args: any }[] = [];
        const wrapper = await mountView((cmd, args) => {
            calls.push({ cmd, args: args ?? {} });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_favorite_tracks') return page(mockTracks, 7);
            if (cmd === 'get_local_playlist_tracks') return page([]);
            return null;
        });

        await openPlaylist('Favorites', wrapper);

        expect(calls.some(c => c.cmd === 'get_favorite_tracks')).toBe(true);
        // El id sintético -1 nunca debe llegar a la consulta de playlists.
        expect(calls.some(c => c.cmd === 'get_local_playlist_tracks' && c.args.playlistId === -1)).toBe(false);
        expect(wrapper.findAll('.track-row').length).toBe(2);

        // El contador de la entrada Favoritos es el real, no 0.
        const favoritesRow = wrapper.findAll('.playlist-item').find((el: any) => el.text().includes('Favorites'));
        expect(favoritesRow!.text()).toContain('7 tracks');
    });

    it('Favoritos queda marcado como seleccionado con su id numérico', async () => {
        const wrapper = await mountView((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_favorite_tracks') return page(mockTracks, 2);
            return null;
        });

        const favoritesRow = wrapper.findAll('.playlist-item').find((el: any) => el.text().includes('Favorites'));
        expect(favoritesRow!.classes().join(' ')).not.toContain('bg-primary/10');

        await favoritesRow!.trigger('click');
        await flushPromises();

        const selected = wrapper.findAll('.playlist-item').find((el: any) => el.text().includes('Favorites'));
        expect(selected!.classes().join(' ')).toContain('bg-primary/10');
    });
});

describe('PlaylistView — menú de opciones (auditoría 26)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        routerState.query = {};
        document.body.innerHTML = '';
        vi.spyOn(window, 'confirm').mockReturnValue(false);
    });

    afterEach(() => {
        // El diálogo unificado es un singleton de módulo: cierra lo pendiente
        // para no contaminar el test siguiente.
        const { dialogQueue, resolveDialog } = useDialog();
        while (dialogQueue.value.length > 0) resolveDialog(null);
        vi.restoreAllMocks();
        document.body.innerHTML = '';
    });

    function menuButton(wrapper: any, name: string) {
        const row = wrapper.findAll('.playlist-item').find((el: any) => el.text().includes(name));
        const btns = row!.findAll('button');
        return btns[btns.length - 1];
    }

    it('abrir el menú no borra la playlist', async () => {
        const wrapper = await mountView((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return page(mockTracks);
            if (cmd === 'delete_playlist') return null;
            return null;
        });

        await openPlaylist('Chill Vibes', wrapper);
        await menuButton(wrapper, 'Chill Vibes').trigger('click');
        await flushPromises();

        expect(document.body.querySelector('[data-testid="playlist-options-menu"]')).not.toBeNull();
        expect(window.confirm).not.toHaveBeenCalled();
        // La playlist sigue seleccionada: abrir el menú no la tocó.
        expect(wrapper.text()).toContain('Chill Vibes');
    });

    it('el menú ofrece Play, Open, Rename y Delete', async () => {
        const wrapper = await mountView((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return page(mockTracks);
            return null;
        });

        await openPlaylist('Chill Vibes', wrapper);
        await menuButton(wrapper, 'Chill Vibes').trigger('click');
        await flushPromises();

        const menu = document.body.querySelector('[data-testid="playlist-options-menu"]') as HTMLElement;
        // Cada botón lleva antes el ligature del icono Material.
        const labels = Array.from(menu.querySelectorAll('button')).map(
            b => b.textContent?.trim().replace(/^[a-z_]+/, '') ?? '',
        );
        expect(labels).toEqual(['Play', 'Open', 'Rename', 'Delete']);
    });

    it('Delete pide la confirmación unificada de la app (no window.confirm) y no borra hasta aceptar', async () => {
        const calls: { cmd: string; args: any }[] = [];
        const wrapper = await mountView((cmd, args) => {
            calls.push({ cmd, args: args ?? {} });
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return page(mockTracks);
            if (cmd === 'delete_playlist') return null;
            return null;
        });

        await openPlaylist('Chill Vibes', wrapper);
        await menuButton(wrapper, 'Chill Vibes').trigger('click');
        await flushPromises();

        const menu = document.body.querySelector('[data-testid="playlist-options-menu"]') as HTMLElement;
        const deleteBtn = Array.from(menu.querySelectorAll('button'))
            .find(b => (b.textContent ?? '').endsWith('Delete'))!;
        deleteBtn.click();
        await flushPromises();

        // R13: nada de confirm nativo; el diálogo interno queda encolado y la
        // playlist NO se borra mientras está pendiente.
        expect(window.confirm).not.toHaveBeenCalled();
        const { dialogQueue, resolveDialog } = useDialog();
        expect(dialogQueue.value.length).toBe(1);
        expect(calls.some(c => c.cmd === 'delete_playlist')).toBe(false);

        // Al aceptar el diálogo unificado, el borrado sí viaja al backend.
        resolveDialog('confirm');
        await flushPromises();
        expect(calls.some(c => c.cmd === 'delete_playlist')).toBe(true);
    });

    it('un clic fuera cierra el menú', async () => {
        const wrapper = await mountView((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return page(mockTracks);
            return null;
        });

        await openPlaylist('Chill Vibes', wrapper);
        await menuButton(wrapper, 'Chill Vibes').trigger('click');
        await flushPromises();
        expect(document.body.querySelector('[data-testid="playlist-options-menu"]')).not.toBeNull();

        document.dispatchEvent(new MouseEvent('click'));
        await flushPromises();

        expect(document.body.querySelector('[data-testid="playlist-options-menu"]')).toBeNull();
    });
});

describe('PlaylistView — acción rápida /playlists?create=1 (auditoría 46)', () => {
    beforeEach(() => {
        resetMocks();
        vi.clearAllMocks();
        document.body.innerHTML = '';
    });

    afterEach(() => {
        routerState.query = {};
        document.body.innerHTML = '';
    });

    it('montar con ?create=1 abre el formulario y limpia el query del router', async () => {
        routerState.query = { create: '1' };
        const replace = vi.fn().mockResolvedValue(undefined);
        routerState.replace = replace;

        const wrapper = await mountView((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return page(mockTracks);
            return null;
        });

        // El formulario de creación está abierto sin ningún clic. El modal es
        // un <Teleport to="body">, así que se consulta el body (texto único
        // del modal; el encabezado de la página también dice «Create Playlist»).
        expect(document.body.textContent).toContain('Make playlist public');

        // El parámetro se consume: el replace va sin `create`.
        expect(replace).toHaveBeenCalledTimes(1);
        expect(replace.mock.calls[0][0]).toEqual({ query: {} });
    });

    it('montar sin el flag no abre el formulario ni toca el query', async () => {
        routerState.query = {};
        const replace = vi.fn().mockResolvedValue(undefined);
        routerState.replace = replace;

        const wrapper = await mountView((cmd) => {
            if (cmd === 'get_playlists') return mockPlaylists;
            if (cmd === 'get_local_playlist_tracks') return page(mockTracks);
            return null;
        });

        expect(document.body.textContent).not.toContain('Make playlist public');
        expect(replace).not.toHaveBeenCalled();
    });
});
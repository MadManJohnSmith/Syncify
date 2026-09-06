/**
 * Library Composable
 * 
 * State management for library tracks and statistics.
 * 
 * @deprecated Zombie composable. LibraryView manages library state directly via libraryApi. Do not import in production views.
 */

import { ref, computed, watch } from 'vue';
import { libraryApi } from '@/api/library';
import { useAsyncState, debounce } from './useAsyncState';
import type { LibraryTrack, LibraryStats, LibraryPage } from '@/api/types';

export interface LibraryFilters {
    service: string;
    quality: string;
    downloaded: string;
    search: string;
}

const defaultFilters: LibraryFilters = {
    service: 'all',
    quality: 'all',
    downloaded: 'all',
    search: '',
};

/**
 * Composable for library state management
 * 
 * @deprecated Zombie composable. LibraryView manages library state directly via libraryApi. Do not import in production views.
 */
export function useLibrary() {
    // State
    const tracks = ref<LibraryTrack[]>([]);
    const stats = ref<LibraryStats | null>(null);
    const selectedTracks = ref<number[]>([]);
    const filters = ref<LibraryFilters>({ ...defaultFilters });
    const viewMode = ref<'list' | 'grid'>('list');
    const sortBy = ref<'title' | 'artist' | 'album' | 'added'>('title');
    const groupBy = ref<'none' | 'artist' | 'album'>('none');
    const loading = ref(false);
    const error = ref<Error | null>(null);

    // Async state for main operations
    const {
        execute: fetchTracksAsync,
        loading: tracksLoading
    } = useAsyncState(async () => {
        const page = await libraryApi.getLibrary();
        return page.tracks;
    }, []);

    const {
        execute: fetchStatsAsync,
        loading: statsLoading
    } = useAsyncState(() => libraryApi.getLibraryStats());

    // Computed
    const filteredTracks = computed(() => {
        let result = tracks.value;

        // Apply search filter
        if (filters.value.search) {
            const query = filters.value.search.toLowerCase();
            result = result.filter(t =>
                t.title.toLowerCase().includes(query) ||
                (t.artist_name && t.artist_name.toLowerCase().includes(query))
            );
        }

        return result;
    });

    const isLoading = computed(() =>
        loading.value || tracksLoading.value || statsLoading.value
    );

    const hasSelection = computed(() => selectedTracks.value.length > 0);
    const selectedCount = computed(() => selectedTracks.value.length);

    // Actions
    async function fetchTracks(): Promise<void> {
        loading.value = true;
        error.value = null;

        try {
            tracks.value = await fetchTracksAsync();
        } catch (e) {
            error.value = e instanceof Error ? e : new Error(String(e));
        } finally {
            loading.value = false;
        }
    }

    async function fetchStats(): Promise<void> {
        try {
            stats.value = await fetchStatsAsync();
        } catch (e) {
            console.error('Failed to fetch stats:', e);
        }
    }

    async function searchTracks(query: string): Promise<void> {
        if (!query.trim()) {
            await fetchTracks();
            return;
        }

        loading.value = true;
        try {
            const result = await libraryApi.searchTracks(query);
            tracks.value = result.tracks;
        } catch (e) {
            error.value = e instanceof Error ? e : new Error(String(e));
        } finally {
            loading.value = false;
        }
    }

    // Debounced search
    const debouncedSearch = debounce((query: string) => {
        searchTracks(query);
    }, 300);

    // Selection
    function selectTrack(id: number): void {
        if (!selectedTracks.value.includes(id)) {
            selectedTracks.value.push(id);
        }
    }

    function deselectTrack(id: number): void {
        selectedTracks.value = selectedTracks.value.filter(t => t !== id);
    }

    function toggleTrackSelection(id: number): void {
        if (selectedTracks.value.includes(id)) {
            deselectTrack(id);
        } else {
            selectTrack(id);
        }
    }

    function selectAll(): void {
        selectedTracks.value = tracks.value.map(t => t.id);
    }

    function clearSelection(): void {
        selectedTracks.value = [];
    }

    // Filters
    function setFilter<K extends keyof LibraryFilters>(
        key: K,
        value: LibraryFilters[K]
    ): void {
        filters.value[key] = value;
    }

    function resetFilters(): void {
        filters.value = { ...defaultFilters };
    }

    // View options
    function setViewMode(mode: 'list' | 'grid'): void {
        viewMode.value = mode;
    }

    function setSortBy(sort: typeof sortBy.value): void {
        sortBy.value = sort;
    }

    function setGroupBy(group: typeof groupBy.value): void {
        groupBy.value = group;
    }

    // Watch search filter and trigger search
    watch(
        () => filters.value.search,
        (newSearch) => {
            debouncedSearch(newSearch);
        }
    );

    // Initialize on mount
    function initialize(): void {
        fetchTracks();
        fetchStats();
    }

    return {
        // State
        tracks,
        stats,
        filters,
        filteredTracks,
        selectedTracks,
        viewMode,
        sortBy,
        groupBy,
        loading: isLoading,
        error,

        // Computed
        hasSelection,
        selectedCount,

        // Actions
        fetchTracks,
        fetchStats,
        searchTracks,
        initialize,

        // Selection
        selectTrack,
        deselectTrack,
        toggleTrackSelection,
        selectAll,
        clearSelection,

        // Filters
        setFilter,
        resetFilters,

        // View options
        setViewMode,
        setSortBy,
        setGroupBy,
    };
}

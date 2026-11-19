<script setup lang="ts">
/**
 * LibraryAlbumsView.vue — L07 (#104, spec #96): pestaña de álbumes.
 *
 * Lista navegable de álbumes con una fila por álbum que tiene al menos una
 * pista viva en el filtro. La consulta, el orden y la paginación viven en el
 * servidor (`query_library_albums`); la lista se virtualiza con el kernel
 * compartido y solo pide la página siguiente al acercarse al final. No se
 * cargan todos los álbumes ni se filtra en el cliente.
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { queryLibraryAlbums } from '@/api/library'
import { getAccounts, getServices } from '@/api/accounts'
import type {
  Account,
  AlbumCoverageFilter,
  LibraryAlbumQuery,
  LibraryAlbumRow,
  LibraryAlbumSortKey,
  LibraryAlbumsResult,
  Service,
  SortDirection,
} from '@/api/types'
import LibraryTabs from '@/components/LibraryTabs.vue'
import TrackCover from '@/components/TrackCover.vue'
import { useVirtualWindow } from '@/composables/useVirtualWindow'
import { pageQueryKey, usePagedQueryCache } from '@/composables/usePagedQueryCache'
import { onLibraryPagesInvalidated } from '@/composables/libraryCacheInvalidation'
import { useLibraryCacheInvalidation } from '@/composables/useLibraryCacheInvalidation'

defineOptions({ name: 'LibraryAlbumsView' })

const PAGE_SIZE = 100
const ROW_HEIGHT = 76
const OVERSCAN = 8
const SCROLL_THROTTLE_MS = 120
const END_OF_LIST_FOOTER_PX = 48
const TEXT_DEBOUNCE_MS = 250

const ALBUM_SORT_VALUES: readonly LibraryAlbumSortKey[] = [
  'dateAdded',
  'title',
  'artist',
  'year',
]

const route = useRoute()
const router = useRouter()

function firstQueryValue(value: unknown): string {
  if (Array.isArray(value)) {
    return typeof value[0] === 'string' ? value[0] : ''
  }
  return typeof value === 'string' ? value : ''
}

function initialSort(): LibraryAlbumSortKey {
  const raw = firstQueryValue(route?.query?.sort)
  return ALBUM_SORT_VALUES.includes(raw as LibraryAlbumSortKey)
    ? (raw as LibraryAlbumSortKey)
    : 'dateAdded'
}

const albums = ref<LibraryAlbumRow[]>([])
const total = ref(0)
const hasMore = ref(false)
const loading = ref(true)
const loadingMore = ref(false)
const error = ref<string | null>(null)
const accounts = ref<Account[]>([])
const services = ref<Service[]>([])

const text = ref(firstQueryValue(route?.query?.q))
const appliedText = ref(text.value.trim())
const accountId = ref<number | null>(null)
const serviceId = ref<number | null>(null)
const coverage = ref<AlbumCoverageFilter | ''>('')
const sort = ref<LibraryAlbumSortKey>(initialSort())
const direction = ref<SortDirection>(firstQueryValue(route?.query?.dir) === 'asc' ? 'asc' : 'desc')

const sortOptions: Array<{ value: LibraryAlbumSortKey; label: string }> = [
  { value: 'dateAdded', label: 'Date added' },
  { value: 'title', label: 'Title' },
  { value: 'artist', label: 'Artist' },
  { value: 'year', label: 'Year' },
]

const coverageOptions: Array<{ value: AlbumCoverageFilter; label: string }> = [
  { value: 'complete', label: 'Complete' },
  { value: 'partial', label: 'Partial' },
  { value: 'none', label: 'Not downloaded' },
]

function buildQuery(offset: number): LibraryAlbumQuery {
  return {
    text: appliedText.value || null,
    serviceIds: serviceId.value === null ? [] : [serviceId.value],
    accountIds: accountId.value === null ? [] : [accountId.value],
    coverage: coverage.value ? [coverage.value] : [],
    sort: sort.value,
    direction: direction.value,
    offset,
    limit: PAGE_SIZE,
  }
}

// L13 (#110): caché de páginas por clave y deduplicación de peticiones en
// vuelo. La invalidación es explícita por evento; no hay TTL.
const albumPages = usePagedQueryCache<LibraryAlbumQuery, LibraryAlbumsResult>({
  keyOf: pageQueryKey,
  fetchPage: queryLibraryAlbums,
})
useLibraryCacheInvalidation()

let requestSeq = 0

async function loadPage(refresh: boolean): Promise<void> {
  const seq = ++requestSeq
  const offset = refresh ? 0 : albums.value.length
  if (refresh) {
    loading.value = true
    error.value = null
  } else {
    loadingMore.value = true
  }

  try {
    const result = await albumPages.load(buildQuery(offset))
    if (seq !== requestSeq) return
    // Una invalidación en vuelo no se pinta: el evento ya programó la recarga.
    if (result.generation !== albumPages.generation.value) return
    albums.value = refresh ? result.page.albums : albums.value.concat(result.page.albums)
    total.value = result.page.total
    hasMore.value = result.page.hasMore
  } catch (e) {
    if (seq !== requestSeq) return
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    if (seq === requestSeq) {
      loading.value = false
      loadingMore.value = false
    }
  }
}

function loadMore(): void {
  if (!hasMore.value || loading.value || loadingMore.value) return
  void loadPage(false)
}

function reload(): void {
  resetVirtualScroll()
  void loadPage(true)
}

let reloadTimer: ReturnType<typeof setTimeout> | null = null
function scheduleReload(): void {
  if (reloadTimer) clearTimeout(reloadTimer)
  reloadTimer = setTimeout(() => {
    reloadTimer = null
    reload()
  }, 350)
}

const offLibraryInvalidated = onLibraryPagesInvalidated(() => {
  albumPages.invalidate()
  scheduleReload()
})

onUnmounted(() => {
  offLibraryInvalidated()
  if (reloadTimer) clearTimeout(reloadTimer)
})

/** Texto y orden comparten URL con la pestaña de pistas (L03). Cuenta,
 * servicio y cobertura son propios de esta lista y no se escriben. */
function syncUrl(): void {
  if (!route) return
  const query: Record<string, string> = {}
  if (appliedText.value) query.q = appliedText.value
  if (sort.value !== 'dateAdded') query.sort = sort.value
  if (direction.value !== 'desc') query.dir = direction.value
  void router.replace({ path: route.path, query })
}

let textTimer: ReturnType<typeof setTimeout> | null = null
watch(text, (value) => {
  if (textTimer) clearTimeout(textTimer)
  textTimer = setTimeout(() => {
    textTimer = null
    appliedText.value = value.trim()
  }, TEXT_DEBOUNCE_MS)
})

watch([appliedText, accountId, serviceId, coverage, sort, direction], () => {
  syncUrl()
  reload()
})

const {
  containerRef,
  totalHeight: totalVirtualHeight,
  offsetY: virtualOffsetY,
  visibleItems: visibleAlbums,
  handleScroll,
  reset: resetVirtualScroll,
} = useVirtualWindow(albums, {
  rowHeight: ROW_HEIGHT,
  overscan: OVERSCAN,
  footerHeight: () => (hasMore.value ? 0 : END_OF_LIST_FOOTER_PX),
  scrollThrottleMs: SCROLL_THROTTLE_MS,
  onScrollMeasured: (scrollBottom) => {
    if (scrollBottom < 300) loadMore()
  },
})

const isEmpty = computed(() => !loading.value && !error.value && albums.value.length === 0)

function openAlbum(albumId: number): void {
  void router.push({ name: 'AlbumDetail', params: { id: String(albumId) } })
}

function formatCoverage(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return '—'
  return `${Math.round(value * 100)}%`
}

function coverageWidth(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return '0%'
  return `${Math.max(0, Math.min(100, value * 100))}%`
}

function accountLabel(account: Account): string {
  return account.display_name || account.service_name || `Account ${account.id}`
}

function serviceLabel(service: Service): string {
  return service.display_name || service.name
}

onMounted(async () => {
  // La URL es la fuente del texto/orden compartidos; se normaliza al montar
  // (un sort de pistas que no aplica a álbumes cae a fecha añadida).
  syncUrl()
  // Las facetas de filtro son opcionales: si fallan, la lista sigue
  // funcionando sin esos selects (no se inventan valores).
  void getAccounts()
    .then((rows) => {
      accounts.value = rows
    })
    .catch(() => undefined)
  void getServices()
    .then((rows) => {
      services.value = rows
    })
    .catch(() => undefined)
  await loadPage(true)
})
</script>

<template>
  <div class="library-albums-page h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    <header class="px-8 pt-6 pb-2 shrink-0 flex items-end justify-between flex-wrap gap-4">
      <div>
        <div class="flex items-baseline gap-4">
          <h1 class="text-3xl font-bold tracking-tight text-ink">Albums</h1>
          <span class="text-lg text-text-secondary font-medium" data-testid="library-albums-total">
            {{ total.toLocaleString() }} Albums
          </span>
        </div>
        <p class="text-text-secondary mt-1">One row per album with at least one live track in the library</p>
      </div>
    </header>

    <LibraryTabs current="albums" />

    <div class="library-album-filters px-8 py-3 flex items-center gap-2 flex-wrap shrink-0" data-testid="albums-filters">
      <input
        v-model="text"
        type="search"
        data-testid="albums-filter-text"
        placeholder="Filter by title, artist, album..."
        class="px-3 py-1.5 rounded-lg border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-ink w-64"
      />
      <select
        v-model="accountId"
        data-testid="albums-filter-account"
        aria-label="Filter by account"
        class="px-3 py-1.5 rounded-lg border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-ink"
      >
        <option :value="null">All accounts</option>
        <option v-for="account in accounts" :key="account.id" :value="account.id">
          {{ accountLabel(account) }}
        </option>
      </select>
      <select
        v-model="serviceId"
        data-testid="albums-filter-service"
        aria-label="Filter by service"
        class="px-3 py-1.5 rounded-lg border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-ink"
      >
        <option :value="null">All services</option>
        <option v-for="service in services" :key="service.id" :value="service.id">
          {{ serviceLabel(service) }}
        </option>
      </select>
      <select
        v-model="coverage"
        data-testid="albums-filter-coverage"
        aria-label="Filter by download coverage"
        class="px-3 py-1.5 rounded-lg border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-ink"
      >
        <option value="">Any coverage</option>
        <option v-for="option in coverageOptions" :key="option.value" :value="option.value">
          {{ option.label }}
        </option>
      </select>
      <select
        v-model="sort"
        data-testid="albums-sort"
        aria-label="Sort albums"
        class="px-3 py-1.5 rounded-lg border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-ink"
      >
        <option v-for="option in sortOptions" :key="option.value" :value="option.value">
          {{ option.label }}
        </option>
      </select>
      <button
        type="button"
        data-testid="albums-direction"
        class="px-3 py-1.5 rounded-lg border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-ink"
        :title="direction === 'desc' ? 'Descending (click for ascending)' : 'Ascending (click for descending)'"
        @click="direction = direction === 'desc' ? 'asc' : 'desc'"
      >
        <span class="material-symbols-outlined text-[16px] align-middle">{{ direction === 'desc' ? 'arrow_downward' : 'arrow_upward' }}</span>
      </button>
    </div>

    <div v-if="error" class="px-8 py-6" data-testid="albums-error">
      <p class="text-sm font-medium text-error">Could not load albums</p>
      <p class="text-xs text-text-secondary mt-1">{{ error }}</p>
      <button type="button" class="mt-3 px-3 py-1.5 rounded-lg bg-primary text-on-accent text-sm" @click="reload">
        Try again
      </button>
    </div>

    <div v-else-if="loading" class="flex-1 flex items-center justify-center text-text-secondary" data-testid="albums-loading">
      <span class="material-symbols-outlined animate-spin mr-2">progress_activity</span>
      Loading albums…
    </div>

    <div v-else-if="isEmpty" class="flex-1 flex items-center justify-center px-8 text-center" data-testid="albums-empty">
      <div>
        <span class="material-symbols-outlined text-4xl text-gray-300">album</span>
        <p class="text-sm text-text-secondary mt-2">No albums with live tracks match this filter.</p>
      </div>
    </div>

    <div
      v-else
      ref="containerRef"
      data-testid="albums-scroll"
      class="flex-1 min-h-0 overflow-y-auto custom-scrollbar border-t border-gray-200 dark:border-border-dark relative"
      style="contain: strict;"
      @scroll="handleScroll"
    >
      <div :style="{ height: totalVirtualHeight + 'px', position: 'relative', width: '100%' }">
        <div :style="{ transform: `translateY(${virtualOffsetY}px)`, position: 'absolute', top: 0, left: 0, right: 0 }">
          <div
            v-for="album in visibleAlbums"
            :key="album.albumId"
            data-testid="album-row"
            :data-album-id="album.albumId"
            :style="{ height: ROW_HEIGHT + 'px' }"
            class="album-row flex items-center gap-3 px-6 py-2 border-b border-gray-100 dark:border-border-dark/50 box-border cursor-pointer hover:bg-gray-50 dark:hover:bg-surface-highlight/30"
            @click="openAlbum(album.albumId)"
          >
            <TrackCover
              :src="album.coverArtUrl"
              :item-id="album.albumId"
              :alt="album.title"
              size-class="w-12 h-12"
              rounded="rounded-lg"
              icon="album"
            />
            <div class="w-1/3 min-w-[180px]">
              <p class="text-sm font-semibold text-ink truncate">{{ album.title }}</p>
              <p class="text-xs text-text-secondary truncate">
                {{ album.artistName || 'Unknown artist' }}
                <span v-if="album.releaseYear"> · {{ album.releaseYear }}</span>
              </p>
            </div>
            <div class="w-32 text-xs text-text-secondary shrink-0">
              <p>{{ album.trackCount }} track{{ album.trackCount === 1 ? '' : 's' }}</p>
              <p>{{ album.downloadedCount }} downloaded</p>
            </div>
            <div class="w-28 shrink-0" data-testid="album-coverage">
              <div class="h-1.5 rounded-full bg-gray-200 dark:bg-surface-highlight overflow-hidden">
                <div class="h-full bg-success" :style="{ width: coverageWidth(album.coverage) }"></div>
              </div>
              <p class="text-[11px] text-text-secondary mt-1">{{ formatCoverage(album.coverage) }} downloaded</p>
            </div>
            <div class="flex items-center gap-1 flex-wrap w-36 shrink-0" data-testid="album-accounts">
              <span
                v-for="account in album.accounts"
                :key="account.accountId"
                class="px-1.5 py-0.5 rounded bg-primary/10 text-primary text-[10px] font-semibold truncate max-w-[8rem]"
                :title="account.serviceName"
              >{{ account.displayName || 'Unknown account' }}</span>
            </div>
            <div class="flex items-center gap-1 flex-wrap w-36 shrink-0" data-testid="album-services">
              <span
                v-for="service in album.services"
                :key="service.serviceId"
                class="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-surface-highlight text-text-secondary text-[10px] font-semibold"
              >{{ service.serviceName }}</span>
            </div>
            <span class="material-symbols-outlined text-gray-400 ml-auto">chevron_right</span>
          </div>
        </div>
      </div>
      <div v-if="loadingMore" class="py-2 text-center text-xs text-text-secondary">Loading more albums…</div>
    </div>
  </div>
</template>

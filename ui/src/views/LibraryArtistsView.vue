<script setup lang="ts">
/**
 * LibraryArtistsView.vue — L08 (#105, spec #96): pestaña de artistas.
 *
 * Lista navegable de artistas con una fila por artista que tiene al menos una
 * pista viva en el filtro. La consulta, el orden y la paginación viven en el
 * servidor (`query_library_artists`); la lista se virtualiza con el kernel
 * compartido y solo pide la página siguiente al acercarse al final. No se
 * cargan todos los artistas ni se filtra en el cliente.
 */
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { queryLibraryArtists } from '@/api/library'
import { getAccounts, getServices } from '@/api/accounts'
import type {
  Account,
  LibraryArtistQuery,
  LibraryArtistRow,
  LibraryArtistSortKey,
  LibraryArtistsResult,
  Service,
  SortDirection,
} from '@/api/types'
import LibraryTabs from '@/components/LibraryTabs.vue'
import TrackCover from '@/components/TrackCover.vue'
import { useVirtualWindow } from '@/composables/useVirtualWindow'
import { pageQueryKey, usePagedQueryCache } from '@/composables/usePagedQueryCache'
import { onLibraryPagesInvalidated } from '@/composables/libraryCacheInvalidation'
import { useLibraryCacheInvalidation } from '@/composables/useLibraryCacheInvalidation'

defineOptions({ name: 'LibraryArtistsView' })

const PAGE_SIZE = 100
const ROW_HEIGHT = 76
const OVERSCAN = 8
const SCROLL_THROTTLE_MS = 120
const END_OF_LIST_FOOTER_PX = 48
const TEXT_DEBOUNCE_MS = 250

const ARTIST_SORT_VALUES: readonly LibraryArtistSortKey[] = [
  'name',
  'tracks',
  'dateAdded',
]

const route = useRoute()
const router = useRouter()

function firstQueryValue(value: unknown): string {
  if (Array.isArray(value)) {
    return typeof value[0] === 'string' ? value[0] : ''
  }
  return typeof value === 'string' ? value : ''
}

function initialSort(): LibraryArtistSortKey {
  const raw = firstQueryValue(route?.query?.sort)
  return ARTIST_SORT_VALUES.includes(raw as LibraryArtistSortKey)
    ? (raw as LibraryArtistSortKey)
    : 'name'
}

const artists = ref<LibraryArtistRow[]>([])
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
const sort = ref<LibraryArtistSortKey>(initialSort())
/** Una lista de artistas se lee A→Z salvo que la URL diga lo contrario. */
const direction = ref<SortDirection>(firstQueryValue(route?.query?.dir) === 'desc' ? 'desc' : 'asc')

const sortOptions: Array<{ value: LibraryArtistSortKey; label: string }> = [
  { value: 'name', label: 'Name' },
  { value: 'tracks', label: 'Tracks' },
  { value: 'dateAdded', label: 'Date added' },
]

function buildQuery(offset: number): LibraryArtistQuery {
  return {
    text: appliedText.value || null,
    serviceIds: serviceId.value === null ? [] : [serviceId.value],
    accountIds: accountId.value === null ? [] : [accountId.value],
    sort: sort.value,
    direction: direction.value,
    offset,
    limit: PAGE_SIZE,
  }
}

// L13 (#110): misma caché de páginas por clave, deduplicación en vuelo e
// invalidación explícita por evento que la pestaña de álbumes y de pistas.
const artistPages = usePagedQueryCache<LibraryArtistQuery, LibraryArtistsResult>({
  keyOf: pageQueryKey,
  fetchPage: queryLibraryArtists,
})
useLibraryCacheInvalidation()

let requestSeq = 0

async function loadPage(refresh: boolean): Promise<void> {
  const seq = ++requestSeq
  const offset = refresh ? 0 : artists.value.length
  if (refresh) {
    loading.value = true
    error.value = null
  } else {
    loadingMore.value = true
  }

  try {
    const result = await artistPages.load(buildQuery(offset))
    if (seq !== requestSeq) return
    // Una invalidación en vuelo no se pinta: el evento ya programó la recarga.
    if (result.generation !== artistPages.generation.value) return
    artists.value = refresh ? result.page.artists : artists.value.concat(result.page.artists)
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
  artistPages.invalidate()
  scheduleReload()
})

onUnmounted(() => {
  offLibraryInvalidated()
  if (reloadTimer) clearTimeout(reloadTimer)
})

/** Texto y orden comparten URL con la pestaña de pistas (L03). Cuenta y
 * servicio son propios de esta lista y no se escriben. */
function syncUrl(): void {
  if (!route) return
  const query: Record<string, string> = {}
  if (appliedText.value) query.q = appliedText.value
  if (sort.value !== 'name') query.sort = sort.value
  if (direction.value !== 'asc') query.dir = direction.value
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

watch([appliedText, accountId, serviceId, sort, direction], () => {
  syncUrl()
  reload()
})

const {
  containerRef,
  totalHeight: totalVirtualHeight,
  offsetY: virtualOffsetY,
  visibleItems: visibleArtists,
  handleScroll,
  reset: resetVirtualScroll,
} = useVirtualWindow(artists, {
  rowHeight: ROW_HEIGHT,
  overscan: OVERSCAN,
  footerHeight: () => (hasMore.value ? 0 : END_OF_LIST_FOOTER_PX),
  scrollThrottleMs: SCROLL_THROTTLE_MS,
  onScrollMeasured: (scrollBottom) => {
    if (scrollBottom < 300) loadMore()
  },
})

const isEmpty = computed(() => !loading.value && !error.value && artists.value.length === 0)

function openArtist(artistId: number): void {
  void router.push({ name: 'ArtistDetail', params: { id: String(artistId) } })
}

function accountLabel(account: Account): string {
  return account.display_name || account.service_name || `Account ${account.id}`
}

function serviceLabel(service: Service): string {
  return service.display_name || service.name
}

onMounted(async () => {
  // La URL es la fuente del texto/orden compartidos; se normaliza al montar
  // (un sort de pistas que no aplica a artistas cae a nombre A→Z).
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
  <div class="library-artists-page h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    <header class="px-8 pt-6 pb-2 shrink-0 flex items-end justify-between flex-wrap gap-4">
      <div>
        <div class="flex items-baseline gap-4">
          <h1 class="text-3xl font-bold tracking-tight text-ink">Artists</h1>
          <span class="text-lg text-text-secondary font-medium" data-testid="library-artists-total">
            {{ total.toLocaleString() }} Artists
          </span>
        </div>
        <p class="text-text-secondary mt-1">One row per artist with at least one live track in the library</p>
      </div>
    </header>

    <LibraryTabs current="artists" />

    <div class="library-artist-filters px-8 py-3 flex items-center gap-2 flex-wrap shrink-0" data-testid="artists-filters">
      <input
        v-model="text"
        type="search"
        data-testid="artists-filter-text"
        placeholder="Filter by name, track, album..."
        class="px-3 py-1.5 rounded-lg border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-ink w-64"
      />
      <select
        v-model="accountId"
        data-testid="artists-filter-account"
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
        data-testid="artists-filter-service"
        aria-label="Filter by service"
        class="px-3 py-1.5 rounded-lg border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-ink"
      >
        <option :value="null">All services</option>
        <option v-for="service in services" :key="service.id" :value="service.id">
          {{ serviceLabel(service) }}
        </option>
      </select>
      <select
        v-model="sort"
        data-testid="artists-sort"
        aria-label="Sort artists"
        class="px-3 py-1.5 rounded-lg border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-ink"
      >
        <option v-for="option in sortOptions" :key="option.value" :value="option.value">
          {{ option.label }}
        </option>
      </select>
      <button
        type="button"
        data-testid="artists-direction"
        class="px-3 py-1.5 rounded-lg border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark text-sm text-ink"
        :title="direction === 'asc' ? 'Ascending (click for descending)' : 'Descending (click for ascending)'"
        @click="direction = direction === 'asc' ? 'desc' : 'asc'"
      >
        <span class="material-symbols-outlined text-[16px] align-middle">{{ direction === 'asc' ? 'arrow_upward' : 'arrow_downward' }}</span>
      </button>
    </div>

    <div v-if="error" class="px-8 py-6" data-testid="artists-error">
      <p class="text-sm font-medium text-error">Could not load artists</p>
      <p class="text-xs text-text-secondary mt-1">{{ error }}</p>
      <button type="button" class="mt-3 px-3 py-1.5 rounded-lg bg-primary text-on-accent text-sm" @click="reload">
        Try again
      </button>
    </div>

    <div v-else-if="loading" class="flex-1 flex items-center justify-center text-text-secondary" data-testid="artists-loading">
      <span class="material-symbols-outlined animate-spin mr-2">progress_activity</span>
      Loading artists…
    </div>

    <div v-else-if="isEmpty" class="flex-1 flex items-center justify-center px-8 text-center" data-testid="artists-empty">
      <div>
        <span class="material-symbols-outlined text-4xl text-gray-300">person</span>
        <p class="text-sm text-text-secondary mt-2">No artists with live tracks match this filter.</p>
      </div>
    </div>

    <div
      v-else
      ref="containerRef"
      data-testid="artists-scroll"
      class="flex-1 min-h-0 overflow-y-auto custom-scrollbar border-t border-gray-200 dark:border-border-dark relative"
      style="contain: strict;"
      @scroll="handleScroll"
    >
      <div :style="{ height: totalVirtualHeight + 'px', position: 'relative', width: '100%' }">
        <div :style="{ transform: `translateY(${virtualOffsetY}px)`, position: 'absolute', top: 0, left: 0, right: 0 }">
          <div
            v-for="artist in visibleArtists"
            :key="artist.artistId"
            data-testid="artist-row"
            :data-artist-id="artist.artistId"
            :style="{ height: ROW_HEIGHT + 'px' }"
            class="artist-row flex items-center gap-3 px-6 py-2 border-b border-gray-100 dark:border-border-dark/50 box-border cursor-pointer hover:bg-gray-50 dark:hover:bg-surface-highlight/30"
            @click="openArtist(artist.artistId)"
          >
            <TrackCover
              :src="artist.imageUrl"
              :item-id="artist.artistId"
              :alt="artist.name"
              size-class="w-12 h-12"
              rounded="rounded-full"
              icon="person"
            />
            <div class="w-1/4 min-w-[160px]">
              <p class="text-sm font-semibold text-ink truncate">{{ artist.name }}</p>
              <p class="text-xs text-text-secondary">
                {{ artist.trackCount }} track{{ artist.trackCount === 1 ? '' : 's' }}
                · {{ artist.albumCount }} album{{ artist.albumCount === 1 ? '' : 's' }}
                · {{ artist.collaborationCount }} collaboration{{ artist.collaborationCount === 1 ? '' : 's' }}
              </p>
            </div>
            <div class="flex items-center gap-1 flex-wrap w-40 shrink-0" data-testid="artist-accounts">
              <span
                v-for="account in artist.accounts"
                :key="account.accountId"
                class="px-1.5 py-0.5 rounded bg-primary/10 text-primary text-[10px] font-semibold truncate max-w-[8rem]"
                :title="account.serviceName"
              >{{ account.displayName || 'Unknown account' }}</span>
            </div>
            <div class="flex items-center gap-1 flex-wrap w-36 shrink-0" data-testid="artist-services">
              <span
                v-for="service in artist.services"
                :key="service.serviceId"
                class="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-surface-highlight text-text-secondary text-[10px] font-semibold"
              >{{ service.serviceName }}</span>
            </div>
            <span class="text-xs text-text-secondary ml-auto" data-testid="artist-date-added">{{ artist.dateAdded || '' }}</span>
            <span class="material-symbols-outlined text-gray-400">chevron_right</span>
          </div>
        </div>
      </div>
      <div v-if="loadingMore" class="py-2 text-center text-xs text-text-secondary">Loading more artists…</div>
    </div>
  </div>
</template>

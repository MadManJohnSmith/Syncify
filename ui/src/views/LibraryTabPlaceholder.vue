<script setup lang="ts">
/**
 * LibraryTabPlaceholder.vue — L03 (#100, spec #96): armazón de las pestañas
 * /library/albums, /library/artists y /library/duplicates. El contenido NO
 * se simula: cada pestaña muestra deshabilitado el motivo de su carril
 * (álbumes → L07, artistas → L08, duplicados → L10) y un enlace de vuelta a
 * la vista de pistas. No llama a `invoke`: no lee la biblioteca.
 */
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import LibraryTabs from '@/components/LibraryTabs.vue'
import { LIBRARY_TABS, type LibraryTabId } from '@/composables/libraryUrlState'

defineOptions({ name: 'LibraryTabPlaceholder' })

const route = useRoute()

const PLACEHOLDER_REASONS: Record<LibraryTabId, string> = {
  tracks: '',
  albums:
    'The albums tab arrives with L07 (#104): a server-side query_library_albums with download coverage and its own orders. The album membership it builds on lands with L00 (#97). Nothing is simulated here.',
  artists:
    'The artists tab arrives with L08 (#105): a server-side query_library_artists with paginated detail and collaborations. The artist membership it builds on lands with L00 (#97). Nothing is simulated here.',
  duplicates:
    'The duplicates view arrives with L10 (#73): clusters over ISRC and acoustic fingerprint identity, with manual merge, record and undo. Nothing is simulated here.',
}

const tabId = computed<LibraryTabId>(() => {
  const meta = route.meta.libraryTab
  return typeof meta === 'string' && LIBRARY_TABS.some((t) => t.id === meta)
    ? (meta as LibraryTabId)
    : 'albums'
})

const meta = computed(() => LIBRARY_TABS.find((t) => t.id === tabId.value)!)
const reason = computed(() => PLACEHOLDER_REASONS[tabId.value])
</script>

<template>
  <div class="h-full flex flex-col bg-background-light dark:bg-background-dark overflow-hidden">
    <LibraryTabs :current="tabId" />

    <div class="flex-1 flex items-center justify-center px-8">
      <div
        data-testid="library-tab-placeholder"
        :data-tab="tabId"
        class="max-w-md text-center space-y-3"
      >
        <span class="material-symbols-outlined text-4xl text-gray-300 dark:text-gray-600" aria-hidden="true">{{ tabId === 'duplicates' ? 'content_copy' : 'grid_view' }}</span>
        <h1 class="text-2xl font-bold text-ink">{{ meta.label }}</h1>
        <p
          data-testid="library-tab-reason"
          class="text-sm text-text-secondary"
          role="note"
        >{{ reason }}</p>
        <RouterLink
          to="/library"
          data-testid="library-tab-back"
          class="inline-block px-4 py-2 rounded-lg text-sm font-semibold bg-primary text-on-accent shadow-xs"
        >Back to tracks</RouterLink>
      </div>
    </div>
  </div>
</template>

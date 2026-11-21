<script setup lang="ts">
/**
 * LibraryTabs.vue — L03 (#100, spec #96): armazón de pestañas de la
 * biblioteca. Solo navegación: las pestañas de álbumes, artistas y
 * duplicados apuntan a sus rutas de armazón, cuyo contenido completan
 * L07, L08 y L10. El cambio de pestaña conserva los filtros compatibles
 * (`libraryTabQuery`) y descarta los que no aplican.
 */
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { LIBRARY_TABS, libraryTabQuery, type LibraryTabId } from '@/composables/libraryUrlState'

defineOptions({ name: 'LibraryTabs' })

const props = defineProps<{ current: LibraryTabId }>()

const route = useRoute()

const tabs = LIBRARY_TABS

function targetFor(tab: (typeof LIBRARY_TABS)[number]) {
  // La pestaña de pistas hace round-trip del query completo (es su propio
  // estado); las demás solo se llevan los filtros compatibles.
  const query = route?.query ?? {}
  return tab.id === 'tracks'
    ? { path: tab.path, query }
    : { path: tab.path, query: libraryTabQuery(tab.id, query) }
}

function isCurrent(tab: (typeof LIBRARY_TABS)[number]): boolean {
  return tab.id === props.current
}
</script>

<template>
  <nav class="library-tabs px-8 flex items-center gap-1 border-b border-gray-200 dark:border-border-dark shrink-0" data-testid="library-tabs" aria-label="Library sections">
    <RouterLink
      v-for="tab in tabs"
      :key="tab.id"
      :to="targetFor(tab)"
      :data-testid="`library-tab-${tab.id}`"
      :aria-current="isCurrent(tab) ? 'page' : undefined"
      :class="[
        'px-3 py-2 text-sm font-semibold border-b-2 -mb-px transition-colors',
        isCurrent(tab)
          ? 'border-primary text-primary'
          : 'border-transparent text-text-secondary hover:text-ink'
      ]"
    >{{ tab.label }}</RouterLink>
  </nav>
</template>

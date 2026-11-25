<template>
  <div
    :class="['playlist-cover shrink-0 overflow-hidden flex items-center justify-center', rounded, sizeClass, fallbackGradient]"
    :style="wrapperStyle"
    data-testid="playlist-cover"
    :data-cover-kind="showProvider ? 'provider' : tiles.length ? 'mosaic' : 'fallback'"
    :data-tile-count="tiles.length"
  >
    <img
      v-if="showProvider"
      :src="src ?? ''"
      :alt="alt"
      class="w-full h-full object-cover"
      loading="lazy"
      decoding="async"
      data-testid="playlist-cover-provider-img"
      @error="brokenSrc = true"
    />
    <div
      v-else-if="tiles.length"
      :class="['grid w-full h-full', mosaicLayout]"
      data-testid="playlist-cover-mosaic"
    >
      <TrackCover
        v-for="(tile, index) in tiles"
        :key="`${tile.itemId}:${index}`"
        :src="tile.src"
        :item-id="tile.itemId"
        :alt="index === 0 ? alt : ''"
        size-class="w-full h-full"
        rounded=""
        :class="{ 'col-span-2': tiles.length === 3 && index === 2 }"
      />
    </div>
    <span
      v-else
      class="material-symbols-outlined text-text-secondary select-none"
      :style="{ fontSize: iconSize }"
      aria-hidden="true"
    >{{ icon }}</span>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { getArtGradient } from '@/utils/artGradient'
import type { PlaylistMosaicTile } from '@/utils/playlistCatalog'
import TrackCover from '@/components/TrackCover.vue'

const props = withDefaults(defineProps<{
    src?: string | null
    tiles?: PlaylistMosaicTile[]
    alt?: string
    itemId?: number | null
    size?: number
    sizeClass?: string
    rounded?: string
    icon?: string
}>(), {
    src: null,
    tiles: () => [],
    alt: '',
    itemId: null,
    size: 40,
    sizeClass: '',
    rounded: 'rounded-lg',
    icon: 'queue_music',
})

const brokenSrc = ref(false)
watch(() => props.src, () => { brokenSrc.value = false })

const showProvider = computed(() => Boolean(props.src) && !brokenSrc.value)
const fallbackGradient = computed(() => showProvider.value || props.tiles.length ? '' : getArtGradient(props.itemId))
const wrapperStyle = computed(() => props.sizeClass ? {} : { width: `${props.size}px`, height: `${props.size}px` })
const iconSize = computed(() => `${Math.round(props.size * 0.5)}px`)
const mosaicLayout = computed(() => {
    if (props.tiles.length <= 1) return 'grid-cols-1 grid-rows-1'
    if (props.tiles.length === 2) return 'grid-cols-2 grid-rows-1'
    return 'grid-cols-2 grid-rows-2'
})
</script>

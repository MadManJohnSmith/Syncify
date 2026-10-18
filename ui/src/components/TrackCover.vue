<template>
  <div
    :class="['track-cover shrink-0 overflow-hidden flex items-center justify-center', rounded, sizeClass, gradient]"
    :style="wrapperStyle"
    data-testid="track-cover"
  >
    <img
      v-if="showImage"
      :src="resolvedSrc"
      :alt="alt ?? ''"
      class="w-full h-full object-cover"
      loading="lazy"
      decoding="async"
      data-testid="track-cover-img"
      @error="brokenSrc = true"
    />
    <span
      v-else-if="icon"
      class="material-symbols-outlined text-text-secondary select-none"
      :style="{ fontSize: iconSize }"
      aria-hidden="true"
    >{{ icon }}</span>
  </div>
</template>

<script setup lang="ts">
/**
 * Reusable track/album cover art (R14).
 *
 * A single place decides how a cover looks: if there is a URL it is rendered,
 * and if not —or if the URL exists but the file is no longer on disk— it falls
 * back to the track's deterministic gradient. Views only pass `src`, `alt` and
 * the size.
 */
import { computed, ref, watch } from 'vue'
import { getArtGradient } from '@/utils/artGradient'

const props = withDefaults(defineProps<{
    src?: string | null
    alt?: string
    /** Element id: fixes the fallback gradient. */
    itemId?: number | null
    /** Slot size in px (width and height). */
    size?: number
    /** Tailwind classes that set the size; they win over `size`. */
    sizeClass?: string
    /** Rounding; `rounded-md` by default like the rest of the list. */
    rounded?: string
    /** Material icon shown while there is no image. */
    icon?: string
}>(), {
    src: null,
    alt: '',
    itemId: null,
    size: 40,
    sizeClass: '',
    rounded: 'rounded-md',
    icon: 'music_note',
})

const brokenSrc = ref(false)

// A new URL deserves a fresh attempt: without this, reusing the row after
// fixing the cover art would leave the gradient stuck forever.
watch(() => props.src, () => { brokenSrc.value = false })

const showImage = computed(() => Boolean(props.src) && !brokenSrc.value)
const resolvedSrc = computed(() => (showImage.value ? (props.src as string) : ''))
const gradient = computed(() => (showImage.value ? '' : getArtGradient(props.itemId)))
const wrapperStyle = computed(() => (props.sizeClass ? {} : { width: `${props.size}px`, height: `${props.size}px` }))
const iconSize = computed(() => `${Math.round(props.size * 0.5)}px`)
</script>
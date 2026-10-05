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
 * Portada reutilizable de pista/álbum (R14).
 *
 * Un solo sitio decide cómo se ve una carátula: si hay URL se pinta, y si no
 * —o si la URL existe pero el fichero ya no está en disco— cae al gradiente
 * determinista de la pista. Las vistas solo pasan `src`, `alt` y el tamaño.
 */
import { computed, ref, watch } from 'vue'
import { getArtGradient } from '@/utils/artGradient'

const props = withDefaults(defineProps<{
    src?: string | null
    alt?: string
    /** Id del elemento: fija el gradiente de reserva. */
    itemId?: number | null
    /** Tamaño del hueco en px (ancho y alto). */
    size?: number
    /** Clases de Tailwind que fijan el tamaño; ganan sobre `size`. */
    sizeClass?: string
    /** Redondeo; por defecto `rounded-md` como el resto de la lista. */
    rounded?: string
    /** Icono Material mostrado mientras no hay imagen. */
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

// Una URL nueva merece un intento nuevo: sin esto, reutilizar la fila tras
// corregir la carátula dejaría el gradiente pegado para siempre.
watch(() => props.src, () => { brokenSrc.value = false })

const showImage = computed(() => Boolean(props.src) && !brokenSrc.value)
const resolvedSrc = computed(() => (showImage.value ? (props.src as string) : ''))
const gradient = computed(() => (showImage.value ? '' : getArtGradient(props.itemId)))
const wrapperStyle = computed(() => (props.sizeClass ? {} : { width: `${props.size}px`, height: `${props.size}px` }))
const iconSize = computed(() => `${Math.round(props.size * 0.5)}px`)
</script>
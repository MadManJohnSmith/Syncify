<template>
  <section class="space-y-2" data-testid="ediciones-de-pista">
    <button
      type="button"
      class="flex w-full items-center justify-between gap-3 text-left"
      :aria-expanded="abierto"
      data-testid="ediciones-toggle"
      @click="abierto = !abierto"
    >
      <span class="text-sm font-semibold text-gray-900 dark:text-white">Ediciones</span>
      <span class="flex items-center gap-2">
        <span class="text-xs text-gray-500 dark:text-gray-400" data-testid="ediciones-resumen">
          {{ resumen }}
        </span>
        <svg
          class="h-4 w-4 text-gray-400 transition-transform"
          :class="{ 'rotate-180': abierto }"
          viewBox="0 0 20 20"
          fill="currentColor"
          aria-hidden="true"
        >
          <path
            fill-rule="evenodd"
            d="M5.23 7.21a.75.75 0 011.06.02L10 11.17l3.71-3.94a.75.75 0 111.08 1.04l-4.25 4.5a.75.75 0 01-1.08 0l-4.25-4.5a.75.75 0 01.02-1.06z"
            clip-rule="evenodd"
          />
        </svg>
      </span>
    </button>

    <div v-if="abierto" class="space-y-2" data-testid="ediciones-lista">
      <p class="text-xs text-gray-500 dark:text-gray-400">
        Tu biblioteca conserva una sola versión de esta grabación: su álbum no cambia.
        <strong>Confirmar</strong> la marca como comprobada para que deje de contar como
        pendiente; <strong>Descartar</strong> la olvida, y si el servicio la vuelve a traer,
        aparecerá de nuevo.
      </p>

      <p v-if="loading" class="text-sm text-gray-500 dark:text-gray-400">Cargando…</p>
      <p
        v-else-if="error"
        class="text-sm text-red-600 dark:text-red-400"
        data-testid="ediciones-error"
      >
        {{ error }}
      </p>
      <p
        v-else-if="ediciones.length === 0"
        class="text-sm text-gray-500 dark:text-gray-400"
        data-testid="ediciones-vacio"
      >
        Sin otras apariciones registradas.
      </p>
      <ul v-else class="divide-y divide-gray-100 dark:divide-border-dark">
        <li
          v-for="edicion in ediciones"
          :key="`${edicion.service}:${edicion.service_track_id}`"
          class="py-2"
          data-testid="edicion"
          :title="`${edicion.service} · ${edicion.service_track_id}`"
        >
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0">
              <p class="truncate text-sm text-gray-900 dark:text-white" data-testid="edicion-album">
                {{ edicion.album ?? 'Sin datos de álbum (aparición antigua)' }}
              </p>
              <p class="text-xs text-gray-500 dark:text-gray-400">
                {{ nombreDelServicio(edicion.service) }} ·
                <span :data-testid="`edicion-estado-${edicion.validation_status}`">
                  {{ edicion.validation_status === 'verified' ? 'Comprobada' : 'Por revisar' }}
                </span>
              </p>
            </div>
            <div class="flex shrink-0 items-center gap-2">
              <button
                v-if="edicion.validation_status !== 'verified'"
                type="button"
                :disabled="resolviendo === edicion.item_id"
                class="text-xs font-medium text-emerald-600 hover:text-emerald-700 disabled:opacity-50"
                data-testid="edicion-confirmar"
                @click="resolver(edicion, true)"
              >
                Confirmar
              </button>
              <button
                type="button"
                :disabled="resolviendo === edicion.item_id"
                class="text-xs font-medium text-gray-500 hover:text-red-600 disabled:opacity-50"
                data-testid="edicion-descartar"
                @click="resolver(edicion, false)"
              >
                Descartar
              </button>
            </div>
          </div>
        </li>
      </ul>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { getEdicionesDePista, resolverAparicion, type EdicionDePista } from '../api/editions'

const props = defineProps<{ trackId: number | null }>()

const ediciones = ref<EdicionDePista[]>([])
const loading = ref(false)
const error = ref<string | null>(null)
const resolviendo = ref<number | null>(null)
const abierto = ref(false)

const NOMBRES: Record<string, string> = {
  qobuz: 'Qobuz',
  spotify: 'Spotify',
  tidal: 'Tidal',
  apple_music: 'Apple Music',
  deezer: 'Deezer',
}

function nombreDelServicio(service: string): string {
  return NOMBRES[service] ?? service.charAt(0).toUpperCase() + service.slice(1)
}

/** Cuenta álbumes, no apariciones: tres ítems del mismo recopilatorio son uno. */
const albumes = computed(() => {
  const vistos = new Set<string>()
  for (const edicion of ediciones.value) {
    vistos.add(edicion.album ?? `${edicion.service}:${edicion.service_track_id}`)
  }
  return vistos.size
})

const pendientes = computed(
  () => ediciones.value.filter((edicion) => edicion.validation_status !== 'verified').length,
)

const resumen = computed(() => {
  if (loading.value) return 'Cargando…'
  if (ediciones.value.length === 0) return 'Sin otras apariciones'
  const cuenta = albumes.value === 1 ? '1 álbum' : `${albumes.value} álbumes`
  const porRevisar = pendientes.value === 1 ? '1 por revisar' : `${pendientes.value} por revisar`
  return `También aparece en ${cuenta} · ${porRevisar}`
})

async function cargar(trackId: number | null) {
  if (!trackId) {
    ediciones.value = []
    return
  }
  loading.value = true
  error.value = null
  try {
    ediciones.value = await getEdicionesDePista(trackId)
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
    ediciones.value = []
  } finally {
    loading.value = false
  }
}

async function resolver(edicion: EdicionDePista, confirmar: boolean) {
  if (!props.trackId) return
  resolviendo.value = edicion.item_id
  error.value = null
  try {
    await resolverAparicion(edicion.item_id, props.trackId, confirmar)
    await cargar(props.trackId)
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    resolviendo.value = null
  }
}

watch(() => props.trackId, cargar, { immediate: true })
</script>

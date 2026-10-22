<template>
  <section class="space-y-2" data-testid="ediciones-de-pista">
    <h3 class="text-sm font-semibold text-gray-900 dark:text-white">Ediciones</h3>
    <p v-if="loading" class="text-sm text-gray-500 dark:text-gray-400">Cargando ediciones…</p>
    <p v-else-if="error" class="text-sm text-red-600 dark:text-red-400" data-testid="ediciones-error">
      {{ error }}
    </p>
    <p
      v-else-if="ediciones.length === 0"
      class="text-sm text-gray-500 dark:text-gray-400"
      data-testid="ediciones-vacio"
    >
      Sin apariciones registradas.
    </p>
    <ul v-else class="divide-y divide-gray-100 dark:divide-border-dark">
      <li
        v-for="edicion in ediciones"
        :key="`${edicion.service}:${edicion.service_track_id}`"
        class="py-2 text-sm"
        data-testid="edicion"
      >
        <div class="flex items-center justify-between gap-3">
          <span class="font-medium text-gray-900 dark:text-white">
            {{ edicion.album ?? 'Sin álbum registrado' }}
          </span>
          <span class="text-[11px] uppercase tracking-wide text-gray-500 dark:text-gray-400">
            {{ edicion.service }}
          </span>
        </div>
        <p class="text-xs text-gray-500 dark:text-gray-400">
          {{ edicion.service_track_id }} · {{ edicion.validation_status }}
        </p>
      </li>
    </ul>
  </section>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { getEdicionesDePista, type EdicionDePista } from '../api/editions'

const props = defineProps<{ trackId: number | null }>()

const ediciones = ref<EdicionDePista[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

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

watch(() => props.trackId, cargar, { immediate: true })
</script>

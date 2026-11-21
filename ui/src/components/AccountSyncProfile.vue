<template>
  <div class="w-full mt-1 text-xs" :data-testid="`sync-profile-${accountId}`">
    <button
      v-if="!open"
      type="button"
      class="flex w-full items-center gap-1.5 px-2 py-1 rounded text-text-secondary hover:text-primary hover:bg-primary/10 disabled:opacity-50"
      :disabled="loading"
      :data-testid="`sync-profile-open-${accountId}`"
      @click="openPanel"
    >
      <span class="material-symbols-outlined text-[14px]" aria-hidden="true">tune</span>
      <span class="font-medium">Sync profile</span>
      <span
        v-if="profile"
        class="truncate"
        :data-testid="`sync-profile-summary-${accountId}`"
      >{{ summary }}</span>
      <span v-else-if="loading" class="truncate">Loading…</span>
      <span v-else class="truncate">Configure</span>
    </button>

    <div
      v-if="open"
      class="rounded border border-gray-200 dark:border-border-dark bg-gray-50 dark:bg-surface-highlight/40 p-3 space-y-3"
      :data-testid="`sync-profile-panel-${accountId}`"
    >
      <div v-if="loading" class="flex items-center gap-2 text-text-secondary">
        <span class="material-symbols-outlined text-[16px] animate-spin" aria-hidden="true">progress_activity</span>
        <span>Loading sync profile…</span>
      </div>

      <template v-else-if="profile">
        <!-- El error se pinta DENTRO del formulario: si lo sustituyera, la
             ficha se quedaría sin Save ni Close. -->
        <p
          v-if="error"
          class="text-error"
          :data-testid="`sync-profile-error-${accountId}`"
        >{{ error }}</p>

        <!-- Perfil resuelto: una sola lectura, sin mezclar fuentes en la vista. -->
        <p class="font-medium text-gray-900 dark:text-white" :data-testid="`sync-profile-effective-${accountId}`">
          Effective: {{ summary }}
        </p>

        <label class="flex items-start gap-2 cursor-pointer">
          <input
            v-model="form.enabled"
            type="checkbox"
            class="mt-0.5 rounded border-gray-300 text-primary focus:ring-primary"
            :data-testid="`sync-profile-enabled-${accountId}`"
          >
          <span class="text-gray-700 dark:text-gray-300">
            Participate in automatic sync and Sync All
            <span class="block text-text-secondary">Sync now still works while paused.</span>
          </span>
        </label>

        <div class="grid gap-2 sm:grid-cols-2">
          <label class="flex flex-col gap-1">
            <span class="font-medium text-gray-700 dark:text-gray-300">Mode</span>
            <select
              v-model="form.mode"
              class="px-2 py-1 rounded border border-gray-300 dark:border-border-dark bg-white dark:bg-surface-dark text-gray-900 dark:text-white"
              :data-testid="`sync-profile-mode-${accountId}`"
            >
              <option
                v-for="option in profile.modes"
                :key="option.value"
                :value="option.value"
                :disabled="!option.enabled"
              >
                {{ modeLabel(option.value) }}{{ option.enabled ? '' : ' — not available before beta' }}
              </option>
            </select>
            <span class="text-text-secondary">
              {{ profile.mode_source === 'account' ? 'Account override' : 'Service default' }}
            </span>
          </label>

          <label class="flex flex-col gap-1">
            <span class="font-medium text-gray-700 dark:text-gray-300">Interval (hours)</span>
            <input
              v-model.number="form.intervalHours"
              type="number"
              min="1"
              max="720"
              class="px-2 py-1 rounded border border-gray-300 dark:border-border-dark bg-white dark:bg-surface-dark text-gray-900 dark:text-white"
              :data-testid="`sync-profile-interval-${accountId}`"
            >
            <span class="text-text-secondary">
              {{ profile.interval_source === 'account' ? 'Account override' : 'Service default (24 h)' }}
            </span>
          </label>
        </div>

        <p
          v-for="option in unavailableModes"
          :key="option.value"
          class="text-text-secondary"
          :data-testid="`sync-profile-unavailable-${accountId}-${option.value}`"
        >
          {{ modeLabel(option.value) }}: not available before beta ({{ option.reason }}).
        </p>

        <div class="space-y-1">
          <span class="font-medium text-gray-700 dark:text-gray-300">Collections</span>
          <div class="grid grid-cols-2 gap-1">
            <label
              v-for="collection in collections"
              :key="collection.key"
              class="flex items-center gap-2 cursor-pointer text-gray-700 dark:text-gray-300"
            >
              <input
                v-model="form.collections[collection.key]"
                type="checkbox"
                class="rounded border-gray-300 text-primary focus:ring-primary"
                :data-testid="`sync-profile-collection-${accountId}-${collection.key}`"
              >
              <span>{{ collection.label }}</span>
            </label>
          </div>
          <span class="text-text-secondary">
            {{ profile.collections_source === 'account' ? 'Account override' : 'Service default' }}
          </span>
        </div>

        <div class="flex items-center gap-2">
          <button
            type="button"
            class="px-2.5 py-1 rounded bg-primary text-on-accent font-medium disabled:opacity-50"
            :disabled="saving"
            :data-testid="`sync-profile-save-${accountId}`"
            @click="save"
          >
            {{ saving ? 'Saving…' : 'Save profile' }}
          </button>
          <button
            type="button"
            class="px-2 py-1 rounded text-text-secondary hover:text-primary"
            :data-testid="`sync-profile-close-${accountId}`"
            @click="closePanel"
          >
            Close
          </button>
        </div>
      </template>

      <div v-else class="space-y-2">
        <p class="text-error" :data-testid="`sync-profile-error-${accountId}`">
          {{ error ?? 'Sync profile unavailable' }}
        </p>
        <button
          type="button"
          class="px-2 py-1 rounded text-text-secondary hover:text-primary"
          :data-testid="`sync-profile-close-${accountId}`"
          @click="closePanel"
        >
          Close
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import {
  syncProfileApi,
  type SyncProfile,
  type SyncProfileCollections,
} from '@/api/syncProfile'

const props = defineProps<{ accountId: number }>()

const MODE_LABELS: Record<string, string> = {
  import_only: 'Import only',
  mirror_quarantine: 'Mirror with quarantine',
  strict_mirror: 'Strict mirror',
  bidirectional: 'Two-way',
}

const collections = [
  { key: 'favorites' as const, label: 'Favorites' },
  { key: 'playlists' as const, label: 'Playlists' },
  { key: 'albums' as const, label: 'Albums' },
  { key: 'artists' as const, label: 'Artists' },
]

const open = ref(false)
const loading = ref(false)
const saving = ref(false)
const error = ref<string | null>(null)
const profile = ref<SyncProfile | null>(null)

const form = reactive({
  enabled: true,
  mode: 'import_only',
  intervalHours: 24,
  collections: {
    favorites: true,
    playlists: true,
    albums: true,
    artists: true,
  } as SyncProfileCollections,
})

/** Modos modelados que ningún motor ejecuta todavía: se nombran con su motivo
 *  en vez de ofrecerse como si fueran seleccionables. */
const unavailableModes = computed(() =>
  (profile.value?.modes ?? []).filter((mode) => !mode.enabled)
)

const summary = computed(() => {
  const current = profile.value
  if (!current) return ''
  const activeCollections = collections
    .filter((collection) => current.collections[collection.key])
    .map((collection) => collection.label)
  return [
    modeLabel(current.mode),
    `every ${current.interval_hours} h`,
    current.enabled ? 'participates' : 'paused',
    activeCollections.length ? activeCollections.join(', ') : 'no collections',
  ].join(' · ')
})

function modeLabel(mode: string): string {
  return MODE_LABELS[mode] ?? mode
}

function applyProfile(next: SyncProfile): void {
  profile.value = next
  form.enabled = next.enabled
  form.mode = next.mode
  form.intervalHours = next.interval_hours
  Object.assign(form.collections, next.collections)
}

async function load(): Promise<void> {
  loading.value = true
  error.value = null
  try {
    const loaded = await syncProfileApi.getSyncProfile(props.accountId)
    if (!loaded || typeof loaded.account_id !== 'number') {
      error.value = 'Sync profile unavailable'
      profile.value = null
      return
    }
    applyProfile(loaded)
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

async function openPanel(): Promise<void> {
  open.value = true
  if (!profile.value) await load()
}

function closePanel(): void {
  open.value = false
  error.value = null
  if (profile.value) applyProfile(profile.value)
}

async function save(): Promise<void> {
  if (saving.value) return
  if (!Number.isInteger(form.intervalHours) || form.intervalHours < 1 || form.intervalHours > 720) {
    error.value = 'Interval must be between 1 and 720 hours'
    return
  }
  saving.value = true
  error.value = null
  try {
    const saved = await syncProfileApi.updateSyncProfile(props.accountId, {
      enabled: form.enabled,
      mode: form.mode,
      interval_hours: form.intervalHours,
      collections: { ...form.collections },
    })
    applyProfile(saved)
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
  }
}

onMounted(load)
</script>

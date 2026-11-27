<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="fixed inset-0 bg-black/60 flex items-center justify-center z-[200] p-8"
      data-testid="playlist-conflict-mode-dialog"
      @click.self="emit('cancel')"
    >
      <div class="bg-white dark:bg-surface-dark rounded-2xl w-full max-w-lg overflow-hidden shadow-2xl">
        <div class="px-6 py-4 border-b border-gray-200 dark:border-border-dark">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Keep your local changes?</h3>
        </div>
        <div class="p-6 space-y-4">
          <p class="text-sm text-gray-600 dark:text-gray-400">
            “{{ playlistName }}”<template v-if="serviceName"> comes from {{ serviceName }}</template>.
            By default the provider wins and this edit would be overwritten on the next sync.
            Choose how future conflicts should be handled before saving.
          </p>

          <button
            type="button"
            data-testid="conflict-mode-editable"
            :disabled="busy"
            class="w-full text-left p-4 rounded-xl border border-gray-200 dark:border-border-dark hover:border-primary hover:bg-primary/5 transition-colors disabled:opacity-50"
            @click="emit('choose', 'editable')"
          >
            <span class="block text-sm font-semibold text-gray-900 dark:text-white">Editable</span>
            <span class="block text-xs text-gray-500 mt-1">
              Keep editing this playlist. If the provider changes the same thing, the conflict goes to review and nothing is overwritten.
            </span>
          </button>

          <button
            type="button"
            data-testid="conflict-mode-local"
            :disabled="busy"
            class="w-full text-left p-4 rounded-xl border border-gray-200 dark:border-border-dark hover:border-primary hover:bg-primary/5 transition-colors disabled:opacity-50"
            @click="emit('choose', 'gana_local')"
          >
            <span class="block text-sm font-semibold text-gray-900 dark:text-white">Local wins</span>
            <span class="block text-xs text-gray-500 mt-1">
              Keep this playlist as yours. Future provider changes are observed but never applied to it.
            </span>
          </button>
        </div>
        <div class="px-6 py-4 border-t border-gray-200 dark:border-border-dark flex justify-end gap-2">
          <button
            type="button"
            data-testid="conflict-mode-cancel"
            :disabled="busy"
            class="px-4 py-2 text-sm border border-gray-300 dark:border-border-dark text-gray-700 dark:text-gray-300 rounded-lg disabled:opacity-50"
            @click="emit('cancel')"
          >
            Cancel edit
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import type { PlaylistConflictMode } from '@/api/types'

defineProps<{
  open: boolean
  playlistName: string
  serviceName?: string | null
  busy?: boolean
}>()

const emit = defineEmits<{
  (event: 'choose', mode: Extract<PlaylistConflictMode, 'editable' | 'gana_local'>): void
  (event: 'cancel'): void
}>()
</script>

<template>
  <div class="space-y-1.5">
    <div class="flex items-center justify-between">
      <label class="block text-sm font-medium text-gray-700 dark:text-gray-300">{{ label }}</label>
      <span v-if="isValidating" class="text-xs text-text-secondary flex items-center gap-1" data-testid="path-validating">
        <span class="material-symbols-outlined text-[14px] animate-spin">sync</span>
        Validating...
      </span>
      <span v-else-if="validationStatus" :class="['text-xs flex items-center gap-1', validationStatus.valid ? 'text-emerald-500' : 'text-amber-500']" data-testid="path-status">
        <span class="material-symbols-outlined text-[14px]">{{ validationStatus.valid ? 'check_circle' : 'warning' }}</span>
        {{ validationStatus.message }}
      </span>
    </div>

    <div class="flex gap-2">
      <div class="relative flex-1">
        <input 
          type="text"
          :value="currentValue"
          @input="handleInput"
          @blur="handleBlur"
          :placeholder="placeholder || 'Select directory...'"
          :disabled="disabled"
          :class="[
            'w-full px-3 py-2 bg-gray-50 dark:bg-[#121b29]/50 border rounded-lg text-sm text-gray-900 dark:text-white font-mono placeholder-gray-400 dark:placeholder-gray-500 focus:ring-2 outline-none transition-all disabled:opacity-60 disabled:cursor-not-allowed',
            validationStatus && !validationStatus.valid 
              ? 'border-amber-500 dark:border-amber-500 focus:ring-amber-500 focus:border-amber-500' 
              : 'border-gray-300 dark:border-gray-600 focus:ring-primary focus:border-primary'
          ]"
        />
      </div>

      <button 
        type="button"
        @click="handleBrowse"
        :disabled="disabled || isBrowsing"
        class="px-4 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-1.5 shrink-0"
      >
        <span class="material-symbols-outlined text-[18px]">folder_open</span>
        <span>{{ isBrowsing ? 'Selecting...' : 'Browse...' }}</span>
      </button>

      <button 
        v-if="hasReset" 
        type="button"
        @click="handleReset"
        :disabled="disabled"
        class="px-3 py-2 bg-white dark:bg-surface-dark border border-gray-200 dark:border-border-dark hover:bg-gray-50 dark:hover:bg-surface-highlight text-gray-700 dark:text-gray-300 rounded-lg transition-colors disabled:opacity-50 shrink-0 flex items-center justify-center"
        title="Reset to Default Path"
      >
        <span class="material-symbols-outlined text-[20px]">restart_alt</span>
      </button>
    </div>

    <!-- Error / validation feedback message -->
    <p v-if="validationStatus && !validationStatus.valid" class="text-xs text-amber-500 flex items-center gap-1" data-testid="path-error-msg">
      <span class="material-symbols-outlined text-[14px]">error</span>
      {{ validationStatus.message }}
    </p>
    <p v-else-if="subtitle" class="text-xs text-text-secondary">{{ subtitle }}</p>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import { validateDirectoryPath } from '@/api/settings'

const props = defineProps<{
  label: string
  modelValue?: string
  defaultPath?: string
  placeholder?: string
  subtitle?: string
  hasReset?: boolean
  disabled?: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'change', value: string): void
  (e: 'reset'): void
}>()

const isBrowsing = ref(false)
const isValidating = ref(false)
const localValue = ref('')
const validationStatus = ref<{ valid: boolean; message: string } | null>(null)

const currentValue = computed(() => {
  if (props.modelValue !== undefined && props.modelValue !== '') {
    return props.modelValue
  }
  if (localValue.value !== '') {
    return localValue.value
  }
  return props.defaultPath || ''
})

/**
 * Check if the given path is an absolute path (POSIX or Windows).
 */
function isAbsolutePath(path: string): boolean {
  const trimmed = path.trim()
  if (!trimmed) return false
  // POSIX absolute
  if (trimmed.startsWith('/')) return true
  // Windows drive letter: e.g. C:\ or C:/
  if (/^[a-zA-Z]:[\\/]/.test(trimmed)) return true
  // Windows UNC: e.g. \\server\share
  if (/^[\\/]{2}[^\\/]+[\\/]+[^\\/]+/.test(trimmed)) return true
  return false
}

/**
 * Validate path format and filesystem accessibility via Tauri IPC.
 */
async function validatePath(path: string): Promise<boolean> {
  if (props.disabled) {
    validationStatus.value = null
    return true
  }

  const trimmed = path.trim()
  if (!trimmed) {
    validationStatus.value = { valid: false, message: 'Path is required' }
    return false
  }
  if (trimmed.length > 255) {
    validationStatus.value = { valid: false, message: 'Path exceeds 255 characters' }
    return false
  }
  if (!isAbsolutePath(trimmed)) {
    validationStatus.value = { valid: false, message: 'Path must be an absolute path' }
    return false
  }

  isValidating.value = true
  try {
    const res = await validateDirectoryPath(trimmed)
    if (!res.valid) {
      validationStatus.value = {
        valid: false,
        message: res.error_message || 'Directory path is invalid or inaccessible',
      }
      return false
    } else {
      validationStatus.value = {
        valid: true,
        message: 'Valid directory',
      }
      return true
    }
  } catch (err: any) {
    validationStatus.value = {
      valid: false,
      message: err?.message || 'Filesystem validation failed',
    }
    return false
  } finally {
    isValidating.value = false
  }
}

watch(() => props.modelValue, (newVal) => {
  if (newVal !== undefined) {
    localValue.value = newVal
  }
  if (!props.disabled && currentValue.value.trim()) {
    validatePath(currentValue.value)
  } else {
    validationStatus.value = null
  }
}, { immediate: true })

let debounceTimer: ReturnType<typeof setTimeout> | null = null

function handleInput(e: Event) {
  const val = (e.target as HTMLInputElement).value
  localValue.value = val
  emit('update:modelValue', val)

  if (debounceTimer) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(() => {
    validatePath(val)
  }, 300)
}

async function handleBlur() {
  const val = currentValue.value
  const isValid = await validatePath(val)
  if (isValid) {
    emit('change', val)
  }
}

async function handleBrowse() {
  if (isBrowsing.value || props.disabled) return
  isBrowsing.value = true
  try {
    const selected = await open({
      directory: true,
      multiple: false,
      title: `Select ${props.label}`,
      defaultPath: currentValue.value || undefined,
    })

    if (selected && typeof selected === 'string') {
      const isValid = await validatePath(selected)
      if (isValid) {
        localValue.value = selected
        emit('update:modelValue', selected)
        emit('change', selected)
      }
    }
  } catch (err) {
    console.error(`[PathSelector] Error browsing directory for ${props.label}:`, err)
  } finally {
    isBrowsing.value = false
  }
}

async function handleReset() {
  if (props.defaultPath) {
    localValue.value = props.defaultPath
    emit('update:modelValue', props.defaultPath)
    const isValid = await validatePath(props.defaultPath)
    if (isValid) {
      emit('change', props.defaultPath)
    }
  }
  emit('reset')
}

defineExpose({
  validationStatus,
  isValidating,
  validatePath,
  isAbsolutePath,
  currentValue,
})
</script>

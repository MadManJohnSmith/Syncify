<template>
  <Transition name="slide-down">
    <div 
      v-if="isVisible && !isDismissed"
      class="inline-hint bg-primary/10 border border-primary/20 px-4 py-3 rounded-xl flex items-center justify-between gap-4"
    >
      <div class="flex items-center gap-3">
        <span class="material-symbols-outlined text-primary text-xl">lightbulb</span>
        <p class="text-sm text-gray-700 dark:text-gray-300">
          <span class="font-medium">Tip:</span> {{ message }}
        </p>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        <label v-if="showDontShow" class="flex items-center gap-2 text-xs text-gray-500 cursor-pointer">
          <input type="checkbox" v-model="dontShowAgain" class="w-3.5 h-3.5 rounded border-gray-300 text-primary focus:ring-primary">
          Don't show tips
        </label>
        <button @click="dismiss" class="px-3 py-1.5 bg-primary/20 hover:bg-primary/30 text-primary text-xs font-medium rounded-lg transition-colors">
          Got it
        </button>
      </div>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

const props = defineProps<{
  id: string
  message: string
  showDontShow?: boolean
}>()

const emit = defineEmits(['dismiss'])

const isVisible = ref(true)
const isDismissed = ref(false)
const dontShowAgain = ref(false)

function dismiss() {
  isDismissed.value = true
  
  // Save dismissal to localStorage
  localStorage.setItem(`syncify_hint_${props.id}`, 'dismissed')
  
  if (dontShowAgain.value) {
    localStorage.setItem('syncify_disable_all_hints', 'true')
  }
  
  emit('dismiss')
}

onMounted(() => {
  // Check if already dismissed
  if (localStorage.getItem(`syncify_hint_${props.id}`) === 'dismissed') {
    isDismissed.value = true
  }
  
  // Check if all hints disabled
  if (localStorage.getItem('syncify_disable_all_hints') === 'true') {
    isDismissed.value = true
  }
})
</script>

<style scoped>
.slide-down-enter-active,
.slide-down-leave-active {
  transition: all 0.2s ease;
}

.slide-down-enter-from,
.slide-down-leave-to {
  opacity: 0;
  transform: translateY(-10px);
}
</style>

<template>
  <Transition name="fade">
    <div v-if="isVisible" class="splash-screen fixed inset-0 z-[500] bg-gradient-to-br from-gray-900 via-gray-800 to-gray-900 flex flex-col items-center justify-center">
      <!-- Logo -->
      <Transition name="logo">
        <div v-if="showLogo" class="logo-container mb-8">
          <div class="w-28 h-28 rounded-3xl bg-gradient-to-br from-primary to-primary-600 flex items-center justify-center shadow-2xl shadow-primary/30">
            <span class="material-symbols-outlined text-white text-6xl">music_note</span>
          </div>
        </div>
      </Transition>
      
      <!-- App Name -->
      <Transition name="fade-up">
        <div v-if="showText" class="text-center">
          <h1 class="text-4xl font-bold text-white mb-2">Syncify</h1>
          <p class="text-gray-400 text-lg">Your Unified Music Library</p>
        </div>
      </Transition>
      
      <!-- Loading Indicator -->
      <Transition name="fade-up">
        <div v-if="showLoading" class="mt-12 w-64">
          <!-- Progress Bar -->
          <div class="h-1 bg-gray-700 rounded-full overflow-hidden mb-3">
            <div 
              class="h-full bg-primary rounded-full transition-all duration-300 ease-out"
              :style="{ width: progress + '%' }"
            ></div>
          </div>
          
          <!-- Status Text -->
          <p class="text-sm text-gray-400 text-center">{{ loadingText }}</p>
        </div>
      </Transition>
      
      <!-- Version -->
      <p class="absolute bottom-6 text-xs text-gray-600">v2.1.0</p>
    </div>
  </Transition>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

const emit = defineEmits(['ready'])

// State
const isVisible = ref(true)
const showLogo = ref(false)
const showText = ref(false)
const showLoading = ref(false)
const progress = ref(0)
const loadingText = ref('Loading...')

// Loading stages
const loadingStages = [
  { text: 'Loading...', progress: 10 },
  { text: 'Initializing database...', progress: 25 },
  { text: 'Connecting to services...', progress: 50 },
  { text: 'Loading library...', progress: 75 },
  { text: 'Almost ready...', progress: 90 },
  { text: 'Ready!', progress: 100 },
]

let stageIndex = 0
let minDisplayTime = 800

async function startLoading() {
  // Staggered entrance
  setTimeout(() => showLogo.value = true, 100)
  setTimeout(() => showText.value = true, 400)
  setTimeout(() => showLoading.value = true, 700)
  
  // Simulate loading stages
  const stageInterval = setInterval(() => {
    if (stageIndex < loadingStages.length) {
      const stage = loadingStages[stageIndex]
      loadingText.value = stage.text
      progress.value = stage.progress
      stageIndex++
    } else {
      clearInterval(stageInterval)
    }
  }, 400)
}

function hide() {
  isVisible.value = false
  emit('ready')
}

// Called when app is actually ready
function appReady() {
  const elapsed = Date.now() - startTime
  const remaining = Math.max(0, minDisplayTime - elapsed)
  
  // Ensure minimum display time
  setTimeout(() => {
    loadingText.value = 'Ready!'
    progress.value = 100
    setTimeout(hide, 300)
  }, remaining)
}

let startTime = 0

onMounted(() => {
  startTime = Date.now()
  startLoading()
  
  // For demo: complete after 2.5s
  setTimeout(appReady, 2500)
})

defineExpose({ appReady, hide })
</script>

<style scoped>
/* Main fade */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.3s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Logo animation */
.logo-enter-active {
  transition: all 0.5s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.logo-enter-from {
  opacity: 0;
  transform: scale(0.8) translateY(20px);
}

/* Text fade up */
.fade-up-enter-active {
  transition: all 0.4s ease-out;
}

.fade-up-enter-from {
  opacity: 0;
  transform: translateY(10px);
}

/* Logo pulse animation */
.logo-container {
  animation: pulse 2s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { transform: scale(1); }
  50% { transform: scale(1.02); }
}
</style>

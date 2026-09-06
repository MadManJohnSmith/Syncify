<template>
  <div class="relative rounded-xl border border-gray-200 dark:border-border-dark bg-white dark:bg-surface-dark p-4 transition-all hover:border-primary/50 hover:shadow-glow group">
     <div class="flex justify-between items-start mb-4">
       <div class="flex items-center gap-3">
           <div class="h-10 w-10 flex items-center justify-center rounded-lg" :style="{ backgroundColor: color + '1A', color: color }">
               <span :class="isIconText ? 'font-extrabold text-lg italic' : 'material-symbols-outlined text-[24px]'">{{ icon }}</span>
           </div>
           <div>
              <h4 class="font-medium text-gray-900 dark:text-white">{{ name }}</h4>
              <div v-if="isConnected" class="flex items-center gap-1.5 mt-0.5">
                 <span class="relative flex h-2 w-2">
                    <span class="relative inline-flex rounded-full h-2 w-2" :class="statusType === 'warning' ? 'bg-warning' : 'bg-success'"></span>
                 </span>
                 <span class="text-[10px] font-medium text-text-secondary">{{ status || 'Connected' }}</span>
              </div>
              <span v-else class="text-[10px] font-medium text-text-secondary">Not Connected</span>
           </div>
       </div>
     </div>
     
     <div v-if="isConnected" class="mb-3 space-y-2">
        <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300 cursor-pointer">
           <input 
             type="checkbox" 
             :checked="enabled" 
             @change="onEnabledChange"
             data-testid="service-card-enabled"
             class="rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-transparent cursor-pointer"
           >
           Enable for downloads
        </label>
        <label class="flex items-center gap-2 text-xs text-gray-700 dark:text-gray-300 cursor-pointer">
           <input 
             type="checkbox" 
             :checked="autoImport" 
             @change="onAutoImportChange"
             data-testid="service-card-auto-import"
             class="rounded text-primary focus:ring-primary bg-gray-100 dark:bg-surface-highlight border-transparent cursor-pointer"
           >
           Auto-import favorites
        </label>
     </div>
     
     <div class="flex gap-2 text-xs">
        <button 
          v-if="isConnected" 
          type="button"
          @click="onReauth"
          data-testid="service-card-reauth"
          class="flex-1 py-1.5 bg-gray-100 dark:bg-surface-highlight rounded hover:bg-gray-200 dark:hover:bg-[#384866] transition-colors text-gray-700 dark:text-gray-300"
        >
          Re-authenticate
        </button>
        <button 
          v-else 
          type="button"
          @click="onConnect"
          data-testid="service-card-connect"
          class="flex-1 py-1.5 bg-primary/10 text-primary rounded hover:bg-primary hover:text-white transition-colors"
        >
          Connect
        </button>
        <button 
          v-if="isConnected" 
          type="button"
          @click="onDisconnect"
          data-testid="service-card-disconnect"
          class="py-1.5 px-3 text-error hover:bg-error/10 rounded transition-colors"
        >
          Disconnect
        </button>
     </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'

export interface ServiceCardProps {
  id?: string | number
  serviceId?: string
  name: string
  icon: string
  color: string
  isConnected: boolean
  user?: string
  status?: string
  statusType?: 'success' | 'warning' | 'error'
  isIconText?: boolean
  enabled?: boolean
  autoImport?: boolean
}

const props = withDefaults(defineProps<ServiceCardProps>(), {
  id: undefined,
  serviceId: undefined,
  user: undefined,
  status: undefined,
  statusType: 'success',
  isIconText: false,
  enabled: true,
  autoImport: true,
})

const emit = defineEmits<{
  (e: 'connect', serviceId: string): void;
  (e: 'disconnect', serviceId: string): void;
  (e: 'reauth', serviceId: string): void;
  (e: 'update:enabled', value: boolean): void;
  (e: 'update:autoImport', value: boolean): void;
  (e: 'toggleEnabled', serviceId: string, value: boolean): void;
  (e: 'toggleAutoImport', serviceId: string, value: boolean): void;
}>()

const currentServiceId = computed<string>(() => {
  if (props.serviceId !== undefined && props.serviceId !== '') {
    return String(props.serviceId)
  }
  if (props.id !== undefined && props.id !== '') {
    return String(props.id)
  }
  return props.name.toLowerCase()
})

function onConnect() {
  emit('connect', currentServiceId.value)
}

function onDisconnect() {
  emit('disconnect', currentServiceId.value)
}

function onReauth() {
  emit('reauth', currentServiceId.value)
}

function onEnabledChange(event: Event) {
  const target = event.target as HTMLInputElement
  const isChecked = target ? target.checked : false
  emit('update:enabled', isChecked)
  emit('toggleEnabled', currentServiceId.value, isChecked)
}

function onAutoImportChange(event: Event) {
  const target = event.target as HTMLInputElement
  const isChecked = target ? target.checked : false
  emit('update:autoImport', isChecked)
  emit('toggleAutoImport', currentServiceId.value, isChecked)
}
</script>

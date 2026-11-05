<script setup lang="ts">
withDefaults(defineProps<{ icon: string; markClass: string; size?: 'sm' | 'md' | 'lg' }>(), { size: 'md' })
</script>

<template>
  <div
    class="flex shrink-0 items-center justify-center rounded-2xl"
    :class="[
      markClass,
      size === 'sm' ? 'h-9 w-9' : size === 'lg' ? 'h-12 w-12' : 'h-11 w-11',
    ]"
  >
    <span class="material-symbols-outlined" :class="size === 'sm' ? 'text-[19px]' : size === 'lg' ? 'text-[26px]' : 'text-[23px]'">{{ icon }}</span>
  </div>
</template>

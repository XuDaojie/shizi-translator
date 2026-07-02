<script setup lang="ts">
import { useToasts } from '@/lib/toast'
import ToastView from './Toast.vue'

const { items } = useToasts()
</script>

<template>
  <div
    aria-live="polite"
    aria-atomic="true"
    class="pointer-events-none fixed right-4 top-4 z-[100] flex flex-col items-end gap-2"
  >
    <TransitionGroup
      enter-active-class="toast-in"
      leave-active-class="toast-out"
    >
      <ToastView v-for="t in items" :key="t.id" :item="t" />
    </TransitionGroup>
  </div>
</template>

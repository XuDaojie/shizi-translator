<script setup lang="ts">
import { computed } from 'vue'
import { CheckCircle2, AlertCircle, Info, X } from '@lucide/vue'
import type { ToastItem, ToastVariant } from '@/lib/toast'
import { toast } from '@/lib/toast'

const props = defineProps<{ item: ToastItem }>()

const variantClass: Record<ToastVariant, string> = {
  success: 'border-emerald-500/60 text-emerald-700 dark:text-emerald-300',
  error: 'border-destructive/60 text-destructive',
  info: 'border-primary/60 text-primary',
}

const accentBarClass: Record<ToastVariant, string> = {
  success: 'bg-emerald-500',
  error: 'bg-destructive',
  info: 'bg-primary',
}

const iconClass: Record<ToastVariant, string> = {
  success: 'text-emerald-600 dark:text-emerald-400',
  error: 'text-destructive',
  info: 'text-primary',
}

const iconCmp = computed(() => {
  if (props.item.variant === 'success') return CheckCircle2
  if (props.item.variant === 'error') return AlertCircle
  return Info
})

const onClose = (): void => {
  toast.dismiss(props.item.id)
}
</script>

<template>
  <div
    role="status"
    :class="[
      'pointer-events-auto relative flex w-[340px] items-start gap-3 overflow-hidden rounded-lg border bg-card px-3.5 py-3 shadow-lg',
      variantClass[item.variant],
    ]"
  >
    <span
      :class="['absolute inset-y-0 left-0 w-[3px]', accentBarClass[item.variant]]"
      aria-hidden="true"
    />
    <component :is="iconCmp" :class="['mt-0.5 h-4 w-4 shrink-0', iconClass[item.variant]]" />
    <div class="min-w-0 flex-1">
      <p class="text-sm font-medium leading-snug text-foreground">{{ item.title }}</p>
      <p v-if="item.description" class="mt-0.5 text-xs leading-relaxed text-muted-foreground">
        {{ item.description }}
      </p>
    </div>
    <button
      type="button"
      class="-mr-1 -mt-1 rounded p-1 text-muted-foreground/60 transition-colors hover:bg-foreground/5 hover:text-foreground"
      title="关闭"
      aria-label="关闭通知"
      @click="onClose"
    >
      <X class="h-3.5 w-3.5" />
    </button>
  </div>
</template>

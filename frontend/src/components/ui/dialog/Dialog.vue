<script setup lang="ts">
import { computed } from 'vue'
import {
  DialogRoot,
  DialogTrigger,
  DialogPortal,
  DialogOverlay,
  DialogContent,
  DialogTitle,
  DialogDescription,
  type DialogRootProps,
} from 'reka-ui'
import { X } from '@lucide/vue'
import { cn } from '@/lib/utils'

interface Props extends DialogRootProps {
  open?: boolean
  title?: string
  description?: string
  width?: string
  class?: string
}

const props = withDefaults(defineProps<Props>(), {
  width: '480px',
})

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
}>()

const onOpenChange = (value: boolean): void => emit('update:open', value)

const overlayClasses = cn(
  'fixed inset-0 z-50 bg-foreground/30 backdrop-blur-[2px]',
  'data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0',
)

const contentClasses = computed(() =>
  cn(
    'fixed left-1/2 top-1/2 z-50 grid w-full -translate-x-1/2 -translate-y-1/2 gap-4 border bg-background p-6 shadow-lg',
    'rounded-lg',
    'data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95',
    'max-h-[90vh] overflow-y-auto scrollbar-thin',
    props.class,
  ),
)

const closeClasses = cn(
  'absolute right-4 top-4 rounded-sm opacity-70 ring-offset-background transition-opacity hover:opacity-100',
  'focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2',
)
</script>

<template>
  <DialogRoot :open="open" @update:open="onOpenChange">
    <DialogTrigger as-child>
      <slot name="trigger" />
    </DialogTrigger>
    <DialogPortal>
      <DialogOverlay :class="overlayClasses" />
      <DialogContent :class="contentClasses" :style="{ maxWidth: width }">
        <div v-if="title || description" class="flex flex-col gap-1.5">
          <DialogTitle v-if="title" class="text-base font-semibold text-foreground">
            {{ title }}
          </DialogTitle>
          <DialogDescription v-if="description" class="text-sm text-muted-foreground">
            {{ description }}
          </DialogDescription>
        </div>
        <slot />
        <button type="button" :class="closeClasses" aria-label="关闭" @click="onOpenChange(false)">
          <X class="h-4 w-4" />
        </button>
      </DialogContent>
    </DialogPortal>
  </DialogRoot>
</template>

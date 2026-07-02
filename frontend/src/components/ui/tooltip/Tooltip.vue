<script setup lang="ts">
import { computed } from 'vue'
import {
  TooltipRoot,
  TooltipTrigger,
  TooltipPortal,
  TooltipContent,
  type TooltipRootProps,
} from 'reka-ui'
import { cn } from '@/lib/utils'

interface Props extends Omit<TooltipRootProps, 'class'> {
  content?: string
  side?: 'top' | 'right' | 'bottom' | 'left'
  align?: 'start' | 'center' | 'end'
  delayDuration?: number
  class?: string
}

const props = withDefaults(defineProps<Props>(), {
  side: 'top',
  align: 'center',
  delayDuration: 250,
})

const contentClasses = computed(() =>
  cn(
    'z-50 overflow-hidden rounded-md bg-foreground px-3 py-1.5 text-xs text-background shadow-md',
    'data-[state=delayed-open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=delayed-open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=delayed-open]:zoom-in-95',
    'data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2',
    props.class,
  ),
)
</script>

<template>
  <TooltipRoot :delay-duration="delayDuration">
    <TooltipTrigger as-child>
      <slot />
    </TooltipTrigger>
    <TooltipPortal>
      <TooltipContent :class="contentClasses" :side="side" :align="align" :side-offset="6">
        <slot name="content">{{ content }}</slot>
      </TooltipContent>
    </TooltipPortal>
  </TooltipRoot>
</template>

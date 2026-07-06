<script setup lang="ts">
import { cn } from '@/lib/utils'

interface Props {
  title?: string
  description?: string
  className?: string
  bare?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  bare: false,
})
</script>

<template>
  <section :class="cn('rounded-lg border border-border bg-card overflow-hidden', props.className)">
    <header
      v-if="!props.bare && (props.title || props.description || $slots.header)"
      class="px-2.5 pt-2.5 pb-1.5"
    >
      <slot name="header">
        <h3 v-if="props.title" class="text-[13px] font-semibold text-foreground">
          {{ props.title }}
        </h3>
        <p v-if="props.description" class="mt-1 text-xs text-muted-foreground leading-snug">
          {{ props.description }}
        </p>
      </slot>
    </header>
    <slot v-if="props.bare" />
    <div v-else :class="cn('divide-y divide-border', !(props.title || props.description) && 'pt-0')">
      <slot />
    </div>
  </section>
</template>

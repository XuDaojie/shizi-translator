<script setup lang="ts">
import { cn } from '@/lib/utils'
import { Badge } from '@/components/ui/badge'

interface Props {
  title: string
  description?: string
  htmlFor?: string
  className?: string
  vertical?: boolean
  status?: 'wip' | 'planned'
}

const props = withDefaults(defineProps<Props>(), {
  vertical: false,
  status: undefined,
})

const statusLabel: Record<NonNullable<Props['status']>, string> = {
  wip: '开发中',
  planned: '规划中',
}
</script>

<template>
  <div
    :class="
      cn(
        'flex',
        vertical ? 'flex-col gap-3' : 'min-h-[3rem] items-center justify-between gap-6',
        'px-4 py-3',
        'transition-colors duration-150',
        'hover:bg-muted/40',
        props.className,
      )
    "
  >
    <div :class="cn('flex-1 min-w-0', vertical && 'w-full')">
      <div class="flex items-center gap-2 flex-wrap">
        <label
          v-if="title"
          :for="htmlFor"
          class="text-sm font-medium text-foreground cursor-pointer select-none"
        >
          {{ title }}
        </label>
        <Badge
          v-if="status"
          variant="warning"
          :title="status === 'wip' ? '该功能尚未开发完成,留作后续迭代' : '已规划,暂未排期'"
          class="text-[10px] px-1.5 py-0 font-normal"
        >
          {{ statusLabel[status] }}
        </Badge>
      </div>
      <p v-if="description" class="mt-1 text-xs text-muted-foreground leading-relaxed">
        {{ description }}
      </p>
    </div>
    <div :class="cn('shrink-0', vertical && 'w-full')">
      <slot />
    </div>
  </div>
</template>

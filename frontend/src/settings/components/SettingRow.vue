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
        vertical ? 'flex-col gap-2' : 'min-h-[2.375rem] items-center justify-between gap-3',
        'px-2.5 py-2',
        'transition-colors duration-150',
        'hover:bg-muted/40',
        props.className,
      )
    "
  >
    <div :class="cn('flex-1 min-w-0', vertical && 'w-full')">
      <div class="flex min-w-0 items-center gap-2">
        <label
          v-if="title"
          :for="htmlFor"
          class="shrink-0 cursor-pointer select-none text-[13px] font-medium text-foreground"
        >
          {{ title }}
        </label>
        <Badge
          v-if="status"
          variant="warning"
          :title="status === 'wip' ? '该功能尚未开发完成,留作后续迭代' : '已规划,暂未排期'"
          class="shrink-0 px-1.5 py-0 text-[10px] font-normal"
        >
          {{ statusLabel[status] }}
        </Badge>
        <!-- 标题行尾元信息（如协议），贴标题右侧，形态对齐渠道 Header 的 AI 徽标 -->
        <div v-if="$slots['title-end']" class="min-w-0 truncate">
          <slot name="title-end" />
        </div>
      </div>
      <p
        v-if="description"
        class="mt-0.5 whitespace-pre-line text-[11px] leading-snug text-muted-foreground"
      >
        {{ description }}
      </p>
    </div>
    <div :class="cn('shrink-0', vertical && 'w-full')">
      <slot />
    </div>
  </div>
</template>

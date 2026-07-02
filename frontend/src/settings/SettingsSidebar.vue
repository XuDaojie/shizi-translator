<script setup lang="ts">
import type { Component } from 'vue'
import {
  Settings2,
  Languages,
  Keyboard,
  Plug,
  Sliders,
  History as HistoryIcon,
} from '@lucide/vue'
import { Badge } from '@/components/ui/badge'

export interface SettingsCategory {
  id: string
  label: string
  description: string
  icon: Component
  /** 分类旁的小徽标(开发中/规划中/新功能),留空不显示。 */
  badge?: 'wip' | 'new'
}

defineProps<{
  modelValue: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void
}>()

const categories: SettingsCategory[] = [
  {
    id: 'general',
    label: '通用',
    description: '启动、托盘与外观',
    icon: Settings2,
  },
  {
    id: 'translate',
    label: '翻译',
    description: '默认语种与翻译行为',
    icon: Languages,
  },
  {
    id: 'shortcut',
    label: '快捷键',
    description: '划词/截图/取词',
    icon: Keyboard,
  },
  {
    id: 'services',
    label: '服务',
    description: '翻译服务与 API Key',
    icon: Plug,
  },
  {
    id: 'history',
    label: '翻译历史',
    description: '仅展示截图翻译记录',
    icon: HistoryIcon,
    badge: 'wip',
  },
  {
    id: 'advanced',
    label: '高级',
    description: '日志、实验与重置',
    icon: Sliders,
  },
]

const select = (id: string): void => emit('update:modelValue', id)

const badgeLabel = (kind: 'wip' | 'new' | undefined): string => {
  if (kind === 'wip') return '实现中'
  if (kind === 'new') return '新'
  return ''
}
</script>

<template>
  <aside
    class="flex h-full w-[var(--sidebar-width)] shrink-0 flex-col border-r border-border bg-card/40 py-4"
  >
    <div class="px-4 pb-4">
      <h2 class="text-sm font-semibold text-foreground">设置</h2>
      <p class="mt-1 text-xs text-muted-foreground">个性化本应用的使用方式</p>
    </div>

    <nav class="flex-1 overflow-y-auto px-2 scrollbar-thin">
      <ul class="flex flex-col gap-0.5">
        <li v-for="cat in categories" :key="cat.id">
          <button
            type="button"
            :class="[
              'group flex w-full items-start gap-3 rounded-md px-3 py-2 text-left transition-colors duration-150',
              'hover:bg-accent/60',
              modelValue === cat.id && 'bg-accent text-accent-foreground',
            ]"
            @click="select(cat.id)"
          >
            <component
              :is="cat.icon"
              :class="[
                'mt-0.5 h-4 w-4 shrink-0',
                modelValue === cat.id ? 'text-primary' : 'text-muted-foreground',
              ]"
            />
            <span class="flex-1 min-w-0">
              <span class="flex items-center gap-1.5">
                <span
                  :class="[
                    'block text-sm font-medium',
                    modelValue === cat.id ? 'text-foreground' : 'text-foreground/90',
                  ]"
                >
                  {{ cat.label }}
                </span>
                <Badge v-if="cat.badge" variant="warning" class="h-4 px-1 text-[9px]">
                  {{ badgeLabel(cat.badge) }}
                </Badge>
              </span>
              <span class="mt-0.5 block truncate text-[11px] text-muted-foreground">
                {{ cat.description }}
              </span>
            </span>
          </button>
        </li>
      </ul>
    </nav>

    <div class="mt-2 px-4 pt-3 border-t border-border">
      <p class="text-[11px] text-muted-foreground leading-relaxed">
        修改会立即生效,关闭主窗口前请确认保存。
      </p>
    </div>
  </aside>
</template>

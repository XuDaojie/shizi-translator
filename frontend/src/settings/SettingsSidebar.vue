<script setup lang="ts">
import { computed } from 'vue'
import type { Component } from 'vue'
import {
  AlertCircle,
  Check,
  LoaderCircle,
  Settings2,
  Languages,
  Keyboard,
  Plug,
  RotateCcw,
  Sliders,
  History as HistoryIcon,
} from '@lucide/vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { useSettings } from './stores/settings'

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

const { dirty, save, saveStatus } = useSettings()

const saveStatusText = computed(() => {
  if (saveStatus.value === 'idle') return '本机偏好'
  if (saveStatus.value === 'saving') return '正在自动保存…'
  if (saveStatus.value === 'error') return '自动保存失败'
  if (dirty.value) return '有修改待保存'
  return '已自动保存'
})
const saveStatusTone = computed(() => {
  if (saveStatus.value === 'error') return 'text-destructive'
  if (saveStatus.value === 'saving' || dirty.value) return 'text-amber-600 dark:text-amber-400'
  if (saveStatus.value === 'saved') return 'text-emerald-600 dark:text-emerald-400'
  return 'text-muted-foreground'
})
const saveStatusDetail = computed(() => {
  if (saveStatus.value === 'error') return '请检查本地存储权限'
  if (saveStatus.value === 'saving') return '正在写入本地设置'
  if (dirty.value) return '等待写入'
  return '修改会立即生效并自动保存'
})

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
    description: '查看最近翻译记录',
    icon: HistoryIcon,
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
    class="flex h-full w-[var(--sidebar-width)] shrink-0 flex-col border-r border-border bg-card/40 py-3"
  >
    <div class="px-3 pb-3">
      <h2 class="text-sm font-semibold text-foreground">设置</h2>
      <p class="mt-1 text-xs text-muted-foreground">个性化本应用的使用方式</p>
    </div>

    <nav class="flex-1 overflow-y-auto px-2 scrollbar-thin">
      <ul class="flex flex-col gap-0.5">
        <li v-for="cat in categories" :key="cat.id">
          <button
            type="button"
            :class="[
              'group flex w-full items-start gap-2.5 rounded-md px-2.5 py-1.5 text-left transition-colors duration-150',
              'hover:bg-accent/60',
              modelValue === cat.id && 'bg-accent text-accent-foreground',
            ]"
            @click="select(cat.id)"
          >
            <component
              :is="cat.icon"
              :class="[
                'mt-0.5 h-3.5 w-3.5 shrink-0',
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

    <div class="mt-2 border-t border-border px-3 pt-2.5">
      <div :class="['flex items-center gap-1.5 text-[11px] font-medium', saveStatusTone]">
        <LoaderCircle v-if="saveStatus.value === 'saving'" class="h-3 w-3 animate-spin" />
        <AlertCircle v-else-if="saveStatus.value === 'error'" class="h-3 w-3" />
        <RotateCcw v-else-if="dirty.value" class="h-3 w-3" />
        <Check v-else class="h-3 w-3 text-emerald-500" />
        <span>{{ saveStatusText }}</span>
      </div>
      <p class="mt-1 text-[11px] leading-snug text-muted-foreground">
        {{ saveStatusDetail }}
      </p>
      <Button
        v-if="saveStatus.value === 'error'"
        variant="ghost"
        size="sm"
        class="mt-2 h-7 w-full px-2 text-xs"
        @click="save"
      >
        <RotateCcw class="h-3.5 w-3.5" />
        重试保存
      </Button>
    </div>
  </aside>
</template>

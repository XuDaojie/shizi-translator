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
import { t } from '@/i18n'

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
  if (saveStatus.value === 'idle') return t('settings.status.localPreference')
  if (saveStatus.value === 'saving') return t('settings.status.saving')
  if (saveStatus.value === 'error') return t('settings.status.saveFailed')
  if (dirty.value) return t('settings.status.pendingSave')
  return t('settings.status.saved')
})
const saveStatusTone = computed(() => {
  if (saveStatus.value === 'error') return 'text-destructive'
  if (saveStatus.value === 'saving' || dirty.value) return 'text-amber-600 dark:text-amber-400'
  if (saveStatus.value === 'saved') return 'text-emerald-600 dark:text-emerald-400'
  return 'text-muted-foreground'
})

const categories = computed<SettingsCategory[]>(() => [
  {
    id: 'general',
    label: t('settings.category.general'),
    description: t('settings.category.generalDescription'),
    icon: Settings2,
  },
  {
    id: 'translate',
    label: t('settings.category.translate'),
    description: t('settings.category.translateDescription'),
    icon: Languages,
  },
  {
    id: 'shortcut',
    label: t('settings.category.shortcut'),
    description: t('settings.category.shortcutDescription'),
    icon: Keyboard,
  },
  {
    id: 'services',
    label: t('settings.category.services'),
    description: t('settings.category.servicesDescription'),
    icon: Plug,
  },
  {
    id: 'history',
    label: t('settings.category.history'),
    description: t('settings.category.historyDescription'),
    icon: HistoryIcon,
  },
  {
    id: 'advanced',
    label: t('settings.category.advanced'),
    description: t('settings.category.advancedDescription'),
    icon: Sliders,
  },
])

const select = (id: string): void => emit('update:modelValue', id)

const badgeLabel = (kind: 'wip' | 'new' | undefined): string => {
  if (kind === 'wip') return t('common.developing')
  if (kind === 'new') return t('settings.status.new')
  return ''
}
</script>

<template>
  <aside
    class="flex h-full w-[var(--sidebar-width)] shrink-0 flex-col border-r border-border bg-card/40 pt-3"
  >
    <div class="px-3 pb-3">
      <h2 class="text-sm font-semibold text-foreground">{{ t('settings.title') }}</h2>
      <p class="mt-1 text-xs text-muted-foreground">{{ t('settings.subtitle') }}</p>
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

    <div class="shrink-0 border-t border-border px-3">
      <div
        :class="[
          'flex h-7 items-center gap-1.5 text-[11px] font-medium leading-none',
          saveStatusTone,
        ]"
      >
        <LoaderCircle v-if="saveStatus.value === 'saving'" class="h-3 w-3 shrink-0 animate-spin" />
        <AlertCircle v-else-if="saveStatus.value === 'error'" class="h-3 w-3 shrink-0" />
        <RotateCcw v-else-if="dirty.value" class="h-3 w-3 shrink-0" />
        <Check v-else class="h-3 w-3 shrink-0 text-emerald-500" />
        <span class="truncate">{{ saveStatusText }}</span>
      </div>
      <Button
        v-if="saveStatus.value === 'error'"
        variant="ghost"
        size="sm"
        class="mb-1.5 h-7 w-full px-2 text-xs"
        @click="save"
      >
        <RotateCcw class="h-3.5 w-3.5" />
        {{ t('settings.button.retrySave') }}
      </Button>
    </div>
  </aside>
</template>

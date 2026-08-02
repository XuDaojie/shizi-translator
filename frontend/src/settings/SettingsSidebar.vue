<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
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

const props = defineProps<{
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

const navListRef = ref<HTMLElement | null>(null)
const itemRefs = ref<Record<string, HTMLElement | null>>({})
const indicatorReady = ref(false)
const indicatorStyle = ref({ top: '0px', height: '0px' })

const setItemRef = (id: string, el: unknown): void => {
  itemRefs.value[id] = (el as HTMLElement | null) ?? null
}

const updateIndicator = (): void => {
  const list = navListRef.value
  const item = itemRefs.value[props.modelValue]
  if (!list || !item) return

  const listRect = list.getBoundingClientRect()
  const itemRect = item.getBoundingClientRect()
  indicatorStyle.value = {
    top: `${itemRect.top - listRect.top + list.scrollTop}px`,
    height: `${itemRect.height}px`,
  }
  indicatorReady.value = true
}

let resizeObserver: ResizeObserver | null = null

onMounted(() => {
  nextTick(updateIndicator)
  if (typeof ResizeObserver !== 'undefined' && navListRef.value) {
    resizeObserver = new ResizeObserver(() => updateIndicator())
    resizeObserver.observe(navListRef.value)
  }
  window.addEventListener('resize', updateIndicator)
})

onBeforeUnmount(() => {
  resizeObserver?.disconnect()
  resizeObserver = null
  window.removeEventListener('resize', updateIndicator)
})

watch(
  () => props.modelValue,
  () => {
    nextTick(updateIndicator)
  },
)

watch(categories, () => {
  nextTick(updateIndicator)
})

const select = (id: string): void => emit('update:modelValue', id)

const badgeLabel = (kind: 'wip' | 'new' | undefined): string => {
  if (kind === 'wip') return t('common.developing')
  if (kind === 'new') return t('settings.status.new')
  return ''
}
</script>

<template>
  <aside
    class="flex h-full w-[var(--sidebar-width)] shrink-0 flex-col border-r border-border bg-card/40 py-3"
  >
    <div class="px-3 pb-3">
      <h2 class="text-sm font-semibold text-foreground">{{ t('settings.title') }}</h2>
      <p class="mt-1 text-xs text-muted-foreground">{{ t('settings.subtitle') }}</p>
    </div>

    <nav class="flex-1 overflow-y-auto px-2 scrollbar-thin">
      <ul ref="navListRef" class="relative flex flex-col gap-0.5">
        <!-- 滑动高亮指示器：整行 pill -->
        <li
          aria-hidden="true"
          class="settings-nav-indicator pointer-events-none absolute inset-x-0 z-0 rounded-md bg-accent"
          :class="indicatorReady ? 'settings-nav-indicator--ready' : 'opacity-0'"
          :style="indicatorStyle"
        />

        <li v-for="cat in categories" :key="cat.id" class="relative z-[1]">
          <button
            type="button"
            :ref="(el) => setItemRef(cat.id, el)"
            :class="[
              'group flex w-full items-start gap-2.5 rounded-md px-2.5 py-1.5 text-left',
              'settings-nav-item',
              modelValue === cat.id
                ? 'text-accent-foreground'
                : 'hover:bg-accent/40',
            ]"
            :aria-current="modelValue === cat.id ? 'page' : undefined"
            @click="select(cat.id)"
          >
            <component
              :is="cat.icon"
              :class="[
                'mt-0.5 h-3.5 w-3.5 shrink-0 settings-nav-icon',
                modelValue === cat.id ? 'text-primary' : 'text-muted-foreground',
              ]"
            />
            <span class="flex-1 min-w-0">
              <span class="flex items-center gap-1.5">
                <span
                  :class="[
                    'block text-sm font-medium settings-nav-label',
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

<style scoped>
/* 系统级缓动：快出慢入，克制滑动 */
.settings-nav-indicator {
  transition:
    top 280ms cubic-bezier(0.16, 1, 0.3, 1),
    height 280ms cubic-bezier(0.16, 1, 0.3, 1),
    opacity 160ms ease;
}

.settings-nav-indicator--ready {
  opacity: 1;
}

.settings-nav-item {
  transition: background-color 180ms cubic-bezier(0.4, 0, 0.2, 1);
}

.settings-nav-icon,
.settings-nav-label {
  transition: color 180ms cubic-bezier(0.4, 0, 0.2, 1);
}

@media (prefers-reduced-motion: reduce) {
  .settings-nav-indicator,
  .settings-nav-item,
  .settings-nav-icon,
  .settings-nav-label {
    transition: none !important;
  }
}
</style>

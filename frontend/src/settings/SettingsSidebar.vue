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

/** 仅在有修改/保存中/失败/刚保存成功时展示，idle 不占位、不显示「本机偏好」。 */
const showSaveStatus = computed(
  () => dirty.value || saveStatus.value === 'saving' || saveStatus.value === 'error' || saveStatus.value === 'saved',
)

const saveStatusText = computed(() => {
  if (saveStatus.value === 'saving') return t('settings.status.saving')
  if (saveStatus.value === 'error') return t('settings.status.saveFailed')
  if (dirty.value) return t('settings.status.pendingSave')
  if (saveStatus.value === 'saved') return t('settings.status.saved')
  return ''
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
  <!--
    侧栏：不要 py-3 包整列。原先底部 padding 叠在状态栏外，
    状态文字只出现在上半段，看起来「没有上下居中」。
  -->
  <aside
    class="flex h-full w-[var(--sidebar-width)] shrink-0 flex-col border-r border-border bg-settings-sidebar pt-3 pb-0"
  >
    <!-- 品牌头部：图标 + 标题 -->
    <div class="flex items-center gap-2.5 px-3 pb-3">
      <img
        src="/favicon.svg"
        alt=""
        class="h-7 w-7 shrink-0 rounded-lg shadow-sm"
        aria-hidden="true"
      />
      <div class="min-w-0">
        <h2 class="text-sm font-semibold text-foreground">{{ t('settings.title') }}</h2>
        <p class="mt-0.5 truncate text-[11px] text-muted-foreground">
          {{ t('settings.subtitle') }}
        </p>
      </div>
    </div>

    <!-- py-0.5：给首/末项的 ring 与阴影留出渲染空间，避免被 overflow 裁剪 -->
    <nav class="min-h-0 flex-1 overflow-y-auto px-2 py-0.5 scrollbar-thin">
      <ul ref="navListRef" class="relative flex flex-col gap-0.5">
        <!-- 滑动高亮指示器：白色浮起 pill -->
        <li
          aria-hidden="true"
          class="settings-nav-indicator pointer-events-none absolute inset-x-0 z-0 rounded-md bg-card shadow-sm ring-1 ring-border/70"
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
                : 'hover:bg-foreground/5',
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

    <!-- 贴底状态行：始终占位；idle 空白，变更时才显示图标/文案 -->
    <div
      class="settings-save-status shrink-0"
      :class="showSaveStatus ? saveStatusTone : ''"
    >
      <template v-if="showSaveStatus">
        <LoaderCircle v-if="saveStatus.value === 'saving'" class="settings-save-status__icon animate-spin" />
        <AlertCircle v-else-if="saveStatus.value === 'error'" class="settings-save-status__icon" />
        <RotateCcw v-else-if="dirty.value" class="settings-save-status__icon" />
        <Check v-else-if="saveStatus.value === 'saved'" class="settings-save-status__icon text-emerald-500" />
        <span class="settings-save-status__text">{{ saveStatusText }}</span>
        <button
          v-if="saveStatus.value === 'error'"
          type="button"
          class="settings-save-status__retry"
          @click="save"
        >
          {{ t('settings.button.retrySave') }}
        </button>
      </template>
    </div>
  </aside>
</template>

<style scoped>
/* 贴底状态行：始终占位（略高于 h-5），内容垂直居中；idle 无文字 */
.settings-save-status {
  box-sizing: border-box;
  display: flex;
  height: 24px;
  align-items: center;
  gap: 4px;
  border-top: 1px solid hsl(var(--border));
  padding: 0 10px;
  font-size: 11px;
  font-weight: 500;
  line-height: 1;
}

.settings-save-status__icon {
  width: 10px;
  height: 10px;
  flex-shrink: 0;
}

.settings-save-status__text {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  line-height: 1;
}

.settings-save-status__retry {
  flex-shrink: 0;
  margin-left: auto;
  font-size: 10px;
  line-height: 1;
  text-underline-offset: 2px;
}
.settings-save-status__retry:hover {
  text-decoration: underline;
}

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

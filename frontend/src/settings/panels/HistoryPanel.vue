<script setup lang="ts">
import { computed, nextTick, onActivated, onBeforeUnmount, onMounted, reactive, ref, watch, watchEffect } from 'vue'
import { History as HistoryIcon, Trash2, Camera, ScanText, MousePointerSquareDashed, PencilLine, Layers } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import Dialog from '@/components/ui/dialog/Dialog.vue'
import { toast } from '@/lib/toast'
import { speakText } from '@/popup/composables/utils'
import { translationLanguage } from '@/shared/translation-languages'
import SourceCardView from '@/popup/components/SourceCardView.vue'
import ResultCardView from '@/popup/components/ResultCardView.vue'
import LanguageToolbar from '@/popup/components/LanguageToolbar.vue'
import type { AppSettings } from '../types'
import {
  clearHistoryAndReload,
  isEmptyHistory,
  loadHistory,
  resultCardStatus,
  type HistoryResult,
  type HistorySession,
  type HistoryTrigger,
} from '../history'
import { displayModelName, shouldShowTokens } from '@/popup/composables/resultCardMeta'
import { formatDateTime, t } from '@/i18n'

/** keep-alive 缓存名（SettingsPage 使用） */
defineOptions({ name: 'HistoryPanel' })

interface Props {
  state: AppSettings
}
const props = defineProps<Props>()

const langLabel = (code: string): string => translationLanguage(code)?.nativeName ?? code

const TRIGGER_META = computed<Record<HistoryTrigger, { label: string; icon: typeof Camera }>>(() => ({
  selection: { label: t('history.trigger.selection'), icon: MousePointerSquareDashed },
  manual: { label: t('history.trigger.manual'), icon: PencilLine },
  screenshot: { label: t('history.trigger.screenshot'), icon: ScanText },
}))

const FILTERS = computed(() => [
  { id: 'all' as const, label: t('history.filter.all'), icon: Layers },
  { id: 'screenshot' as const, label: t('history.trigger.screenshot'), icon: ScanText },
  { id: 'selection' as const, label: t('history.trigger.selection'), icon: MousePointerSquareDashed },
  { id: 'manual' as const, label: t('history.trigger.manual'), icon: PencilLine },
])

const activeFilter = ref<'all' | HistoryTrigger>('all')
const activeId = ref<string>('')
const showClearConfirm = ref(false)

/** 历史筛选分段 pill 指示器 */
const filterBarRef = ref<HTMLElement | null>(null)
const filterItemRefs = ref<Record<string, HTMLElement | null>>({})
const filterIndicatorReady = ref(false)
const filterIndicatorStyle = ref({ left: '0px', width: '0px', height: '0px', top: '0px' })

const setFilterItemRef = (id: string, el: unknown): void => {
  filterItemRefs.value[id] = (el as HTMLElement | null) ?? null
}

const updateFilterIndicator = (): void => {
  const bar = filterBarRef.value
  const item = filterItemRefs.value[activeFilter.value]
  if (!bar || !item) return
  const barRect = bar.getBoundingClientRect()
  const itemRect = item.getBoundingClientRect()
  filterIndicatorStyle.value = {
    left: `${itemRect.left - barRect.left + bar.scrollLeft}px`,
    top: `${itemRect.top - barRect.top + bar.scrollTop}px`,
    width: `${itemRect.width}px`,
    height: `${itemRect.height}px`,
  }
  filterIndicatorReady.value = true
}

let filterResizeObserver: ResizeObserver | null = null

const ensureFilterIndicatorObserver = (): void => {
  nextTick(() => {
    updateFilterIndicator()
    if (typeof ResizeObserver === 'undefined' || !filterBarRef.value) return
    if (!filterResizeObserver) {
      filterResizeObserver = new ResizeObserver(() => updateFilterIndicator())
    }
    filterResizeObserver.observe(filterBarRef.value)
  })
}

/** 初始即 loading，避免挂载瞬间 sessions=[] 误闪空态。 */
const sessions = ref<HistorySession[]>([])
const loading = ref(true)
const loadError = ref('')
const clearing = ref(false)
/**
 * 详情区（Source/Result 卡 + Markdown）比左侧列表重得多。
 * 先画列表，idle 后再挂详情，避免一次提交 ~90 条 + 多卡 Markdown 堵死主线程。
 */
const detailReady = ref(false)
let isMounted = false
let refreshRequestId = 0
let detailIdleHandle: number | null = null

const cancelDetailSchedule = (): void => {
  if (detailIdleHandle == null) return
  if (typeof cancelIdleCallback === 'function') {
    cancelIdleCallback(detailIdleHandle)
  } else {
    window.clearTimeout(detailIdleHandle)
  }
  detailIdleHandle = null
}

/** 列表已提交后再挂详情；timeout 保证低负载时也不会一直不渲染。 */
const scheduleDetailReady = (requestId: number): void => {
  cancelDetailSchedule()
  detailReady.value = false
  const run = (): void => {
    detailIdleHandle = null
    if (!isMounted || requestId !== refreshRequestId) return
    detailReady.value = true
  }
  if (typeof requestIdleCallback === 'function') {
    detailIdleHandle = requestIdleCallback(run, { timeout: 160 }) as unknown as number
  } else {
    detailIdleHandle = window.setTimeout(run, 48)
  }
}

const refreshHistory = async (opts?: { silent?: boolean }): Promise<void> => {
  const requestId = ++refreshRequestId
  const silent = opts?.silent === true && sessions.value.length > 0
  if (isMounted && !silent) {
    loading.value = true
    loadError.value = ''
    detailReady.value = false
  }
  try {
    const nextSessions = await loadHistory(props.state.translation.historyLimit)
    if (!isMounted || requestId !== refreshRequestId) return

    sessions.value = nextSessions
    // 等列表 VDOM/DOM 提交后再调度详情
    await nextTick()
    if (!isMounted || requestId !== refreshRequestId) return
    scheduleDetailReady(requestId)
  } catch (err) {
    if (!isMounted || requestId !== refreshRequestId) return
    if (!silent) {
      sessions.value = []
      detailReady.value = false
    }
    loadError.value = err instanceof Error ? err.message : String(err)
    toast.error(t('history.loadFailed'), loadError.value)
  } finally {
    if (!isMounted || requestId !== refreshRequestId) return
    if (!silent) loading.value = false
  }
}

const filteredSessions = computed<HistorySession[]>(() =>
  activeFilter.value === 'all'
    ? sessions.value
    : sessions.value.filter((s) => s.trigger === activeFilter.value),
)
/* 空态只看全部历史，与原型一致；筛选无命中保留筛选栏并展示筛选空态 */
const isEmpty = computed(() => isEmptyHistory(sessions.value))
const activeFilterLabel = computed(() => FILTERS.value.find((f) => f.id === activeFilter.value)?.label ?? '')
const activeSession = computed<HistorySession | null>(() =>
  activeId.value ? filteredSessions.value.find((s) => s.id === activeId.value) ?? null : null,
)

/* 首条默认选中 */
watchEffect(() => {
  if (!activeId.value && filteredSessions.value.length > 0) {
    activeId.value = filteredSessions.value[0].id
  }
  if (activeId.value && !filteredSessions.value.some((s) => s.id === activeId.value)) {
    activeId.value = filteredSessions.value[0]?.id ?? ''
  }
})

const formatDetailTime = (iso: string): string => formatDateTime(iso, { dateStyle: 'medium', timeStyle: 'short' })

const formatTime = (iso: string): string => formatDateTime(iso, { dateStyle: 'medium', timeStyle: 'short' })

type Bucket = { label: string; entries: HistorySession[] }
const grouped = computed<Bucket[]>(() => {
  const now = new Date()
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()
  const startOfYesterday = startOfToday - 86400000
  const startOfWeek = startOfToday - 7 * 86400000
  const today: HistorySession[] = []
  const yesterday: HistorySession[] = []
  const week: HistorySession[] = []
  const older: HistorySession[] = []
  for (const s of sessions.value) {
    const t = new Date(s.timestamp).getTime()
    if (t >= startOfToday) today.push(s)
    else if (t >= startOfYesterday) yesterday.push(s)
    else if (t >= startOfWeek) week.push(s)
    else older.push(s)
  }
  const out: Bucket[] = []
  if (today.length) out.push({ label: t('history.today'), entries: today })
  if (yesterday.length) out.push({ label: t('history.yesterday'), entries: yesterday })
  if (week.length) out.push({ label: t('history.thisWeek'), entries: week })
  if (older.length) out.push({ label: t('history.older'), entries: older })
  return out
})

const filteredGrouped = computed<Bucket[]>(() => {
  if (activeFilter.value === 'all') return grouped.value
  return grouped.value
    .map((b) => ({ ...b, entries: b.entries.filter((s) => s.trigger === activeFilter.value) }))
    .filter((b) => b.entries.length > 0)
})
const isFilterEmpty = computed(() => !isEmpty.value && filteredGrouped.value.length === 0)

const copy = async (text: string, isSource = false): Promise<void> => {
  if (!text) { toast.error(t('history.copyFailed'), t('history.noCopyText')); return }
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
    } else {
      const ta = document.createElement('textarea')
      ta.value = text
      ta.style.position = 'fixed'
      ta.style.opacity = '0'
      document.body.appendChild(ta)
      ta.select()
      document.execCommand('copy')
      document.body.removeChild(ta)
    }
    toast.success(t(isSource ? 'history.sourceCopied' : 'history.resultCopied'), text.length > 30 ? `${text.slice(0, 30)}…` : text)
  } catch (err) {
    toast.error(t('history.copyFailed'), err instanceof Error ? err.message : t('history.clipboardPermission'))
  }
}

const clearAll = async (): Promise<void> => {
  if (clearing.value) return
  clearing.value = true
  try {
    const nextSessions = await clearHistoryAndReload()
    if (!isMounted) return
    refreshRequestId += 1
    sessions.value = nextSessions
    loadError.value = ''
    loading.value = false
    showClearConfirm.value = false
    activeId.value = ''
    toast.success(t('settings.toast.historyCleared'))
  } catch (err) {
    if (!isMounted) return
    toast.error(t('history.clearFailed'), err instanceof Error ? err.message : String(err))
  } finally {
    if (isMounted) clearing.value = false
  }
}

const retryResult = (r: HistoryResult): void => {
  toast.info(t('history.retranslateRequested'), `${r.serviceName} · ${r.modelName || t('history.defaultModel')}`)
}

/* 卡片折叠 / 展开全文：按 sessionId + serviceInstanceId 记录（与弹窗结果卡对齐）。 */
const collapsedMap = reactive<Record<string, boolean>>({})
const expandedMap = reactive<Record<string, boolean>>({})
const cardKey = (sessionId: string, r: HistoryResult): string => `${sessionId}:${r.serviceInstanceId}`
const isCollapsed = (sessionId: string, r: HistoryResult): boolean => collapsedMap[cardKey(sessionId, r)] ?? false
const isExpanded = (sessionId: string, r: HistoryResult): boolean => expandedMap[cardKey(sessionId, r)] ?? false
const toggleCollapse = (sessionId: string, r: HistoryResult): void => {
  const k = cardKey(sessionId, r)
  collapsedMap[k] = !collapsedMap[k]
}
const toggleExpand = (sessionId: string, r: HistoryResult): void => {
  const k = cardKey(sessionId, r)
  expandedMap[k] = !expandedMap[k]
}
/** 与弹窗一致：LLM 且有 usage 时展示 Token；MT 不展示。 */
const showResultTokens = (r: HistoryResult): boolean =>
  shouldShowTokens(r.protocol, r.inputTokens != null || r.outputTokens != null)

const resultModelName = (r: HistoryResult): string =>
  displayModelName(r.protocol, r.modelName)

const speakSource = (): void => {
  const text = activeSession.value?.source
  if (!text) { toast.error(t('history.speakFailed'), t('history.noSourceToSpeak')); return }
  const lang = activeSession.value?.sourceLang && activeSession.value.sourceLang !== 'auto'
    ? activeSession.value.sourceLang
    : 'en-US'
  speakText(text, lang)
}

const speak = (text: string): void => {
  if (!text) { toast.error(t('history.speakFailed'), t('history.noResultToSpeak')); return }
  speakText(text, activeSession.value?.targetLang || 'zh-CN')
}

const triggerIcon = (trigger: HistoryTrigger): typeof Camera => TRIGGER_META.value[trigger]?.icon ?? Camera

/** 解析结果对应的服务 type，供 ServiceIcon 与设置页服务列表统一。 */
const serviceTypeOf = (r: HistoryResult): string => {
  if (r.serviceType) return r.serviceType
  const inst = props.state.services.find((s) => s.id === r.serviceInstanceId)
  return inst?.type ?? r.serviceInstanceId
}

const cardStatus = (r: HistoryResult): 'success' | 'loading' | 'pending' | 'error' | 'aborted' =>
  resultCardStatus(r)

const resultText = (r: HistoryResult): string =>
  r.status === 'error' ? (r.errorMessage || r.translation) : r.translation

/** 筛选栏在 DOM 中：非 loading/error/空历史时展示 */
const showFilterChrome = computed(
  () => !loading.value && !loadError.value && !isEmpty.value,
)

onMounted(() => {
  isMounted = true
  void refreshHistory()
  window.addEventListener('resize', updateFilterIndicator)
})

// keep-alive 再次进入：静默刷新，不闪 loading、不堵导航动画
// 注意：首次挂载也会触发 onActivated，此时 loading 中由 onMounted 负责，勿重复全量拉
onActivated(() => {
  if (loading.value && sessions.value.length === 0) return
  if (sessions.value.length > 0) {
    detailReady.value = true
    void refreshHistory({ silent: true })
  }
})

onBeforeUnmount(() => {
  isMounted = false
  cancelDetailSchedule()
  filterResizeObserver?.disconnect()
  filterResizeObserver = null
  window.removeEventListener('resize', updateFilterIndicator)
})

watch(activeFilter, () => {
  nextTick(updateFilterIndicator)
})

// 加载完成、筛选栏挂载后定位 pill（默认「全部」）
watch(showFilterChrome, (visible) => {
  if (visible) {
    ensureFilterIndicatorObserver()
  } else {
    filterIndicatorReady.value = false
  }
})
</script>

<template>
  <!-- 自管高度：与服务页一致，左右分栏各自独立滚动 -->
  <div class="flex h-full min-h-0 flex-col gap-3 overflow-hidden">
    <div
      v-if="loading"
      class="flex min-h-0 flex-1 flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border py-16 text-center text-muted-foreground"
    >
      <HistoryIcon class="h-5 w-5" />
      <p class="text-sm">{{ t('history.loading') }}</p>
    </div>

    <div
      v-else-if="loadError"
      class="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-destructive/40 py-16 text-center"
    >
      <HistoryIcon class="h-5 w-5 text-destructive" />
      <div class="flex flex-col gap-1">
        <p class="text-sm font-medium text-foreground">{{ t('history.loadFailed') }}</p>
        <p class="text-[12px] text-muted-foreground">{{ loadError }}</p>
      </div>
      <Button variant="outline" size="sm" @click="refreshHistory">{{ t('common.retry') }}</Button>
    </div>

    <div
      v-else-if="isEmpty"
      class="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border py-16 text-center"
    >
      <div class="flex h-12 w-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
        <HistoryIcon class="h-5 w-5" />
      </div>
      <div class="flex flex-col gap-1">
        <p class="text-sm font-medium text-foreground">{{ t('history.empty') }}</p>
        <p class="text-[12px] text-muted-foreground">{{ t('history.emptyDescription') }}</p>
      </div>
    </div>

    <template v-else>
      <!-- 触发方式筛选（固定顶栏；清空作为筛选栏右侧次要操作） -->
      <div class="shrink-0">
        <div
          ref="filterBarRef"
          class="relative flex items-center gap-1 rounded-md border border-border bg-card p-1 text-[12px]"
          role="tablist"
          :aria-label="t('history.filter.all')"
        >
          <span
            aria-hidden="true"
            class="module-seg-indicator pointer-events-none absolute z-0 rounded bg-accent"
            :class="filterIndicatorReady ? 'module-seg-indicator--ready' : 'opacity-0'"
            :style="filterIndicatorStyle"
          />
          <button
            v-for="f in FILTERS"
            :key="f.id"
            type="button"
            role="tab"
            :ref="(el) => setFilterItemRef(f.id, el)"
            :title="f.label"
            :aria-selected="activeFilter === f.id"
            class="module-seg-item relative z-[1] flex h-7 items-center gap-1.5 rounded px-2.5"
            :class="activeFilter === f.id
              ? (filterIndicatorReady ? 'text-foreground' : 'rounded bg-accent text-foreground')
              : 'text-muted-foreground hover:text-foreground'"
            @click="activeFilter = f.id"
          >
            <component :is="f.icon" class="h-3.5 w-3.5" />
            <span class="whitespace-nowrap">{{ f.label }}</span>
          </button>
          <button
            type="button"
            :title="t('settings.button.clearHistory')"
            class="relative z-[1] ml-auto flex h-7 items-center gap-1.5 rounded px-2.5 text-muted-foreground transition-colors hover:text-destructive disabled:opacity-50"
            :disabled="clearing"
            @click="showClearConfirm = true"
          >
            <Trash2 class="h-3.5 w-3.5" />
            <span class="whitespace-nowrap">{{ t('settings.button.clearHistory') }}</span>
          </button>
        </div>
      </div>

      <!-- 筛选无结果 → 保留筛选栏，替换左右网格 -->
      <div
        v-if="isFilterEmpty"
        class="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border py-16 text-center"
      >
        <div class="flex h-12 w-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
          <component :is="FILTERS.find((f) => f.id === activeFilter)?.icon ?? HistoryIcon" class="h-5 w-5" />
        </div>
        <div class="flex flex-col gap-1">
          <p class="text-sm font-medium text-foreground">{{ t('history.filterEmpty', { filter: activeFilterLabel }) }}</p>
          <p class="text-[12px] text-muted-foreground">{{ t('history.filterEmptyDescription') }}</p>
        </div>
      </div>

      <!-- 左右独立滚动 -->
      <div v-else class="flex min-h-0 flex-1 gap-4 overflow-hidden">
        <!-- 左:列表 -->
        <aside class="flex w-[240px] shrink-0 flex-col gap-3 overflow-y-auto overscroll-contain scrollbar-thin">
          <template v-for="bucket in filteredGrouped" :key="bucket.label">
            <header class="flex items-center gap-2 px-1 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
              <span>{{ bucket.label }}</span>
              <span class="text-[10px] opacity-60">{{ t('history.recordCount', { count: bucket.entries.length }) }}</span>
              <div class="h-px flex-1 bg-border" />
            </header>
            <ul class="flex flex-col gap-1">
              <li
                v-for="s in bucket.entries"
                :key="s.id"
                class="history-list-item flex cursor-pointer flex-col gap-1.5 rounded-md border border-transparent p-2 transition-colors hover:bg-accent/40"
                :class="activeId === s.id ? 'border-primary/40 bg-accent' : ''"
                @click="activeId = s.id"
              >
                <div class="flex items-center gap-1.5 text-[10px] text-muted-foreground">
                  <span class="font-mono">{{ formatTime(s.timestamp) }}</span>
                  <span class="flex items-center rounded border border-border bg-background/60 px-1 py-0.5" :title="TRIGGER_META[s.trigger]?.label">
                    <component :is="triggerIcon(s.trigger)" class="h-3 w-3" />
                  </span>
                  <span class="inline-flex items-center gap-0.5 rounded border border-border bg-background/60 px-1 py-0.5 font-mono tabular-nums" :title="t('history.resultCount', { count: s.results.length })">
                    <Layers class="h-2.5 w-2.5" />
                    {{ s.results.length }}
                  </span>
                  <span class="ml-auto flex items-center gap-1">
                    <template v-if="s.results.some((r) => r.status === 'pending')">
                      <span class="inline-flex items-center gap-0.5 rounded border border-accent/40 bg-accent/10 px-1 py-0.5 text-accent" :title="t('popup.status.translating')">
                        <span class="h-1.5 w-1.5 rounded-full bg-accent" />
                      </span>
                    </template>
                    <template v-if="s.results.some((r) => r.status !== 'success') && !s.results.some((r) => r.status === 'pending')">
                      <span class="h-1.5 w-1.5 shrink-0 rounded-full bg-destructive" :title="t('history.errorCount', { count: s.results.filter((r) => r.status !== 'success').length })" />
                    </template>
                  </span>
                </div>
                <div class="line-clamp-2 text-[12px] leading-snug text-foreground">{{ s.source }}</div>
              </li>
            </ul>
          </template>
        </aside>

        <!-- 右:详情 -->
        <section class="flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto overscroll-contain scrollbar-thin">
          <div v-if="!activeSession" class="flex flex-1 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border py-16 text-center text-muted-foreground">
            <HistoryIcon class="h-6 w-6" />
            <p class="text-sm">{{ t('history.selectSession') }}</p>
          </div>

          <template v-else>
            <header class="flex shrink-0 items-center gap-2 pb-3">
              <component :is="triggerIcon(activeSession.trigger)" class="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
              <h2 class="text-sm leading-none text-foreground">{{ TRIGGER_META[activeSession.trigger]?.label }}</h2>
              <span class="ml-auto text-[11px] leading-none font-mono tabular-nums text-muted-foreground/50">{{ formatDetailTime(activeSession.timestamp) }}</span>
            </header>

            <!-- 详情卡延后挂载：列表先出，Markdown 结果卡 idle 后再渲 -->
            <div v-if="!detailReady" class="flex flex-col gap-2 py-6 text-center text-[12px] text-muted-foreground">
              {{ t('history.loading') }}
            </div>
            <div v-else class="flex flex-col gap-1.5 pb-2">
              <SourceCardView
                :text="activeSession.source"
                :lang-label="langLabel(activeSession.sourceLang)"
                @copy="copy(activeSession.source, true)"
                @speak="speakSource"
              />
              <LanguageToolbar :source="activeSession.sourceLang" :target="activeSession.targetLang" readonly />
              <section>
                <ul class="results flex flex-col gap-2">
                  <li v-for="r in activeSession.results" :key="r.serviceInstanceId + r.modelName" class="relative">
                    <ResultCardView
                      :engine-name="r.serviceName"
                      :service-type="serviceTypeOf(r)"
                      :model-name="resultModelName(r)"
                      :status="cardStatus(r)"
                      :text="resultText(r)"
                      :collapsed="isCollapsed(activeSession.id, r)"
                      :expanded="isExpanded(activeSession.id, r)"
                      :show-tokens="showResultTokens(r)"
                      :input-tokens="r.inputTokens ?? 0"
                      :output-tokens="r.outputTokens ?? 0"
                      :show-actions="r.status !== 'pending'"
                      :show-refresh="false"
                      :markdown-render="state.translation.markdownRender"
                      @copy="copy(resultText(r))"
                      @refresh="retryResult(r)"
                      @speak="speak(resultText(r))"
                      @toggle-collapse="toggleCollapse(activeSession.id, r)"
                      @toggle-expand="toggleExpand(activeSession.id, r)"
                    />
                  </li>
                </ul>
              </section>
            </div>
          </template>
        </section>
      </div>
    </template>

    <!-- 清空确认 -->
    <Dialog v-model:open="showClearConfirm" :title="t('history.clearTitle')" :description="t('history.clearDescription')" width="420px">
      <div class="flex justify-end gap-2 pt-2">
        <Button variant="ghost" size="sm" :disabled="clearing" @click="showClearConfirm = false">{{ t('common.cancel') }}</Button>
        <Button variant="destructive" size="sm" :disabled="clearing" @click="clearAll">
          <Trash2 class="h-3.5 w-3.5" />
          {{ t('history.confirmClear') }}
        </Button>
      </div>
    </Dialog>
  </div>
</template>

<style scoped>
/* 历史筛选分段 pill：只动指示器，列表瞬时切换 */
.module-seg-indicator {
  transition:
    left 280ms cubic-bezier(0.16, 1, 0.3, 1),
    top 280ms cubic-bezier(0.16, 1, 0.3, 1),
    width 280ms cubic-bezier(0.16, 1, 0.3, 1),
    height 280ms cubic-bezier(0.16, 1, 0.3, 1),
    opacity 160ms ease;
}

.module-seg-indicator--ready {
  opacity: 1;
}

.module-seg-item {
  transition: color 180ms cubic-bezier(0.4, 0, 0.2, 1);
}

/* 长列表跳过屏外布局/绘制，减轻切入时主线程压力 */
.history-list-item {
  content-visibility: auto;
  contain-intrinsic-size: auto 72px;
}

@media (prefers-reduced-motion: reduce) {
  .module-seg-indicator,
  .module-seg-item {
    transition: none !important;
  }
}
</style>

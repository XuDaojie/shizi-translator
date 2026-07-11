<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watchEffect } from 'vue'
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
const sessions = ref<HistorySession[]>([])
const loading = ref(false)
const loadError = ref('')
const clearing = ref(false)
let isMounted = false
let refreshRequestId = 0

const refreshHistory = async (): Promise<void> => {
  const requestId = ++refreshRequestId
  if (isMounted) {
    loading.value = true
    loadError.value = ''
  }
  try {
    const nextSessions = await loadHistory(props.state.translation.historyLimit)
    if (!isMounted || requestId !== refreshRequestId) return
    sessions.value = nextSessions
  } catch (err) {
    if (!isMounted || requestId !== refreshRequestId) return
    sessions.value = []
    loadError.value = err instanceof Error ? err.message : String(err)
    toast.error(t('history.loadFailed'), loadError.value)
  } finally {
    if (!isMounted || requestId !== refreshRequestId) return
    loading.value = false
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

/* === 滚动布局测高（复刻原型 updateScrollMetrics） === */
const rootRef = ref<HTMLElement>()
const headerRef = ref<HTMLElement>()
let metricsObserver: ResizeObserver | null = null

const findScroller = (el: HTMLElement | null): HTMLElement | null => {
  let node = el
  while (node) {
    const oy = getComputedStyle(node).overflowY
    if (oy === 'auto' || oy === 'scroll') return node
    node = node.parentElement
  }
  return null
}

const updateScrollMetrics = (): void => {
  const root = rootRef.value
  const header = headerRef.value
  if (!root || !header) return
  const scroller = findScroller(root.parentElement)
  if (!scroller) return
  const clientH = scroller.clientHeight
  const padTop = parseFloat(getComputedStyle(scroller).paddingTop) || 0
  const padBottom = parseFloat(getComputedStyle(scroller).paddingBottom) || 0
  const contentH = clientH - padTop - padBottom
  const headerH = header.offsetHeight
  const GAP = 12
  const asideTop = headerH + GAP
  root.style.setProperty('--history-header-h', `${headerH}px`)
  root.style.setProperty('--history-aside-top', `${asideTop}px`)
  root.style.setProperty('--history-aside-h', `${Math.max(contentH - asideTop - 8, 0)}px`)
}

onMounted(() => {
  isMounted = true
  void refreshHistory()
  updateScrollMetrics()
  metricsObserver = new ResizeObserver(updateScrollMetrics)
  const scroller = findScroller(rootRef.value?.parentElement ?? null)
  if (scroller) metricsObserver.observe(scroller)
  if (headerRef.value) metricsObserver.observe(headerRef.value)
})

onBeforeUnmount(() => {
  isMounted = false
  metricsObserver?.disconnect()
  metricsObserver = null
})
</script>

<template>
  <div ref="rootRef" class="flex flex-col gap-3">
    <div v-if="loading" class="flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border py-16 text-center text-muted-foreground">
      <HistoryIcon class="h-5 w-5" />
        <p class="text-sm">{{ t('history.loading') }}</p>
    </div>

    <div v-else-if="loadError" class="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-destructive/40 py-16 text-center">
      <HistoryIcon class="h-5 w-5 text-destructive" />
      <div class="flex flex-col gap-1">
        <p class="text-sm font-medium text-foreground">{{ t('history.loadFailed') }}</p>
        <p class="text-[12px] text-muted-foreground">{{ loadError }}</p>
      </div>
        <Button variant="outline" size="sm" @click="refreshHistory">{{ t('common.retry') }}</Button>
    </div>

    <div v-else-if="isEmpty" class="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border py-16 text-center">
      <div class="flex h-12 w-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
        <HistoryIcon class="h-5 w-5" />
      </div>
      <div class="flex flex-col gap-1">
        <p class="text-sm font-medium text-foreground">{{ t('history.empty') }}</p>
        <p class="text-[12px] text-muted-foreground">{{ t('history.emptyDescription') }}</p>
      </div>
    </div>

    <template v-else>
      <!-- 触发方式筛选（sticky 置顶，与原型对齐；清空作为筛选栏右侧次要操作） -->
      <div ref="headerRef" class="sticky top-0 z-30 shrink-0 bg-background pb-4">
        <div class="-mt-[10px] h-[10px] bg-background" aria-hidden="true" />
        <div class="flex items-center gap-1 rounded-md border border-border bg-card p-1 text-[12px]">
          <button
            v-for="f in FILTERS"
            :key="f.id"
            :title="f.label"
            class="flex h-7 items-center gap-1.5 rounded px-2.5 transition-colors"
            :class="activeFilter === f.id ? 'bg-accent text-foreground' : 'text-muted-foreground hover:text-foreground'"
            @click="activeFilter = f.id"
          >
            <component :is="f.icon" class="h-3.5 w-3.5" />
            <span class="whitespace-nowrap">{{ f.label }}</span>
          </button>
          <button
            type="button"
          :title="t('settings.button.clearHistory')"
            class="ml-auto flex h-7 items-center gap-1.5 rounded px-2.5 text-muted-foreground transition-colors hover:text-destructive disabled:opacity-50"
            :disabled="clearing"
            @click="showClearConfirm = true"
          >
            <Trash2 class="h-3.5 w-3.5" />
          <span class="whitespace-nowrap">{{ t('settings.button.clearHistory') }}</span>
          </button>
        </div>
      </div>

      <!-- 筛选无结果 → 保留筛选栏，替换左右网格（与原型对齐） -->
      <div
        v-if="isFilterEmpty"
        class="flex flex-1 flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border py-16 text-center"
      >
        <div class="flex h-12 w-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
          <component :is="FILTERS.find((f) => f.id === activeFilter)?.icon ?? HistoryIcon" class="h-5 w-5" />
        </div>
        <div class="flex flex-col gap-1">
          <p class="text-sm font-medium text-foreground">{{ t('history.filterEmpty', { filter: activeFilterLabel }) }}</p>
          <p class="text-[12px] text-muted-foreground">{{ t('history.filterEmptyDescription') }}</p>
        </div>
      </div>

      <!-- 左右布局 -->
      <div v-else class="flex gap-4">
        <!-- 左:列表（独立滚动） -->
        <aside class="w-[240px] shrink-0 self-start sticky top-[var(--history-aside-top)] max-h-[var(--history-aside-h)] flex min-h-0 flex-col gap-3 overflow-y-auto scrollbar-thin">
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
                class="flex cursor-pointer flex-col gap-1.5 rounded-md border border-transparent p-2 transition-colors hover:bg-accent/40"
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
        <section class="flex min-w-0 flex-1 flex-col">
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

            <div class="flex flex-col gap-1.5">
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

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watchEffect } from 'vue'
import { History as HistoryIcon, Trash2, Camera, ScanText, MousePointerSquareDashed, PencilLine, Layers } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import Dialog from '@/components/ui/dialog/Dialog.vue'
import { toast } from '@/lib/toast'
import { speakText } from '@/popup/composables/utils'
import { LANGUAGES } from '../tokens'
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

interface Props {
  state: AppSettings
}
const props = defineProps<Props>()

const LANG_MAP = new Map(LANGUAGES.map((l) => [l.value, l.label]))
const LANG_SHORT_MAP = new Map(LANGUAGES.map((l) => [l.value.split('-')[0], l.label]))
const langLabel = (code: string): string => LANG_MAP.get(code) ?? LANG_SHORT_MAP.get(code) ?? code

const TRIGGER_META: Record<HistoryTrigger, { label: string; icon: typeof Camera }> = {
  selection: { label: '划词翻译', icon: MousePointerSquareDashed },
  manual: { label: '手动输入', icon: PencilLine },
  screenshot: { label: '截图翻译', icon: ScanText },
}

const FILTERS = [
  { id: 'all' as const, label: '全部', icon: Layers },
  { id: 'screenshot' as const, label: '截图翻译', icon: ScanText },
  { id: 'selection' as const, label: '划词翻译', icon: MousePointerSquareDashed },
  { id: 'manual' as const, label: '手动输入', icon: PencilLine },
]

const activeFilter = ref<'all' | HistoryTrigger>('all')
const activeId = ref<string>('')
const showClearConfirm = ref(false)
const sessions = ref<HistorySession[]>([])
const loading = ref(false)
const loadError = ref('')

const refreshHistory = async (): Promise<void> => {
  loading.value = true
  loadError.value = ''
  try {
    sessions.value = await loadHistory(props.state.translation.historyLimit)
  } catch (err) {
    sessions.value = []
    loadError.value = err instanceof Error ? err.message : String(err)
    toast.error('读取翻译历史失败', loadError.value)
  } finally {
    loading.value = false
  }
}

const isEmpty = computed(() => isEmptyHistory(sessions.value))
const activeSession = computed<HistorySession | null>(() =>
  activeId.value ? sessions.value.find((s) => s.id === activeId.value) ?? null : null,
)

/* 首条默认选中 */
watchEffect(() => {
  if (!activeId.value && sessions.value.length > 0) {
    activeId.value = sessions.value[0].id
  }
  if (activeId.value && !sessions.value.some((s) => s.id === activeId.value)) {
    activeId.value = sessions.value[0]?.id ?? ''
  }
})

const formatDetailTime = (iso: string): string => {
  const d = new Date(iso)
  const Y = d.getFullYear()
  const MO = String(d.getMonth() + 1).padStart(2, '0')
  const DD = String(d.getDate()).padStart(2, '0')
  const HH = String(d.getHours()).padStart(2, '0')
  const MM = String(d.getMinutes()).padStart(2, '0')
  const SS = String(d.getSeconds()).padStart(2, '0')
  return `${Y}-${MO}-${DD} ${HH}:${MM}:${SS}`
}

const formatTime = (iso: string): string => {
  const d = new Date(iso)
  const now = new Date()
  const sameDay = (a: Date, b: Date): boolean =>
    a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
  const HH = String(d.getHours()).padStart(2, '0')
  const MM = String(d.getMinutes()).padStart(2, '0')
  if (sameDay(d, now)) return `${HH}:${MM}`
  const y = new Date(now); y.setDate(now.getDate() - 1)
  if (sameDay(d, y)) return `昨天 ${HH}:${MM}`
  const diff = Math.floor((now.getTime() - d.getTime()) / 86400000)
  if (diff < 7) return `${diff} 天前`
  const MO = String(d.getMonth() + 1).padStart(2, '0')
  const DD = String(d.getDate()).padStart(2, '0')
  return `${MO}-${DD} ${HH}:${MM}`
}

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
  if (today.length) out.push({ label: '今天', entries: today })
  if (yesterday.length) out.push({ label: '昨天', entries: yesterday })
  if (week.length) out.push({ label: '本周', entries: week })
  if (older.length) out.push({ label: '更早', entries: older })
  return out
})

const filteredGrouped = computed<Bucket[]>(() => {
  if (activeFilter.value === 'all') return grouped.value
  return grouped.value
    .map((b) => ({ ...b, entries: b.entries.filter((s) => s.trigger === activeFilter.value) }))
    .filter((b) => b.entries.length > 0)
})

const copy = async (text: string, isSource = false): Promise<void> => {
  if (!text) { toast.error('复制失败', '该记录没有可复制的文本'); return }
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
    toast.success(isSource ? '已复制原文' : '已复制译文', text.length > 30 ? `${text.slice(0, 30)}…` : text)
  } catch (err) {
    toast.error('复制失败', err instanceof Error ? err.message : '请检查浏览器权限')
  }
}

const clearAll = async (): Promise<void> => {
  try {
    sessions.value = await clearHistoryAndReload()
    showClearConfirm.value = false
    activeId.value = ''
    toast.success('已清空翻译历史')
  } catch (err) {
    toast.error('清空翻译历史失败', err instanceof Error ? err.message : String(err))
  }
}

const retryResult = (r: HistoryResult): void => {
  toast.info('已请求重新翻译', `${r.serviceName} · ${r.modelName || '默认模型'}`)
}

/* 卡片折叠态：按 sessionId + serviceInstanceId 记录。 */
const collapsedMap = reactive<Record<string, boolean>>({})
const cardKey = (sessionId: string, r: HistoryResult): string => `${sessionId}:${r.serviceInstanceId}`
const isCollapsed = (sessionId: string, r: HistoryResult): boolean => collapsedMap[cardKey(sessionId, r)] ?? false
const toggleCollapse = (sessionId: string, r: HistoryResult): void => {
  const k = cardKey(sessionId, r)
  collapsedMap[k] = !collapsedMap[k]
}

const speakSource = (): void => {
  const text = activeSession.value?.source
  if (!text) { toast.error('朗读失败', '该记录没有原文可朗读'); return }
  const lang = activeSession.value?.sourceLang && activeSession.value.sourceLang !== 'auto'
    ? activeSession.value.sourceLang
    : 'en-US'
  speakText(text, lang)
}

const speak = (text: string): void => {
  if (!text) { toast.error('朗读失败', '该记录没有可朗读的译文'); return }
  speakText(text, activeSession.value?.targetLang || 'zh-CN')
}

const triggerIcon = (t: HistoryTrigger): typeof Camera => TRIGGER_META[t]?.icon ?? Camera

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
  void refreshHistory()
  updateScrollMetrics()
  metricsObserver = new ResizeObserver(updateScrollMetrics)
  const scroller = findScroller(rootRef.value?.parentElement ?? null)
  if (scroller) metricsObserver.observe(scroller)
  if (headerRef.value) metricsObserver.observe(headerRef.value)
})

onBeforeUnmount(() => {
  metricsObserver?.disconnect()
  metricsObserver = null
})
</script>

<template>
  <div ref="rootRef" class="flex flex-col gap-3">
    <div class="flex items-center justify-end">
      <Button variant="ghost" size="sm" :disabled="isEmpty || loading" class="text-muted-foreground hover:text-destructive" @click="showClearConfirm = true">
        <Trash2 class="h-3.5 w-3.5" />
        清空全部
      </Button>
    </div>

    <div v-if="loading" class="flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border py-16 text-center text-muted-foreground">
      <HistoryIcon class="h-5 w-5" />
      <p class="text-sm">正在加载翻译历史...</p>
    </div>

    <div v-else-if="loadError" class="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-destructive/40 py-16 text-center">
      <HistoryIcon class="h-5 w-5 text-destructive" />
      <div class="flex flex-col gap-1">
        <p class="text-sm font-medium text-foreground">翻译历史加载失败</p>
        <p class="text-[12px] text-muted-foreground">{{ loadError }}</p>
      </div>
      <Button variant="outline" size="sm" @click="refreshHistory">重试</Button>
    </div>

    <div v-else-if="isEmpty" class="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border py-16 text-center">
      <div class="flex h-12 w-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
        <HistoryIcon class="h-5 w-5" />
      </div>
      <div class="flex flex-col gap-1">
        <p class="text-sm font-medium text-foreground">暂无翻译历史</p>
        <p class="text-[12px] text-muted-foreground">手动输入、划词或截图 OCR 翻译后，结果会自动保存在这里。</p>
      </div>
    </div>

    <template v-else>
      <!-- 触发方式筛选（sticky 冻结顶部） -->
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
        </div>
      </div>

      <!-- 左右布局 -->
      <div class="flex gap-4">
        <!-- 左:列表（独立滚动） -->
        <aside class="w-[240px] shrink-0 self-start sticky top-[var(--history-aside-top)] max-h-[var(--history-aside-h)] flex min-h-0 flex-col gap-3 overflow-y-auto scrollbar-thin">
          <template v-for="bucket in filteredGrouped" :key="bucket.label">
            <header class="flex items-center gap-2 px-1 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
              <span>{{ bucket.label }}</span>
              <span class="text-[10px] opacity-60">{{ bucket.entries.length }} 条</span>
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
                  <span class="inline-flex items-center gap-0.5 rounded border border-border bg-background/60 px-1 py-0.5 font-mono tabular-nums" :title="`${s.results.length} 个翻译渠道`">
                    <Layers class="h-2.5 w-2.5" />
                    {{ s.results.length }}
                  </span>
                  <template v-if="s.results.some((r) => r.status !== 'success')">
                    <span class="h-1.5 w-1.5 shrink-0 rounded-full bg-destructive" :title="`${s.results.filter((r) => r.status !== 'success').length} 个翻译结果异常`" />
                  </template>
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
            <p class="text-sm">从左侧选一条会话查看详情</p>
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
                      :model-name="r.modelName"
                      :status="cardStatus(r)"
                      :text="resultText(r)"
                      :collapsed="isCollapsed(activeSession.id, r)"
                      :show-tokens="false"
                      :input-tokens="r.inputTokens ?? undefined"
                      :output-tokens="r.outputTokens ?? undefined"
                      :show-actions="r.status !== 'pending'"
                      :show-refresh="false"
                      @copy="copy(resultText(r))"
                      @refresh="retryResult(r)"
                      @speak="speak(resultText(r))"
                      @toggle-collapse="toggleCollapse(activeSession.id, r)"
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
    <Dialog v-model:open="showClearConfirm" title="清空全部翻译历史?" description="此操作不可撤销，所有翻译历史都会被永久删除。" width="420px">
      <div class="flex justify-end gap-2 pt-2">
        <Button variant="ghost" size="sm" @click="showClearConfirm = false">取消</Button>
        <Button variant="destructive" size="sm" @click="clearAll">
          <Trash2 class="h-3.5 w-3.5" />
          确认清空
        </Button>
      </div>
    </Dialog>
  </div>
</template>

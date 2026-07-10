<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watchEffect } from 'vue'
import { History as HistoryIcon, Trash2, Camera, ScanText, MousePointerSquareDashed, ClipboardList, PencilLine, Layers } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import Dialog from '@/components/ui/dialog/Dialog.vue'
import { toast } from '@/lib/toast'
import { speakText } from '@/popup/composables/utils'
import { LANGUAGES } from '../tokens'
import SourceCardView from '@/popup/components/SourceCardView.vue'
import ResultCardView from '@/popup/components/ResultCardView.vue'
import LanguageToolbar from '@/popup/components/LanguageToolbar.vue'
import type { AppSettings, OcrHistoryEntry } from '../types'

/** 本地适配类型（不污染 types.ts，未来接后端多渠道后整体删除）。 */
type HistoryTrigger = 'selection' | 'clipboard' | 'manual' | 'screenshot'
interface HistoryResult {
  serviceInstanceId: string
  serviceName: string
  modelName: string
  translation: string
  status: 'success' | 'loading' | 'pending' | 'error' | 'aborted'
  inputTokens: number
  outputTokens: number
}
interface HistorySession {
  id: string
  timestamp: string
  trigger: HistoryTrigger
  sourceLang: string
  targetLang: string
  source: string
  results: HistoryResult[]
}

interface Props {
  state: AppSettings
}
const props = defineProps<Props>()

const LANG_MAP = new Map(LANGUAGES.map((l) => [l.value, l.label]))
const LANG_SHORT_MAP = new Map(LANGUAGES.map((l) => [l.value.split('-')[0], l.label]))
const langLabel = (code: string): string => LANG_MAP.get(code) ?? LANG_SHORT_MAP.get(code) ?? code

const TRIGGER_META: Record<HistoryTrigger, { label: string; icon: typeof Camera }> = {
  selection: { label: '划词翻译', icon: MousePointerSquareDashed },
  clipboard: { label: '剪贴板', icon: ClipboardList },
  manual: { label: '手动输入', icon: PencilLine },
  screenshot: { label: '截图翻译', icon: ScanText },
}

const FILTERS = [
  { id: 'all' as const, label: '全部', icon: Layers },
  { id: 'screenshot' as const, label: '截图翻译', icon: ScanText },
  { id: 'selection' as const, label: '划词翻译', icon: MousePointerSquareDashed },
  { id: 'manual' as const, label: '手动输入', icon: PencilLine },
  { id: 'clipboard' as const, label: '剪贴板', icon: ClipboardList },
]

const activeFilter = ref<'all' | HistoryTrigger>('all')
const activeId = ref<string>('')
const showClearConfirm = ref(false)

/** DEV ONLY：历史面板空数据时的 mock 演示数据，覆盖四桶/4 种 trigger/success+error/多渠道多语言。
 *  真实 OCR 历史写入后自动隐藏；点「清空全部」也会隐藏（mockDismissed）。
 *  后续接后端多渠道历史后整体删除。 */
const MOCK_SESSIONS: HistorySession[] = (() => {
  const now = Date.now()
  const min = 60 * 1000
  const hour = 60 * min
  const day = 24 * hour
  return [
    {
      id: 'mock-1',
      timestamp: new Date(now - 2 * hour).toISOString(),
      trigger: 'screenshot',
      sourceLang: 'en',
      targetLang: 'zh-CN',
      source: 'Please select your preferred language from the list below to continue.',
      results: [{ serviceInstanceId: 'openai', serviceName: 'OpenAI', modelName: 'gpt-4o', translation: '请从下方列表中选择您偏好的语言以继续。', status: 'success', inputTokens: 142, outputTokens: 86 }],
    },
    {
      id: 'mock-2',
      timestamp: new Date(now - 45 * min).toISOString(),
      trigger: 'selection',
      sourceLang: 'en',
      targetLang: 'zh-CN',
      source: 'The quick brown fox jumps over the lazy dog.',
      results: [{ serviceInstanceId: 'claude', serviceName: 'Claude', modelName: 'claude-sonnet-4', translation: '敏捷的棕色狐狸跳过了懒狗。', status: 'success', inputTokens: 98, outputTokens: 64 }],
    },
    {
      id: 'mock-3',
      timestamp: new Date(now - 26 * hour).toISOString(),
      trigger: 'manual',
      sourceLang: 'ja',
      targetLang: 'zh-CN',
      source: '設定を変更するには、このボタンをクリックしてください。',
      results: [{ serviceInstanceId: 'deepseek', serviceName: 'DeepSeek', modelName: 'deepseek-chat', translation: '要更改设置，请点击此按钮。', status: 'success', inputTokens: 156, outputTokens: 92 }],
    },
    {
      id: 'mock-4',
      timestamp: new Date(now - 3 * day).toISOString(),
      trigger: 'screenshot',
      sourceLang: 'ko',
      targetLang: 'zh-CN',
      source: '계속하려면 아래 버튼을 클릭하세요. 도움이 필요하시면 고객센터로 문의해 주세요.',
      results: [{ serviceInstanceId: 'openai', serviceName: 'OpenAI', modelName: 'gpt-4o', translation: '请点击下方按钮继续。如需帮助，请联系客服中心。', status: 'success', inputTokens: 204, outputTokens: 118 }],
    },
    {
      id: 'mock-5',
      timestamp: new Date(now - 5 * day).toISOString(),
      trigger: 'clipboard',
      sourceLang: 'en',
      targetLang: 'zh-CN',
      source: 'Error: Unable to connect to the server. Please check your network connection and try again later.',
      results: [{ serviceInstanceId: 'claude', serviceName: 'Claude', modelName: 'claude-sonnet-4', translation: '', status: 'error', inputTokens: 0, outputTokens: 0 }],
    },
    {
      id: 'mock-6',
      timestamp: new Date(now - 12 * day).toISOString(),
      trigger: 'screenshot',
      sourceLang: 'en',
      targetLang: 'zh-CN',
      source: 'Welcome to our application. We hope you enjoy your experience and find it useful.',
      results: [{ serviceInstanceId: 'deepseek', serviceName: 'DeepSeek', modelName: 'deepseek-chat', translation: '欢迎使用我们的应用程序。希望您使用愉快并觉得它有用。', status: 'success', inputTokens: 132, outputTokens: 78 }],
    },
  ]
})()

/** 点「清空全部」后隐藏 mock 演示数据（dev only）。 */
const mockDismissed = ref(false)

/** OcrHistoryEntry -> 伪 HistorySession（spec 7.1）。OCR 记录单结果，trigger 恒为 screenshot。
 *  无真实数据时回落到 MOCK_SESSIONS（dev only），让历史面板有演示效果。 */
const adaptedSessions = computed<HistorySession[]>(() => {
  if (props.state.ocrHistory.length > 0) {
    return props.state.ocrHistory.map((e: OcrHistoryEntry) => {
      const svc = e.serviceInstanceId ? props.state.services.find((s) => s.id === e.serviceInstanceId) : undefined
      return {
        id: e.id,
        timestamp: e.timestamp,
        trigger: 'screenshot',
        sourceLang: e.sourceLang,
        targetLang: e.targetLang,
        source: e.source,
        results: [{
          serviceInstanceId: e.serviceInstanceId ?? 'unknown',
          serviceName: svc?.name ?? '(已删除)',
          modelName: '',
          translation: e.translation,
          status: (e.translation ? 'success' : 'error') as 'success' | 'error',
          inputTokens: 0,
          outputTokens: 0,
        }],
      }
    })
  }
  // DEV ONLY：无真实 OCR 历史时回落 mock 演示数据
  return mockDismissed.value ? [] : MOCK_SESSIONS
})

const isEmpty = computed(() => adaptedSessions.value.length === 0)
const activeSession = computed<HistorySession | null>(() =>
  activeId.value ? adaptedSessions.value.find((s) => s.id === activeId.value) ?? null : null,
)

/* 首条默认选中 */
watchEffect(() => {
  if (!activeId.value && adaptedSessions.value.length > 0) {
    activeId.value = adaptedSessions.value[0].id
  }
  if (activeId.value && !adaptedSessions.value.some((s) => s.id === activeId.value)) {
    activeId.value = adaptedSessions.value[0]?.id ?? ''
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
  for (const s of adaptedSessions.value) {
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

const clearAll = (): void => {
  props.state.ocrHistory = []
  mockDismissed.value = true  // DEV ONLY：同时隐藏 mock 演示数据
  showClearConfirm.value = false
  activeId.value = ''
  toast.success('已清空翻译历史')
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
  const inst = props.state.services.find((s) => s.id === r.serviceInstanceId)
  if (inst?.type) return inst.type
  // mock / 旧数据可能把 type 写在 serviceInstanceId
  return r.serviceInstanceId
}

const cardStatus = (r: HistoryResult): 'success' | 'loading' | 'pending' | 'error' | 'aborted' => r.status

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
    <!-- 顶部说明 + 清空全部 -->
    <div class="flex items-center justify-between gap-4 rounded-md border border-amber-200/70 bg-amber-50/40 px-3 py-2 dark:border-amber-900/40 dark:bg-amber-900/10">
      <div class="flex items-start gap-2 text-[12px] leading-relaxed text-amber-900/80 dark:text-amber-200/80">
        <span class="mt-0.5 h-1.5 w-1.5 shrink-0 rounded-full bg-amber-500" />
        <span>此功能正在开发中 · 仅记录截图翻译(OCR)结果,划词/取词/输入框翻译不计入</span>
      </div>
      <Button variant="ghost" size="sm" :disabled="isEmpty" class="text-muted-foreground hover:text-destructive" @click="showClearConfirm = true">
        <Trash2 class="h-3.5 w-3.5" />
        清空全部
      </Button>
    </div>

    <!-- 空状态 -->
    <div v-if="isEmpty" class="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border py-16 text-center">
      <div class="flex h-12 w-12 items-center justify-center rounded-full bg-muted text-muted-foreground">
        <HistoryIcon class="h-5 w-5" />
      </div>
      <div class="flex flex-col gap-1">
        <p class="text-sm font-medium text-foreground">暂无截图翻译记录</p>
        <p class="text-[12px] text-muted-foreground">使用快捷键截图翻译后,识别与翻译结果会自动保存在这里。</p>
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
                      :text="r.translation"
                      :collapsed="isCollapsed(activeSession.id, r)"
                      :show-tokens="false"
                      :input-tokens="r.inputTokens"
                      :output-tokens="r.outputTokens"
                      :show-actions="r.status !== 'pending'"
                      :show-refresh="false"
                      @copy="copy(r.translation)"
                      @refresh="retryResult(r)"
                      @speak="speak(r.translation)"
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
    <Dialog v-model:open="showClearConfirm" title="清空全部翻译历史?" description="此操作不可撤销,所有截图翻译记录都将被永久删除。" width="420px">
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

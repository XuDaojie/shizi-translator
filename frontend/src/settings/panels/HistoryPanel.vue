<script setup lang="ts">
import { computed, ref } from 'vue'
import { Copy, History as HistoryIcon, Trash2, Camera } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import Dialog from '@/components/ui/dialog/Dialog.vue'
import { toast } from '@/lib/toast'
import { LANGUAGES } from '../tokens'
import type { AppSettings, OcrHistoryEntry } from '../types'

interface Props {
  state: AppSettings
}
const props = defineProps<Props>()

/** 全局快捷键面板里"截图翻译"那个 binding,用于空状态提示。 */
const ocrShortcut = computed(
  () =>
    props.state.shortcut.bindings.find((b) => b.id === 'translate-screenshot')?.keys ??
    'Ctrl+Shift+S',
)

/** ISO 码 → 显示名(找不到时回退到原码,避免 UI 出现 raw code)。 */
const LANG_MAP = new Map(LANGUAGES.map((l) => [l.value, l.label]))
const langLabel = (code: string): string =>
  LANG_MAP.get(code) ?? code

const showClearConfirm = ref(false)

/** 时间格式化:今天 HH:MM,昨天"昨天 HH:MM",本周"X 天前",更早"MM-DD HH:MM"。 */
const formatTime = (iso: string): string => {
  const d = new Date(iso)
  const now = new Date()
  const sameDay = (a: Date, b: Date): boolean =>
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()

  const HH = String(d.getHours()).padStart(2, '0')
  const MM = String(d.getMinutes()).padStart(2, '0')
  if (sameDay(d, now)) return `${HH}:${MM}`

  const yesterday = new Date(now)
  yesterday.setDate(now.getDate() - 1)
  if (sameDay(d, yesterday)) return `昨天 ${HH}:${MM}`

  const diffDays = Math.floor((now.getTime() - d.getTime()) / (24 * 60 * 60 * 1000))
  if (diffDays < 7) return `${diffDays} 天前`

  const MO = String(d.getMonth() + 1).padStart(2, '0')
  const DD = String(d.getDate()).padStart(2, '0')
  return `${MO}-${DD} ${HH}:${MM}`
}

/** 按日期分桶,只展示今天/昨天/本周/更早四档,每桶内部按时间倒序。 */
type Bucket = { label: string; entries: OcrHistoryEntry[] }

const grouped = computed<Bucket[]>(() => {
  const now = new Date()
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()
  const startOfYesterday = startOfToday - 24 * 60 * 60 * 1000
  const startOfWeek = startOfToday - 7 * 24 * 60 * 60 * 1000

  const today: OcrHistoryEntry[] = []
  const yesterday: OcrHistoryEntry[] = []
  const week: OcrHistoryEntry[] = []
  const older: OcrHistoryEntry[] = []

  for (const e of props.state.ocrHistory) {
    const t = new Date(e.timestamp).getTime()
    if (t >= startOfToday) today.push(e)
    else if (t >= startOfYesterday) yesterday.push(e)
    else if (t >= startOfWeek) week.push(e)
    else older.push(e)
  }

  const result: Bucket[] = []
  if (today.length) result.push({ label: '今天', entries: today })
  if (yesterday.length) result.push({ label: '昨天', entries: yesterday })
  if (week.length) result.push({ label: '本周', entries: week })
  if (older.length) result.push({ label: '更早', entries: older })
  return result
})

const isEmpty = computed(() => props.state.ocrHistory.length === 0)

/** 找某条记录所属的 service 实例(用于显示"由 X 翻译"),已删除时降级为 null。 */
const serviceName = (entry: OcrHistoryEntry): string | null => {
  if (!entry.serviceInstanceId) return null
  const inst = props.state.services.find((s) => s.id === entry.serviceInstanceId)
  return inst?.name ?? null
}

const copy = async (entry: OcrHistoryEntry): Promise<void> => {
  const text = entry.translation || entry.source
  if (!text) {
    toast.error('复制失败', '该记录没有可复制的文本')
    return
  }
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
    } else {
      // 后备方案:走 textarea + execCommand
      const ta = document.createElement('textarea')
      ta.value = text
      ta.style.position = 'fixed'
      ta.style.opacity = '0'
      document.body.appendChild(ta)
      ta.select()
      document.execCommand('copy')
      document.body.removeChild(ta)
    }
    toast.success('已复制译文', text.length > 30 ? `${text.slice(0, 30)}…` : text)
  } catch (err) {
    toast.error('复制失败', err instanceof Error ? err.message : '请检查浏览器权限')
  }
}

const remove = (entry: OcrHistoryEntry): void => {
  props.state.ocrHistory.splice(
    props.state.ocrHistory.findIndex((e) => e.id === entry.id),
    1,
  )
  toast.info('已删除', entry.translation.slice(0, 30) || entry.source.slice(0, 30))
}

const clearAll = (): void => {
  props.state.ocrHistory = []
  showClearConfirm.value = false
  toast.success('已清空翻译历史')
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <!-- 顶部说明 + 清空全部 -->
    <div
      class="flex items-center justify-between gap-4 rounded-md border border-amber-200/70 bg-amber-50/40 px-3 py-2 dark:border-amber-900/40 dark:bg-amber-900/10"
    >
      <div class="flex items-start gap-2 text-[12px] leading-relaxed text-amber-900/80 dark:text-amber-200/80">
        <span class="mt-0.5 h-1.5 w-1.5 shrink-0 rounded-full bg-amber-500" />
        <span>
          此功能正在开发中 · 仅记录截图翻译(OCR)结果,划词/取词/输入框翻译不计入
        </span>
      </div>
      <Button
        variant="ghost"
        size="sm"
        :disabled="isEmpty"
        class="text-muted-foreground hover:text-destructive"
        @click="showClearConfirm = true"
      >
        <Trash2 class="h-3.5 w-3.5" />
        清空全部
      </Button>
    </div>

    <!-- 空状态 -->
    <div
      v-if="isEmpty"
      class="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-border py-16 text-center"
    >
      <div
        class="flex h-12 w-12 items-center justify-center rounded-full bg-muted text-muted-foreground"
      >
        <HistoryIcon class="h-5 w-5" />
      </div>
      <div class="flex flex-col gap-1">
        <p class="text-sm font-medium text-foreground">暂无截图翻译记录</p>
        <p class="text-[12px] text-muted-foreground">
          使用快捷键 <kbd class="rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[11px]">{{ ocrShortcut }}</kbd>
          截取屏幕区域,识别与翻译结果会自动保存在这里。
        </p>
      </div>
    </div>

    <!-- 分组列表 -->
    <template v-else>
      <section v-for="bucket in grouped" :key="bucket.label" class="flex flex-col gap-2">
        <header class="flex items-center gap-2 px-1">
          <h3 class="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
            {{ bucket.label }}
          </h3>
          <span class="text-[10px] text-muted-foreground/60">
            {{ bucket.entries.length }} 条
          </span>
          <div class="h-px flex-1 bg-border" />
        </header>

        <ul class="flex flex-col gap-2">
          <li
            v-for="entry in bucket.entries"
            :key="entry.id"
            class="group flex flex-col gap-2 rounded-md border border-border bg-card px-4 py-3 transition-colors hover:border-muted-foreground/30 hover:bg-accent/30"
          >
            <!-- 顶部:时间 + 语种 + 操作 -->
            <div class="flex items-center justify-between gap-3">
              <div class="flex min-w-0 items-center gap-2 text-[11px] text-muted-foreground">
                <span class="font-mono">{{ formatTime(entry.timestamp) }}</span>
                <span class="text-muted-foreground/40">·</span>
                <span class="flex items-center gap-1">
                  <Badge variant="outline" class="h-4 px-1.5 text-[10px] font-normal">
                    {{ langLabel(entry.sourceLang) }}
                  </Badge>
                  <span class="text-muted-foreground/50">→</span>
                  <Badge variant="outline" class="h-4 px-1.5 text-[10px] font-normal">
                    {{ langLabel(entry.targetLang) }}
                  </Badge>
                </span>
                <template v-if="serviceName(entry)">
                  <span class="text-muted-foreground/40">·</span>
                  <span class="truncate">由 {{ serviceName(entry) }} 翻译</span>
                </template>
              </div>
              <div class="flex shrink-0 items-center gap-0.5 opacity-60 transition-opacity group-hover:opacity-100">
                <Button
                  variant="ghost"
                  size="sm"
                  class="h-7 px-2 text-[11px] text-muted-foreground hover:text-foreground"
                  :title="`复制译文:${entry.translation.slice(0, 20)}${entry.translation.length > 20 ? '…' : ''}`"
                  @click="copy(entry)"
                >
                  <Copy class="h-3.5 w-3.5" />
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  class="h-7 px-2 text-[11px] text-muted-foreground hover:text-destructive"
                  title="删除此条"
                  @click="remove(entry)"
                >
                  <Trash2 class="h-3.5 w-3.5" />
                </Button>
              </div>
            </div>

            <!-- 原文(mono,浅色,2 行截断) -->
            <p
              v-if="entry.source"
              class="font-mono text-[12px] leading-relaxed text-muted-foreground line-clamp-2"
            >
              {{ entry.source }}
            </p>

            <!-- 译文(主色) -->
            <p
              v-if="entry.translation"
              class="text-sm leading-relaxed text-foreground"
            >
              {{ entry.translation }}
            </p>

            <!-- 完全空(识别失败) -->
            <p
              v-if="!entry.source && !entry.translation"
              class="flex items-center gap-1.5 text-[12px] italic text-muted-foreground/70"
            >
              <Camera class="h-3 w-3" />
              此条记录没有可用文本
            </p>
          </li>
        </ul>
      </section>
    </template>

    <!-- 清空确认(通过 v-model:open 编程式打开,无 trigger) -->
    <Dialog
      v-model:open="showClearConfirm"
      title="清空全部翻译历史?"
      description="此操作不可撤销,所有截图翻译记录都将被永久删除。"
      width="420px"
    >
      <div class="flex justify-end gap-2 pt-2">
        <Button variant="ghost" size="sm" @click="showClearConfirm = false">取消</Button>
        <Button
          variant="destructive"
          size="sm"
          @click="clearAll"
        >
          <Trash2 class="h-3.5 w-3.5" />
          确认清空
        </Button>
      </div>
    </Dialog>
  </div>
</template>

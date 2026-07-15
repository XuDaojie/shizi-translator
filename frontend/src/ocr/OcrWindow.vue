<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { getTauriApis, copyText } from '@/popup/composables/utils'
import { createLogger } from '@public/logger.js'
import type { OcrRunMeta, OcrStatus, RecognizeImageResponse } from './types'

const logger = createLogger('ocr')

const status = ref<OcrStatus>('idle')
const previewUrl = ref<string | null>(null)
const text = ref('')
const meta = ref<OcrRunMeta | null>(null)
const errorMessage = ref('')
const engineSummary = ref('')
const copyHint = ref('')
const hasLastImage = ref(false)

const isLoading = computed(() => status.value === 'loading')
const hasPreview = computed(() => Boolean(previewUrl.value))
const hasText = computed(() => text.value.length > 0)
const canRerecognize = computed(() => hasLastImage.value && !isLoading.value)

let unlistenResult: (() => void) | null = null
let unlistenFailed: (() => void) | null = null
let disposed = false

function formatBytes(n: number | null | undefined): string {
  if (n == null) return '—'
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / (1024 * 1024)).toFixed(2)} MB`
}

function applySuccess(payload: RecognizeImageResponse): void {
  const m = payload.meta
  previewUrl.value = `data:image/png;base64,${payload.previewPngBase64}`
  text.value = payload.text ?? ''
  meta.value = m
  engineSummary.value = m.model ? `${m.engine} · ${m.model}` : m.engine
  errorMessage.value = ''
  copyHint.value = ''
  status.value = 'success'
  hasLastImage.value = true
  logger.info('OCR 识别成功', {
    engine: m.engine,
    model: m.model,
    latencyMs: m.latencyMs,
    textLen: text.value.length,
  })
}

function applyError(message: string): void {
  errorMessage.value = message || '识别失败'
  status.value = 'error'
  copyHint.value = ''
  logger.warn('OCR 识别失败', errorMessage.value)
}

async function onCapture(): Promise<void> {
  const apis = getTauriApis()
  if (!apis) {
    applyError('Tauri API 未就绪')
    return
  }
  status.value = 'loading'
  errorMessage.value = ''
  copyHint.value = ''
  try {
    await apis.invoke('start_ocr_capture')
    // 结果由 ocr:recognize-result / ocr:recognize-failed 事件回传
  } catch (e) {
    applyError(String(e))
  }
}

async function onOpenFile(): Promise<void> {
  const apis = getTauriApis()
  if (!apis) {
    applyError('Tauri API 未就绪')
    return
  }
  status.value = 'loading'
  errorMessage.value = ''
  copyHint.value = ''
  try {
    const result = await apis.invoke<RecognizeImageResponse | null>('pick_and_recognize_image')
    if (result == null) {
      // 用户取消文件选择，保持 idle（若此前有成功结果则回到 success）
      status.value = meta.value ? 'success' : 'idle'
      return
    }
    applySuccess(result)
  } catch (e) {
    applyError(String(e))
  }
}

async function onClipboard(): Promise<void> {
  const apis = getTauriApis()
  if (!apis) {
    applyError('Tauri API 未就绪')
    return
  }
  status.value = 'loading'
  errorMessage.value = ''
  copyHint.value = ''
  try {
    const result = await apis.invoke<RecognizeImageResponse>('recognize_clipboard_image')
    applySuccess(result)
  } catch (e) {
    applyError(String(e))
  }
}

async function onRerecognize(): Promise<void> {
  const apis = getTauriApis()
  if (!apis) {
    applyError('Tauri API 未就绪')
    return
  }
  status.value = 'loading'
  errorMessage.value = ''
  copyHint.value = ''
  try {
    const result = await apis.invoke<RecognizeImageResponse>('rerecognize_last_image')
    applySuccess(result)
  } catch (e) {
    applyError(String(e))
  }
}

async function onCopy(): Promise<void> {
  if (!text.value) return
  const ok = await copyText(text.value)
  copyHint.value = ok ? '已复制' : '复制失败'
  if (ok) {
    window.setTimeout(() => {
      if (copyHint.value === '已复制') copyHint.value = ''
    }, 1500)
  }
}

async function setWindowTitle(): Promise<void> {
  const apis = getTauriApis()
  if (!apis) return
  try {
    await (apis.getCurrentWindow() as ReturnType<typeof apis.getCurrentWindow> & {
      setTitle: (title: string) => Promise<void>
    }).setTitle('Shizi 文字识别')
  } catch (e) {
    logger.warn('设置 OCR 窗口标题失败', String(e))
  }
}

async function setupListeners(): Promise<void> {
  const apis = getTauriApis()
  if (!apis) {
    logger.warn('Tauri API 未就绪，无法监听 OCR 事件')
    return
  }
  try {
    const u1 = await apis.listen<RecognizeImageResponse>('ocr:recognize-result', (ev) => {
      applySuccess(ev.payload)
    })
    const u2 = await apis.listen<string>('ocr:recognize-failed', (ev) => {
      applyError(String(ev.payload))
    })
    if (disposed) {
      u1()
      u2()
      return
    }
    unlistenResult = u1
    unlistenFailed = u2
  } catch (e) {
    logger.warn('注册 OCR 事件监听失败', String(e))
  }
}

onMounted(() => {
  void setWindowTitle()
  void setupListeners()
})

onUnmounted(() => {
  disposed = true
  unlistenResult?.()
  unlistenFailed?.()
  unlistenResult = null
  unlistenFailed = null
})
</script>

<template>
  <div class="flex h-screen flex-col bg-background text-foreground">
    <!-- 顶栏 -->
    <header class="flex shrink-0 items-center gap-2 border-b border-border px-4 py-3">
      <div class="flex flex-wrap items-center gap-2">
        <Button variant="default" size="sm" :disabled="isLoading" @click="onCapture">
          截图
        </Button>
        <Button variant="outline" size="sm" :disabled="isLoading" @click="onOpenFile">
          打开文件
        </Button>
        <Button variant="outline" size="sm" :disabled="isLoading" @click="onClipboard">
          从剪贴板
        </Button>
        <Button
          variant="outline"
          size="sm"
          :disabled="!canRerecognize"
          @click="onRerecognize"
        >
          重新识别
        </Button>
      </div>
      <div class="ml-auto min-w-0 truncate text-xs text-muted-foreground" :title="engineSummary">
        <span v-if="isLoading" class="text-primary">识别中…</span>
        <span v-else-if="engineSummary">{{ engineSummary }}</span>
        <span v-else>未选择引擎结果</span>
      </div>
    </header>

    <!-- 错误条 -->
    <div
      v-if="status === 'error' && errorMessage"
      class="shrink-0 border-b border-destructive/30 bg-destructive/10 px-4 py-2 text-sm text-destructive"
      role="alert"
    >
      {{ errorMessage }}
    </div>

    <!-- 主区 -->
    <main class="grid min-h-0 flex-1 grid-cols-1 gap-0 md:grid-cols-2">
      <!-- 左：预览 -->
      <section class="flex min-h-0 flex-col border-b border-border md:border-b-0 md:border-r">
        <div class="shrink-0 px-4 py-2 text-xs font-medium text-muted-foreground">
          图片预览
        </div>
        <div class="flex min-h-0 flex-1 items-center justify-center bg-muted/40 p-4">
          <img
            v-if="hasPreview"
            :src="previewUrl!"
            alt="识别预览"
            class="max-h-full max-w-full object-contain"
          />
          <p v-else class="max-w-xs text-center text-sm text-muted-foreground">
            <template v-if="isLoading">正在识别，请稍候…</template>
            <template v-else>
              暂无预览。请使用「截图」「打开文件」或「从剪贴板」开始识别。
            </template>
          </p>
        </div>
      </section>

      <!-- 右：文本 -->
      <section class="flex min-h-0 flex-col">
        <div class="flex shrink-0 items-center gap-2 px-4 py-2">
          <span class="text-xs font-medium text-muted-foreground">识别文本</span>
          <div class="ml-auto flex items-center gap-2">
            <span v-if="copyHint" class="text-xs text-muted-foreground">{{ copyHint }}</span>
            <Button variant="secondary" size="sm" :disabled="!hasText" @click="onCopy">
              复制
            </Button>
          </div>
        </div>
        <div class="min-h-0 flex-1 px-4 pb-4">
          <textarea
            class="h-full min-h-[12rem] w-full resize-none rounded-md border border-input bg-card px-3 py-2 font-mono text-sm leading-relaxed text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring"
            readonly
            :value="text"
            :placeholder="isLoading ? '识别中…' : '识别结果将显示在这里'"
            spellcheck="false"
          />
        </div>
      </section>
    </main>

    <!-- 底栏 meta -->
    <footer class="shrink-0 border-t border-border bg-muted/30 px-4 py-2">
      <dl
        v-if="meta"
        class="flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground"
      >
        <div class="flex gap-1">
          <dt>原图</dt>
          <dd class="font-medium text-foreground">{{ meta.sourceWidth }}×{{ meta.sourceHeight }}</dd>
        </div>
        <div class="flex gap-1">
          <dt>送模</dt>
          <dd class="font-medium text-foreground">
            {{ meta.sentWidth }}×{{ meta.sentHeight }}
            <span v-if="meta.scaled">(已缩放)</span>
          </dd>
        </div>
        <div class="flex gap-1">
          <dt>PNG</dt>
          <dd class="font-medium text-foreground">{{ formatBytes(meta.pngBytes) }}</dd>
        </div>
        <div class="flex gap-1">
          <dt>耗时</dt>
          <dd class="font-medium text-foreground">{{ meta.latencyMs }} ms</dd>
        </div>
        <div class="flex gap-1">
          <dt>HTTP</dt>
          <dd class="font-medium text-foreground">{{ meta.httpStatus ?? '—' }}</dd>
        </div>
        <div class="flex gap-1">
          <dt>引擎</dt>
          <dd class="font-medium text-foreground">{{ meta.engine }}</dd>
        </div>
        <div class="flex gap-1">
          <dt>模型</dt>
          <dd class="font-medium text-foreground">{{ meta.model || '—' }}</dd>
        </div>
      </dl>
      <p v-else class="text-xs text-muted-foreground">
        元信息将在识别成功后显示（原图尺寸、送模尺寸、耗时等）
      </p>
    </footer>
  </div>
</template>

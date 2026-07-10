<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { Toaster } from '@/components/ui/toast'
import { createLogger } from '@public/logger.js'
import PopupToolbar from './components/PopupToolbar.vue'
import SourceCard from './components/SourceCard.vue'
import LanguageToolbar from './components/LanguageToolbar.vue'
import ResultCard from './components/ResultCard.vue'
import StatusBar from './components/StatusBar.vue'
import { useTranslationEvents, type CardState, type TranslationEventPayload } from './composables/useTranslationEvents'
import { usePopupHeight } from './composables/usePopupHeight'
import {
  createMainWindowReadyGate,
  doubleRaf,
} from './composables/mainWindowReady'
import { getTauriApis } from './composables/utils'
import { toast } from '@/lib/toast'
import { matchShortcutKeys } from '@/lib/matchShortcut'
import { LANGUAGES } from './data/languages'
import type { AppConfig } from '@/types/config'

const logger = createLogger('translate')

/* === 顶层状态（spec 6.1） === */
const popupRef = ref<HTMLElement | null>(null)
const sourceText = ref('')
const sessionSourceLang = ref('auto')
const sessionTargetLang = ref('zh-CN')
const isTranslating = ref(false)
const currentBatchId = ref<string | null>(null)
const cards = reactive<Map<string, CardState>>(new Map())
const pinned = ref(false)
const sourceBadge = ref<'selectedText' | 'ocrText' | null>(null)
const detectedLangBadge = ref('')
const charCount = ref(0)
const statusInfo = ref<{ text: string; loading: boolean; action: { label: string; onClick: () => void } | null }>({
  text: '就绪', loading: false, action: null,
})
const pendingConfigRefresh = ref<AppConfig | null>(null)
/** 程序快捷键「打开设置」；默认 Ctrl+,，随 app-config 同步。 */
const openSettingsKeys = ref('Ctrl+,')

const popupHeight = usePopupHeight(popupRef)

const showMainWindow = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) return
  const win = apis.getCurrentWindow()
  await win.show()
  await win.setFocus()
}

const readyGate = createMainWindowReadyGate({
  timeoutMs: 2000,
  show: showMainWindow,
  onTimeoutWarn: (msg) => logger.warn(msg),
})

const setStatus = (text: string, loading: boolean, action: { label: string; onClick: () => void } | null): void => {
  statusInfo.value = { text, loading, action }
}

/* === 引擎/语言标签 === */
const sourceLangLabel = computed(() => LANGUAGES.find((l) => l.value === sessionSourceLang.value)?.label ?? '自动检测')
const detectedOrLabel = computed(() => {
  if (detectedLangBadge.value) return detectedLangBadge.value
  if (sessionSourceLang.value === 'auto') return '检测中…'
  return sourceLangLabel.value
})

/* === batchStatus（复刻旧 updateBatchStatus） === */
const updateBatchStatus = (): void => {
  const list = Array.from(cards.values())
  if (list.length === 0) return
  const allFinished = list.every((c) => c.status === 'finished')
  const allFailed = list.every((c) => c.status === 'failed' || c.status === 'cancelled')
  const anyTranslating = list.some((c) => c.status === 'translating')
  if (allFinished) {
    isTranslating.value = false
    currentBatchId.value = null
    sourceBadge.value = null
    if (sessionSourceLang.value === 'auto') {
      const detected = list.find((c) => c.detectedSourceLang)?.detectedSourceLang ?? ''
      detectedLangBadge.value = detected
    }
    setStatus('翻译完成', false, { label: '重试', onClick: retryTranslation })
    applyPendingConfigRefresh()
  } else if (allFailed) {
    isTranslating.value = false
    currentBatchId.value = null
    detectedLangBadge.value = ''
    setStatus('翻译失败', false, { label: '重试', onClick: retryTranslation })
    applyPendingConfigRefresh()
  } else if (anyTranslating) {
    setStatus('翻译中…', true, { label: '取消', onClick: cancelTranslation })
  } else {
    isTranslating.value = false
    currentBatchId.value = null
    sourceBadge.value = null
    detectedLangBadge.value = ''
    setStatus('部分完成', false, { label: '重试', onClick: retryTranslation })
    applyPendingConfigRefresh()
  }
}

/* === 事件分派 === */
const onStarted = (payload: TranslationEventPayload, isNewBatch: boolean): void => {
  if (isNewBatch) {
    if (payload.sourceText !== undefined) sourceText.value = payload.sourceText
    charCount.value = sourceText.value.length
    sourceBadge.value = payload.sourceType ?? null
    detectedLangBadge.value = sessionSourceLang.value === 'auto' ? '检测中…' : ''
    setStatus('翻译中…', true, { label: '取消', onClick: cancelTranslation })
  }
}
const onDetectedLang = (lang: string | null): void => {
  if (sessionSourceLang.value === 'auto' && lang) detectedLangBadge.value = lang
}

const events = useTranslationEvents({
  cards,
  getIsTranslating: () => isTranslating.value,
  setIsTranslating: (v) => { isTranslating.value = v },
  getCurrentBatchId: () => currentBatchId.value,
  setCurrentBatchId: (id) => { currentBatchId.value = id },
  onStarted,
  onBatchStatusChange: updateBatchStatus,
  onDetectedLang,
  onConfigChanged: (cfg) => {
    if (cfg.logLevel) logger.setLevel(cfg.logLevel)
    if (cfg.shortcuts?.['open-settings'] !== undefined) {
      openSettingsKeys.value = cfg.shortcuts['open-settings']
    }
    refreshCardsFromConfig(cfg)
  },
  logger,
})

const onAppShortcutKeydown = (e: KeyboardEvent): void => {
  if (!matchShortcutKeys(openSettingsKeys.value, e)) return
  e.preventDefault()
  const apis = getTauriApis()
  if (!apis) return
  void apis.invoke('open_settings').catch((err: unknown) => {
    toast.error('打开设置失败', String(err))
  })
}

onBeforeUnmount(() => {
  events.unlisten()
  readyGate.dispose()
  window.removeEventListener('keydown', onAppShortcutKeydown)
})

/* === 卡片配置同步（复刻旧 refreshCardsFromConfig + syncServiceCards） === */
const enabledPayloads = (config: AppConfig): Array<{ serviceInstanceId: string; serviceType: string; serviceName: string }> =>
  (config.services || [])
    .filter((s) => s.enabled)
    .map((s) => ({ serviceInstanceId: s.id, serviceType: s.serviceType, serviceName: s.name }))

const refreshCardsFromConfig = (config: AppConfig): void => {
  const payloads = enabledPayloads(config)
  const enabledIds = new Set(payloads.map((p) => p.serviceInstanceId))
  if (isTranslating.value) {
    pendingConfigRefresh.value = config
    cards.forEach((card, id) => {
      if (!enabledIds.has(id) && card.status !== 'translating') cards.delete(id)
    })
    payloads.forEach((p) => {
      const card = cards.get(p.serviceInstanceId)
      if (card) { card.serviceName = p.serviceName; card.serviceType = p.serviceType }
    })
    return
  }
  pendingConfigRefresh.value = null
  cards.forEach((card, id) => {
    if (!enabledIds.has(id) && card.status !== 'translating') cards.delete(id)
  })
  payloads.forEach((p) => {
    let card = cards.get(p.serviceInstanceId)
    if (!card) {
      card = {
        serviceInstanceId: p.serviceInstanceId,
        serviceName: p.serviceName,
        serviceType: p.serviceType,
        modelName: '',
        text: '',
        status: 'pending',
        collapsed: true, // 空闲默认收缩
        collapseUserOverride: false,
        expanded: false,
        hasOverflow: false,
        showActions: false,
        usage: null,
        detectedSourceLang: null,
      }
      cards.set(p.serviceInstanceId, card)
    } else {
      card.serviceName = p.serviceName
      card.serviceType = p.serviceType
    }
  })
}

const applyPendingConfigRefresh = (): void => {
  if (!pendingConfigRefresh.value) return
  const cfg = pendingConfigRefresh.value
  pendingConfigRefresh.value = null
  refreshCardsFromConfig(cfg)
}

/* === 翻译触发 === */
const startManualTranslation = async (): Promise<void> => {
  if (isTranslating.value) return
  const text = sourceText.value.trim()
  if (!text) { toast.info('请输入要翻译的文本'); return }
  const apis = getTauriApis()
  if (!apis) { toast.info('Tauri API 未就绪，请在桌面应用中运行'); return }
  try {
    await apis.invoke('start_translation', { text })
  } catch (e) {
    toast.error('翻译失败', String(e))
    logger.error('手动翻译失败', String(e))
  }
}

async function cancelTranslation(): Promise<void> {
  const apis = getTauriApis()
  if (!apis) return
  try {
    await apis.invoke('cancel_translation')
  } catch (e) {
    toast.error('取消失败', String(e))
    logger.warn('取消翻译失败', String(e))
  }
}

async function retryTranslation(): Promise<void> {
  if (isTranslating.value) return
  const apis = getTauriApis()
  if (!apis) { toast.info('Tauri API 未就绪'); return }
  try {
    await apis.invoke('retry_translation')
  } catch (e) {
    toast.error('重试失败', String(e))
    logger.error('重试失败', String(e))
  }
}

/* === 语言选择（复刻旧 selectLang/swapLangs） === */
const onSelectSource = async (code: string): Promise<void> => {
  sessionSourceLang.value = code
  detectedLangBadge.value = code === 'auto' ? '检测中…' : ''
  await persistSessionLanguages()
}
const onSelectTarget = async (code: string): Promise<void> => {
  sessionTargetLang.value = code
  await persistSessionLanguages()
}
const onSwap = async (): Promise<void> => {
  if (sessionSourceLang.value === 'auto' || sessionTargetLang.value === 'auto') {
    toast.info('自动检测不支持交换')
    return
  }
  const tmp = sessionSourceLang.value
  sessionSourceLang.value = sessionTargetLang.value
  sessionTargetLang.value = tmp
  await persistSessionLanguages()
}
const persistSessionLanguages = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) return
  try {
    await apis.invoke('set_session_languages', { sourceLang: sessionSourceLang.value, targetLang: sessionTargetLang.value })
  } catch (e) {
    toast.error('语言设置失败', String(e))
  }
}

/* === 原文输入 === */
const onSourceInput = (): void => {
  charCount.value = sourceText.value.length
  if (!sourceText.value.trim()) {
    cards.forEach((c) => {
      c.collapsed = true
      c.collapseUserOverride = false
    })
  }
}

/* === 待回填原文 + Edge 环境采集（复刻旧 applyPendingSourceText/collectEdgeTranslateEnv） === */
const applyPendingSourceText = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) return
  try {
    const text = await apis.invoke<string>('take_pending_source_text')
    if (text) {
      sourceText.value = text
      charCount.value = text.length
    }
  } catch (e) {
    toast.error('回填原文失败', String(e))
  }
}

const collectEdgeTranslateEnv = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) return
  try {
    const userAgent = navigator.userAgent
    const langs = navigator.languages ?? [navigator.language]
    const acceptLanguage = langs
      .map((l, i) => (i === 0 ? l : `${l};q=${(1 - i * 0.1).toFixed(1)}`))
      .join(',')
    await apis.invoke('save_edge_translate_env', { userAgent, acceptLanguage })
  } catch (e) {
    logger.warn('采集 Edge 翻译环境失败', String(e))
  }
}

/* === 初始化（复刻旧 initCards） === */
const initCards = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) return
  try {
    const [config, langs] = await Promise.all([
      apis.invoke<AppConfig>('get_app_config'),
      apis.invoke<{ sourceLang: string; targetLang: string }>('get_session_languages'),
    ])
    if (config?.logLevel) logger.setLevel(config.logLevel)
    if (config?.shortcuts?.['open-settings'] !== undefined) {
      openSettingsKeys.value = config.shortcuts['open-settings']
    }
    sessionSourceLang.value = langs?.sourceLang ?? 'auto'
    sessionTargetLang.value = langs?.targetLang ?? 'zh-CN'
    refreshCardsFromConfig(config)
  } catch {
    return
  }
}

const runColdStartReady = async (): Promise<void> => {
  try {
    await initCards()
    await nextTick()
    await popupHeight.adjustNow()
    await popupHeight.whenFirstSized
    await doubleRaf()
  } catch (e) {
    logger.warn('冷启动 ready 流水线异常，仍尝试 show', String(e))
  } finally {
    await readyGate.notifyReady()
  }
}

onMounted(() => {
  charCount.value = sourceText.value.length
  void runColdStartReady()
  void collectEdgeTranslateEnv()
  void applyPendingSourceText()
  window.addEventListener('focus', () => {
    void applyPendingSourceText()
  })
  window.addEventListener('keydown', onAppShortcutKeydown)
})
</script>

<template>
  <div id="popup" ref="popupRef" class="popup">
    <PopupToolbar v-model:pinned="pinned" />

    <div class="content">
      <SourceCard
        v-model="sourceText"
        :lang-label="sourceLangLabel"
        :source-badge="sourceBadge"
        :detected-lang="detectedOrLabel"
        @submit="startManualTranslation"
        @input="onSourceInput"
      />

      <LanguageToolbar
        :source="sessionSourceLang"
        :target="sessionTargetLang"
        @update:source="onSelectSource"
        @update:target="onSelectTarget"
        @swap="onSwap"
      />

      <div class="results">
        <ResultCard
          v-for="card in cards.values()"
          :key="card.serviceInstanceId"
          :card="card"
          :target-lang="sessionTargetLang"
        />
      </div>
    </div>

    <StatusBar
      :text="statusInfo.text"
      :loading="statusInfo.loading"
      :action="statusInfo.action"
      :char-count="charCount"
    />
  </div>
  <Toaster />
</template>

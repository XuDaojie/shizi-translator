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
import { applyPendingSourceIfCurrent, getTauriApis } from './composables/utils'
import { POPUP_MESSAGE_KEYS } from './composables/resultCardMeta'
import { toast } from '@/lib/toast'
import { matchShortcutKeys } from '@/lib/matchShortcut'
import { translationLanguage } from '@/shared/translation-languages'
import type { AppConfig } from '@/types/config'
import { locale, reloadCurrentLocale, t, type MessageKey, type MessageParams } from '@/i18n'

const logger = createLogger('translate')
let disposed = false
let unlistenLanguageChanged: (() => void) | null = null
let sourceRevision = 0

const applyDocumentLanguageAndTitle = async (): Promise<void> => {
  document.documentElement.lang = locale.value
  const apis = getTauriApis()
  if (!apis) return
  try {
    await (apis.getCurrentWindow() as ReturnType<typeof apis.getCurrentWindow> & {
      setTitle: (title: string) => Promise<void>
    }).setTitle(t('window.popupTitle'))
  } catch (error) {
    logger.warn('更新翻译窗口标题失败', String(error))
  }
}

const reloadAndApplyLanguage = async (): Promise<void> => {
  try {
    await reloadCurrentLocale()
    if (!disposed) await applyDocumentLanguageAndTitle()
  } catch (error) {
    logger.warn('刷新界面语言失败', String(error))
  }
}

const setupLanguageSync = async (): Promise<void> => {
  const apis = getTauriApis()
  if (!apis) {
    await applyDocumentLanguageAndTitle()
    return
  }
  try {
    const unlisten = await apis.listen('interface-language:changed', () => {
      void reloadAndApplyLanguage()
    })
    if (disposed) {
      unlisten()
      return
    }
    unlistenLanguageChanged = unlisten
    await reloadAndApplyLanguage()
  } catch (error) {
    logger.warn('监听界面语言变更失败', String(error))
  }
}

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
type StatusAction = { key: MessageKey; params?: MessageParams; onClick: () => void }
const statusInfo = ref<{ key: MessageKey; params: MessageParams; loading: boolean; action: StatusAction | null }>({
  key: POPUP_MESSAGE_KEYS.ready, params: {}, loading: false, action: null,
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
  onTimeoutWarn: (key, params) => logger.warn(t(key, params)),
})

const setStatus = (key: MessageKey, loading: boolean, action: StatusAction | null, params: MessageParams = {}): void => {
  statusInfo.value = { key, params, loading, action }
}

/* === 引擎/语言标签 === */
const sourceLangLabel = computed(() => translationLanguage(sessionSourceLang.value)?.nativeName ?? sessionSourceLang.value)
const languageLabel = (code: string): string => {
  const lang = translationLanguage(code)
  if (!lang) return code
  const translated = t(lang.nameKey as MessageKey)
  return translated === lang.nativeName ? lang.nativeName : `${lang.nativeName} (${translated})`
}
const detectedOrLabel = computed(() => {
  if (detectedLangBadge.value) return languageLabel(detectedLangBadge.value)
  if (sessionSourceLang.value === 'auto') return t(POPUP_MESSAGE_KEYS.detecting)
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
    setStatus('popup.status.completed', false, { key: POPUP_MESSAGE_KEYS.retry, onClick: retryTranslation })
    applyPendingConfigRefresh()
  } else if (allFailed) {
    isTranslating.value = false
    currentBatchId.value = null
    detectedLangBadge.value = ''
    setStatus('popup.status.failed', false, { key: POPUP_MESSAGE_KEYS.retry, onClick: retryTranslation })
    applyPendingConfigRefresh()
  } else if (anyTranslating) {
    setStatus(POPUP_MESSAGE_KEYS.translating, true, { key: POPUP_MESSAGE_KEYS.cancel, onClick: cancelTranslation })
  } else {
    isTranslating.value = false
    currentBatchId.value = null
    sourceBadge.value = null
    detectedLangBadge.value = ''
    setStatus('popup.status.partial', false, { key: POPUP_MESSAGE_KEYS.retry, onClick: retryTranslation })
    applyPendingConfigRefresh()
  }
}

/* === 事件分派 === */
const onStarted = (payload: TranslationEventPayload, isNewBatch: boolean): void => {
  if (isNewBatch) {
    sourceRevision += 1
    if (payload.sourceText !== undefined) sourceText.value = payload.sourceText
    charCount.value = sourceText.value.length
    sourceBadge.value = payload.sourceType ?? null
    detectedLangBadge.value = ''
    setStatus(POPUP_MESSAGE_KEYS.translating, true, { key: POPUP_MESSAGE_KEYS.cancel, onClick: cancelTranslation })
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
    toast.error(t('popup.error.openSettings'), String(err))
  })
}

onBeforeUnmount(() => {
  disposed = true
  unlistenLanguageChanged?.()
  events.unlisten()
  readyGate.dispose()
  window.removeEventListener('keydown', onAppShortcutKeydown)
})

/* === 卡片配置同步（复刻旧 refreshCardsFromConfig + syncServiceCards） === */
const enabledPayloads = (config: AppConfig): Array<{ serviceInstanceId: string; serviceType: string; serviceName: string; protocol: string; modelName: string }> =>
  (config.services || [])
    .filter((s) => s.enabled)
    .map((s) => ({
      serviceInstanceId: s.id,
      serviceType: s.serviceType,
      serviceName: s.name,
      protocol: s.protocol || '',
      modelName: s.protocol === 'microsoft_edge' ? '' : (s.model || ''),
    }))

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
      if (card) {
        card.serviceName = p.serviceName
        card.serviceType = p.serviceType
        card.protocol = p.protocol
        card.modelName = p.modelName
      }
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
        protocol: p.protocol,
        modelName: p.modelName,
        text: '',
        status: 'pending',
        collapsed: true, // 空闲默认收缩
        collapseUserOverride: false,
        expanded: false,
        hasOverflow: false,
        showActions: false,
        usage: null,
        detectedSourceLang: null,
        errorTitleKey: null,
        errorMessage: '',
      }
      cards.set(p.serviceInstanceId, card)
    } else {
      card.serviceName = p.serviceName
      card.serviceType = p.serviceType
      card.protocol = p.protocol
      card.modelName = p.modelName
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
  if (!text) { toast.info(t(POPUP_MESSAGE_KEYS.emptySource)); return }
  const apis = getTauriApis()
  if (!apis) { toast.info(t('popup.error.tauriUnavailable')); return }
  try {
    await apis.invoke('start_translation', { text })
  } catch (e) {
    toast.error(t(POPUP_MESSAGE_KEYS.translationFailed), String(e))
    logger.error('手动翻译失败', String(e))
  }
}

async function cancelTranslation(): Promise<void> {
  const apis = getTauriApis()
  if (!apis) return
  try {
    await apis.invoke('cancel_translation')
  } catch (e) {
    toast.error(t('popup.error.cancelFailed'), String(e))
    logger.warn('取消翻译失败', String(e))
  }
}

async function retryTranslation(): Promise<void> {
  if (isTranslating.value) return
  const apis = getTauriApis()
  if (!apis) { toast.info(t('popup.error.tauriUnavailable')); return }
  try {
    await apis.invoke('retry_translation')
  } catch (e) {
    toast.error(t('popup.error.retryFailed'), String(e))
    logger.error('重试失败', String(e))
  }
}

/* === 语言选择（复刻旧 selectLang/swapLangs） === */
const onSelectSource = async (code: string): Promise<void> => {
  sessionSourceLang.value = code
  detectedLangBadge.value = ''
  await persistSessionLanguages()
}
const onSelectTarget = async (code: string): Promise<void> => {
  sessionTargetLang.value = code
  await persistSessionLanguages()
}
const onSwap = async (): Promise<void> => {
  if (sessionSourceLang.value === 'auto' || sessionTargetLang.value === 'auto') {
    toast.info(t('popup.error.swapAuto'))
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
    toast.error(t('popup.error.languageSaveFailed'), String(e))
  }
}

/* === 原文输入 === */
const onSourceInput = (): void => {
  sourceRevision += 1
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
    await applyPendingSourceIfCurrent(
      () => apis.invoke<string | null>('take_pending_source_text'),
      () => sourceRevision,
      (text) => {
        sourceText.value = text
        charCount.value = text.length
      },
    )
  } catch (e) {
    toast.error(t('popup.error.pendingSourceFailed'), String(e))
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
  let silentAutostart = false
  try {
    const apis = getTauriApis()
    if (apis) {
      silentAutostart = Boolean(await apis.invoke<boolean>('is_autostart_launch'))
    }
    await initCards()
    await nextTick()
    await popupHeight.adjustNow()
    await popupHeight.whenFirstSized
    await doubleRaf()
  } catch (e) {
    logger.warn('冷启动 ready 流水线异常，仍尝试 show', String(e))
  } finally {
    if (silentAutostart) {
      // 开机自启：仅托盘驻留，不强制弹出翻译窗
      readyGate.dispose()
    } else {
      await readyGate.notifyReady()
    }
  }
}

onMounted(() => {
  void setupLanguageSync()
  charCount.value = sourceText.value.length
  void runColdStartReady()
  void collectEdgeTranslateEnv()
  void applyPendingSourceText()
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
      :text="t(statusInfo.key, statusInfo.params)"
      :loading="statusInfo.loading"
      :action="statusInfo.action ? { label: t(statusInfo.action.key, statusInfo.action.params), onClick: statusInfo.action.onClick } : null"
      :char-count="charCount"
    />
  </div>
  <Toaster />
</template>

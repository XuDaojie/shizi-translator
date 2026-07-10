import type { AppConfig } from '@/types/config'
import { batchIdFromSession } from './utils'

export type CardStatus = 'pending' | 'translating' | 'finished' | 'failed' | 'cancelled'

export interface CardState {
  serviceInstanceId: string
  serviceName: string
  serviceType: string
  modelName: string
  text: string
  status: CardStatus
  collapsed: boolean
  expanded: boolean
  hasOverflow: boolean
  showActions: boolean
  usage: { inputTokens: number; outputTokens: number } | null
  detectedSourceLang: string | null
}

export interface TranslationEventPayload {
  type: 'started' | 'delta' | 'finished' | 'failed' | 'cancelled'
  sessionId?: string
  serviceInstanceId?: string
  serviceName?: string
  serviceType?: string
  modelName?: string
  sourceText?: string
  sourceType?: 'selectedText' | 'ocrText' | null
  text?: string
  fullText?: string
  message?: string
  detectedSourceLang?: string | null
  usage?: { inputTokens: number; outputTokens: number } | null
}

export interface UseTranslationEventsOptions {
  cards: Map<string, CardState>
  getIsTranslating: () => boolean
  setIsTranslating: (v: boolean) => void
  getCurrentBatchId: () => string | null
  setCurrentBatchId: (id: string | null) => void
  /** started 事件（含 isNewBatch 标志）-- 由父组件回填 sourceText/sourceBadge/langBadge/状态栏。 */
  onStarted: (payload: TranslationEventPayload, isNewBatch: boolean) => void
  /** finished/failed/cancelled 后调用--由父组件更新状态栏（updateBatchStatus）。 */
  onBatchStatusChange: () => void
  /** source=auto 且收到 detectedSourceLang 时上抛（更新 .lang-badge）。 */
  onDetectedLang: (lang: string | null) => void
  /** app-config:changed 事件--由父组件 refreshCardsFromConfig（含翻译中延迟逻辑）。 */
  onConfigChanged: (config: AppConfig) => void
  logger: { info: (msg: string, meta?: unknown) => void; warn: (msg: string, meta?: unknown) => void }
}

function ensureCard(cards: Map<string, CardState>, payload: TranslationEventPayload): CardState {
  const id = payload.serviceInstanceId ?? 'default'
  let card = cards.get(id)
  if (!card) {
    card = {
      serviceInstanceId: id,
      serviceName: payload.serviceName ?? '翻译',
      serviceType: payload.serviceType ?? '',
      modelName: payload.modelName ?? '',
      text: '',
      status: 'pending',
      collapsed: false,
      expanded: false,
      hasOverflow: false,
      showActions: false,
      usage: null,
      detectedSourceLang: null,
    }
    cards.set(id, card)
  }
  return card
}

function resetCardForNewBatch(card: CardState): void {
  card.status = 'pending'
  card.text = ''
  card.showActions = false
  card.usage = null
  card.expanded = false
  card.hasOverflow = false
  card.detectedSourceLang = null
}

export interface UseTranslationEventsReturn {
  /** 直接分派一个 translation:event payload（供测试与真实 listen 共用）。 */
  dispatch: (payload: TranslationEventPayload) => void
  /** 注销监听。 */
  unlisten: () => void
}

export function useTranslationEvents(opts: UseTranslationEventsOptions): UseTranslationEventsReturn {
  const dispatch = (payload: TranslationEventPayload): void => {
    switch (payload.type) {
      case 'started': {
        const batchId = batchIdFromSession(payload.sessionId)
        const isNewBatch = batchId !== opts.getCurrentBatchId()
        if (isNewBatch) {
          opts.logger.info('翻译开始', { batch: batchId })
          opts.setCurrentBatchId(batchId)
          opts.cards.forEach(resetCardForNewBatch)
          opts.setIsTranslating(true)
        }
        opts.onStarted(payload, isNewBatch)
        const card = ensureCard(opts.cards, payload)
        card.serviceName = payload.serviceName ?? card.serviceName
        card.serviceType = payload.serviceType ?? card.serviceType
        card.modelName = payload.modelName ?? card.modelName
        card.status = 'translating'
        card.text = ''
        card.showActions = false
        card.usage = null
        card.expanded = false
        card.hasOverflow = false
        card.detectedSourceLang = null
        card.collapsed = false
        break
      }
      case 'delta': {
        if (batchIdFromSession(payload.sessionId) !== opts.getCurrentBatchId()) return
        const card = opts.cards.get(payload.serviceInstanceId ?? 'default')
        if (!card) return
        card.text += payload.text ?? ''
        break
      }
      case 'finished': {
        if (batchIdFromSession(payload.sessionId) !== opts.getCurrentBatchId()) return
        const card = opts.cards.get(payload.serviceInstanceId ?? 'default')
        if (!card) return
        card.text = payload.fullText ?? card.text
        card.status = 'finished'
        card.usage = payload.usage ?? null
        card.showActions = true
        card.detectedSourceLang = payload.detectedSourceLang ?? null
        if (payload.detectedSourceLang) opts.onDetectedLang(payload.detectedSourceLang)
        opts.onBatchStatusChange()
        break
      }
      case 'failed': {
        if (batchIdFromSession(payload.sessionId) !== opts.getCurrentBatchId()) return
        opts.logger.warn('翻译失败', { session: payload.sessionId, message: payload.message })
        const card = opts.cards.get(payload.serviceInstanceId ?? 'default')
        if (!card) return
        card.text = payload.message ?? '翻译失败'
        card.status = 'failed'
        card.showActions = false
        card.usage = null
        opts.onBatchStatusChange()
        break
      }
      case 'cancelled': {
        if (batchIdFromSession(payload.sessionId) !== opts.getCurrentBatchId()) return
        const card = opts.cards.get(payload.serviceInstanceId ?? 'default')
        if (!card) return
        card.text += '\n[已取消]'
        card.status = 'cancelled'
        opts.onBatchStatusChange()
        break
      }
      default:
        break
    }
  }

  // 监听 Tauri 事件；window.__TAURI__ 可能未就绪（单测/纯浏览器），降级跳过。
  const t = (typeof window !== 'undefined' ? (window as { __TAURI__?: { event?: { listen?: (e: string, h: (ev: { payload: TranslationEventPayload }) => void) => Promise<() => void> } } }).__TAURI__ : undefined)
  const listenFn = t?.event?.listen
  let unlistenTranslation: (() => void) | null = null
  let unlistenConfig: (() => void) | null = null
  if (listenFn) {
    listenFn('translation:event', (ev) => dispatch(ev.payload)).then((fn) => { unlistenTranslation = fn })
    listenFn('app-config:changed', (ev) => {
      const cfg = ev.payload as unknown as AppConfig
      opts.onConfigChanged(cfg)
    }).then((fn) => { unlistenConfig = fn })
  }

  return {
    dispatch,
    unlisten: () => {
      unlistenTranslation?.()
      unlistenConfig?.()
    },
  }
}

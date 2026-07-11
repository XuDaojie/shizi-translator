/**
 * 结果卡右下角元信息展示规则：
 * - 机器翻译（microsoft_edge）无模型概念，也不展示 Token
 * - LLM 才显示 modelName / usage
 */

export function isMachineTranslateProtocol(protocol: string | undefined | null): boolean {
  return (protocol ?? '').trim() === 'microsoft_edge'
}

/** 结果卡展示用模型名；MT 或空占位返回空串（不渲染标签） */
export function displayModelName(
  protocol: string | undefined | null,
  modelName: string | undefined | null,
): string {
  if (isMachineTranslateProtocol(protocol)) return ''
  const name = (modelName ?? '').trim()
  if (!name || name === '—' || name === '-') return ''
  return name
}

/** 是否展示输入/输出 Token：MT 永不展示；其余需有 usage 数据 */
export function shouldShowTokens(
  protocol: string | undefined | null,
  hasUsage: boolean,
): boolean {
  if (isMachineTranslateProtocol(protocol)) return false
  return hasUsage
}
import type { MessageKey, MessageParams } from '@/i18n'

type ResultStatus = 'pending' | 'translating' | 'finished' | 'failed' | 'cancelled'
export type ResultViewStatus = 'success' | 'loading' | 'error' | 'aborted' | 'pending'

export function showResultActions(showActions: boolean, showRefresh: boolean, status: ResultViewStatus): boolean {
  return showActions || (showRefresh && (status === 'error' || status === 'aborted'))
}

export const POPUP_MESSAGE_KEYS = {
  ready: 'popup.status.ready',
  detecting: 'popup.status.detecting',
  translating: 'popup.status.translating',
  emptySource: 'popup.error.emptySource',
  retry: 'popup.button.retry',
  cancel: 'popup.button.cancel',
  copySuccess: 'popup.toast.copySuccess',
  translationFailed: 'popup.error.translationFailed',
  cancelled: 'popup.status.cancelled',
} as const satisfies Record<string, MessageKey>

export function resultStatusMeta(status: ResultStatus): { key: MessageKey; params: MessageParams } | null {
  if (status === 'failed') return { key: POPUP_MESSAGE_KEYS.translationFailed, params: {} }
  if (status === 'cancelled') return { key: POPUP_MESSAGE_KEYS.cancelled, params: {} }
  if (status === 'translating') return { key: POPUP_MESSAGE_KEYS.translating, params: {} }
  return null
}

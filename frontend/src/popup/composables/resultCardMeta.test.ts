import { describe, expect, it } from 'vitest'
import type { InterfaceLanguageSnapshot } from '@/lib/tauri'
import { createI18nForTest } from '@/i18n'
import zhCN from '@/i18n/locales/zh-CN.json'
import enUS from '@/i18n/locales/en-US.json'
import { displayModelName, isMachineTranslateProtocol, POPUP_MESSAGE_KEYS, resultStatusMeta, shouldShowTokens } from './resultCardMeta'

const snapshot = (locale: string, revision: number): InterfaceLanguageSnapshot => ({
  configuredLocale: locale,
  locale,
  revision,
  languages: [],
  userMessages: {},
  errors: [],
})

describe('resultCardMeta', () => {
  it('识别微软 Edge 机器翻译协议', () => {
    expect(isMachineTranslateProtocol('microsoft_edge')).toBe(true)
    expect(isMachineTranslateProtocol('openai_chat')).toBe(false)
    expect(isMachineTranslateProtocol('')).toBe(false)
  })

  it('MT 不展示模型名，即使配置残留 gpt-4o-mini', () => {
    expect(displayModelName('microsoft_edge', 'gpt-4o-mini')).toBe('')
    expect(displayModelName('microsoft_edge', '')).toBe('')
  })

  it('LLM 展示真实模型名，过滤占位符', () => {
    expect(displayModelName('openai_chat', 'deepseek-chat')).toBe('deepseek-chat')
    expect(displayModelName('claude_messages', '—')).toBe('')
    expect(displayModelName('openai_chat', '  ')).toBe('')
  })

  it('MT 永不展示 Token；LLM 仅在有 usage 时展示', () => {
    expect(shouldShowTokens('microsoft_edge', true)).toBe(false)
    expect(shouldShowTokens('microsoft_edge', false)).toBe(false)
    expect(shouldShowTokens('openai_chat', true)).toBe(true)
    expect(shouldShowTokens('openai_chat', false)).toBe(false)
  })

  it('状态元数据返回消息 key 而非可见中文', () => {
    expect(resultStatusMeta('failed')).toEqual({ key: 'popup.error.translationFailed', params: {} })
    expect(resultStatusMeta('cancelled')).toEqual({ key: 'popup.status.cancelled', params: {} })
    expect(resultStatusMeta('translating')).toEqual({ key: 'popup.status.translating', params: {} })
    expect(resultStatusMeta('finished')).toBeNull()
  })

  it('集中声明弹窗状态、动作和 toast 的消息 key 契约', () => {
    expect(POPUP_MESSAGE_KEYS).toEqual({
      ready: 'popup.status.ready',
      detecting: 'popup.status.detecting',
      translating: 'popup.status.translating',
      emptySource: 'popup.error.emptySource',
      retry: 'popup.button.retry',
      cancel: 'popup.button.cancel',
      copySuccess: 'popup.toast.copySuccess',
      translationFailed: 'popup.error.translationFailed',
      cancelled: 'popup.status.cancelled',
    })
  })

  it('同一消息 key 随运行时 locale 切换重新渲染', async () => {
    const i18n = createI18nForTest(async (locale) => locale === 'en-US' ? enUS : zhCN)
    await i18n.applySnapshot(snapshot('zh-CN', 1))
    expect([
      i18n.t(POPUP_MESSAGE_KEYS.ready),
      i18n.t(POPUP_MESSAGE_KEYS.retry),
      i18n.t(POPUP_MESSAGE_KEYS.copySuccess),
    ]).toEqual(['就绪', '重试', '已复制到剪贴板'])

    await i18n.applySnapshot(snapshot('en-US', 2))
    expect([
      i18n.t(POPUP_MESSAGE_KEYS.ready),
      i18n.t(POPUP_MESSAGE_KEYS.retry),
      i18n.t(POPUP_MESSAGE_KEYS.copySuccess),
    ]).toEqual(['Ready', 'Retry', 'Copied to clipboard'])
  })
})

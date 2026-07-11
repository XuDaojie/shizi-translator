import { describe, expect, it } from 'vitest'
import { displayModelName, isMachineTranslateProtocol, resultStatusMeta, shouldShowTokens } from './resultCardMeta'

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
})

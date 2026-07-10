import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  clearHistoryAndReload,
  isEmptyHistory,
  loadHistory,
  resultCardStatus,
  type HistorySession,
} from './history'
import { invokeClearTranslationHistory, invokeListTranslationHistory } from '@/lib/tauri'

vi.mock('@/lib/tauri', () => ({
  invokeListTranslationHistory: vi.fn(),
  invokeClearTranslationHistory: vi.fn(),
}))

const session: HistorySession = {
  id: 'batch-1',
  timestamp: '2026-07-11T00:00:00Z',
  trigger: 'manual',
  sourceLang: 'auto',
  targetLang: 'zh-CN',
  source: 'hello',
  results: [
    {
      serviceInstanceId: 'svc-a',
      serviceName: 'DeepSeek',
      serviceType: 'deepseek',
      protocol: 'openai_chat',
      modelName: 'deepseek-chat',
      translation: '你好',
      errorMessage: '',
      status: 'success',
      inputTokens: 1,
      outputTokens: 2,
    },
  ],
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe('history data helpers', () => {
  it('空数组被识别为空状态', () => {
    expect(isEmptyHistory([])).toBe(true)
  })

  it('loadHistory 读取后端 session', async () => {
    vi.mocked(invokeListTranslationHistory).mockResolvedValue([session])

    await expect(loadHistory()).resolves.toEqual([session])
    expect(invokeListTranslationHistory).toHaveBeenCalledWith(undefined)
  })

  it('clearHistoryAndReload 先清空再刷新', async () => {
    vi.mocked(invokeListTranslationHistory).mockResolvedValue([session])

    await expect(clearHistoryAndReload()).resolves.toEqual([session])

    expect(invokeClearTranslationHistory).toHaveBeenCalledTimes(1)
    expect(invokeListTranslationHistory).toHaveBeenCalledTimes(1)
  })

  it('结果状态映射到 ResultCardView 状态', () => {
    expect(resultCardStatus({ ...session.results[0], status: 'success' })).toBe('success')
    expect(resultCardStatus({ ...session.results[0], status: 'pending' })).toBe('pending')
    expect(resultCardStatus({ ...session.results[0], status: 'error' })).toBe('error')
    expect(resultCardStatus({ ...session.results[0], status: 'cancelled' })).toBe('aborted')
  })
})

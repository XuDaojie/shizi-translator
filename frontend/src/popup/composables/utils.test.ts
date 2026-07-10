import { describe, expect, it, vi, beforeEach } from 'vitest'
import { batchIdFromSession, copyText } from './utils'

describe('batchIdFromSession', () => {
  it('从 batchId:serviceId 形式的 sessionId 提取 batchId', () => {
    expect(batchIdFromSession('batch-001:svc-a')).toBe('batch-001')
  })

  it('无冒号的 sessionId 返回 null', () => {
    expect(batchIdFromSession('no-colon')).toBeNull()
  })

  it('非字符串输入返回 null', () => {
    expect(batchIdFromSession(undefined)).toBeNull()
    expect(batchIdFromSession(null)).toBeNull()
    expect(batchIdFromSession(123 as unknown as string)).toBeNull()
  })
})

describe('copyText', () => {
  beforeEach(() => {
    vi.stubGlobal('navigator', {
      clipboard: { writeText: vi.fn(() => Promise.resolve()) },
    })
  })

  it('复制成功返回 true', async () => {
    const ok = await copyText('hello')
    expect(ok).toBe(true)
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith('hello')
  })

  it('clipboard 不可用时返回 false', async () => {
    vi.stubGlobal('navigator', {})
    const ok = await copyText('hello')
    expect(ok).toBe(false)
  })

  it('writeText 抛错时返回 false', async () => {
    vi.stubGlobal('navigator', {
      clipboard: { writeText: vi.fn(() => Promise.reject(new Error('denied'))) },
    })
    const ok = await copyText('hello')
    expect(ok).toBe(false)
  })
})

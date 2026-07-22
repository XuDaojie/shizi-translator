import { describe, expect, it, vi, beforeEach } from 'vitest'
import { applyPendingSourceIfCurrent, batchIdFromSession, copyText } from './utils'

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

describe('applyPendingSourceIfCurrent', () => {
  it('原文版本变化后忽略迟到的 pending 结果', async () => {
    let resolvePending!: (text: string) => void
    const pending = new Promise<string>((resolve) => { resolvePending = resolve })
    let revision = 0
    let sourceText = '新原文'

    const request = applyPendingSourceIfCurrent(
      () => pending,
      () => revision,
      (text) => { sourceText = text },
    )
    revision += 1
    resolvePending('旧原文')
    const applied = await request

    expect(sourceText).toBe('新原文')
    expect(applied).toBeNull()
  })

  it('revision 未变时 apply 并返回原文', async () => {
    let sourceText = ''
    const applied = await applyPendingSourceIfCurrent(
      async () => 'hello',
      () => 0,
      (text) => { sourceText = text },
    )
    expect(sourceText).toBe('hello')
    expect(applied).toBe('hello')
  })
})

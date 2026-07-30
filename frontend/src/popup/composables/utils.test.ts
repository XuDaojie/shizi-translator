import { describe, expect, it, vi, beforeEach } from 'vitest'
import {
  applyPendingSourceIfCurrent,
  applyRemoveBlankIfActive,
  batchIdFromSession,
  copyText,
  prepareSourceWithRemoveBlank,
  removeBlankLines,
} from './utils'

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

describe('removeBlankLines', () => {
  it('去掉纯空行与仅空白行，保留非空行', () => {
    expect(removeBlankLines('第一行\n\n第三行\n   \n第六行')).toBe('第一行\n第三行\n第六行')
  })

  it('无空行时原样返回', () => {
    expect(removeBlankLines('a\nb\nc')).toBe('a\nb\nc')
  })

  it('空字符串与全空白返回空串', () => {
    expect(removeBlankLines('')).toBe('')
    expect(removeBlankLines('\n  \n\t\n')).toBe('')
  })

  it('保留行内前后空格', () => {
    expect(removeBlankLines('  a  \n\n  b  ')).toBe('  a  \n  b  ')
  })

  it('规范化 CRLF / 单独 CR（OCR 与 Windows 常见）', () => {
    expect(removeBlankLines('第一行\r\n\r\n第三行\r\n\r\n第六行')).toBe('第一行\n第三行\n第六行')
    expect(removeBlankLines('a\r\rb\r')).toBe('a\nb')
  })
})

describe('applyRemoveBlankIfActive', () => {
  it('关闭时原样返回', () => {
    expect(applyRemoveBlankIfActive(false, 'a\n\nb')).toBe('a\n\nb')
  })

  it('开启时去掉空行', () => {
    expect(applyRemoveBlankIfActive(true, 'a\n\nb')).toBe('a\nb')
  })
})

describe('prepareSourceWithRemoveBlank', () => {
  it('关闭时不改动', () => {
    expect(prepareSourceWithRemoveBlank(false, 'a\n\nb')).toEqual({
      text: 'a\n\nb',
      changed: false,
    })
  })

  it('开启且有空行时 changed=true', () => {
    expect(prepareSourceWithRemoveBlank(true, 'a\n\nb')).toEqual({
      text: 'a\nb',
      changed: true,
    })
  })

  it('开启但无空行时 changed=false', () => {
    expect(prepareSourceWithRemoveBlank(true, 'a\nb')).toEqual({
      text: 'a\nb',
      changed: false,
    })
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

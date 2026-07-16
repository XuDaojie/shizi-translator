import { describe, expect, it } from 'vitest'
import {
  pickDefaultOcrServiceId,
  buildOcrChannelOptions,
  reconcileSelectedOcrServiceId,
} from './sessionChannel'
import type { OcrServiceInstanceConfig } from '@/types/config'

function svc(
  id: string,
  enabled: boolean,
  name = id,
  serviceType = 'windows-media-ocr',
  model = '',
): OcrServiceInstanceConfig {
  return {
    id,
    serviceType,
    name,
    enabled,
    apiKey: null,
    endpoint: '',
    model,
    preferredLang: '',
    ocrPrompt: '',
  }
}

describe('pickDefaultOcrServiceId', () => {
  it('优先 enabled', () => {
    expect(
      pickDefaultOcrServiceId([svc('a', false), svc('b', true), svc('c', false)]),
    ).toBe('b')
  })

  it('无 enabled 取第一项', () => {
    expect(pickDefaultOcrServiceId([svc('a', false), svc('b', false)])).toBe('a')
  })

  it('空列表返回 null', () => {
    expect(pickDefaultOcrServiceId([])).toBeNull()
  })
})

describe('buildOcrChannelOptions', () => {
  it('列出全部实例且含摘要', () => {
    const opts = buildOcrChannelOptions([
      svc('w', true, 'Windows', 'windows-media-ocr', ''),
      svc('v', false, 'GPT', 'openai-vision', 'gpt-4o'),
    ])
    expect(opts).toHaveLength(2)
    expect(opts[0]).toMatchObject({ value: 'w', label: 'Windows' })
    expect(opts[1].description).toContain('gpt-4o')
  })
})

describe('reconcileSelectedOcrServiceId', () => {
  it('当前 id 仍存在则保留', () => {
    expect(
      reconcileSelectedOcrServiceId(
        [svc('a', true), svc('b', false)],
        'b',
      ),
    ).toBe('b')
  })

  it('当前 id 不存在则回落默认', () => {
    expect(
      reconcileSelectedOcrServiceId(
        [svc('a', false), svc('b', true)],
        'gone',
      ),
    ).toBe('b')
  })

  it('current 为 null 时取默认', () => {
    expect(
      reconcileSelectedOcrServiceId([svc('a', true)], null),
    ).toBe('a')
  })
})

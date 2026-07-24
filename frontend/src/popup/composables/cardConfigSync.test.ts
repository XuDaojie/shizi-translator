import { describe, expect, it } from 'vitest'
import type { AppConfig } from '@/types/config'
import type { CardState } from './useTranslationEvents'
import { enabledPayloads, syncCardsFromEnabledServices } from './cardConfigSync'

function minimalConfig(services: Array<{ id: string; name: string; enabled: boolean; protocol?: string; model?: string }>): AppConfig {
  return {
    interfaceLanguage: 'zh-CN',
    targetLang: 'zh-CN',
    defaultSourceLang: 'auto',
    autoCopy: false,
    restoreClipboard: false,
    historyLimit: 100,
    services: services.map((s) => ({
      id: s.id,
      serviceType: s.id,
      name: s.name,
      enabled: s.enabled,
      protocol: (s.protocol as AppConfig['services'][0]['protocol']) || 'openai_chat',
      apiKey: null,
      endpoint: '',
      model: s.model ?? 'm',
      timeoutSeconds: 60,
      systemPrompt: '',
      translationPrompt: '',
      reflectionPrompt: '',
      reflectionEnabled: false,
      chainOfThought: 'off',
    })),
    ocrServices: [],
    windowPrecreate: {
      manual: { popup: true, overlay: false },
      autostart: { popup: false, overlay: false },
    },
    collectUsage: false,
    logLevel: 'info',
    updateChannel: 'stable',
    autoCheckUpdate: true,
    popupUiBackend: 'webview',
    shortcuts: {},
  }
}

function card(id: string, status: CardState['status'] = 'pending'): CardState {
  return {
    serviceInstanceId: id,
    serviceName: id,
    serviceType: id,
    protocol: 'openai_chat',
    modelName: 'm',
    text: status === 'finished' ? 'ok' : '',
    status,
    collapsed: true,
    collapseUserOverride: false,
    expanded: false,
    hasOverflow: false,
    showActions: false,
    usage: null,
    detectedSourceLang: null,
    errorTitleKey: null,
    errorMessage: '',
  }
}

describe('enabledPayloads', () => {
  it('按 services 数组顺序只保留已启用项', () => {
    const cfg = minimalConfig([
      { id: 'a', name: 'A', enabled: true },
      { id: 'b', name: 'B', enabled: false },
      { id: 'c', name: 'C', enabled: true },
    ])
    expect(enabledPayloads(cfg).map((p) => p.serviceInstanceId)).toEqual(['a', 'c'])
  })

  it('microsoft_edge 清空 modelName', () => {
    const cfg = minimalConfig([
      { id: 'ms', name: 'Edge', enabled: true, protocol: 'microsoft_edge', model: 'gpt-x' },
    ])
    expect(enabledPayloads(cfg)[0].modelName).toBe('')
  })
})

describe('syncCardsFromEnabledServices', () => {
  it('空闲时按启用服务顺序重建 Map 迭代序', () => {
    const cards = new Map<string, CardState>([
      ['c', card('c')],
      ['a', card('a')],
    ])
    const payloads = enabledPayloads(
      minimalConfig([
        { id: 'a', name: 'A', enabled: true },
        { id: 'b', name: 'B', enabled: true },
        { id: 'c', name: 'C', enabled: true },
      ]),
    )
    syncCardsFromEnabledServices(cards, payloads, { isTranslating: false })
    expect([...cards.keys()]).toEqual(['a', 'b', 'c'])
    expect(cards.get('b')?.status).toBe('pending')
  })

  it('禁用再启用后顺序仍跟随配置而非旧插入序', () => {
    const cards = new Map<string, CardState>([
      ['a', card('a')],
      ['c', card('c')],
    ])
    // 模拟：曾删过 b，Map 只剩 a,c；配置顺序 a,b,c
    const payloads = enabledPayloads(
      minimalConfig([
        { id: 'a', name: 'A', enabled: true },
        { id: 'b', name: 'B', enabled: true },
        { id: 'c', name: 'C', enabled: true },
      ]),
    )
    syncCardsFromEnabledServices(cards, payloads, { isTranslating: false })
    expect([...cards.keys()]).toEqual(['a', 'b', 'c'])
  })

  it('空闲时移除已禁用服务卡片', () => {
    const cards = new Map<string, CardState>([
      ['a', card('a')],
      ['b', card('b')],
    ])
    const payloads = enabledPayloads(
      minimalConfig([
        { id: 'a', name: 'A', enabled: true },
        { id: 'b', name: 'B', enabled: false },
      ]),
    )
    syncCardsFromEnabledServices(cards, payloads, { isTranslating: false })
    expect([...cards.keys()]).toEqual(['a'])
  })

  it('翻译中不新增未参与批次的服务，但重排已有卡', () => {
    const cards = new Map<string, CardState>([
      ['c', card('c', 'translating')],
      ['a', card('a', 'translating')],
    ])
    const payloads = enabledPayloads(
      minimalConfig([
        { id: 'a', name: 'A-new', enabled: true },
        { id: 'b', name: 'B', enabled: true },
        { id: 'c', name: 'C-new', enabled: true },
      ]),
    )
    syncCardsFromEnabledServices(cards, payloads, { isTranslating: true })
    // b 未在当前 Map 中，翻译中不新增
    expect([...cards.keys()]).toEqual(['a', 'c'])
    expect(cards.get('a')?.serviceName).toBe('A-new')
    expect(cards.get('c')?.serviceName).toBe('C-new')
  })

  it('翻译中保留仍在 translating 但已禁用的卡片（挂末尾）', () => {
    const cards = new Map<string, CardState>([
      ['a', card('a', 'translating')],
      ['b', card('b', 'translating')],
    ])
    const payloads = enabledPayloads(
      minimalConfig([
        { id: 'a', name: 'A', enabled: true },
        { id: 'b', name: 'B', enabled: false },
      ]),
    )
    syncCardsFromEnabledServices(cards, payloads, { isTranslating: true })
    expect([...cards.keys()]).toEqual(['a', 'b'])
    expect(cards.get('b')?.status).toBe('translating')
  })
})

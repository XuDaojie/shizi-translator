import { describe, expect, it } from 'vitest'
import { exportSettings, importSettings, parseImportedSettings } from './config-io'
import type { AppSettings } from './types'

const baseState: AppSettings = {
  general: {
    launchAtLogin: true,
    theme: 'system',
    language: 'zh-CN',
    updateChannel: 'stable',
    autoCheckUpdate: true,
  },
  windowPrecreate: {
    manual: { popup: true, overlay: false },
    autostart: { popup: false, overlay: false },
  },
  translation: {
    defaultSourceLang: 'auto',
    defaultTargetLang: '中文',
    autoCopy: false,
    restoreClipboard: true,
    autoPaste: false,
    showPhonetic: false,
    showAlternatives: false,
    autoDetect: true,
    wordLookupDelay: 300,
    historyLimit: 200,
  },
  shortcut: {
    bindings: [],
  },
  services: [
    { id: 'svc-1', apiKey: 'sk-live-aaa', name: 'DeepSeek', enabled: true, protocol: 'openai_chat', type: 'deepseek', model: 'deepseek-chat', endpoint: 'https://api.deepseek.com', note: '', pulledModels: [], keyStatus: 'idle', chainOfThought: 'off', systemPrompt: '', translationPrompt: '', reflectionPrompt: '', reflectionEnabled: false },
    { id: 'svc-2', apiKey: 'sk-live-bbb', name: 'Claude', enabled: false, protocol: 'claude_messages', type: 'claude', model: 'claude-sonnet-4-20250514', endpoint: 'https://api.anthropic.com', note: '', pulledModels: [], keyStatus: 'idle', chainOfThought: 'off', systemPrompt: '', translationPrompt: '', reflectionPrompt: '', reflectionEnabled: false },
  ],
  ocrServices: [],
  customServiceTypes: [],
  advanced: {
    logLevel: 'info',
    betaLookup: false,
    betaVoice: false,
    collectUsage: false,
  },
}

describe('config-io', () => {
  it('导出配置时剔除 API Key', () => {
    const exported = exportSettings(baseState)
    for (const svc of exported.services) {
      expect(svc.apiKey).toBe('')
    }
    expect(exported.services[0].name).toBe('DeepSeek')
  })

  it('导入配置时保留本地已有服务的 API Key', () => {
    const incoming = exportSettings(baseState)
    incoming.services[0].name = '改名'
    const merged = importSettings(baseState, incoming)
    expect(merged.services[0].name).toBe('改名')
    expect(merged.services[0].apiKey).toBe('sk-live-aaa')
    expect(merged.services[1].apiKey).toBe('sk-live-bbb')
  })

  it('导入配置时新增服务使用导入的 API Key（空字符串）', () => {
    const incoming = exportSettings(baseState)
    incoming.services.push({
      id: 'svc-new',
      apiKey: '',
      name: '新服务',
      enabled: true,
      protocol: 'openai_chat',
      type: 'custom',
      model: 'gpt-4o',
      endpoint: 'https://api.openai.com',
      note: '',
      pulledModels: [],
      keyStatus: 'idle',
      chainOfThought: 'off',
      systemPrompt: '',
      translationPrompt: '',
      reflectionPrompt: '',
      reflectionEnabled: false,
    })
    const merged = importSettings(baseState, incoming)
    expect(merged.services).toHaveLength(3)
    expect(merged.services[2].apiKey).toBe('')
    expect(merged.services[2].name).toBe('新服务')
  })

  it('parseImportedSettings 解析合法 JSON', () => {
    const parsed = parseImportedSettings(JSON.stringify(baseState))
    expect(parsed.services[0].name).toBe('DeepSeek')
  })

  it('parseImportedSettings 非法 JSON 抛错', () => {
    expect(() => parseImportedSettings('{broken json')).toThrow('JSON 格式无效')
  })

  it('parseImportedSettings 空字符串抛错', () => {
    expect(() => parseImportedSettings('')).toThrow()
  })
})

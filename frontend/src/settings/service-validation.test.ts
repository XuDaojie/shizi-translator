import { describe, expect, it } from 'vitest'
import { validateServiceForEnable } from './service-validation'
import type { ServiceInstance, ServiceMeta } from './types'

const meta: ServiceMeta = {
  id: 'deepseek',
  name: 'DeepSeek',
  description: '',
  builtin: true,
  category: 'llm',
  keyRequired: true,
  protocols: [{
    id: 'openai_chat',
    label: 'OpenAI Chat',
    defaultEndpoint: 'https://api.deepseek.com',
    defaultModel: 'deepseek-chat',
    editableEndpoint: true,
    status: 'available',
  }],
}

const inst = (over: Partial<ServiceInstance>): ServiceInstance => ({
  id: 'deepseek-1',
  type: 'deepseek',
  name: 'DeepSeek',
  enabled: false,
  protocol: 'openai_chat',
  apiKey: 'sk-x',
  endpoint: 'https://api.deepseek.com',
  model: 'deepseek-chat',
  note: '',
  pulledModels: [],
  keyStatus: 'idle',
  chainOfThought: 'off',
  systemPrompt: '',
  translationPrompt: '',
  reflectionPrompt: '',
  reflectionEnabled: false,
  ...over,
})

describe('validateServiceForEnable', () => {
  it('配置完整时允许开启', () => {
    expect(validateServiceForEnable(inst({}), meta)).toBeNull()
  })

  it('缺 API Key 时阻止开启', () => {
    expect(validateServiceForEnable(inst({ apiKey: '' }), meta)).toBe('请先填写 API Key')
  })

  it('endpoint 非 http(s) 时阻止开启', () => {
    expect(validateServiceForEnable(inst({ endpoint: 'ftp://bad' }), meta)).toBe(
      'Endpoint 请输入有效的 http(s) 地址'
    )
  })

  it('model 为空时阻止开启', () => {
    expect(validateServiceForEnable(inst({ model: '' }), meta)).toBe('Model 不能为空')
  })

  it('协议不可用时阻止开启', () => {
    const badMeta = {
      ...meta,
      protocols: [{ ...meta.protocols[0], status: 'planned' as const }],
    }
    expect(validateServiceForEnable(inst({}), badMeta)).toBe('当前协议不可用')
  })

  it('不需要 key 的服务跳过 key 检查', () => {
    const noKeyMeta = { ...meta, keyRequired: false }
    expect(validateServiceForEnable(inst({ apiKey: '' }), noKeyMeta)).toBeNull()
  })
})

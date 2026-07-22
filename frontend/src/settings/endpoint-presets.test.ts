import { describe, expect, it } from 'vitest'
import { applyEndpointPreset, matchEndpointPreset } from './endpoint-presets'
import type { ServiceEndpointPreset } from './types'

const presets: ServiceEndpointPreset[] = [
  {
    id: 'ark-api',
    label: '方舟 API',
    endpoint: 'https://ark.cn-beijing.volces.com/api/v3',
    defaultModel: 'doubao-seed-1-6-250615',
  },
  {
    id: 'coding-plan',
    label: 'Coding Plan',
    endpoint: 'https://ark.cn-beijing.volces.com/api/coding/v3',
    defaultModel: 'ark-code-latest',
  },
  {
    id: 'agent-plan',
    label: 'Agent Plan',
    endpoint: 'https://ark.cn-beijing.volces.com/api/plan/v3',
    defaultModel: 'ark-code-latest',
  },
]

describe('matchEndpointPreset', () => {
  it('matches trailing slash variants', () => {
    expect(
      matchEndpointPreset('https://ark.cn-beijing.volces.com/api/coding/v3/', presets)?.id,
    ).toBe('coding-plan')
  })

  it('returns undefined for custom endpoint', () => {
    expect(matchEndpointPreset('https://example.com/v1', presets)).toBeUndefined()
  })
})

describe('applyEndpointPreset', () => {
  it('fills empty model with preset default', () => {
    const next = applyEndpointPreset(presets[1], { endpoint: '', model: '' })
    expect(next.endpoint).toBe(presets[1].endpoint)
    expect(next.model).toBe('ark-code-latest')
  })

  it('switches model when still previous preset default', () => {
    const next = applyEndpointPreset(
      presets[1],
      { endpoint: presets[0].endpoint, model: 'doubao-seed-1-6-250615' },
      { previousPresetDefaultModel: 'doubao-seed-1-6-250615' },
    )
    expect(next.model).toBe('ark-code-latest')
  })

  it('keeps user custom model', () => {
    const next = applyEndpointPreset(
      presets[1],
      { endpoint: presets[0].endpoint, model: 'ep-user-custom-123' },
      { previousPresetDefaultModel: 'doubao-seed-1-6-250615' },
    )
    expect(next.model).toBe('ep-user-custom-123')
  })
})

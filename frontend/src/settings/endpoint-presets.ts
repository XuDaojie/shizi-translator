import type { ServiceEndpointPreset } from './types'

/** 当前 endpoint 是否精确匹配某预设（忽略尾部 `/`）。 */
export function matchEndpointPreset(
  endpoint: string,
  presets: ServiceEndpointPreset[] | undefined,
): ServiceEndpointPreset | undefined {
  if (!presets?.length) return undefined
  const norm = endpoint.trim().replace(/\/+$/, '')
  if (!norm) return undefined
  return presets.find((p) => p.endpoint.replace(/\/+$/, '') === norm)
}

/**
 * 应用预设到实例字段。
 * - 始终写入 endpoint
 * - 若预设带 defaultModel：仅当当前 model 为空，或仍等于上一预设默认时覆盖，避免冲掉用户自定义模型
 */
export function applyEndpointPreset(
  preset: ServiceEndpointPreset,
  current: { endpoint: string; model: string },
  options?: { previousPresetDefaultModel?: string },
): { endpoint: string; model: string } {
  const endpoint = preset.endpoint
  const nextDefault = preset.defaultModel?.trim() ?? ''
  if (!nextDefault) {
    return { endpoint, model: current.model }
  }
  const model = current.model.trim()
  if (!model) {
    return { endpoint, model: nextDefault }
  }
  const prevDefault = options?.previousPresetDefaultModel?.trim()
  if (prevDefault && model === prevDefault) {
    return { endpoint, model: nextDefault }
  }
  return { endpoint, model: current.model }
}

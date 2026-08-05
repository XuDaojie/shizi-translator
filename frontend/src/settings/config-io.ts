import type { AppSettings } from './types'

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T

export function exportSettings(state: AppSettings): AppSettings {
  const exported = clone(state)
  const includeKeys = exported.advanced?.backup?.includeApiKeys ?? true
  if (!includeKeys) {
    exported.services = exported.services.map((s) => ({ ...s, apiKey: '' }))
    exported.ocrServices = (exported.ocrServices ?? []).map((s) => ({ ...s, apiKey: '' }))
    if (exported.advanced?.backup?.webdav) {
      exported.advanced.backup.webdav.password = ''
    }
  }
  return exported
}

export function importSettings(
  current: AppSettings,
  incoming: AppSettings,
): AppSettings {
  const localKeys = new Map(current.services.map((s) => [s.id, s.apiKey]))
  const merged = clone(incoming)
  merged.services = merged.services.map((s) => ({
    ...s,
    apiKey: localKeys.get(s.id) ?? s.apiKey ?? '',
  }))
  return merged
}

export function parseImportedSettings(json: string): AppSettings {
  try {
    return JSON.parse(json) as AppSettings
  } catch {
    throw new Error('JSON 格式无效')
  }
}

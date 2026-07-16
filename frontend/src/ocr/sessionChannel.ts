import type { OcrServiceInstanceConfig } from '@/types/config'

export function pickDefaultOcrServiceId(
  services: OcrServiceInstanceConfig[],
): string | null {
  const enabled = services.find((s) => s.enabled)
  if (enabled) return enabled.id
  return services[0]?.id ?? null
}

export function buildOcrChannelOptions(
  services: OcrServiceInstanceConfig[],
): { value: string; label: string; description?: string }[] {
  return services.map((s) => {
    const parts = [s.serviceType, s.model].filter((x) => x && x.trim().length > 0)
    return {
      value: s.id,
      label: s.name || s.id,
      description: parts.length ? parts.join(' · ') : undefined,
    }
  })
}

/** 配置变更后：当前 id 仍存在则保留，否则回落默认 */
export function reconcileSelectedOcrServiceId(
  services: OcrServiceInstanceConfig[],
  currentId: string | null,
): string | null {
  if (currentId && services.some((s) => s.id === currentId)) return currentId
  return pickDefaultOcrServiceId(services)
}

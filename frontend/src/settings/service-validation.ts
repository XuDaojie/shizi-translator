import type { ServiceInstance, ServiceMeta } from './types'

export function validateServiceForEnable(
  instance: ServiceInstance,
  meta?: ServiceMeta
): string | null {
  const protocol = meta?.protocols.find((p) => p.id === instance.protocol)
  if (protocol && protocol.status !== 'available') {
    return '当前协议不可用'
  }

  if (meta?.keyRequired !== false && !(instance.apiKey ?? '').trim()) {
    return '请先填写 API Key'
  }

  let url: URL
  try {
    url = new URL(instance.endpoint.trim())
  } catch {
    return 'Endpoint 请输入有效的 http(s) 地址'
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    return 'Endpoint 请输入有效的 http(s) 地址'
  }

  if (meta?.keyRequired !== false && !instance.model.trim()) {
    return 'Model 不能为空'
  }

  return null
}

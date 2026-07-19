import type { AppConfig } from '@/types/config'
import type { CardState } from './useTranslationEvents'

export type EnabledServicePayload = {
  serviceInstanceId: string
  serviceType: string
  serviceName: string
  protocol: string
  modelName: string
}

/** 按 config.services 数组顺序取已启用服务，供结果卡排序与元数据同步。 */
export function enabledPayloads(config: AppConfig): EnabledServicePayload[] {
  return (config.services || [])
    .filter((s) => s.enabled)
    .map((s) => ({
      serviceInstanceId: s.id,
      serviceType: s.serviceType,
      serviceName: s.name,
      protocol: s.protocol || '',
      modelName: s.protocol === 'microsoft_edge' ? '' : s.model || '',
    }))
}

function createPendingCard(p: EnabledServicePayload): CardState {
  return {
    serviceInstanceId: p.serviceInstanceId,
    serviceName: p.serviceName,
    serviceType: p.serviceType,
    protocol: p.protocol,
    modelName: p.modelName,
    text: '',
    status: 'pending',
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

function applyMeta(card: CardState, p: EnabledServicePayload): void {
  card.serviceName = p.serviceName
  card.serviceType = p.serviceType
  card.protocol = p.protocol
  card.modelName = p.modelName
}

/**
 * 按启用服务列表顺序重建 cards Map。
 * - 空闲：增删改卡片，Map 迭代序 = 配置启用序
 * - 翻译中：不新增卡片；可删非 translating 的已禁用卡；元数据可更新；
 *   仍保留的卡按配置序重排（翻译中新启用不参与当前批次）
 */
export function syncCardsFromEnabledServices(
  cards: Map<string, CardState>,
  payloads: EnabledServicePayload[],
  options: { isTranslating: boolean },
): void {
  const next = new Map<string, CardState>()

  for (const p of payloads) {
    const existing = cards.get(p.serviceInstanceId)
    if (existing) {
      applyMeta(existing, p)
      next.set(p.serviceInstanceId, existing)
    } else if (!options.isTranslating) {
      next.set(p.serviceInstanceId, createPendingCard(p))
    }
  }

  // 翻译中：保留仍在输出、但已从启用列表移除的卡（挂在末尾）
  if (options.isTranslating) {
    for (const [id, card] of cards) {
      if (!next.has(id) && card.status === 'translating') {
        next.set(id, card)
      }
    }
  }

  cards.clear()
  for (const [id, card] of next) {
    cards.set(id, card)
  }
}

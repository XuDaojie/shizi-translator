/**
 * 结果卡右下角元信息展示规则：
 * - 机器翻译（microsoft_edge）无模型概念，也不展示 Token
 * - LLM 才显示 modelName / usage
 */

export function isMachineTranslateProtocol(protocol: string | undefined | null): boolean {
  return (protocol ?? '').trim() === 'microsoft_edge'
}

/** 结果卡展示用模型名；MT 或空占位返回空串（不渲染标签） */
export function displayModelName(
  protocol: string | undefined | null,
  modelName: string | undefined | null,
): string {
  if (isMachineTranslateProtocol(protocol)) return ''
  const name = (modelName ?? '').trim()
  if (!name || name === '—' || name === '-') return ''
  return name
}

/** 是否展示输入/输出 Token：MT 永不展示；其余需有 usage 数据 */
export function shouldShowTokens(
  protocol: string | undefined | null,
  hasUsage: boolean,
): boolean {
  if (isMachineTranslateProtocol(protocol)) return false
  return hasUsage
}

/** 与 components.css `.result-text-clip { max-height: 6.4em }` 保持一致 */
export const COLLAPSED_MAX_HEIGHT_EM = 6.4

export function collapsedMaxHeightPx(fontSizePx: number): number {
  return COLLAPSED_MAX_HEIGHT_EM * (fontSizePx || 13)
}

/**
 * 展开/收起动画的 max-height 起止（px）。
 * 内容未超出折叠上限时返回 null（无需动画，交给 CSS 默认值）。
 */
export function clipExpandRange(
  contentHeightPx: number,
  fontSizePx: number,
): { collapsedPx: number; expandedPx: number } | null {
  const collapsedPx = collapsedMaxHeightPx(fontSizePx)
  if (contentHeightPx <= collapsedPx + 1) return null
  return { collapsedPx, expandedPx: contentHeightPx }
}

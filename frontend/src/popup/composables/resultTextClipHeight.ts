/** 与 components.css `.result-text-clip { max-height: 6.4em }` 保持一致 */
export const COLLAPSED_MAX_HEIGHT_EM = 6.4

export function collapsedMaxHeightPx(fontSizePx: number): number {
  return COLLAPSED_MAX_HEIGHT_EM * (fontSizePx || 13)
}

/**
 * 正文是否超出折叠上限，以及展开终点高度（px）。
 * 供 ResultCardView 写入 --result-clip-expanded；实际动画由 CSS class 切换完成。
 * 内容未超出折叠上限时返回 null。
 */
export function clipExpandRange(
  contentHeightPx: number,
  fontSizePx: number,
): { collapsedPx: number; expandedPx: number } | null {
  const collapsedPx = collapsedMaxHeightPx(fontSizePx)
  if (contentHeightPx <= collapsedPx + 1) return null
  return { collapsedPx, expandedPx: contentHeightPx }
}

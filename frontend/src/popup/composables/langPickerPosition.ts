/** 锚点矩形（与 DOMRect 兼容的子集） */
export interface AnchorRect {
  top: number
  left: number
  bottom: number
  width: number
}

export interface LangPickerPosition {
  left: number
  width: number
  /** 向下展开时设置 */
  top?: number
  /** 向上展开时设置 */
  bottom?: number
  maxHeight: number
  openBelow: boolean
}

export interface ComputeLangPickerPositionOptions {
  /** 锚点与面板间距，默认 4 */
  gap?: number
  /** 理想总高度（搜索栏 + 列表），默认 280 */
  idealHeight?: number
  /** 最小可用高度，默认 120 */
  minHeight?: number
  /** 下方至少有这么多空间才优先向下（否则与上方比），默认 160 */
  preferBelowMin?: number
}

/**
 * 根据锚点与视口高度计算 fixed 语言选择面板位置。
 * 下方空间不足时改为向上展开，避免被窗口底边或 scroll 容器裁成一条线。
 */
export function computeLangPickerPosition(
  anchor: AnchorRect,
  viewportHeight: number,
  opts: ComputeLangPickerPositionOptions = {},
): LangPickerPosition {
  const gap = opts.gap ?? 4
  const idealHeight = opts.idealHeight ?? 280
  const minHeight = opts.minHeight ?? 120
  const preferBelowMin = opts.preferBelowMin ?? 160

  const spaceBelow = viewportHeight - anchor.bottom - gap
  const spaceAbove = anchor.top - gap
  const openBelow = spaceBelow >= Math.min(preferBelowMin, spaceAbove) || spaceBelow >= spaceAbove
  const available = Math.max(minHeight, openBelow ? spaceBelow : spaceAbove)
  const maxHeight = Math.min(idealHeight, available)

  if (openBelow) {
    return {
      left: anchor.left,
      width: anchor.width,
      top: anchor.bottom + gap,
      maxHeight,
      openBelow: true,
    }
  }
  return {
    left: anchor.left,
    width: anchor.width,
    bottom: viewportHeight - anchor.top + gap,
    maxHeight,
    openBelow: false,
  }
}

export function langPickerPositionToStyle(pos: LangPickerPosition): Record<string, string> {
  const style: Record<string, string> = {
    position: 'fixed',
    left: `${pos.left}px`,
    width: `${pos.width}px`,
    maxHeight: `${pos.maxHeight}px`,
    zIndex: '1000',
  }
  if (pos.openBelow && pos.top !== undefined) {
    style.top = `${pos.top}px`
    style.bottom = 'auto'
  } else if (pos.bottom !== undefined) {
    style.bottom = `${pos.bottom}px`
    style.top = 'auto'
  }
  return style
}

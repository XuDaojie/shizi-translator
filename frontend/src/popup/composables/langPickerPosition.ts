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
  /** 视口边缘安全留白，默认 8，避免贴边被 OS/窗口裁切 */
  edgeMargin?: number
  /** 理想总高度（搜索栏 + 列表），默认 280 */
  idealHeight?: number
}

export interface SnapPickerMaxHeightInput {
  /** 方向可用的最大高度（已含 edge 约束） */
  availableMax: number
  searchHeight: number
  optionHeight: number
  /** 列表上下 padding 之和，默认 8 */
  listPaddingY?: number
  /** 面板边框垂直合计，默认 1（0.5px*2） */
  borderY?: number
  /** 内容自然高度（search + list.scrollHeight），用于不超过真实内容 */
  contentHeight: number
}

/**
 * 根据锚点与视口高度计算 fixed 语言选择面板位置。
 * - 优先选能放下 idealHeight 的一侧；两侧都不够则选空间更大的一侧
 * - 高度不超过可用空间（含 edgeMargin），避免面板底部被窗口裁成半截选项
 */
export function computeLangPickerPosition(
  anchor: AnchorRect,
  viewportHeight: number,
  opts: ComputeLangPickerPositionOptions = {},
): LangPickerPosition {
  const gap = opts.gap ?? 4
  const edgeMargin = opts.edgeMargin ?? 8
  const idealHeight = opts.idealHeight ?? 280

  const spaceBelow = viewportHeight - anchor.bottom - gap - edgeMargin
  const spaceAbove = anchor.top - gap - edgeMargin

  let openBelow: boolean
  if (spaceBelow >= idealHeight) {
    openBelow = true
  } else if (spaceAbove >= idealHeight) {
    openBelow = false
  } else {
    openBelow = spaceBelow >= spaceAbove
  }

  const available = Math.max(0, openBelow ? spaceBelow : spaceAbove)
  // 绝不超过 available，否则会顶出视口把最后一行裁半截
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

/**
 * 将 maxHeight 吸附到「完整选项行」：避免列表可视区底部切在半行上。
 * 内容能完整放下时直接用 contentHeight；需要滚动时按整行裁切 availableMax。
 */
export function snapPickerMaxHeight(input: SnapPickerMaxHeightInput): number {
  const {
    availableMax,
    searchHeight,
    optionHeight,
    listPaddingY = 8,
    borderY = 1,
    contentHeight,
  } = input

  if (availableMax <= 0) return 0
  const natural = Math.max(contentHeight, searchHeight)
  // 内容整体可放下：不滚动、不切行
  if (natural <= availableMax) {
    return Math.max(0, Math.ceil(natural))
  }
  if (optionHeight <= 0) {
    return Math.max(0, Math.floor(availableMax))
  }

  const chrome = searchHeight + listPaddingY + borderY
  const listBudget = availableMax - chrome
  if (listBudget < optionHeight) {
    return Math.max(0, Math.floor(Math.min(availableMax, chrome + optionHeight)))
  }

  const fullRows = Math.floor(listBudget / optionHeight)
  const snapped = chrome + fullRows * optionHeight
  return Math.max(0, Math.floor(Math.min(snapped, availableMax)))
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

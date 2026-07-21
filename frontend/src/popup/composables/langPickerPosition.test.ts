import { describe, expect, it } from 'vitest'
import {
  computeLangPickerPosition,
  langPickerPositionToStyle,
  snapPickerMaxHeight,
} from './langPickerPosition'

describe('computeLangPickerPosition', () => {
  const anchor = { top: 100, left: 20, bottom: 128, width: 380 }

  it('下方能放下 ideal 时向下展开，高度为 ideal', () => {
    const pos = computeLangPickerPosition(anchor, 800)
    expect(pos.openBelow).toBe(true)
    expect(pos.top).toBe(132)
    expect(pos.bottom).toBeUndefined()
    expect(pos.maxHeight).toBe(280)
    expect(pos.left).toBe(20)
    expect(pos.width).toBe(380)
  })

  it('下方不够 ideal、上方够时改为向上展开', () => {
    // 视口 800，锚点 bottom=720 → 下方仅约 68，上方约 692
    const nearBottom = { top: 700, left: 10, bottom: 728, width: 400 }
    const pos = computeLangPickerPosition(nearBottom, 800)
    expect(pos.openBelow).toBe(false)
    expect(pos.bottom).toBe(800 - 700 + 4)
    expect(pos.maxHeight).toBe(280)
  })

  it('两侧都不够 ideal 时选空间更大的一侧，高度=available', () => {
    // 视口 400，锚点中部：上 150-edge，下 200-edge
    const mid = { top: 150, left: 0, bottom: 180, width: 300 }
    const pos = computeLangPickerPosition(mid, 400, { idealHeight: 280, edgeMargin: 8 })
    // spaceBelow = 400-180-4-8 = 208；spaceAbove = 150-4-8 = 138 → 向下
    expect(pos.openBelow).toBe(true)
    expect(pos.maxHeight).toBe(208)
  })

  it('下方仅略大于 prefer 旧逻辑阈值但远小于 ideal 时仍应向上（有足够上方）', () => {
    // 旧逻辑 preferBelowMin=160 会错误向下；新逻辑上方能装 ideal 应向上
    const nearBottom = { top: 500, left: 0, bottom: 620, width: 400 }
    const pos = computeLangPickerPosition(nearBottom, 800, { idealHeight: 280, edgeMargin: 8 })
    // spaceBelow = 800-620-4-8 = 168；spaceAbove = 500-4-8 = 488
    // 下方 < ideal，上方 >= ideal → 向上
    expect(pos.openBelow).toBe(false)
    expect(pos.maxHeight).toBe(280)
  })

  it('edgeMargin 会从可用高度中扣除', () => {
    const a = { top: 50, left: 0, bottom: 80, width: 200 }
    const pos = computeLangPickerPosition(a, 200, { idealHeight: 500, edgeMargin: 8, gap: 4 })
    // spaceBelow = 200-80-4-8 = 108
    expect(pos.openBelow).toBe(true)
    expect(pos.maxHeight).toBe(108)
  })
})

describe('snapPickerMaxHeight', () => {
  it('吸附到完整行数，不切半行', () => {
    // search 36 + pad 8 + border 1 + 7*28 = 36+8+1+196 = 241
    const h = snapPickerMaxHeight({
      availableMax: 250,
      searchHeight: 36,
      optionHeight: 28,
      listPaddingY: 8,
      borderY: 1,
      contentHeight: 600,
    })
    expect(h).toBe(36 + 8 + 1 + 7 * 28)
    expect((h - 36 - 8 - 1) % 28).toBe(0)
  })

  it('内容更矮时收拢到 contentHeight', () => {
    const h = snapPickerMaxHeight({
      availableMax: 280,
      searchHeight: 36,
      optionHeight: 28,
      listPaddingY: 8,
      borderY: 1,
      contentHeight: 120,
    })
    expect(h).toBe(120)
  })

  it('available 极小时不超过 available', () => {
    const h = snapPickerMaxHeight({
      availableMax: 40,
      searchHeight: 36,
      optionHeight: 28,
      contentHeight: 400,
    })
    expect(h).toBeLessThanOrEqual(40)
  })
})

describe('langPickerPositionToStyle', () => {
  it('向下展开写出 top', () => {
    const style = langPickerPositionToStyle({
      left: 10,
      width: 400,
      top: 50,
      maxHeight: 200,
      openBelow: true,
    })
    expect(style.position).toBe('fixed')
    expect(style.top).toBe('50px')
    expect(style.bottom).toBe('auto')
    expect(style.maxHeight).toBe('200px')
  })

  it('向上展开写出 bottom', () => {
    const style = langPickerPositionToStyle({
      left: 10,
      width: 400,
      bottom: 80,
      maxHeight: 180,
      openBelow: false,
    })
    expect(style.bottom).toBe('80px')
    expect(style.top).toBe('auto')
  })
})

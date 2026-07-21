import { describe, expect, it } from 'vitest'
import { computeLangPickerPosition, langPickerPositionToStyle } from './langPickerPosition'

describe('computeLangPickerPosition', () => {
  const anchor = { top: 100, left: 20, bottom: 128, width: 380 }

  it('下方空间充足时向下展开', () => {
    const pos = computeLangPickerPosition(anchor, 800)
    expect(pos.openBelow).toBe(true)
    expect(pos.top).toBe(132)
    expect(pos.bottom).toBeUndefined()
    expect(pos.maxHeight).toBe(280)
    expect(pos.left).toBe(20)
    expect(pos.width).toBe(380)
  })

  it('靠近视口底部时改为向上展开', () => {
    const nearBottom = { top: 700, left: 10, bottom: 728, width: 400 }
    const pos = computeLangPickerPosition(nearBottom, 800)
    expect(pos.openBelow).toBe(false)
    expect(pos.bottom).toBe(800 - 700 + 4)
    expect(pos.top).toBeUndefined()
    expect(pos.maxHeight).toBeLessThanOrEqual(280)
    expect(pos.maxHeight).toBe(Math.min(280, 700 - 4))
  })

  it('下方只有一条缝时向上展开并限制高度', () => {
    const nearBottom = { top: 760, left: 0, bottom: 788, width: 400 }
    const pos = computeLangPickerPosition(nearBottom, 800)
    expect(pos.openBelow).toBe(false)
    // 上方约 756px，理想 280
    expect(pos.maxHeight).toBe(280)
  })

  it('上下都极窄时仍给出 minHeight', () => {
    const mid = { top: 40, left: 0, bottom: 60, width: 200 }
    const pos = computeLangPickerPosition(mid, 100, { minHeight: 120, idealHeight: 280 })
    expect(pos.maxHeight).toBe(120)
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

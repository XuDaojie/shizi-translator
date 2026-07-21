import { describe, expect, it } from 'vitest'
import { clipExpandRange, collapsedMaxHeightPx, COLLAPSED_MAX_HEIGHT_EM } from './resultTextClipHeight'

describe('resultTextClipHeight', () => {
  it('collapsedMaxHeightPx = 6.4em × fontSize', () => {
    expect(collapsedMaxHeightPx(13)).toBeCloseTo(COLLAPSED_MAX_HEIGHT_EM * 13)
    expect(collapsedMaxHeightPx(16)).toBeCloseTo(102.4)
  })

  it('fontSize 为 0 时回退 13', () => {
    expect(collapsedMaxHeightPx(0)).toBeCloseTo(COLLAPSED_MAX_HEIGHT_EM * 13)
  })

  it('内容未溢出返回 null', () => {
    const collapsed = collapsedMaxHeightPx(13)
    expect(clipExpandRange(collapsed, 13)).toBeNull()
    expect(clipExpandRange(collapsed + 1, 13)).toBeNull()
  })

  it('内容溢出返回 collapsed / expanded 起止', () => {
    const collapsed = collapsedMaxHeightPx(13)
    const range = clipExpandRange(collapsed + 40, 13)
    expect(range).toEqual({
      collapsedPx: collapsed,
      expandedPx: collapsed + 40,
    })
  })
})

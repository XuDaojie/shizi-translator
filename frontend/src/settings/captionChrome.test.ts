import { describe, expect, it } from 'vitest'
import { captionChromeForTheme, parseCssRgb } from './captionChrome'

describe('parseCssRgb', () => {
  it('parses comma-separated rgb()', () => {
    expect(parseCssRgb('rgb(252, 250, 247)')).toEqual({ r: 252, g: 250, b: 247 })
  })

  it('parses space-separated rgb() and rgba()', () => {
    expect(parseCssRgb('rgb(19 16 13)')).toEqual({ r: 19, g: 16, b: 13 })
    expect(parseCssRgb('rgba(38, 43, 54, 1)')).toEqual({ r: 38, g: 43, b: 54 })
  })

  it('returns null for empty or non-rgb values', () => {
    expect(parseCssRgb('')).toBeNull()
    expect(parseCssRgb('oklch(98.5% 0.004 70)')).toBeNull()
  })
})

describe('captionChromeForTheme', () => {
  it('uses the light sidebar token, not computed CSS, when theme resolves to light', () => {
    expect(captionChromeForTheme(false)).toEqual({
      caption: [252, 250, 247],
      text: [38, 43, 54],
      darkButtons: false,
    })
  })

  it('uses the dark sidebar token when theme resolves to dark', () => {
    expect(captionChromeForTheme(true)).toEqual({
      caption: [19, 16, 13],
      text: [240, 242, 244],
      darkButtons: true,
    })
  })
})

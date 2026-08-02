import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const css = readFileSync(resolve(__dirname, 'main.css'), 'utf8')

describe('theme color tokens', () => {
  it('maps popover tokens to valid hsl colors', () => {
    expect(css).toContain('--color-popover: hsl(var(--popover));')
    expect(css).toContain('--color-popover-foreground: hsl(var(--popover-foreground));')
  })

  it('uses fixed orange brand primary (no blue accent override)', () => {
    expect(css).toContain('--primary: 20 74% 48%;')
    expect(css).not.toContain('accent-blue')
    expect(css).not.toMatch(/--primary:\s*222 70% 48%;/)
  })

  it('keeps sidebar token white to match prototype nav (bg-card/40)', () => {
    expect(css).toMatch(/--sidebar:\s*0\s+0%\s+100%;/)
  })
})

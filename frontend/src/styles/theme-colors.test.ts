import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const css = readFileSync(resolve(__dirname, 'main.css'), 'utf8')

describe('theme color tokens', () => {
  it('maps popover tokens to valid hsl colors', () => {
    expect(css).toContain('--color-popover: hsl(var(--popover));')
    expect(css).toContain('--color-popover-foreground: hsl(var(--popover-foreground));')
  })
})

import { describe, expect, it } from 'vitest'
import { matchShortcutKeys } from './matchShortcut'

function keyEvent( partial: Partial<KeyboardEvent> & { key: string }): KeyboardEvent {
  return {
    key: partial.key,
    code: partial.code ?? '',
    ctrlKey: partial.ctrlKey ?? false,
    altKey: partial.altKey ?? false,
    shiftKey: partial.shiftKey ?? false,
    metaKey: partial.metaKey ?? false,
  } as KeyboardEvent
}

describe('matchShortcutKeys', () => {
  it('matches Ctrl+,', () => {
    expect(
      matchShortcutKeys('Ctrl+,', keyEvent({ key: ',', code: 'Comma', ctrlKey: true })),
    ).toBe(true)
  })

  it('matches Ctrl+, via code when key is mangled', () => {
    expect(
      matchShortcutKeys('Ctrl+,', keyEvent({ key: '<', code: 'Comma', ctrlKey: true })),
    ).toBe(true)
  })

  it('rejects when modifier missing', () => {
    expect(matchShortcutKeys('Ctrl+,', keyEvent({ key: ',' }))).toBe(false)
  })

  it('matches Alt+D case-insensitively', () => {
    expect(
      matchShortcutKeys('Alt+D', keyEvent({ key: 'd', altKey: true })),
    ).toBe(true)
  })

  it('rejects empty binding', () => {
    expect(matchShortcutKeys('', keyEvent({ key: 'a', ctrlKey: true }))).toBe(false)
  })

  it('rejects extra modifiers', () => {
    expect(
      matchShortcutKeys('Ctrl+,', keyEvent({ key: ',', ctrlKey: true, shiftKey: true })),
    ).toBe(false)
  })
})

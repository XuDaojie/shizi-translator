import { describe, expect, it } from 'vitest'
import {
  isPromptDirty,
  isPromptDefault,
  shouldShowDefaultPreview,
  shouldShowCharCount,
} from './setting-textarea-logic'

describe('setting-textarea-logic', () => {
  it('空串不算 dirty；显式改过才 dirty', () => {
    expect(isPromptDirty({ modelValue: '', defaultValue: 'DEF', showReset: true })).toBe(false)
    expect(isPromptDirty({ modelValue: 'DEF', defaultValue: 'DEF', showReset: true })).toBe(false)
    expect(isPromptDirty({ modelValue: '自定义', defaultValue: 'DEF', showReset: true })).toBe(true)
  })

  it('空或等于默认 → isDefault', () => {
    expect(isPromptDefault({ modelValue: '', defaultValue: 'DEF' })).toBe(true)
    expect(isPromptDefault({ modelValue: 'DEF', defaultValue: 'DEF' })).toBe(true)
    expect(isPromptDefault({ modelValue: 'x', defaultValue: 'DEF' })).toBe(false)
  })

  it('空态预览：空 model + 有 default + 未 focus + 未 collapsed', () => {
    expect(
      shouldShowDefaultPreview({
        modelValue: '',
        defaultValue: 'DEF',
        focused: false,
        collapsed: false,
      }),
    ).toBe(true)
    expect(
      shouldShowDefaultPreview({
        modelValue: '',
        defaultValue: 'DEF',
        focused: true,
        collapsed: false,
      }),
    ).toBe(false)
  })

  it('字数：focus 或 dirty 或有内容时显示', () => {
    expect(shouldShowCharCount({ collapsed: true, focused: true, dirty: true, charCount: 1 })).toBe(false)
    expect(shouldShowCharCount({ collapsed: false, focused: true, dirty: false, charCount: 0 })).toBe(true)
    expect(shouldShowCharCount({ collapsed: false, focused: false, dirty: true, charCount: 0 })).toBe(true)
    expect(shouldShowCharCount({ collapsed: false, focused: false, dirty: false, charCount: 3 })).toBe(true)
    expect(shouldShowCharCount({ collapsed: false, focused: false, dirty: false, charCount: 0 })).toBe(false)
  })
})

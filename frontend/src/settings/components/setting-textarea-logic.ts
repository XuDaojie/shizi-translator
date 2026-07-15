export function isPromptDirty(opts: {
  modelValue: string
  defaultValue?: string
  showReset: boolean
}): boolean {
  if (!opts.showReset || opts.defaultValue === undefined) return false
  if (!opts.modelValue.trim()) return false
  return opts.modelValue !== opts.defaultValue
}

export function isPromptDefault(opts: {
  modelValue: string
  defaultValue?: string
}): boolean {
  if (opts.defaultValue === undefined) return false
  return !opts.modelValue.trim() || opts.modelValue === opts.defaultValue
}

export function shouldShowDefaultPreview(opts: {
  modelValue: string
  defaultValue?: string
  focused: boolean
  collapsed: boolean
}): boolean {
  return (
    !opts.collapsed &&
    !opts.modelValue.trim() &&
    !!opts.defaultValue?.trim() &&
    !opts.focused
  )
}

export function shouldShowCharCount(opts: {
  collapsed: boolean
  focused: boolean
  dirty: boolean
  charCount: number
}): boolean {
  if (opts.collapsed) return false
  return opts.focused || opts.dirty || opts.charCount > 0
}

/** 重置语义：写空串以走默认。 */
export function resetPromptValue(): string {
  return ''
}

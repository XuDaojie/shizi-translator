import { computed, ref } from 'vue'
import { describe, expect, it } from 'vitest'
import { findSelectedLabel } from './selected-label'

describe('findSelectedLabel', () => {
  it('value 不变时随最新 options 刷新标签', () => {
    const value = ref('auto')
    const options = ref([{ value: 'auto', label: '自动' }])
    const label = computed(() => findSelectedLabel(options.value, value.value))

    options.value = [{ value: 'auto', label: 'Auto' }]

    expect(label.value).toBe('Auto')
  })

  it('无匹配或空 value 时返回 undefined', () => {
    const options = [{ value: 'auto', label: 'Auto' }]

    expect(findSelectedLabel(options, 'missing')).toBeUndefined()
    expect(findSelectedLabel(options, undefined)).toBeUndefined()
  })
})

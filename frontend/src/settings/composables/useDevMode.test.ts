import { afterEach, describe, expect, it } from 'vitest'
import { useDevMode } from './useDevMode'

/**
 * 直接改写 import.meta.env.DEV（用类型断言绕过 vite/client 的 readonly 标注）
 * 以覆盖 false 分支。vitest 默认 mode=test，import.meta.env.DEV 为 true，
 * 不改写时只能验证 true 分支。
 */
const setDev = (value: boolean): void => {
  ;(import.meta.env as { DEV: boolean }).DEV = value
}

describe('useDevMode', () => {
  const original = import.meta.env.DEV
  afterEach(() => {
    setDev(original)
  })

  it('DEV 为 true 时返回 true', () => {
    setDev(true)
    expect(useDevMode()).toBe(true)
  })

  it('DEV 为 false 时返回 false', () => {
    setDev(false)
    expect(useDevMode()).toBe(false)
  })
})

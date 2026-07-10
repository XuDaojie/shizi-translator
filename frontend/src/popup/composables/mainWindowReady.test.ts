import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createMainWindowReadyGate } from './mainWindowReady'

describe('createMainWindowReadyGate', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it('ready 路径只 show 一次', async () => {
    const show = vi.fn(async () => {})
    const warn = vi.fn()
    const gate = createMainWindowReadyGate({
      timeoutMs: 2000,
      show,
      onTimeoutWarn: warn,
    })
    await gate.notifyReady()
    await gate.notifyReady()
    expect(show).toHaveBeenCalledTimes(1)
    expect(warn).not.toHaveBeenCalled()
  })

  it('超时强制 show 并 warn', async () => {
    const show = vi.fn(async () => {})
    const warn = vi.fn()
    createMainWindowReadyGate({
      timeoutMs: 2000,
      show,
      onTimeoutWarn: warn,
    })
    await vi.advanceTimersByTimeAsync(2000)
    expect(show).toHaveBeenCalledTimes(1)
    expect(warn).toHaveBeenCalled()
  })

  it('ready 已 show 后超时 no-op', async () => {
    const show = vi.fn(async () => {})
    const warn = vi.fn()
    const gate = createMainWindowReadyGate({
      timeoutMs: 2000,
      show,
      onTimeoutWarn: warn,
    })
    await gate.notifyReady()
    await vi.advanceTimersByTimeAsync(2000)
    expect(show).toHaveBeenCalledTimes(1)
    expect(warn).not.toHaveBeenCalled()
  })

  it('超时后 ready 不再二次 show', async () => {
    const show = vi.fn(async () => {})
    const warn = vi.fn()
    const gate = createMainWindowReadyGate({
      timeoutMs: 2000,
      show,
      onTimeoutWarn: warn,
    })
    await vi.advanceTimersByTimeAsync(2000)
    await gate.notifyReady()
    expect(show).toHaveBeenCalledTimes(1)
  })
})

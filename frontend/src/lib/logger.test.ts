import { describe, it, expect, vi } from 'vitest'
import { createLogger, redactText, clampBuffer } from '../../public/logger.js'

const makeDeps = () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
  now: () => '2026-07-08T00:00:00.000Z',
  addEventListener: vi.fn(),
  setTimeout: vi.fn(() => 'timer-id') as unknown as (fn: () => void, ms: number) => unknown,
  clearTimeout: vi.fn(),
})

describe('clampBuffer', () => {
  it('超限丢弃最旧', () => {
    const buf = Array.from({ length: 1005 }, (_, i) => ({ msg: `msg-${i}` }))
    clampBuffer(buf, 1000)
    expect(buf).toHaveLength(1000)
    expect(buf[0].msg).toBe('msg-5')
    expect(buf[999].msg).toBe('msg-1004')
  })

  it('未超限不变', () => {
    const buf = [{ msg: 'a' }, { msg: 'b' }]
    clampBuffer(buf, 1000)
    expect(buf).toHaveLength(2)
  })
})

describe('createLogger', () => {
  it('按 level 过滤：info 下 debug 不入队', async () => {
    const deps = makeDeps()
    const logger = createLogger('test', deps)
    logger.setLevel('info')
    logger.debug('dropped')
    logger.info('kept')
    expect(deps.invoke).not.toHaveBeenCalled()
    await logger.flush()
    expect(deps.invoke).toHaveBeenCalledTimes(1)
    const entries = (deps.invoke.mock.calls[0][1] as { entries: Array<{ message: string }> }).entries
    expect(entries).toHaveLength(1)
    expect(entries[0].message).toBe('kept')
  })

  it('debug 等级下 debug 入队', async () => {
    const deps = makeDeps()
    const logger = createLogger('test', deps)
    logger.setLevel('debug')
    logger.debug('dbg')
    await logger.flush()
    const entries = (deps.invoke.mock.calls[0][1] as { entries: Array<{ message: string }> }).entries
    expect(entries[0].message).toBe('dbg')
  })

  it('满 50 条立即 flush', () => {
    const deps = makeDeps()
    const logger = createLogger('test', deps)
    logger.setLevel('error')
    for (let i = 0; i < 50; i++) logger.error(`m-${i}`)
    expect(deps.invoke).toHaveBeenCalledTimes(1)
  })

  it('invoke 失败重试一次后丢弃', async () => {
    const deps = makeDeps()
    deps.invoke = vi.fn().mockRejectedValue(new Error('boom'))
    const logger = createLogger('test', deps)
    logger.setLevel('error')
    logger.error('x')
    await logger.flush()
    expect(deps.invoke).toHaveBeenCalledTimes(2)
  })

  it('redactText info 摘要、debug 全文', () => {
    const text = 'Hello, this is a long translation text.'
    expect(redactText(text, 'info')).toContain('[len=39]')
    expect(redactText(text, 'info')).not.toContain('translation text.')
    expect(redactText(text, 'debug')).toBe(text)
  })

  it('addEventListener 注册 visibilitychange 与 beforeunload', () => {
    const deps = makeDeps()
    createLogger('test', deps)
    const types = deps.addEventListener.mock.calls.map((c) => c[0])
    expect(types).toContain('visibilitychange')
    expect(types).toContain('beforeunload')
  })
})

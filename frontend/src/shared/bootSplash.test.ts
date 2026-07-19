import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { dismissBootSplash } from './bootSplash'

class FakeClassList {
  private readonly items = new Set<string>()

  add(token: string): void {
    this.items.add(token)
  }

  contains(token: string): boolean {
    return this.items.has(token)
  }
}

class FakeElement {
  id = ''
  className = ''
  classList = new FakeClassList()
  isConnected = true
  private readonly listeners = new Map<string, Set<EventListener>>()

  addEventListener(type: string, listener: EventListener): void {
    let set = this.listeners.get(type)
    if (!set) {
      set = new Set()
      this.listeners.set(type, set)
    }
    set.add(listener)
  }

  removeEventListener(type: string, listener: EventListener): void {
    this.listeners.get(type)?.delete(listener)
  }

  dispatchEvent(event: Event): boolean {
    for (const listener of this.listeners.get(event.type) ?? []) {
      listener.call(this, event)
    }
    return true
  }

  remove(): void {
    this.isConnected = false
    if (this.id) registry.delete(this.id)
  }
}

const registry = new Map<string, FakeElement>()

class FakeTransitionEvent extends Event {
  readonly propertyName: string
  constructor(type: string, init?: { propertyName?: string }) {
    super(type)
    this.propertyName = init?.propertyName ?? ''
  }
}

function installDom(): void {
  registry.clear()
  const body = {
    appendChild(el: FakeElement) {
      if (el.id) registry.set(el.id, el)
      el.isConnected = true
      return el
    },
    get innerHTML() {
      return ''
    },
    set innerHTML(_value: string) {
      registry.clear()
    },
  }

  vi.stubGlobal('document', {
    body,
    createElement(_tag: string) {
      return new FakeElement()
    },
    getElementById(id: string) {
      const el = registry.get(id)
      return el?.isConnected ? el : null
    },
  })
  vi.stubGlobal('TransitionEvent', FakeTransitionEvent)
  vi.stubGlobal(
    'requestAnimationFrame',
    (cb: FrameRequestCallback) => {
      cb(0)
      return 0
    },
  )
}

describe('dismissBootSplash', () => {
  beforeEach(() => {
    installDom()
  })

  afterEach(() => {
    registry.clear()
    vi.unstubAllGlobals()
    vi.useRealTimers()
  })

  it('节点不存在时 no-op 且不抛错', async () => {
    await expect(dismissBootSplash()).resolves.toBeUndefined()
  })

  it('存在节点时添加 hide class 并最终移除', async () => {
    vi.useFakeTimers()
    const el = document.createElement('div') as unknown as FakeElement
    el.id = 'boot-splash'
    el.className = 'boot-splash'
    document.body.appendChild(el as unknown as Node)

    const pending = dismissBootSplash({ fallbackRemoveMs: 50 })
    await Promise.resolve()
    await Promise.resolve()

    expect(el.classList.contains('boot-splash--hide')).toBe(true)
    expect(document.getElementById('boot-splash')).toBe(el)

    await vi.advanceTimersByTimeAsync(50)
    await pending
    expect(document.getElementById('boot-splash')).toBeNull()
  })

  it('二次调用幂等', async () => {
    vi.useFakeTimers()
    const el = document.createElement('div') as unknown as FakeElement
    el.id = 'boot-splash'
    document.body.appendChild(el as unknown as Node)

    const first = dismissBootSplash({ fallbackRemoveMs: 50 })
    await Promise.resolve()
    await Promise.resolve()
    await vi.advanceTimersByTimeAsync(50)
    await first
    expect(document.getElementById('boot-splash')).toBeNull()

    await expect(dismissBootSplash()).resolves.toBeUndefined()
  })

  it('transitionend(opacity) 可提前移除', async () => {
    vi.useFakeTimers()
    const el = document.createElement('div') as unknown as FakeElement
    el.id = 'boot-splash'
    document.body.appendChild(el as unknown as Node)

    const promise = dismissBootSplash({ fallbackRemoveMs: 10_000 })
    await Promise.resolve()
    await Promise.resolve()

    el.dispatchEvent(new FakeTransitionEvent('transitionend', { propertyName: 'opacity' }))
    await promise
    expect(document.getElementById('boot-splash')).toBeNull()
  })
})

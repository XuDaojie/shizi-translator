export interface DismissBootSplashOptions {
  /** 默认 `boot-splash` */
  rootId?: string
  /** 默认 `boot-splash--hide` */
  hideClass?: string
  /** transition 未触发时的强制移除超时，默认 400ms */
  fallbackRemoveMs?: number
}

/** 双 rAF：等布局 + paint 一帧（best-effort） */
function doubleRaf(): Promise<void> {
  return new Promise((resolve) => {
    if (typeof requestAnimationFrame !== 'function') {
      resolve()
      return
    }
    requestAnimationFrame(() => {
      requestAnimationFrame(() => resolve())
    })
  })
}

/**
 * Vue mount 后调用：双 rAF → 淡出 → 移除冷启动 splash。
 * 节点不存在时 no-op；多次调用安全。
 */
export async function dismissBootSplash(
  options: DismissBootSplashOptions = {},
): Promise<void> {
  const rootId = options.rootId ?? 'boot-splash'
  const hideClass = options.hideClass ?? 'boot-splash--hide'
  const fallbackRemoveMs = options.fallbackRemoveMs ?? 400

  const el = document.getElementById(rootId)
  if (!el) return

  await doubleRaf()

  // 竞态：前一次 dismiss 可能已移除
  if (!el.isConnected) return

  await new Promise<void>((resolve) => {
    let settled = false
    const finish = (): void => {
      if (settled) return
      settled = true
      globalThis.clearTimeout(timer)
      el.removeEventListener('transitionend', onEnd)
      el.remove()
      resolve()
    }

    const onEnd = (event: Event): void => {
      // 仅在 opacity 过渡结束时收尾；非 TransitionEvent（测试/兜底）也允许结束
      if (event instanceof TransitionEvent && event.propertyName !== 'opacity') {
        return
      }
      finish()
    }

    const timer = globalThis.setTimeout(finish, fallbackRemoveMs)
    el.addEventListener('transitionend', onEnd)
    el.classList.add(hideClass)
  })
}

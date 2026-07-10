export interface MainWindowReadyOptions {
  timeoutMs: number
  show: () => Promise<void>
  onTimeoutWarn: (message: string) => void
}

export interface MainWindowReadyGate {
  /** UI 就绪后调用；与超时 race，先到者 show，其后 no-op */
  notifyReady: () => Promise<void>
  /** 是否已 show（含超时路径） */
  hasShown: () => boolean
  /** 取消超时定时器（组件卸载） */
  dispose: () => void
}

export function createMainWindowReadyGate(
  opts: MainWindowReadyOptions,
): MainWindowReadyGate {
  let shown = false
  let timer: ReturnType<typeof setTimeout> | null = null

  const doShow = async (fromTimeout: boolean): Promise<void> => {
    if (shown) return
    shown = true
    if (timer !== null) {
      clearTimeout(timer)
      timer = null
    }
    if (fromTimeout) {
      opts.onTimeoutWarn(
        `翻译弹窗 ready 超时（${opts.timeoutMs}ms），强制 show`,
      )
    }
    try {
      await opts.show()
    } catch {
      /* best-effort：show 失败不抛到调用方，避免阻塞 */
    }
  }

  timer = setTimeout(() => {
    void doShow(true)
  }, opts.timeoutMs)

  return {
    notifyReady: () => doShow(false),
    hasShown: () => shown,
    dispose: () => {
      if (timer !== null) {
        clearTimeout(timer)
        timer = null
      }
    },
  }
}

/** 双 rAF：等布局 + paint 一帧（best-effort） */
export function doubleRaf(): Promise<void> {
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

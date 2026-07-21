import { onBeforeUnmount, onMounted, type Ref } from 'vue'
import { getTauriApis } from './utils'

export interface UsePopupHeightReturn {
  /** 至少完成一次基于真实 offsetHeight 的 setSize（或无 Tauri 时 resolve） */
  whenFirstSized: Promise<void>
  /** 立即测高并 setSize（绕过仅 height 未变的短路时仍更新 lastHeight） */
  adjustNow: () => Promise<void>
}

/**
 * 弹窗高度自适应：ResizeObserver 观察 .popup，rAF 节流后调
 * getCurrentWindow().setSize({ type:'Logical', width:420, height:h })。
 * 复刻旧 translate.js 的 adjustHeight + initMaxHeight。
 * 暴露 whenFirstSized / adjustNow 供启动高度折叠链路使用。
 *
 * setSize 走串行队列：折叠动画期间每帧都会改高，若并发 await IPC 可能乱序
 * 收尾，窗口卡在偏大高度；队列保证最终落到最近一次测到的高度。
 */
export function usePopupHeight(popupRef: Ref<HTMLElement | null>): UsePopupHeightReturn {
  let resizeRaf: number | null = null
  let lastHeight = 0
  let observer: ResizeObserver | null = null
  let firstSizedResolved = false
  let resolveFirstSized: () => void = () => {}
  const whenFirstSized = new Promise<void>((resolve) => {
    resolveFirstSized = resolve
  })

  /** 待应用高度；pump 循环中可被更新为更新值 */
  let pendingHeight: number | null = null
  let pumping = false

  const markFirstSized = (): void => {
    if (firstSizedResolved) return
    firstSizedResolved = true
    resolveFirstSized()
  }

  const applySize = async (h: number): Promise<void> => {
    const apis = getTauriApis()
    if (apis) {
      try {
        await apis.getCurrentWindow().setSize({ type: 'Logical', width: 420, height: h })
      } catch {
        /* best-effort */
      }
    }
    markFirstSized()
  }

  const pump = async (): Promise<void> => {
    if (pumping) return
    pumping = true
    try {
      while (pendingHeight !== null) {
        const h = pendingHeight
        pendingHeight = null
        await applySize(h)
      }
    } finally {
      pumping = false
      // pump 期间又有新高度入队
      if (pendingHeight !== null) {
        void pump()
      }
    }
  }

  const enqueueSize = (h: number): Promise<void> => {
    pendingHeight = h
    return pump()
  }

  const measureAndApply = async (): Promise<void> => {
    const el = popupRef.value
    if (!el) {
      // 无 DOM 时：非 Tauri/测试环境直接放行，避免永远不 resolve
      if (!getTauriApis() && !firstSizedResolved) {
        markFirstSized()
      }
      return
    }
    const h = el.offsetHeight
    if (h === lastHeight && firstSizedResolved) return
    lastHeight = h
    await enqueueSize(h)
  }

  const adjust = (): void => {
    if (resizeRaf !== null) cancelAnimationFrame(resizeRaf)
    resizeRaf = requestAnimationFrame(() => {
      void measureAndApply()
    })
  }

  const adjustNow = (): Promise<void> => measureAndApply()

  const initMaxHeight = (): void => {
    const el = popupRef.value
    if (!el || typeof window === 'undefined') return
    const maxPopupH = Math.floor(window.screen.availHeight * 0.8)
    el.style.maxHeight = maxPopupH + 'px'
  }

  onMounted(() => {
    initMaxHeight()
    observer = new ResizeObserver(adjust)
    if (popupRef.value) observer.observe(popupRef.value)
    // 字体加载完成后重测（旧代码 document.fonts.ready.then(autoResize)）
    if (typeof document !== 'undefined' && document.fonts) {
      document.fonts.ready.then(adjust).catch(() => {})
    }
    // 无 Tauri 时 onMounted 也 resolve，避免测试挂死
    if (!getTauriApis()) {
      void measureAndApply()
    }
  })

  onBeforeUnmount(() => {
    if (resizeRaf !== null) cancelAnimationFrame(resizeRaf)
    observer?.disconnect()
    observer = null
  })

  return { whenFirstSized, adjustNow }
}

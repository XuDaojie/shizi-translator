import { onBeforeUnmount, onMounted, type Ref } from 'vue'
import { getTauriApis } from './utils'

/**
 * 弹窗高度自适应：ResizeObserver 观察 .popup，rAF 节流后调
 * getCurrentWindow().setSize({ type:'Logical', width:420, height:h })。
 * 复刻旧 translate.js 的 adjustHeight + initMaxHeight。
 */
export function usePopupHeight(popupRef: Ref<HTMLElement | null>): void {
  let resizeRaf: number | null = null
  let lastHeight = 0
  let observer: ResizeObserver | null = null

  const adjust = (): void => {
    if (resizeRaf !== null) cancelAnimationFrame(resizeRaf)
    resizeRaf = requestAnimationFrame(() => {
      const el = popupRef.value
      if (!el) return
      const h = el.offsetHeight
      if (h === lastHeight) return
      lastHeight = h
      const apis = getTauriApis()
      if (apis) {
        apis.getCurrentWindow()
          .setSize({ type: 'Logical', width: 420, height: h })
          .catch(() => {})
      }
    })
  }

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
  })

  onBeforeUnmount(() => {
    if (resizeRaf !== null) cancelAnimationFrame(resizeRaf)
    observer?.disconnect()
    observer = null
  })
}

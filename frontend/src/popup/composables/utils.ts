/** Tauri 全局 API 句柄（withGlobalTauri: true，window.__TAURI__ 可用）。
 *  弹窗三页统一走此入口，不引 @tauri-apps/api。 */
export interface TauriApis {
  invoke: <T = unknown>(cmd: string, args?: Record<string, unknown>) => Promise<T>
  listen: <T = unknown>(event: string, handler: (event: { payload: T }) => void) => Promise<UnlistenFn>
  getCurrentWindow: () => {
    setAlwaysOnTop: (top: boolean) => Promise<void>
    setSize: (size: LogicalSize) => Promise<void>
    show: () => Promise<void>
    setFocus: () => Promise<void>
  }
}
type UnlistenFn = () => void
interface LogicalSize { type: 'Logical'; width: number; height: number }

export function getTauriApis(): TauriApis | null {
  const t = (typeof window !== 'undefined' ? (window as { __TAURI__?: Record<string, unknown> }).__TAURI__ : undefined) as Record<string, Record<string, unknown>> | undefined
  const invoke = t?.core?.invoke as TauriApis['invoke'] | undefined
  const listen = t?.event?.listen as TauriApis['listen'] | undefined
  const getCurrentWindow = t?.window?.getCurrentWindow as TauriApis['getCurrentWindow'] | undefined
  if (!invoke || !listen || !getCurrentWindow) return null
  return { invoke, listen, getCurrentWindow }
}

/** batchId 从 "{batchId}:{serviceInstanceId}" 形式的 sessionId 提取。非字符串/无冒号返回 null。 */
export function batchIdFromSession(sessionId: unknown): string | null {
  if (typeof sessionId !== 'string') return null
  const idx = sessionId.indexOf(':')
  if (idx === -1) return null
  return sessionId.slice(0, idx)
}

/** 朗读：speechSynthesis 不可用时静默忽略（旧 translate.js 用 toast 提示，由调用方决定）。 */
export function speakText(text: string, lang: string): void {
  if (typeof window === 'undefined' || !('speechSynthesis' in window)) return
  window.speechSynthesis.cancel()
  const utter = new SpeechSynthesisUtterance(text)
  utter.lang = lang
  utter.rate = 0.95
  window.speechSynthesis.speak(utter)
}

/** 复制到剪贴板，成功返回 true，失败/不可用返回 false。 */
export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text)
      return true
    }
    return false
  } catch {
    return false
  }
}

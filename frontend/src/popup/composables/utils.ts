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
    /** 销毁当前窗口（绕过 close→hide）；下次唤起会重建。 */
    destroy: () => Promise<void>
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

/**
 * 取后端 pending 原文；若 revision 未变则 apply。
 * @returns 实际 apply 的文本；未 apply 时返回 null（供冷启动补触发翻译）。
 */
export async function applyPendingSourceIfCurrent(
  load: () => Promise<string | null>,
  getRevision: () => number,
  apply: (text: string) => void,
): Promise<string | null> {
  const revision = getRevision()
  const text = await load()
  if (text && revision === getRevision()) {
    apply(text)
    return text
  }
  return null
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

/**
 * 去除仅空白（含空）行，保留非空行原有内容与顺序。
 * 先把 \\r\\n / \\r 规范为 \\n（OCR / Windows 剪贴板常见），再按行过滤。
 */
export function removeBlankLines(text: string): string {
  return text
    .replace(/\r\n/g, '\n')
    .replace(/\r/g, '\n')
    .split('\n')
    .filter((line) => line.trim() !== '')
    .join('\n')
}

/** 开启「去除空行」时对原文做过滤；关闭时原样返回。 */
export function applyRemoveBlankIfActive(active: boolean, text: string): string {
  return active ? removeBlankLines(text) : text
}

/**
 * 开启去除空行时准备用于展示/下发的原文。
 * `changed` 为 true 表示相对入参做了清洗，调用方应用清洗后的文本重译。
 */
export function prepareSourceWithRemoveBlank(
  active: boolean,
  text: string,
): { text: string; changed: boolean } {
  const next = applyRemoveBlankIfActive(active, text)
  return { text: next, changed: next !== text }
}

import { invokeSetWindowCaptionChrome, isTauriReady } from '@/lib/tauri'
import type { WindowCaptionChrome } from '@/lib/tauri'

const LIGHT_FALLBACK = {
  caption: [252, 250, 247] as [number, number, number],
  text: [38, 43, 54] as [number, number, number],
}
const DARK_FALLBACK = {
  caption: [19, 16, 13] as [number, number, number],
  text: [240, 242, 244] as [number, number, number],
}

/** 解析 getComputedStyle 常见的 rgb / rgba 字符串。 */
export function parseCssRgb(
  color: string,
): { r: number; g: number; b: number } | null {
  const match = color
    .trim()
    .match(/^rgba?\(\s*([0-9.]+)\s*[, ]\s*([0-9.]+)\s*[, ]\s*([0-9.]+)/i)
  if (!match) return null
  return {
    r: Math.round(Number(match[1])),
    g: Math.round(Number(match[2])),
    b: Math.round(Number(match[3])),
  }
}

/** 标题栏色跟 token 走，不读 getComputedStyle（首帧未就绪时会落到黑）。 */
export function captionChromeForTheme(dark: boolean): WindowCaptionChrome {
  const colors = dark ? DARK_FALLBACK : LIGHT_FALLBACK
  return {
    caption: colors.caption,
    text: colors.text,
    darkButtons: dark,
  }
}

/** 把原生标题栏底色对齐 `--settings-sidebar`；失败静默。 */
export async function syncSettingsCaptionChrome(dark: boolean): Promise<void> {
  if (!isTauriReady()) return
  try {
    await invokeSetWindowCaptionChrome(captionChromeForTheme(dark))
  } catch {
    // 非 Win11 / 无 HWND：保持系统标题栏
  }
}
